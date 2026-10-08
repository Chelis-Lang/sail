//! Running Verus on one unit, with or without the shim, and extracting what
//! it reported.
//!
//! Every run adds the same flags: `--output-json --time` (structured
//! per-function results and timings) and `--no-report-long-running` (stops
//! timing-dependent diagnostics; see `docs/survey/verus-z3.md` §8). Both
//! sides of a comparison therefore run with identical arguments.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::corpus::Unit;
use crate::toolchain::Installed;
use crate::{Error, Result};

/// Which solver Verus talks to.
#[derive(Debug, Clone)]
pub enum Solver {
    /// The pinned Z3, as plain Verus uses it.
    Plain,
    /// The `sail-z3` shim in front of the pinned Z3.
    Shim {
        /// The `sail-z3` executable.
        shim: PathBuf,
        /// Where the shim writes traces, if tracing.
        trace_dir: Option<PathBuf>,
    },
}

/// Flags added to every run.
pub const COMMON_FLAGS: &[&str] = &["--output-json", "--time", "--no-report-long-running"];

/// One function's result, from Verus's `--time` breakdown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FunctionResult {
    /// The module, as Verus reports it.
    pub module: String,
    /// The function path.
    pub function: String,
    /// Whether all of its queries succeeded.
    pub success: bool,
    /// Resource units Z3 spent on it.
    pub rlimit: u64,
    /// Solver time in microseconds.
    pub time_micros: u64,
}

/// What one Verus run produced.
#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    /// Exit code (`None` if killed by a signal).
    pub exit_code: Option<i32>,
    /// Everything Verus wrote to stderr (diagnostics).
    pub stderr: String,
    /// Verus's `--output-json` document, if stdout parsed as JSON.
    pub json: Option<Value>,
    /// Wall-clock time of the whole run, in milliseconds.
    pub wall_ms: f64,
    /// User CPU seconds of the process tree (from `time`), if measured.
    pub user_s: Option<f64>,
    /// System CPU seconds of the process tree, if measured.
    pub sys_s: Option<f64>,
    /// Peak resident set size of the largest single process, in bytes.
    pub max_rss_bytes: Option<u64>,
}

impl RunRecord {
    /// The `verification-results` object.
    #[must_use]
    pub fn verification_results(&self) -> Option<&Value> {
        self.json.as_ref()?.get("verification-results")
    }

    /// Per-function results from the `--time` breakdown, sorted.
    #[must_use]
    pub fn functions(&self) -> Vec<FunctionResult> {
        let mut out = Vec::new();
        let modules = self
            .json
            .as_ref()
            .and_then(|j| j.pointer("/times-ms/smt/smt-run-module-times"))
            .and_then(Value::as_array);
        for module in modules.into_iter().flatten() {
            let name = module
                .get("module")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let fns = module.get("function-breakdown").and_then(Value::as_array);
            for f in fns.into_iter().flatten() {
                out.push(FunctionResult {
                    module: name.to_owned(),
                    function: f
                        .get("function")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    success: f.get("success").and_then(Value::as_bool).unwrap_or(false),
                    rlimit: f.get("rlimit").and_then(Value::as_u64).unwrap_or(0),
                    time_micros: f.get("time-micros").and_then(Value::as_u64).unwrap_or(0),
                });
            }
        }
        out.sort();
        out
    }

    /// Total solver time Verus reports (`smt-init` plus `smt-run`), in ms.
    #[must_use]
    pub fn smt_ms(&self) -> Option<u64> {
        self.json
            .as_ref()?
            .pointer("/times-ms/smt/total")
            .and_then(Value::as_u64)
    }
}

/// Parses the output of `/usr/bin/time -l` (BSD) or `-v` (GNU) into
/// `(user_s, sys_s, max_rss_bytes)`.
fn parse_time(text: &str) -> (Option<f64>, Option<f64>, Option<u64>) {
    let (mut user, mut sys, mut rss) = (None, None, None);
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("User time (seconds):") {
            user = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("System time (seconds):") {
            sys = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Maximum resident set size (kbytes):") {
            rss = v.trim().parse::<u64>().ok().map(|k| k * 1024);
        } else if let Some(v) = line.strip_suffix("maximum resident set size") {
            rss = v.trim().parse().ok();
        } else if line.ends_with(" sys") && line.contains(" real ") {
            // BSD: "  0.10 real  0.00 user  0.00 sys"
            let words: Vec<&str> = line.split_whitespace().collect();
            for pair in words.windows(2) {
                if let [value, unit] = pair {
                    match *unit {
                        "user" => user = value.parse().ok(),
                        "sys" => sys = value.parse().ok(),
                        _ => {}
                    }
                }
            }
        }
    }
    (user, sys, rss)
}

fn time_flag() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("-l")
    } else if cfg!(target_os = "linux") {
        Some("-v")
    } else {
        None
    }
}

/// Runs Verus on `unit` against `solver`, using `work` (created if needed) as
/// the working and scratch output directory.
///
/// # Errors
///
/// `SAIL3020` if the scratch directory cannot be prepared or Verus cannot be
/// started; `SAIL3021` if the timing wrapper failed to run Verus. A Verus run
/// that fails verification is not an error.
pub fn run_unit(
    installed: &Installed,
    unit: &Unit,
    solver: &Solver,
    work: &Path,
) -> Result<RunRecord> {
    // Verus runs with `work` as its current directory, so every path handed
    // to it (and to `time -o`) must be absolute.
    let work = &std::path::absolute(work).map_err(|e| {
        Error::new(
            "SAIL3020",
            format!("cannot resolve {}: {e}", work.display()),
        )
    })?;
    let _ = std::fs::remove_dir_all(work);
    std::fs::create_dir_all(work)
        .map_err(|e| Error::new("SAIL3020", format!("cannot create {}: {e}", work.display())))?;
    let out_dir = work.to_string_lossy().into_owned();
    let args: Vec<String> = unit
        .args
        .iter()
        .map(|a| a.replace("{out}", &out_dir))
        .collect();
    let root = unit.source_dir.join(&unit.root);
    let time_file = work.join("time.txt");
    let timer = Path::new("/usr/bin/time");
    let time_mode = time_flag().filter(|_| timer.is_file());
    let mut cmd = match time_mode {
        Some(flag) => {
            let mut c = Command::new(timer);
            c.arg(flag).arg("-o").arg(&time_file).arg(&installed.verus);
            c
        }
        None => Command::new(&installed.verus),
    };
    cmd.args(&args)
        .arg(&root)
        .args(COMMON_FLAGS)
        .current_dir(work)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("VERUS_Z3_PATH")
        .env_remove("SAIL_Z3_PATH")
        .env_remove("SAIL_TRACE_DIR")
        .envs(&unit.env);
    if let Solver::Shim { shim, trace_dir } = solver {
        cmd.env("VERUS_Z3_PATH", shim)
            .env("SAIL_Z3_PATH", &installed.z3);
        if let Some(dir) = trace_dir {
            cmd.env("SAIL_TRACE_DIR", dir);
        }
    }
    let started = Instant::now();
    let output = cmd.output().map_err(|e| {
        Error::new(
            "SAIL3020",
            format!("cannot run {}: {e}", installed.verus.display()),
        )
    })?;
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    let timing = std::fs::read_to_string(&time_file);
    if time_mode.is_some() && timing.is_err() {
        // `time` did not run Verus to completion: this is an infrastructure
        // failure, never a Verus result.
        return Err(Error::new(
            "SAIL3021",
            format!(
                "no timing report from {}; Verus did not run: {}",
                timer.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let (user_s, sys_s, max_rss_bytes) = parse_time(&timing.unwrap_or_default());
    Ok(RunRecord {
        exit_code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        json: serde_json::from_slice(&output.stdout).ok(),
        wall_ms,
        user_s,
        sys_s,
        max_rss_bytes,
    })
}

/// Machine details recorded with every benchmark and conformance result.
#[must_use]
pub fn machine_info() -> BTreeMap<String, String> {
    let mut info = BTreeMap::new();
    let mut capture = |key: &str, program: &str, args: &[&str]| {
        if let Ok(out) = Command::new(program).args(args).output()
            && out.status.success()
        {
            info.insert(
                key.to_owned(),
                String::from_utf8_lossy(&out.stdout).trim().to_owned(),
            );
        }
    };
    capture("uname", "uname", &["-srm"]);
    if cfg!(target_os = "macos") {
        capture("cpu", "sysctl", &["-n", "machdep.cpu.brand_string"]);
        capture("cpus", "sysctl", &["-n", "hw.ncpu"]);
        capture("memory_bytes", "sysctl", &["-n", "hw.memsize"]);
    } else {
        capture("cpus", "nproc", &[]);
    }
    info.insert("host".to_owned(), crate::toolchain::host().to_owned());
    info
}

#[cfg(test)]
mod tests {
    use super::parse_time;

    #[test]
    fn parses_bsd_time() {
        let text = "        2.10 real         1.50 user         0.25 sys\n             1163264  maximum resident set size\n";
        assert_eq!(parse_time(text), (Some(1.5), Some(0.25), Some(1_163_264)));
    }

    #[test]
    fn parses_gnu_time() {
        let text = "\tUser time (seconds): 1.50\n\tSystem time (seconds): 0.25\n\tMaximum resident set size (kbytes): 2048\n";
        assert_eq!(parse_time(text), (Some(1.5), Some(0.25), Some(2_097_152)));
    }
}
