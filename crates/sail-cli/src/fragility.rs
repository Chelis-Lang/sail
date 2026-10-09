//! `sail fragility`: how fragile are this crate's proofs?
//!
//! ```text
//! sail fragility [--seeds N] [--shuffles K] [--jobs J] [--near-limit F] [--top K]
//!                [--json] [--keep-traces DIR] [--traces DIR] [--z3 PATH]
//!                [--focus] [--output json] [cargo verus verify arguments…]
//! ```
//!
//! Runs `cargo verus verify <arguments>` (or `cargo verus focus`, which
//! partial verification such as `--verify-function` requires: chosen with
//! `--focus`, or automatically when the Verus arguments select part of the
//! crate) once, with `sail-z3` in pure
//! pass-through (no daemon, no cache) and tracing on, so every solver
//! session is exactly what plain Verus sent. It adds `--output-json --time`
//! after `--` (unless already given) to read Verus's per-function
//! breakdown. Then it re-runs every captured session under `N`
//! configurations of the same pinned Z3 (`sail-portfolio`, ADR 0018) and
//! reports per-check fragility: a human summary on stdout, or the whole
//! report as JSON with `--json` (or `--output json`, as `sail verify` and
//! the MCP `fragility` tool spell it).
//!
//! This is an opt-in measurement. It never changes a verdict, and it never
//! reads or writes the cache, so it cannot change the cache keys or answers
//! of `sail verify`. `--traces DIR` measures sessions captured earlier
//! instead of running Verus.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use sail_portfolio::fragility::{self, Options, Report};
use sail_portfolio::{Solver, sessions};
use serde::Serialize;
use serde_json::Value;

/// Exit code for a setup or usage failure.
const EXIT_SETUP: u8 = 2;

/// The flags Verus needs for the per-function breakdown.
const VERUS_FLAGS: &[&str] = &["--output-json", "--time"];

/// Parsed command line.
#[derive(Debug, Clone, PartialEq)]
struct Args {
    options: Options,
    top: usize,
    json: bool,
    focus: bool,
    keep_traces: Option<PathBuf>,
    traces: Option<PathBuf>,
    z3: Option<PathBuf>,
    timeout_secs: u64,
    passthrough: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        options: Options::default(),
        top: 10,
        json: false,
        focus: false,
        keep_traces: None,
        traces: None,
        z3: None,
        timeout_secs: 3600,
        passthrough: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{arg} needs a value"));
        let number = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--seeds" => a.options.seeds = number(value()?)?,
            "--shuffles" => a.options.shuffles = number(value()?)?,
            "--jobs" => a.options.jobs = usize::try_from(number(value()?)?).unwrap_or(1),
            "--top" => a.top = usize::try_from(number(value()?)?).unwrap_or(0),
            "--timeout-secs" => a.timeout_secs = number(value()?)?,
            "--near-limit" => {
                a.options.near_limit = value()?
                    .parse()
                    .map_err(|_| "--near-limit needs a fraction".to_owned())?;
            }
            "--json" => a.json = true,
            "--output" => {
                let v = value()?;
                if v != "json" {
                    return Err(format!("--output {v}: only `json` is supported"));
                }
                a.json = true;
            }
            "--focus" => a.focus = true,
            "--keep-traces" => a.keep_traces = Some(value()?.into()),
            "--traces" => a.traces = Some(value()?.into()),
            "--z3" => a.z3 = Some(value()?.into()),
            _ => {
                // The first argument sail does not know starts the
                // `cargo verus verify` arguments.
                a.passthrough.push(arg.clone());
                a.passthrough.extend(it.by_ref().cloned());
            }
        }
    }
    if a.options.seeds == 0 || a.options.jobs == 0 || a.timeout_secs == 0 {
        return Err("--seeds, --jobs and --timeout-secs must be at least 1".to_owned());
    }
    Ok(a)
}

/// Verus flags that select part of a crate. `cargo verus` accepts them
/// only under `focus`, so they switch the capture to `cargo verus focus`.
const PARTIAL: &[&str] = &[
    "--verify-module",
    "--verify-only-module",
    "--verify-function",
    "--verify-root",
];

/// Whether the Verus arguments (after `--`) select part of the crate.
fn partial(args: &[String]) -> bool {
    args.iter()
        .skip_while(|a| *a != "--")
        .any(|a| PARTIAL.contains(&a.as_str()))
}

/// `cargo verus verify` arguments with Verus's breakdown flags added after
/// `--` (once).
fn with_verus_flags(args: &[String]) -> Vec<String> {
    let mut out = args.to_vec();
    let after = args.iter().position(|a| a == "--");
    let verus_part = after.map_or(&[][..], |p| args.get(p + 1..).unwrap_or_default());
    let missing: Vec<String> = VERUS_FLAGS
        .iter()
        .filter(|f| !verus_part.iter().any(|a| a == *f))
        .map(|f| (*f).to_owned())
        .collect();
    if !missing.is_empty() {
        if after.is_none() {
            out.push("--".to_owned());
        }
        out.extend(missing);
    }
    out
}

/// One function from Verus's `--time` breakdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct VerusFunction {
    function: String,
    success: bool,
    rlimit: u64,
}

/// The functions in every `--output-json` document on `stdout`.
fn verus_functions(stdout: &[u8]) -> Vec<VerusFunction> {
    let mut out = Vec::new();
    let stream = serde_json::Deserializer::from_slice(stdout).into_iter::<Value>();
    for doc in stream.map_while(Result::ok) {
        let modules = doc
            .pointer("/times-ms/smt/smt-run-module-times")
            .and_then(Value::as_array);
        for f in modules
            .into_iter()
            .flatten()
            .filter_map(|m| m.get("function-breakdown")?.as_array())
            .flatten()
        {
            out.push(VerusFunction {
                function: f
                    .get("function")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                success: f.get("success").and_then(Value::as_bool).unwrap_or(false),
                rlimit: f.get("rlimit").and_then(Value::as_u64).unwrap_or(0),
            });
        }
    }
    out
}

/// The JSON document `--json` prints.
#[derive(Serialize)]
struct JsonOutput<'a> {
    /// Format version of this document.
    schema_version: u32,
    /// Always `fragility`: an opt-in measurement, not a verification result.
    mode: &'static str,
    /// Verus's exit code on the capture run (`None` with `--traces`).
    verus_exit_code: Option<i32>,
    /// Verus's per-function breakdown from the capture run.
    verus_functions: &'a [VerusFunction],
    /// Whether checks were attributed to a function (only when Verus
    /// reported exactly one function that used solver effort).
    attributed: bool,
    /// The fragility report.
    report: &'a Report,
}

fn fail(err: &mut dyn std::io::Write, code: &str, msg: &str) -> ExitCode {
    let _ = writeln!(err, "error[{code}]: sail fragility: {msg}");
    ExitCode::from(EXIT_SETUP)
}

/// What the capture produced.
struct Captured {
    exit_code: Option<i32>,
    functions: Vec<VerusFunction>,
}

/// Runs `cargo verus <subcommand>` through the shim with tracing into
/// `dir`.
fn capture(
    cargo_verus: &Path,
    subcommand: &str,
    shim: &Path,
    z3: &Path,
    dir: &Path,
    args: &[String],
) -> Result<Captured, String> {
    let output = Command::new(cargo_verus)
        .arg("verus")
        .arg(subcommand)
        .args(with_verus_flags(args).iter().map(OsString::from))
        .env("VERUS_Z3_PATH", shim)
        .env("SAIL_Z3_PATH", z3)
        .env("SAIL_TRACE_DIR", dir)
        .env_remove("SAIL_DAEMON_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", cargo_verus.display()))?;
    Ok(Captured {
        exit_code: output.status.code(),
        functions: verus_functions(&output.stdout),
    })
}

/// A fresh trace directory under the sail cache.
fn fresh_trace_dir() -> Result<PathBuf, String> {
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    Ok(cache
        .join("fragility")
        .join(format!("{}-{nanos}", std::process::id())))
}

/// Runs `sail fragility <args>`.
pub fn fragility(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let a = match parse(args) {
        Ok(a) => a,
        Err(e) => return fail(err, "SAIL0110", &e),
    };
    let shim = match std::env::current_exe() {
        Ok(me) => me.with_file_name("sail-z3"),
        Err(e) => return fail(err, "SAIL0102", &format!("cannot locate sail: {e}")),
    };
    let cargo_verus = crate::verify::cargo_verus();
    let z3 = a.z3.clone().or_else(|| {
        cargo_verus
            .as_deref()
            .and_then(|cv| crate::verify::real_z3(cv, &shim))
    });
    let Some(z3) = z3 else {
        return fail(
            err,
            "SAIL0103",
            "cannot find the Z3 Verus uses (set VERUS_Z3_PATH or pass --z3)",
        );
    };
    let (dir, captured, temporary) = if let Some(dir) = a.traces.clone() {
        (dir, None, false)
    } else {
        let Some(cargo_verus) = cargo_verus else {
            return fail(
                err,
                "SAIL0101",
                "`cargo-verus` is not on PATH (or set SAIL_CARGO_VERUS)",
            );
        };
        if !shim.is_file() {
            return fail(
                err,
                "SAIL0102",
                &format!("sail-z3 not found next to sail at {}", shim.display()),
            );
        }
        let (dir, temporary) = match a.keep_traces.clone() {
            Some(d) => (d, false),
            None => match fresh_trace_dir() {
                Ok(d) => (d, true),
                Err(e) => return fail(err, "SAIL0104", &e),
            },
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return fail(
                err,
                "SAIL0111",
                &format!("cannot create {}: {e}", dir.display()),
            );
        }
        match capture(
            &cargo_verus,
            if a.focus || partial(&a.passthrough) {
                "focus"
            } else {
                "verify"
            },
            &shim,
            &z3,
            &dir,
            &a.passthrough,
        ) {
            Ok(c) => (dir, Some(c), temporary),
            Err(e) => return fail(err, "SAIL0105", &e),
        }
    };
    let result = measure(&a, &z3, &dir, captured.as_ref());
    if temporary {
        let _ = std::fs::remove_dir_all(&dir);
    }
    match result {
        Ok((report, attributed)) => print(&a, &report, attributed, captured.as_ref(), out, err),
        Err(e) => fail(err, "SAIL0112", &e),
    }
}

/// Prints the result: JSON with `--json`, else a human summary.
fn print(
    a: &Args,
    report: &Report,
    attributed: bool,
    captured: Option<&Captured>,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    if a.json {
        let doc = JsonOutput {
            schema_version: 1,
            mode: "fragility",
            verus_exit_code: captured.and_then(|c| c.exit_code),
            verus_functions: captured.map(|c| c.functions.as_slice()).unwrap_or_default(),
            attributed,
            report,
        };
        match serde_json::to_string_pretty(&doc) {
            Ok(text) => {
                let _ = writeln!(out, "{text}");
            }
            Err(e) => return fail(err, "SAIL0113", &e.to_string()),
        }
    } else {
        if let Some(c) = captured {
            let failed = c.functions.iter().filter(|f| !f.success).count();
            let _ = writeln!(
                out,
                "verus: exit {:?}, {} functions ({} failing); checks attributed to a function: {}",
                c.exit_code,
                c.functions.len(),
                failed,
                if attributed {
                    "yes"
                } else {
                    "no (more than one function; see ADR 0018)"
                }
            );
        }
        let _ = write!(out, "{}", fragility::render(report, a.top));
    }
    ExitCode::SUCCESS
}

fn measure(
    a: &Args,
    z3: &Path,
    dir: &Path,
    captured: Option<&Captured>,
) -> Result<(Report, bool), String> {
    let found = sessions::discover(dir).map_err(|e| e.to_string())?;
    if found.is_empty() {
        return Err(format!(
            "no solver sessions were captured in {}{}",
            dir.display(),
            match captured.and_then(|c| c.exit_code) {
                Some(0) => " (Verus exited 0: Cargo may have found the crate up to date; \
                            change a source file or run `cargo clean -p <crate>`, then retry)"
                    .to_owned(),
                Some(c) => format!(" (Verus exited {c})"),
                None => String::new(),
            }
        ));
    }
    let solver = Solver {
        path: z3.to_path_buf(),
        timeout: std::time::Duration::from_secs(a.timeout_secs),
    };
    let mut report =
        fragility::measure(&solver, &found, &a.options, |_, _| {}).map_err(|e| e.to_string())?;
    let names: Vec<String> = captured
        .map(|c| {
            c.functions
                .iter()
                .filter(|f| f.rlimit > 0)
                .map(|f| f.function.clone())
                .collect()
        })
        .unwrap_or_default();
    let attributed = fragility::attribute_single_function(&mut report, &names);
    Ok((report, attributed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn parses_sail_flags_then_passes_the_rest_through() {
        let a = parse(&strings(
            "--seeds 3 --json --focus -p demo -- --verify-function f",
        ));
        let Ok(a) = a else { unreachable!("{a:?}") };
        assert_eq!(a.options.seeds, 3);
        assert!(a.json && a.focus);
        assert_eq!(a.passthrough, strings("-p demo -- --verify-function f"));
        // A sail flag after the first cargo argument belongs to cargo.
        let a = parse(&strings("-p demo --seeds 3"));
        let Ok(a) = a else { unreachable!("{a:?}") };
        assert_eq!(a.options.seeds, 8);
        assert_eq!(a.passthrough, strings("-p demo --seeds 3"));
        assert!(parse(&strings("--seeds 0")).is_err());
        assert!(parse(&strings("--seeds x")).is_err());
        assert!(parse(&strings("--json --seeds")).is_err());
        let a = parse(&strings("--output json -p x"));
        assert!(a.is_ok_and(|a| a.json));
        assert!(parse(&strings("--output text")).is_err());
    }

    #[test]
    fn partial_selectors_need_focus() {
        assert!(partial(&strings(
            "-p x -- --verify-root --verify-function f"
        )));
        assert!(partial(&strings("-- --verify-module m")));
        assert!(!partial(&strings("-p x -- --rlimit 20")));
        // Before `--` they are cargo arguments, not Verus selectors.
        assert!(!partial(&strings("--verify-function f")));
    }

    #[test]
    fn adds_verus_flags_once() {
        assert_eq!(with_verus_flags(&[]), strings("-- --output-json --time"));
        assert_eq!(
            with_verus_flags(&strings("-p x -- --time")),
            strings("-p x -- --time --output-json")
        );
        assert_eq!(
            with_verus_flags(&strings("-- --output-json --time")),
            strings("-- --output-json --time")
        );
    }

    #[test]
    fn reads_the_function_breakdown() {
        let stdout = br#"{"times-ms":{"smt":{"smt-run-module-times":[{"module":"","function-breakdown":[{"function":"m::f","success":true,"rlimit":5},{"function":"m::g","success":false,"rlimit":7}]}]}}}
{"times-ms":{"smt":{"smt-run-module-times":[]}}}"#;
        let fns = verus_functions(stdout);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[1].function, "m::g");
        assert!(!fns[1].success);
        assert_eq!(verus_functions(b"not json"), vec![]);
    }
}
