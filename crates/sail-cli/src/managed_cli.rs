//! `sail verify`'s managed verification (ADR 0023, ADR 0046) and background
//! submission. This is what every `sail verify` runs, except the explicit
//! fragility measurement and seed portfolio ([`crate::verify`]).

use std::path::PathBuf;
use std::process::ExitCode;

use sail_api::project::{Currentness, Execution, ProjectResult, SCHEMA_VERSION};

struct Options {
    background: bool,
    json: bool,
    args: Vec<String>,
}

/// Sail's own options before any `--`: `--background` and `--output json`.
/// (`--incremental` is removed earlier; `--engine` is read with the
/// arguments by [`crate::jobs::engine_options`].) Everything else goes to
/// `cargo verus verify`.
fn options(args: &[String]) -> Result<Options, (&'static str, String)> {
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
            "--background" => result.background = true,
            "--output" | "--output=json" => {
                let format = if arg == "--output" {
                    iter.next().map(String::as_str)
                } else {
                    Some("json")
                };
                if format != Some("json") {
                    return Err((
                        "SAIL0106",
                        format!(
                            "unknown output format `{}` (expected `--output json`)",
                            format.unwrap_or_default()
                        ),
                    ));
                }
                result.json = true;
            }
            other if other.starts_with("--output=") => {
                return Err((
                    "SAIL0106",
                    format!(
                        "unknown output format `{}` (expected `--output json`)",
                        &other["--output=".len()..]
                    ),
                ));
            }
            _ => result.args.push(arg.clone()),
        }
    }
    Ok(result)
}

pub(crate) fn config(args: Vec<String>) -> Result<sail_verus::managed::Config, String> {
    let cargo_verus =
        crate::verify::cargo_verus().map_err(|message| format!("error[SAIL0101]: {message}"))?;
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
    // A message that carries its own code (Sail's Verus missing, an
    // unsupported configuration) keeps it on stderr.
    let (code, text) = message
        .strip_prefix("error[")
        .and_then(|rest| rest.split_once("]: "))
        .unwrap_or(("SAIL3100", message));
    let _ = writeln!(err, "error[{code}]: sail verify: {text}");
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
            engine: None,
        };
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string(&result).unwrap_or_default()
        );
    }
    ExitCode::from(2)
}

/// Runs managed verification. Without `--output json`, stdout and stderr
/// are what plain `cargo verus verify` prints, and the exit code is its exit
/// code; with it, stdout is one `sail-project-result` document.
pub(crate) fn verify(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let options = match options(args) {
        Ok(o) => o,
        Err((code, message)) => {
            let _ = writeln!(err, "error[{code}]: sail verify: {message}");
            return ExitCode::from(2);
        }
    };
    if options.background {
        let mut args = options.args;
        if options.json {
            args.splice(0..0, ["--output".into(), "json".into()]);
        }
        return crate::jobs::background(&args, out, err);
    }
    if std::env::var_os("SAIL_JOB_GUARD_PARENT").is_some_and(|v| v == "1") {
        crate::portfolio::guard_parent();
    }
    let config = match config(options.args) {
        Ok(c) => c,
        Err(message) => return failure(&message, out, err, options.json),
    };
    let (mode, args) = match crate::jobs::engine_options(&config.args) {
        Ok(options) => options,
        Err(message) => return failure(&message, out, err, options.json),
    };
    let mut config = config;
    config.args = args;
    let result = if mode == sail_api::jobs::Mode::Interactive {
        sail_verus::engine::verify(&config, err)
    } else if options.json {
        sail_verus::managed::run(&config, err)
    } else {
        sail_verus::managed::run_showing_output(&config, out, err)
    };
    match result {
        Ok(result) => {
            if options.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&result).unwrap_or_default()
                );
            }
            ExitCode::from(
                result
                    .exit_code
                    .and_then(|n| u8::try_from(n & 0xff).ok())
                    .unwrap_or(1),
            )
        }
        Err(error) => failure(&error.to_string(), out, err, options.json),
    }
}

#[cfg(test)]
mod tests {
    use super::options;

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).into()).collect()
    }

    #[test]
    fn only_flags_before_the_separator_are_sails() {
        let o = options(&args(&["--", "--background", "--output", "json"]))
            .unwrap_or_else(|e| unreachable!("fixture: {e:?}"));
        assert!(!o.json && !o.background);
        assert_eq!(o.args, ["--", "--background", "--output", "json"]);
        let o = options(&args(&[
            "--output=json",
            "-p",
            "demo",
            "--background",
            "--",
            "--rlimit",
            "10",
        ]))
        .unwrap_or_else(|e| unreachable!("fixture: {e:?}"));
        assert!(o.json && o.background);
        assert_eq!(o.args, ["-p", "demo", "--", "--rlimit", "10"]);
    }

    #[test]
    fn only_json_output_is_known() {
        for bad in [&["--output", "yaml"][..], &["--output"], &["--output=yaml"]] {
            assert_eq!(options(&args(bad)).err().map(|e| e.0), Some("SAIL0106"));
        }
    }
}
