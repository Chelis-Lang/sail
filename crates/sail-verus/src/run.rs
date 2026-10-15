//! Running Verus on one unit, with or without the shim, and extracting what
//! it reported.
//!
//! Every run adds the same flags: `--output-json --time` (structured
//! per-function results and timings) and `--no-report-long-running` (stops
//! timing-dependent diagnostics; see `docs/survey/verus-z3.md` §8). Both
//! sides of a comparison therefore run with identical arguments.
//!
//! `cargo-verus` units run `cargo verus <command> <cargo args> -- <verus
//! args> <common flags>` in the package directory of a scratch copy made by
//! [`prepare_unit`], with the pinned Rust toolchain, a target directory emptied
//! before each run, and Cargo's status output off (`CARGO_TERM_QUIET`), so stderr
//! holds only diagnostics. Each verified crate prints its own
//! `--output-json` document, so a run can produce several.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::corpus::{Build, Unit};
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
        /// The cache daemon's socket (`SAIL_DAEMON_SOCKET`), for cached
        /// sessions (ADR 0011). `None` runs pure pass-through.
        daemon_socket: Option<PathBuf>,
        /// Also pass the logging flags `sail verify`'s managed driver adds
        /// to every verified target ([`crate::adapter::OBSERVATION_LOG_FLAGS`]),
        /// with a log directory under the run's work directory, so that
        /// Verus runs exactly as `sail verify` runs it (ADR 0045). `false`
        /// is the bare shim, kept for diagnosing a difference.
        observation_logs: bool,
    },
    /// The optional interactive source engine (ADR 0027) with the pinned Z3.
    Engine(EngineRun),
}

/// How the engine conformance arm runs the interactive source engine.
///
/// Every run gets its own event journal and focus report under its work
/// directory.
#[derive(Debug, Clone)]
pub struct EngineRun {
    /// The engine's `verus` launcher.
    pub verus: PathBuf,
    /// The engine's `cargo-verus` frontend.
    pub cargo_verus: PathBuf,
    /// Exact crate-qualified function for the focused phase, if any. The
    /// independent full phase always runs and is what is compared.
    pub function: Option<String>,
}

/// Files the engine writes under a unit's work directory.
pub mod engine_files {
    /// Source observation journal (JSONL).
    pub const EVENTS: &str = "engine.events.jsonl";
    /// Focused-phase report.
    pub const FOCUS: &str = "engine.focus.json";
}

/// Flags added to every run.
///
/// `-Z write-long-types-to-disk=no` makes rustc print long type names inline
/// instead of writing them to a randomly named file, whose name it then
/// reports in the diagnostics. Without it, plain Verus's own stderr differs
/// between identical runs on some inputs, under Rust 1.98.0 (ADR 0006
/// update). It changes nothing sent to the solver.
pub const COMMON_FLAGS: &[&str] = &[
    "--output-json",
    "--time",
    "--no-report-long-running",
    "-Z",
    "write-long-types-to-disk=no",
];

/// One function's result, from Verus's `--time` breakdown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FunctionResult {
    /// Which `--output-json` document (crate) it came from, counting from 0
    /// in stdout order.
    pub output: usize,
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
    /// Verus's `--output-json` documents, in stdout order: one for a
    /// `verus` run, one per verified crate for `cargo verus`.
    pub outputs: Vec<Value>,
    /// Whatever followed the last JSON document on stdout (normally empty).
    pub stdout_rest: String,
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
    /// The sum of a numeric `verification-results` field (for example
    /// `verified` or `errors`) over all documents that have it.
    #[must_use]
    pub fn total(&self, field: &str) -> Option<u64> {
        self.outputs
            .iter()
            .filter_map(|o| o.get("verification-results")?.get(field)?.as_u64())
            .reduce(u64::saturating_add)
    }

    /// Per-function results from the `--time` breakdown, sorted.
    #[must_use]
    pub fn functions(&self) -> Vec<FunctionResult> {
        let mut out = Vec::new();
        for (output, doc) in self.outputs.iter().enumerate() {
            Self::push_functions(output, doc, &mut out);
        }
        out.sort();
        out
    }

    fn push_functions(output: usize, doc: &Value, out: &mut Vec<FunctionResult>) {
        let modules = doc
            .pointer("/times-ms/smt/smt-run-module-times")
            .and_then(Value::as_array);
        for module in modules.into_iter().flatten() {
            let name = module
                .get("module")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let fns = module.get("function-breakdown").and_then(Value::as_array);
            for f in fns.into_iter().flatten() {
                out.push(FunctionResult {
                    output,
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
    }

    /// Total solver time Verus reports (`smt-init` plus `smt-run`), in ms,
    /// summed over all documents.
    #[must_use]
    pub fn smt_ms(&self) -> Option<u64> {
        self.outputs
            .iter()
            .filter_map(|o| o.pointer("/times-ms/smt/total")?.as_u64())
            .reduce(u64::saturating_add)
    }
}

/// Splits stdout into its JSON documents and whatever follows the last one.
fn parse_outputs(stdout: &[u8]) -> (Vec<Value>, String) {
    let mut stream = serde_json::Deserializer::from_slice(stdout).into_iter::<Value>();
    let mut docs = Vec::new();
    let mut end = 0;
    while let Some(Ok(doc)) = stream.next() {
        docs.push(doc);
        end = stream.byte_offset();
    }
    let rest = stdout.get(end..).unwrap_or_default();
    (docs, String::from_utf8_lossy(rest).trim().to_owned())
}

/// Environment that could make Cargo or rustc behave differently from a
/// plain `cargo verus` in a clean shell (for example when the harness itself
/// runs under `cargo run`). Removed for `cargo-verus` units.
const CARGO_ENV_REMOVED: &[&str] = &[
    "CARGO",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_MAKEFLAGS",
    "CARGO_MANIFEST_DIR",
    "CARGO_PKG_NAME",
    "CARGO_PRIMARY_PACKAGE",
    "RUSTC",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTDOC",
    "RUSTFLAGS",
];

/// Applies the pinned toolchain and quiet, deterministic Cargo settings.
fn cargo_env(cmd: &mut Command, installed: &Installed) {
    for var in CARGO_ENV_REMOVED {
        cmd.env_remove(var);
    }
    cmd.env("RUSTUP_TOOLCHAIN", &installed.pins.verus.rust_toolchain)
        .env("CARGO_TERM_QUIET", "true")
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_TERM_PROGRESS_WHEN", "never");
}

fn prepare_libc(installed: &Installed, unit: &Unit, scratch: &Path) -> Result<Unit> {
    let fail = |m: String| Error::new("SAIL3022", m);
    let scratch = std::path::absolute(scratch).map_err(|e| fail(e.to_string()))?;
    let source = scratch.join("src");
    copy_tree(&unit.source_dir, &source).map_err(|e| fail(e.to_string()))?;
    let target = scratch.join("libc-target");
    let status = Command::new("cargo")
        .args(["build", "--release", "--locked", "--manifest-path"])
        .arg(source.join("test_libc/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("RUSTUP_TOOLCHAIN", &installed.pins.verus.rust_toolchain)
        .env_remove("RUSTFLAGS")
        .status()
        .map_err(|e| fail(e.to_string()))?;
    if !status.success() {
        return Err(fail("test_libc build failed".to_owned()));
    }
    let candidates: Vec<PathBuf> = std::fs::read_dir(target.join("release/deps"))
        .map_err(|e| fail(e.to_string()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                n.to_string_lossy().starts_with("liblibc-")
                    && n.to_string_lossy().ends_with(".rlib")
            })
        })
        .collect();
    let [libc] = candidates.as_slice() else {
        return Err(fail("test_libc produced no unique libc rlib".to_owned()));
    };
    let mut prepared = unit.clone();
    prepared.source_dir = source;
    prepared.build = Build::Verus;
    for arg in &mut prepared.args {
        if arg == "libc=build/liblibc.rlib" {
            *arg = format!("libc={}", libc.display());
        }
    }
    Ok(prepared)
}

/// Copies the directory tree `from` into `to` (created if needed), skipping
/// `.git` and the fetch marker, and recreating symlinks as symlinks.
///
/// # Errors
///
/// Any I/O error.
pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == ".sail-fetched" {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            copy_symlink(&src, &dst)?;
        } else if kind.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(std::fs::read_link(src)?, dst)
}

#[cfg(not(unix))]
fn copy_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ())
}

/// The nearest directory at or above `dir` that has a `Cargo.toml`.
fn enclosing_cargo_project(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|d| d.join("Cargo.toml").is_file())
}

/// Prepares `unit` to run under `scratch` and returns the unit to run.
///
/// `verus` units run in place and are returned unchanged. A `cargo-verus`
/// unit gets a scratch copy of its project in `<scratch>/src` (Cargo writes
/// `Cargo.lock` into the tree, and the fetched source is never modified),
/// with its Verus dependencies patched to `verus_source` if
/// `patch_verus_deps` is set, and its lockfile resolved once with
/// `cargo metadata` so that both sides of a comparison start from the same
/// tree.
///
/// `scratch` must not be inside another Cargo project, or Cargo would treat
/// the copy as part of that project's workspace.
///
/// # Errors
///
/// `SAIL3022` if `scratch` is inside a Cargo project, if the copy, the patch
/// or dependency resolution fails, or if patching is asked for without a
/// Verus source.
pub fn prepare_unit(
    installed: &Installed,
    unit: &Unit,
    verus_source: Option<&Path>,
    scratch: &Path,
) -> Result<Unit> {
    if unit.build == Build::LibcVerus {
        return prepare_libc(installed, unit, scratch);
    }
    if unit.build != Build::CargoVerus {
        return Ok(unit.clone());
    }
    let fail = |what: String| Error::new("SAIL3022", format!("{}: {what}", unit.id));
    let scratch = std::path::absolute(scratch)
        .map_err(|e| fail(format!("cannot resolve {}: {e}", scratch.display())))?;
    if let Some(outer) = enclosing_cargo_project(&scratch) {
        // Cargo would take the copy for a member of that workspace.
        return Err(fail(format!(
            "scratch directory {} is inside the Cargo project at {}",
            scratch.display(),
            outer.display()
        )));
    }
    let src = scratch.join("src");
    let _ = std::fs::remove_dir_all(&src);
    copy_tree(&unit.source_dir, &src).map_err(|e| {
        fail(format!(
            "cannot copy {} to {}: {e}",
            unit.source_dir.display(),
            src.display()
        ))
    })?;
    let package = src.join(&unit.root);
    if unit.patch_verus_deps {
        let verus = verus_source
            .ok_or_else(|| fail("patch_verus_deps needs the pinned Verus source".to_owned()))?;
        crate::patch::patch_verus_deps(&package, &src, verus).map_err(|e| fail(e.to_string()))?;
    }
    let mut cmd = Command::new("cargo");
    cargo_env(&mut cmd, installed);
    let output = cmd
        .args(["metadata", "--format-version", "1"])
        .current_dir(&package)
        .stdin(Stdio::null())
        .env("CARGO_TARGET_DIR", scratch.join("metadata-target"))
        .output()
        .map_err(|e| fail(format!("cannot run cargo metadata: {e}")))?;
    if !output.status.success() {
        return Err(fail(format!(
            "cargo metadata failed in {}: {}",
            package.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut prepared = unit.clone();
    prepared.source_dir = src;
    prepared.target_dir = Some(scratch.join("target"));
    Ok(prepared)
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

/// Adds the arguments, working directory and Cargo settings for `unit` to
/// `cmd`, which runs `verus` or `cargo-verus`.
fn add_invocation(
    cmd: &mut Command,
    installed: &Installed,
    unit: &Unit,
    work: &Path,
    target_dir: Option<&Path>,
) {
    let out_dir = work.to_string_lossy().into_owned();
    let args: Vec<String> = unit
        .args
        .iter()
        .map(|a| a.replace("{out}", &out_dir))
        .collect();
    let root = unit.source_dir.join(&unit.root);
    if unit.build == Build::CargoVerus {
        cargo_env(cmd, installed);
        cmd.arg("verus")
            .args(&unit.cargo_args)
            .arg("--")
            .args(&args)
            .args(COMMON_FLAGS)
            .current_dir(&root);
        if let Some(dir) = target_dir {
            cmd.env("CARGO_TARGET_DIR", dir);
        }
    } else {
        cmd.args(&args)
            .arg(&root)
            .args(COMMON_FLAGS)
            .current_dir(work);
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
    let cargo = unit.build == Build::CargoVerus;
    let target_dir = if cargo {
        let dir = unit.target_dir.as_deref().ok_or_else(|| {
            Error::new(
                "SAIL3020",
                format!("{}: cargo-verus unit was not prepared", unit.id),
            )
        })?;
        let _ = std::fs::remove_dir_all(dir);
        Some(dir)
    } else {
        None
    };
    let program = match (solver, cargo) {
        (Solver::Engine(engine), true) => &engine.cargo_verus,
        (Solver::Engine(engine), false) => &engine.verus,
        (_, true) => &installed.cargo_verus,
        (_, false) => &installed.verus,
    };
    if !program.is_file() {
        return Err(Error::new(
            "SAIL3020",
            format!("{} is not installed", program.display()),
        ));
    }
    let time_file = work.join("time.txt");
    let timer = Path::new("/usr/bin/time");
    let time_mode = time_flag().filter(|_| timer.is_file());
    let mut cmd = match time_mode {
        Some(flag) => {
            let mut c = Command::new(timer);
            c.arg(flag).arg("-o").arg(&time_file).arg(program);
            c
        }
        None => Command::new(program),
    };
    add_invocation(&mut cmd, installed, unit, work, target_dir);
    let observation_logs = matches!(
        solver,
        Solver::Shim {
            observation_logs: true,
            ..
        }
    )
    .then(|| work.join("sail-logs"));
    if let Some(logs) = &observation_logs {
        cmd.args(crate::adapter::OBSERVATION_LOG_FLAGS).arg(logs);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    solver_env(&mut cmd, installed, unit, solver);
    if let Solver::Engine(engine) = solver {
        engine_env(&mut cmd, installed, engine, work);
    }
    let started = Instant::now();
    let output = run_with_timeout_retaining(&mut cmd, unit_timeout(), Some(work))
        .map_err(|e| Error::new(e.0, format!("{}: {}", program.display(), e.1)))?;
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    if let Some(dir) = target_dir {
        // Build products are large and never compared; keep the disk free.
        let _ = std::fs::remove_dir_all(dir);
    }
    if let Some(logs) = &observation_logs {
        // So are the logs: the managed driver reads them, the harness does not.
        let _ = std::fs::remove_dir_all(logs);
    }
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
    // Kept next to the run for inspecting differences; best effort.
    let _ = std::fs::write(work.join("stdout.txt"), &output.stdout);
    let _ = std::fs::write(work.join("stderr.txt"), &output.stderr);
    let (user_s, sys_s, max_rss_bytes) = parse_time(&timing.unwrap_or_default());
    let (outputs, stdout_rest) = parse_outputs(&output.stdout);
    Ok(RunRecord {
        exit_code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        outputs,
        stdout_rest,
        wall_ms,
        user_s,
        sys_s,
        max_rss_bytes,
    })
}

/// The per-unit time limit: `SAIL_UNIT_TIMEOUT` seconds, default 3600. A
/// unit that exceeds it is an infrastructure error (`SAIL3022`), never a
/// Verus result.
pub(crate) fn unit_timeout() -> Duration {
    std::env::var("SAIL_UNIT_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(Duration::from_secs(3600), Duration::from_secs)
}

/// Runs `cmd` to completion, capturing stdout and stderr, but kills its
/// whole process group (the timing wrapper, Verus, shims and solvers) if it
/// runs longer than `limit`.
pub(crate) fn run_with_timeout(
    cmd: &mut Command,
    limit: Duration,
) -> std::result::Result<std::process::Output, (&'static str, String)> {
    run_with_timeout_retaining(cmd, limit, None)
}

// Retain the raw streams while draining the pipes, including before a timeout.
// Capture is best effort, like the completed-run files below run_unit. It never
// changes a command, stream, deadline or infrastructure-error classification.
fn run_with_timeout_retaining(
    cmd: &mut Command,
    limit: Duration,
    work: Option<&Path>,
) -> std::result::Result<std::process::Output, (&'static str, String)> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ("SAIL3020", format!("cannot run: {e}")))?;
    let read_all = |r: Option<Box<dyn std::io::Read + Send>>, name: &str| {
        let capture = work.and_then(|dir| std::fs::File::create(dir.join(name)).ok());
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut r) = r {
                let _ = RetainedReader {
                    reader: &mut r,
                    capture,
                }
                .read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = read_all(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        "stdout.txt",
    );
    let err = read_all(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        "stderr.txt",
    );
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let pgid = child.id();
                let _ = Command::new("kill")
                    .args(["-s", "KILL", "--", &format!("-{pgid}")])
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                return Err((
                    "SAIL3022",
                    format!(
                        "exceeded the unit time limit of {}s; process group killed",
                        limit.as_secs()
                    ),
                ));
            }
            Err(e) => return Err(("SAIL3020", format!("lost the child process: {e}"))),
        }
    };
    Ok(std::process::Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

struct RetainedReader<R> {
    reader: R,
    capture: Option<std::fs::File>,
}

impl<R: std::io::Read> std::io::Read for RetainedReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        use std::io::Write as _;
        let size = self.reader.read(bytes)?;
        if let Some(capture) = &mut self.capture
            && capture.write_all(&bytes[..size]).is_err()
        {
            // A full/unavailable evidence disk must not stop draining a pipe
            // and hang or alter the verifier. Completed output is still kept
            // in memory; timeout evidence may be incomplete on an I/O failure.
            self.capture = None;
        }
        Ok(size)
    }
}

/// Sets the solver environment for one side of a comparison.
fn solver_env(cmd: &mut Command, installed: &Installed, unit: &Unit, solver: &Solver) {
    for key in [
        "SAIL_ENGINE",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE_CONTEXT",
    ] {
        cmd.env_remove(key);
    }
    cmd.env_remove("VERUS_Z3_PATH")
        .env_remove("SAIL_Z3_PATH")
        .env_remove("SAIL_TRACE_DIR")
        .env_remove("SAIL_DAEMON_SOCKET")
        .envs(&unit.env)
        .env("SAIL_SOLVER_MODE", "default");
    if !installed.z3_bundled {
        // This release bundles no z3, so plain Verus's launcher would find
        // none: name the pinned one, as the sail side does through
        // SAIL_Z3_PATH. Both sides use the same binary.
        cmd.env("VERUS_Z3_PATH", &installed.z3);
    }
    if let Solver::Shim {
        shim,
        trace_dir,
        daemon_socket,
        ..
    } = solver
    {
        cmd.env("VERUS_Z3_PATH", shim)
            .env("SAIL_Z3_PATH", &installed.z3);
        if let Some(dir) = trace_dir {
            cmd.env("SAIL_TRACE_DIR", dir);
        }
        if let Some(socket) = daemon_socket {
            cmd.env("SAIL_DAEMON_SOCKET", socket);
        }
    }
}

/// The interactive engine's private controls, all scoped to `work`.
fn engine_env(cmd: &mut Command, installed: &Installed, engine: &EngineRun, work: &Path) {
    cmd.env_remove("SAIL_ENGINE_FUNCTION");
    // The same pinned solver bytes as the plain side.
    cmd.env("VERUS_Z3_PATH", &installed.z3)
        .env("SAIL_ENGINE", "interactive")
        .env("SAIL_ENGINE_EVENTS", work.join(engine_files::EVENTS))
        .env("SAIL_ENGINE_FOCUS_REPORT", work.join(engine_files::FOCUS))
        .env("SAIL_ENGINE_CONTEXT", "sail-conformance-engine-v1");
    if let Some(function) = &engine.function {
        cmd.env("SAIL_ENGINE_FUNCTION", function);
    }
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
    use super::{RunRecord, parse_outputs, parse_time};
    use serde_json::json;

    #[cfg(unix)]
    #[test]
    fn timeout_retains_raw_partial_streams_without_a_verifier_result() {
        let root =
            std::env::temp_dir().join(format!("sail-timeout-capture-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let pid_file = root.join("child.pid");
        let mut command = std::process::Command::new("sh");
        command
            .args([
                "-c",
                "printf '%s' \"$$\" > \"$1\"; printf 'partial\\000\\377'; printf 'failure\\000\\376' >&2; exec sleep 20",
                "sail-timeout-fixture",
            ])
            .arg(&pid_file);
        let result = super::run_with_timeout_retaining(
            &mut command,
            std::time::Duration::from_secs(2),
            Some(&root),
        );
        assert!(matches!(result, Err(("SAIL3022", _))), "{result:?}");
        assert_eq!(
            std::fs::read(root.join("stdout.txt")).ok(),
            Some(b"partial\0\xff".to_vec())
        );
        assert_eq!(
            std::fs::read(root.join("stderr.txt")).ok(),
            Some(b"failure\0\xfe".to_vec())
        );
        let pid = std::fs::read_to_string(pid_file).unwrap_or_default();
        assert_ne!(pid, "");
        assert!(
            std::process::Command::new("kill")
                .args(["-0", "--", &pid])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| !status.success())
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn completed_capture_preserves_binary_streams_exit_and_failed_capture_fallback() {
        let root =
            std::env::temp_dir().join(format!("sail-completed-capture-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        // A directory at stderr.txt refuses the capture file without changing
        // draining, raw output or the command's verification-failure exit code.
        assert!(std::fs::create_dir_all(root.join("stderr.txt")).is_ok());
        let script = "printf 'answer\\000\\377'; printf 'diagnostic\\000\\376' >&2; exit 9";
        let mut command = std::process::Command::new("sh");
        command.args(["-c", script]);
        let result = super::run_with_timeout_retaining(
            &mut command,
            std::time::Duration::from_secs(5),
            Some(&root),
        );
        let Ok(output) = result else {
            unreachable!("fixture failed to run: {result:?}")
        };
        assert_eq!(output.status.code(), Some(9));
        assert_eq!(output.stdout, b"answer\0\xff");
        assert_eq!(output.stderr, b"diagnostic\0\xfe");
        let mut ordinary = std::process::Command::new("sh");
        ordinary.args(["-c", script]);
        let Ok(plain) = super::run_with_timeout(&mut ordinary, std::time::Duration::from_secs(5))
        else {
            unreachable!("ordinary fixture failed to run")
        };
        assert_eq!(output.status, plain.status);
        assert_eq!(output.stdout, plain.stdout);
        assert_eq!(output.stderr, plain.stderr);
        assert_eq!(
            std::fs::read(root.join("stdout.txt")).ok(),
            Some(output.stdout)
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn conformance_forces_default_mode_on_both_sides() {
        let Ok(pins) = crate::toolchain::Pins::embedded() else {
            unreachable!("embedded pins invalid")
        };
        let installed = super::Installed {
            pins,
            verus: "verus".into(),
            cargo_verus: "cargo-verus".into(),
            z3: "z3".into(),
            z3_bundled: true,
        };
        let unit = super::Unit {
            id: "fixture".to_owned(),
            project: "fixture".to_owned(),
            source_dir: "fixture".into(),
            root: "lib.rs".to_owned(),
            build: crate::corpus::Build::Verus,
            args: Vec::new(),
            cargo_args: Vec::new(),
            patch_verus_deps: false,
            target_dir: None,
            env: std::collections::BTreeMap::from([(
                "SAIL_SOLVER_MODE".to_owned(),
                "accept-first-proof".to_owned(),
            )]),
            expected: None,
        };
        for solver in [
            super::Solver::Plain,
            super::Solver::Shim {
                shim: "sail-z3".into(),
                trace_dir: None,
                daemon_socket: None,
                observation_logs: false,
            },
        ] {
            let mut cmd = std::process::Command::new("never-executed");
            cmd.env("SAIL_SOLVER_MODE", "accept-first-proof");
            super::solver_env(&mut cmd, &installed, &unit, &solver);
            let mode = cmd
                .get_envs()
                .find(|(k, _)| *k == "SAIL_SOLVER_MODE")
                .and_then(|(_, v)| v);
            assert_eq!(mode, Some(std::ffi::OsStr::new("default")));
        }
    }

    #[test]
    fn finds_an_enclosing_cargo_project() {
        let root = std::env::temp_dir().join(format!("sail-enclosing-{}", std::process::id()));
        let inner = root.join("a").join("b");
        assert!(std::fs::create_dir_all(&inner).is_ok());
        assert_eq!(super::enclosing_cargo_project(&inner), None);
        assert!(std::fs::write(root.join("Cargo.toml"), "[workspace]\n").is_ok());
        assert_eq!(super::enclosing_cargo_project(&inner), Some(root.as_path()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn splits_concatenated_json_documents() {
        let (docs, rest) = parse_outputs(b"{\n \"a\": 1\n}\n{\"b\": [2]}\n");
        assert_eq!(docs, vec![json!({"a": 1}), json!({"b": [2]})]);
        assert_eq!(rest, "");
        let (docs, rest) = parse_outputs(b"{\"a\": 1}\nverification results:: 1 verified\n");
        assert_eq!(docs, vec![json!({"a": 1})]);
        assert_eq!(rest, "verification results:: 1 verified");
        let (docs, rest) = parse_outputs(b"");
        assert!(docs.is_empty() && rest.is_empty());
    }

    #[test]
    fn functions_and_totals_span_all_documents() {
        let doc = |f: &str, ok: bool, verified: u64| {
            json!({
                "verification-results": {"verified": verified, "errors": u64::from(!ok)},
                "times-ms": {"smt": {"total": 5, "smt-run-module-times": [
                    {"module": "m", "function-breakdown": [{"function": f, "success": ok, "rlimit": 3}]}
                ]}},
            })
        };
        let r = RunRecord {
            exit_code: Some(1),
            stderr: String::new(),
            outputs: vec![doc("f", true, 2), doc("f", false, 4)],
            stdout_rest: String::new(),
            wall_ms: 0.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        };
        assert_eq!(r.total("verified"), Some(6));
        assert_eq!(r.total("errors"), Some(1));
        assert_eq!(r.smt_ms(), Some(10));
        let fns = r.functions();
        assert_eq!(fns.len(), 2);
        assert_eq!((fns[0].output, fns[0].success), (0, true));
        assert_eq!((fns[1].output, fns[1].success), (1, false));
    }

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
