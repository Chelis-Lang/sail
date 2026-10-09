//! Explicit managed verification and background submission.

use std::path::PathBuf;
use std::process::ExitCode;

use sail_api::project::{Currentness, Execution, ProjectResult, SCHEMA_VERSION};

struct Options {
    background: bool,
    json: bool,
    args: Vec<String>,
}

fn options(args: &[String]) -> Result<Option<Options>, String> {
    let before = args.iter().take_while(|a| a.as_str() != "--");
    if !before
        .clone()
        .any(|a| a == "--incremental" || a == "--background")
    {
        return Ok(None);
    }
    if before
        .clone()
        .any(|a| a.starts_with("--portfolio") || a.starts_with("--measure-fragility"))
    {
        return Err("managed workspace verification cannot be combined with portfolio or fragility measurement".into());
    }
    let mut result = Options {
        background: false,
        json: false,
        args: Vec::new(),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--" => {
                result.args.push(arg.clone());
                result.args.extend(iter.cloned());
                break;
            }
            "--incremental" => {}
            "--background" => result.background = true,
            "--output=json" => result.json = true,
            "--output" => {
                if iter.next().map(String::as_str) != Some("json") {
                    return Err("expected --output json".into());
                }
                result.json = true;
            }
            _ => result.args.push(arg.clone()),
        }
    }
    Ok(Some(result))
}

fn config(args: Vec<String>) -> Result<sail_verus::managed::Config, String> {
    let cargo_verus = crate::verify::cargo_verus()
        .ok_or("error[SAIL0101]: cargo-verus not found; set SAIL_CARGO_VERUS")?;
    let sail = std::env::current_exe().map_err(|e| e.to_string())?;
    let shim = sail.with_file_name("sail-z3");
    if !shim.is_file() {
        return Err("error[SAIL0102]: sail-z3 must be installed next to sail".into());
    }
    let z3 = crate::verify::real_z3(&cargo_verus, &shim)
        .ok_or("error[SAIL0103]: cannot find the solver; set VERUS_Z3_PATH")?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let socket = std::env::var_os("SAIL_DAEMON_SOCKET")
        .map_or_else(|| crate::default_socket(&cache), PathBuf::from);
    let daemon = crate::verify::ensure_daemon(&socket);
    let cargo = std::env::var_os("CARGO")
        .map(PathBuf::from)
        .or_else(|| crate::verify::on_path("cargo"))
        .ok_or("error[SAIL3100]: cargo is not on PATH")?;
    Ok(sail_verus::managed::Config {
        workspace: std::env::current_dir().map_err(|e| e.to_string())?,
        cache,
        sail,
        cargo_verus,
        cargo,
        shim,
        z3,
        socket: daemon.then_some(socket),
        args,
        force: std::env::var_os("SAIL_MANAGED_FORCE").is_some_and(|v| v == "1"),
    })
}

fn failure(
    message: &str,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    json: bool,
) -> ExitCode {
    let _ = writeln!(err, "error[SAIL3100]: managed verification: {message}");
    if json {
        let result = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: SCHEMA_VERSION,
            run_id: String::new(),
            workspace: std::env::current_dir()
                .map_or_else(|_| String::new(), |p| p.to_string_lossy().into()),
            context_sha256: String::new(),
            input_revision: String::new(),
            currentness: Currentness::Unknown,
            exit_code: None,
            targets: Vec::new(),
            execution: Execution::default(),
            notes: Vec::new(),
            error: Some(sail_api::output::ErrorInfo {
                code: "SAIL3100".into(),
                message: message.into(),
            }),
        };
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string(&result).unwrap_or_default()
        );
    }
    ExitCode::from(2)
}

/// Handles managed flags, or leaves ordinary verification to its existing path.
pub(crate) fn verify(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Option<ExitCode> {
    let options = match options(args) {
        Ok(Some(o)) => o,
        Ok(None) => return None,
        Err(message) => return Some(failure(&message, out, err, false)),
    };
    if options.background {
        let mut args = options.args;
        if options.json {
            args.extend(["--output".into(), "json".into()]);
        }
        return Some(crate::jobs::background(&args, out, err));
    }
    if std::env::var_os("SAIL_JOB_GUARD_PARENT").is_some_and(|v| v == "1") {
        crate::portfolio::guard_parent();
    }
    let config = match config(options.args) {
        Ok(c) => c,
        Err(message) => return Some(failure(&message, out, err, options.json)),
    };
    match sail_verus::managed::run(&config, err) {
        Ok(result) => {
            if options.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&result).unwrap_or_default()
                );
            } else {
                let _ = writeln!(
                    out,
                    "{} targets: {} verified now, {} reused ({:?})",
                    result.targets.len(),
                    result.execution.verifier_invocations,
                    result.execution.reused_targets,
                    result.currentness
                );
            }
            Some(ExitCode::from(
                result
                    .exit_code
                    .and_then(|n| u8::try_from(n & 0xff).ok())
                    .unwrap_or(1),
            ))
        }
        Err(error) => Some(failure(&error.to_string(), out, err, options.json)),
    }
}

#[cfg(test)]
mod tests {
    use super::options;

    #[test]
    fn only_flags_before_separator_select_managed_verification() {
        let args = |xs: &[&str]| xs.iter().map(|x| (*x).into()).collect::<Vec<String>>();
        assert!(options(&args(&["--", "--incremental"])).is_ok_and(|o| o.is_none()));
        let Some(o) = options(&args(&[
            "--incremental",
            "--output=json",
            "-p",
            "demo",
            "--",
            "--rlimit",
            "10",
        ]))
        .unwrap_or_else(|e| unreachable!("fixture: {e}")) else {
            unreachable!("managed flags ignored")
        };
        assert!(o.json && !o.background);
        assert_eq!(o.args, ["-p", "demo", "--", "--rlimit", "10"]);
        assert!(
            options(&args(&[
                "--incremental",
                "--portfolio",
                "accept-first-proof"
            ]))
            .is_err()
        );
    }
}
