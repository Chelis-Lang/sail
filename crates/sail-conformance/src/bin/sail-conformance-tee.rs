//! Complete compiler capture (#632, VI-0039): a tee in front of the Verus driver.
//!
//! `cargo verus` sets `RUSTC_WRAPPER` to the `verus` beside its own
//! executable. The harness places the unchanged `cargo-verus` bytes in a
//! shadow directory with this program as `verus`; Cargo then starts this
//! program once per compiler invocation. It starts the real driver named by
//! `SAIL_CONFORMANCE_TEE_DRIVER` with the same arguments, environment and
//! standard input, forwards every byte the driver writes to stdout and
//! stderr unchanged, and afterwards records the exact bytes of each stream
//! under `SAIL_CONFORMANCE_TEE_DIR`, together with the invocation's identity
//! (package, target and an argument digest).
//!
//! Forwarding never depends on the record: bytes are forwarded before they
//! are retained, and a record that cannot be written leaves no completed
//! directory, which the reader reports as an incomplete capture.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use sail_conformance::invocations::{Meta, argument_digest, hex_sha256};

const DRIVER: &str = "SAIL_CONFORMANCE_TEE_DRIVER";
const DIR: &str = "SAIL_CONFORMANCE_TEE_DIR";

/// Copies `from` to `to` chunk by chunk, forwarding first and retaining
/// second. Returns the retained bytes and the first forwarding error.
fn pump(mut from: impl Read, mut to: impl Write) -> (Vec<u8>, Option<String>) {
    let mut kept = Vec::new();
    let mut error = None;
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        match from.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let bytes = chunk.get(..n).unwrap_or_default();
                if error.is_none()
                    && let Err(e) = to.write_all(bytes).and_then(|()| to.flush())
                {
                    error = Some(e.to_string());
                }
                kept.extend_from_slice(bytes);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                error.get_or_insert(format!("read: {e}"));
                break;
            }
        }
    }
    (kept, error)
}

fn lossy(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn env_string(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

fn meta(raw: &[OsString], started_ns: u128) -> Meta {
    let argv = lossy(raw);
    let values = |flag: &str| -> Vec<String> {
        argv.iter()
            .enumerate()
            .filter_map(|(i, arg)| {
                if arg == flag {
                    argv.get(i + 1).cloned()
                } else {
                    arg.strip_prefix(&format!("{flag}=")).map(str::to_owned)
                }
            })
            .collect()
    };
    let value_after = |flag: &str| values(flag).into_iter().next();
    let mut crate_types = values("--crate-type");
    crate_types.sort();
    let driver_env: BTreeMap<String, String> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let k = k.to_string_lossy().into_owned();
            (k.starts_with("__VERUS_DRIVER_") || k.starts_with(" __VERUS_DRIVER_"))
                .then(|| (k, v.to_string_lossy().into_owned()))
        })
        .collect();
    Meta {
        version: 3,
        package_name: env_string("CARGO_PKG_NAME"),
        package_version: env_string("CARGO_PKG_VERSION"),
        manifest_dir: env_string("CARGO_MANIFEST_DIR"),
        crate_name: value_after("--crate-name"),
        crate_types,
        test: argv.iter().any(|a| a == "--test"),
        bin_name: env_string("CARGO_BIN_NAME"),
        compile_target: value_after("--target"),
        arg_sha256: argument_digest(&argv, &driver_env),
        requested_arg_sha256: argument_digest(&argv, &driver_env),
        stdin_captured: argv.iter().any(|arg| arg == "-"),
        stdin_len: 0,
        stdin_sha256: hex_sha256(b""),
        argv,
        requested_driver_env: driver_env.clone(),
        driver_env,
        work_root: std::env::var_os(DIR)
            .and_then(|p| {
                PathBuf::from(p)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default(),
        source_root: env_string("SAIL_CONFORMANCE_TEE_SOURCE").unwrap_or_default(),
        target_root: env_string("SAIL_CONFORMANCE_TEE_TARGET").unwrap_or_default(),
        runtime_root: std::env::var_os(DRIVER)
            .and_then(|p| {
                PathBuf::from(p)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default(),
        started_ns,
        pid: std::process::id(),
        exit_code: None,
        stdout_len: 0,
        stderr_len: 0,
        stdout_sha256: String::new(),
        stderr_sha256: String::new(),
        forward_error: None,
    }
}

fn record(
    dir: &Path,
    meta: &Meta,
    stdout: &[u8],
    stderr: &[u8],
    stdin: &[u8],
) -> std::io::Result<()> {
    let name = format!("{:032}-{}", meta.started_ns, meta.pid);
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(tmp.join("stdout"), stdout)?;
    std::fs::write(tmp.join("stderr"), stderr)?;
    std::fs::write(tmp.join("stdin"), stdin)?;
    let json = serde_json::to_vec_pretty(meta).map_err(std::io::Error::other)?;
    std::fs::write(tmp.join("meta.json"), json)?;
    // The rename is the completion mark.
    std::fs::rename(&tmp, dir.join(name))
}

fn failure(message: impl std::fmt::Display) -> ExitCode {
    // This remains observable even when both file channels fail and a build
    // script consumes stderr or clears all capture environment variables.
    let _ = sail_verus::run::fail_capture_supervisor();
    let message = format!("error[SAIL3018]: {message}\n");
    if let Some(path) = std::env::var_os("SAIL_CONFORMANCE_TEE_ERRORS")
        && let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path)
    {
        let _ = file.write_all(message.as_bytes());
    }
    let _ = std::io::stderr().write_all(message.as_bytes());
    ExitCode::from(125)
}

fn configure_observation(
    command: &mut Command,
    metadata: &mut Meta,
    name: &str,
) -> Result<(), String> {
    if metadata.package_name.is_some()
        && metadata.argv.iter().any(|a| a == "--error-format=json")
        && metadata.argv.iter().any(|a| {
            a.strip_prefix("--json=")
                .is_some_and(|v| v.split(',').any(|v| v == "artifacts"))
        })
        && let Some(root) = std::env::var_os("SAIL_CONFORMANCE_TEE_LOGS")
    {
        let logs = PathBuf::from(root).join(name);
        sail_verus::run::capture_observation_options(
            command,
            metadata.driver_env.get(" __VERUS_DRIVER_ARGS__").cloned(),
            &logs,
        )
        .map_err(|e| e.to_string())?;
        for (key, value) in command.get_envs() {
            if let (Some(key), Some(value)) = (key.to_str(), value.and_then(|v| v.to_str()))
                && (key.starts_with("__VERUS_DRIVER_") || key.starts_with(" __VERUS_DRIVER_"))
            {
                metadata.driver_env.insert(key.to_owned(), value.to_owned());
            }
        }
        metadata.arg_sha256 = argument_digest(&metadata.argv, &metadata.driver_env);
    }
    Ok(())
}

type Stream = (Vec<u8>, Option<String>);
fn finish_input(input: Option<std::thread::JoinHandle<Stream>>) -> Result<Vec<u8>, String> {
    let Some(thread) = input else {
        return Ok(Vec::new());
    };
    for _ in 0..100 {
        if thread.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    if !thread.is_finished() {
        return Err("compiler ended before complete stdin capture".to_owned());
    }
    let (bytes, error) = thread
        .join()
        .map_err(|_| "stdin thread panicked".to_owned())?;
    if let Some(error) = error {
        return Err(format!("stdin forwarding: {error}"));
    }
    Ok(bytes)
}

fn complete(mut child: std::process::Child, mut metadata: Meta, dir: &Path) -> ExitCode {
    let input = child
        .stdin
        .take()
        .map(|pipe| std::thread::spawn(move || pump(std::io::stdin().lock(), pipe)));
    let out = child.stdout.take();
    let err = child.stderr.take();
    let out_thread = std::thread::spawn(move || match out {
        Some(pipe) => pump(pipe, std::io::stdout().lock()),
        None => (Vec::new(), Some("no stdout pipe".to_owned())),
    });
    let err_thread = std::thread::spawn(move || match err {
        Some(pipe) => pump(pipe, std::io::stderr().lock()),
        None => (Vec::new(), Some("no stderr pipe".to_owned())),
    });
    let status = child.wait();
    let (stdout, out_error) = out_thread
        .join()
        .unwrap_or_else(|_| (Vec::new(), Some("stdout thread panicked".to_owned())));
    let (stderr, err_error) = err_thread
        .join()
        .unwrap_or_else(|_| (Vec::new(), Some("stderr thread panicked".to_owned())));
    let code = status
        .as_ref()
        .ok()
        .and_then(std::process::ExitStatus::code);
    let stdin = match finish_input(input) {
        Ok(input) => input,
        Err(error) => return failure(error),
    };
    metadata.stdin_len = stdin.len();
    metadata.stdin_sha256 = hex_sha256(&stdin);
    metadata.exit_code = code;
    metadata.stdout_len = stdout.len();
    metadata.stderr_len = stderr.len();
    metadata.stdout_sha256 = hex_sha256(&stdout);
    metadata.stderr_sha256 = hex_sha256(&stderr);
    metadata.forward_error = out_error.or(err_error).or_else(|| {
        std::env::vars_os()
            .any(|(k, v)| k.to_str().is_none() || v.to_str().is_none())
            .then(|| "non-UTF-8 invocation metadata cannot be paired".to_owned())
    });
    if let Some(error) = &metadata.forward_error {
        return failure(error);
    }
    if let Err(error) = record(dir, &metadata, &stdout, &stderr, &stdin) {
        return failure(format!("cannot complete capture: {error}"));
    }
    match code {
        Some(code) => ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1)),
        None => ExitCode::from(101),
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args == [OsString::from(sail_verus::run::SUPERVISOR_ARG)]
        && std::env::var_os(sail_verus::run::SUPERVISOR_ENV).as_deref()
            == Some(std::ffi::OsStr::new("1"))
        && std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(std::ffi::OsStr::to_owned))
            .as_deref()
            == Some(std::ffi::OsStr::new("sail-conformance-tee"))
    {
        return match sail_verus::run::capture_supervisor_main() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(125),
        };
    }
    let Some(dir) = std::env::var_os(DIR).map(PathBuf::from) else {
        return failure(format!("{DIR} is not set"));
    };
    let started_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!("{started_ns:032}-{}", std::process::id());
    // Reserve before every fallible preflight, not just before spawning. If
    // the independent health channel is inaccessible, this incomplete marker
    // still prevents an earlier complete job from concealing this invocation.
    if let Err(e) = std::fs::create_dir(dir.join(format!("{name}.tmp"))) {
        return failure(format!("cannot reserve capture: {e}"));
    }
    let Some(driver) = std::env::var_os(DRIVER).map(PathBuf::from) else {
        return failure(format!("{DRIVER} is not set"));
    };
    let Some(health) = std::env::var_os("SAIL_CONFORMANCE_TEE_ERRORS") else {
        return failure("capture health record is not set");
    };
    if let Err(error) = std::fs::OpenOptions::new().append(true).open(health) {
        return failure(format!("capture health record is inaccessible: {error}"));
    }
    if args.iter().any(|a| a.to_str().is_none()) {
        return failure("non-UTF-8 compiler arguments cannot be captured");
    }
    let mut metadata = meta(&args, started_ns);
    let mut command = Command::new(&driver);
    command
        .args(&args)
        .stdin(if metadata.stdin_captured {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Err(e) = configure_observation(&mut command, &mut metadata, &name) {
        return failure(e);
    }
    match command.spawn() {
        Ok(child) => complete(child, metadata, &dir),
        Err(e) => failure(format!("cannot start {}: {e}", driver.display())),
    }
}
