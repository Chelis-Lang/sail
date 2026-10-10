//! Private Cargo and Verus-driver roles for the same installed Sail executable.
//!
//! Metadata, rustc probes, build scripts, and non-verification packages are
//! forwarded with their original argument bytes. Verified targets alone get
//! isolated observation sidecars; these flags leave solver input unchanged.

use crate::managed::{Invocation, Runtime, TargetIdentity, digest, event, file_digest, json};
use crate::{Error, Result};
use std::ffi::OsString;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn required(name: &str) -> Result<OsString> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::new("SAIL3110", format!("private adapter missing {name}")))
}

fn status(status: std::process::ExitStatus) -> ExitCode {
    ExitCode::from(
        status
            .code()
            .and_then(|n| u8::try_from(n & 255).ok())
            .unwrap_or(1),
    )
}

/// Dispatches installed private executable aliases before public CLI parsing.
/// Returns `None` for an ordinary Sail invocation.
#[must_use]
pub fn dispatch() -> Option<ExitCode> {
    let name = std::env::args_os()
        .next()
        .and_then(|p| PathBuf::from(p).file_name().map(std::ffi::OsStr::to_owned));
    let role = name.as_deref()?.to_str()?;
    let outcome = match role {
        "sail-cargo" => cargo(),
        "sail-verus-driver" => driver(),
        _ => return None,
    };
    Some(outcome.unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::from(2)
    }))
}

fn cargo() -> Result<ExitCode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut cmd = Command::new(required("SAIL_MANAGED_CARGO")?);
    cmd.args(&args);
    let build = args.iter().any(|s| s == "build" || s == "check");
    if !build {
        return cmd
            .status()
            .map(status)
            .map_err(|e| Error::new("SAIL3110", e.to_string()));
    }
    cmd.env("SAIL_MANAGED_WRAPPER", required("RUSTC_WRAPPER")?)
        .env("RUSTC_WRAPPER", required("SAIL_MANAGED_PROXY")?);
    // Cargo messages remain on stdout; target Verus documents live in sidecars.
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let path = PathBuf::from(required("SAIL_MANAGED_ARTIFACTS")?);
    let mut records = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::new("SAIL3110", e.to_string()))?;
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3110", e.to_string()))?;
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout).split(b'\n') {
            let line = line.map_err(|e| Error::new("SAIL3110", e.to_string()))?;
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line)
                && matches!(
                    value["reason"].as_str(),
                    Some("compiler-artifact" | "build-script-executed")
                )
            {
                records
                    .write_all(&line)
                    .and_then(|()| records.write_all(b"\n"))
                    .map_err(|e| Error::new("SAIL3110", e.to_string()))?;
            }
            std::io::stdout()
                .write_all(&line)
                .and_then(|()| std::io::stdout().write_all(b"\n"))
                .map_err(|e| Error::new("SAIL3110", e.to_string()))?;
        }
    }
    child
        .wait()
        .map(status)
        .map_err(|e| Error::new("SAIL3110", e.to_string()))
}

fn argument(args: &[OsString], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].to_string_lossy().into_owned())
}

fn verifying(args: &[OsString]) -> bool {
    if argument(args, "--crate-name").as_deref() == Some("___")
        || std::env::var("CARGO_CRATE_NAME").is_ok_and(|v| v.starts_with("build_script_"))
    {
        return false;
    }
    let package = ["CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR"].map(std::env::var);
    let [Ok(name), Ok(version), Ok(manifest)] = package else {
        return false;
    };
    let hash = digest(
        Path::new(&manifest)
            .join("Cargo.toml")
            .to_string_lossy()
            .as_bytes(),
    );
    let id = format!("{name}-{version}-{}", hash.get(..12).unwrap_or_default());
    std::env::var(format!("__VERUS_DRIVER_VERIFY_{id}")).as_deref() == Ok("1")
}

fn custom_compiler_context(args: &[OsString]) -> bool {
    if args
        .iter()
        .any(|a| a == "--sysroot" || a.to_string_lossy().starts_with("--sysroot="))
    {
        return true;
    }
    let target = argument(args, "--target").or_else(|| {
        args.iter()
            .find_map(|a| a.to_str()?.strip_prefix("--target=").map(str::to_owned))
    });
    target.is_some_and(|v| {
        Path::new(&v)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|v| v.eq_ignore_ascii_case("json"))
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "Engine fallback is kept alongside the owned subprocess drain and seal check."
)]
fn driver() -> Result<ExitCode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let wrapper = PathBuf::from(required("SAIL_MANAGED_WRAPPER")?);
    if !verifying(&args) {
        return Command::new(wrapper)
            .args(args)
            .status()
            .map(status)
            .map_err(|e| Error::new("SAIL3111", e.to_string()));
    }
    if custom_compiler_context(&args) {
        return Err(Error::new(
            "SAIL3111",
            "incremental verification does not support a custom sysroot or JSON target specification",
        ));
    }
    let runtime_path = PathBuf::from(required("SAIL_MANAGED_RUNTIME")?);
    let runtime: Runtime = serde_json::from_slice(
        &std::fs::read(runtime_path).map_err(|e| Error::new("SAIL3111", e.to_string()))?,
    )
    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let identity = TargetIdentity {
        manifest: PathBuf::from(required("CARGO_MANIFEST_DIR")?).join("Cargo.toml"),
        name: argument(&args, "--crate-name").unwrap_or_default(),
        crate_type: argument(&args, "--crate-type").unwrap_or_default(),
        rustc_args_sha256: digest(
            &args
                .iter()
                .flat_map(|s| s.as_encoded_bytes().iter().copied().chain([0]))
                .collect::<Vec<_>>(),
        ),
    };
    crate::managed::retire_receipts(&runtime, &identity)?;
    let dir = runtime.sidecars.join(identity.key());
    std::fs::create_dir_all(dir.join("provenance"))
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let logs = dir.join("logs");
    let mut cmd = Command::new(&wrapper);
    cmd.args(&args)
        .args([
            "--output-json",
            "--time",
            "--log",
            "air",
            "--log",
            "smt-transcript",
            "--log-dir",
        ])
        .arg(&logs)
        .env("SAIL_PROVENANCE_DIR", dir.join("provenance"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let source_events = dir.join("engine.events.jsonl");
    if let Some(engine) = &runtime.engine {
        cmd.env("SAIL_ENGINE", "interactive")
            .env("SAIL_ENGINE_EVENTS", &source_events)
            .env("SAIL_ENGINE_FOCUS_REPORT", dir.join("focus.source.json"));
        if let Some(function) = &engine.function {
            cmd.env("SAIL_ENGINE_FUNCTION", function);
        }
    }
    let relay = runtime
        .engine
        .as_ref()
        .map(|_| crate::engine_events::Relay::start(source_events.clone(), &runtime, &identity));
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    json(
        &runtime.invocations.join(format!("{}.started", child.id())),
        &serde_json::json!({"target":identity.name,"run_id":runtime.run_id}),
    )?;
    event(
        &serde_json::json!({"event":"target-started","run_id":runtime.run_id,"target":identity.name}),
    );
    let stdout = child.stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    });
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        let mut buf = vec![0; 65536];
        loop {
            let n = pipe
                .read(&mut buf)
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            if n == 0 {
                break;
            }
            stderr.extend_from_slice(&buf[..n]);
            if runtime.engine.is_none() {
                std::io::stderr()
                    .write_all(&buf[..n])
                    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            }
        }
    }
    let exit = child
        .wait()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let stdout = stdout
        .ok_or_else(|| Error::new("SAIL3111", "missing solver output pipe"))?
        .join()
        .map_err(|_| Error::new("SAIL3111", "solver output reader failed"))?
        .map_err(|e| Error::new("SAIL3111", format!("cannot capture solver output: {e}")))?;
    if let Some(relay) = relay {
        relay.finish();
    }
    let (stdout, stderr, exit) = if let Some(engine) = &runtime.engine {
        if crate::engine_events::sealed(&source_events) {
            save_focus(&runtime, &identity, &dir)?;
            (stdout, stderr, exit)
        } else {
            // A source worker that did not reach the late EXEC seal contributes
            // no proof receipt or focused evidence. Stock verification starts
            // from its original compiler invocation and immutable solver mode.
            std::fs::write(dir.join("engine.failed.stdout"), &stdout)
                .and_then(|()| std::fs::write(dir.join("engine.failed.stderr"), &stderr))
                .map_err(|e| Error::new("SAIL3121", e.to_string()))?;
            let _ = std::fs::remove_file(dir.join("focus.source.json"));
            let output = Command::new(&engine.stock_wrapper)
                .args(&args)
                .args([
                    "--output-json",
                    "--time",
                    "--log",
                    "air",
                    "--log",
                    "smt-transcript",
                    "--log-dir",
                ])
                .arg(&logs)
                .env_remove("SAIL_ENGINE")
                .env_remove("SAIL_ENGINE_FUNCTION")
                .env_remove("SAIL_ENGINE_EVENTS")
                .env_remove("SAIL_ENGINE_FOCUS_REPORT")
                .env("SAIL_PROVENANCE_DIR", dir.join("provenance"))
                .output()
                .map_err(|e| Error::new("SAIL3121", e.to_string()))?;
            (output.stdout, output.stderr, output.status)
        }
    } else {
        (stdout, stderr, exit)
    };
    if runtime.engine.is_some() {
        std::io::stderr()
            .write_all(&stderr)
            .map_err(|e| Error::new("SAIL3121", e.to_string()))?;
    }
    finish(&runtime, identity, &dir, &stdout, &stderr, exit.code())?;
    Ok(status(exit))
}

fn finish(
    runtime: &Runtime,
    identity: TargetIdentity,
    dir: &Path,
    stdout: &[u8],
    stderr: &[u8],
    exit: Option<i32>,
) -> Result<()> {
    std::fs::write(dir.join("verus.stdout"), stdout)
        .and_then(|()| std::fs::write(dir.join("verus.stderr"), stderr))
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let (docs, rest) = crate::attribution::json_documents(stdout);
    std::io::stderr()
        .write_all(&rest)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let logs = crate::attribution::read_logs(&dir.join("logs")).unwrap_or_default();
    let provenance = if runtime.daemon {
        crate::attribution::Provenance::Records(crate::attribution::read_session_records(
            &dir.join("provenance"),
        ))
    } else {
        crate::attribution::Provenance::NoCache
    };
    let attr = crate::attribution::attribute(&docs, Some(&logs), &provenance, None);
    // Forwarded selectors are per package. A dependency did not receive the
    // requesting package's focused flags, so its full proof is not scoped away.
    let mut report_args = runtime.args.clone();
    if crate::managed::focused(&report_args) && !actual_focus() {
        report_args.truncate(
            report_args
                .iter()
                .position(|a| a == "--")
                .unwrap_or(report_args.len()),
        );
    }
    let report = crate::report::build(&crate::report::Inputs {
        args: &report_args,
        exit_code: exit,
        solver_path: Some(runtime.z3.to_string_lossy().into_owned()),
        solver_sha256: Some(runtime.z3_sha256.clone()),
        daemon: runtime.daemon,
        attribution: &attr,
        diagnostics: &crate::diagnostics::parse(&rendered_diagnostics(stderr)),
    });
    let success =
        crate::managed::receipt_complete(&report) && !crate::managed::focused(&report_args);
    let dependencies = driver_dependencies()?;
    let invocation = Invocation {
        identity,
        run_id: runtime.run_id.clone(),
        report,
        success,
        dependencies,
    };
    json(
        &runtime
            .invocations
            .join(format!("{}.json", invocation.identity.key())),
        &invocation,
    )?;
    event(
        &serde_json::json!({"event":"target-complete","run_id":runtime.run_id,"target":invocation.identity.name,"success":success}),
    );
    // Solver bytes are evidence, never a transformed input.
    if file_digest(&runtime.original_wrapper).is_err() {
        return Err(Error::new(
            "SAIL3111",
            "Verus disappeared during verification",
        ));
    }
    Ok(())
}

fn rendered_diagnostics(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .fold(String::new(), |mut text, line| {
            let value = serde_json::from_str::<serde_json::Value>(line).ok();
            if let Some(rendered) = value.as_ref().and_then(|v| v["rendered"].as_str()) {
                text.push_str(rendered);
            } else {
                text.push_str(line);
                text.push('\n');
            }
            text
        })
}

fn actual_focus() -> bool {
    let package = ["CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR"].map(std::env::var);
    let [Ok(name), Ok(version), Ok(manifest)] = package else {
        return true;
    };
    let hash = digest(
        Path::new(&manifest)
            .join("Cargo.toml")
            .to_string_lossy()
            .as_bytes(),
    );
    let id = format!("{name}-{version}-{}", hash.get(..12).unwrap_or_default());
    [
        " __VERUS_DRIVER_ARGS__".to_owned(),
        format!(" __VERUS_DRIVER_ARGS_FOR_{id}"),
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok())
    .any(|value| {
        [
            "--verify-function",
            "--verify-module",
            "--verify-only-module",
            "--verify-root",
        ]
        .iter()
        .any(|flag| value.contains(flag))
    })
}

fn driver_dependencies() -> Result<std::collections::BTreeMap<PathBuf, String>> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut files = std::collections::BTreeMap::new();
    for pair in args.windows(2).filter(|w| w[0] == "--extern") {
        if let Some((_, path)) = pair[1].to_str().and_then(|s| s.split_once('=')) {
            let path = PathBuf::from(path);
            if path.is_file() {
                files.insert(path.clone(), file_digest(&path)?);
            }
            let vir = path.with_extension("vir");
            if vir.is_file() {
                files.insert(vir.clone(), file_digest(&vir)?);
            }
        }
    }
    Ok(files)
}

fn save_focus(runtime: &Runtime, identity: &TargetIdentity, dir: &Path) -> Result<()> {
    let path = dir.join("focus.source.json");
    if !path.is_file() {
        return Ok(());
    }
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path).map_err(|e| Error::new("SAIL3121", e.to_string()))?,
    )
    .map_err(|e| Error::new("SAIL3121", e.to_string()))?;
    let function = source["function"]
        .as_str()
        .ok_or_else(|| Error::new("SAIL3121", "focused source evidence has no function"))?;
    let logs = crate::attribution::read_logs(&dir.join("logs/focus")).unwrap_or_default();
    let docs = vec![source["verus"].clone()];
    let attr = crate::attribution::attribute(
        &docs,
        Some(&logs),
        &crate::attribution::Provenance::NoCache,
        None,
    );
    let mut args = runtime.args.clone();
    args.extend([
        "--".to_owned(),
        "--verify-function".to_owned(),
        function.to_owned(),
    ]);
    let report = crate::report::build(&crate::report::Inputs {
        args: &args,
        exit_code: Some(i32::from(
            source["verus"]["verification-results"]["errors"]
                .as_u64()
                .unwrap_or(1)
                > 0,
        )),
        solver_path: Some(runtime.z3.to_string_lossy().into_owned()),
        solver_sha256: Some(runtime.z3_sha256.clone()),
        daemon: false,
        attribution: &attr,
        diagnostics: &[],
    });
    json(
        &dir.join("focus.report.json"),
        &crate::engine::FocusTarget {
            function: function.to_owned(),
            package_id: runtime
                .packages
                .get(&identity.manifest)
                .cloned()
                .unwrap_or_default(),
            target: identity.name.clone(),
            kind: if identity.crate_type == "rlib" {
                "lib".into()
            } else {
                identity.crate_type.clone()
            },
            report,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_custom_toolchain_inputs_fail_closed() {
        for args in [
            vec!["--sysroot", "/custom"],
            vec!["--sysroot=/custom"],
            vec!["--target", "custom.json"],
            vec!["--target=custom.json"],
        ] {
            assert!(custom_compiler_context(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        assert!(!custom_compiler_context(&[
            "--target".into(),
            "x86_64-unknown-linux-gnu".into()
        ]));
    }
}
