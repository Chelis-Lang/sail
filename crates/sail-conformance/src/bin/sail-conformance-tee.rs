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
            k.starts_with("__VERUS_DRIVER_")
                .then(|| (k, v.to_string_lossy().into_owned()))
        })
        .collect();
    Meta {
        version: 1,
        package_name: env_string("CARGO_PKG_NAME"),
        package_version: env_string("CARGO_PKG_VERSION"),
        manifest_dir: env_string("CARGO_MANIFEST_DIR"),
        crate_name: value_after("--crate-name"),
        crate_types,
        test: argv.iter().any(|a| a == "--test"),
        bin_name: env_string("CARGO_BIN_NAME"),
        compile_target: value_after("--target"),
        arg_sha256: argument_digest(&argv, &driver_env),
        argv,
        driver_env,
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

fn record(dir: &Path, meta: &Meta, stdout: &[u8], stderr: &[u8]) -> std::io::Result<()> {
    let name = format!("{:032}-{}", meta.started_ns, meta.pid);
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(tmp.join("stdout"), stdout)?;
    std::fs::write(tmp.join("stderr"), stderr)?;
    let json = serde_json::to_vec_pretty(meta).map_err(std::io::Error::other)?;
    std::fs::write(tmp.join("meta.json"), json)?;
    // The rename is the completion mark.
    std::fs::rename(&tmp, dir.join(name))
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(driver) = std::env::var_os(DRIVER).map(PathBuf::from) else {
        eprintln!("error[SAIL3018]: {DRIVER} is not set");
        return ExitCode::from(125);
    };
    let Some(dir) = std::env::var_os(DIR).map(PathBuf::from) else {
        eprintln!("error[SAIL3018]: {DIR} is not set");
        return ExitCode::from(125);
    };
    let started_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    // Reserve before spawning: a killed tee or a failed write must leave an
    // incomplete record rather than disappearing from the capture inventory.
    let marker = dir.join(format!("{started_ns:032}-{}.tmp", std::process::id()));
    if let Err(e) = std::fs::create_dir(&marker) {
        eprintln!("error[SAIL3018]: cannot reserve capture: {e}");
        return ExitCode::from(125);
    }
    let mut child = match Command::new(&driver)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("error[SAIL3018]: cannot start {}: {e}", driver.display());
            return ExitCode::from(125);
        }
    };
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
    {
        let mut meta = meta(&args, started_ns);
        meta.exit_code = code;
        meta.stdout_len = stdout.len();
        meta.stderr_len = stderr.len();
        meta.stdout_sha256 = hex_sha256(&stdout);
        meta.stderr_sha256 = hex_sha256(&stderr);
        meta.forward_error = out_error.or(err_error).or_else(|| {
            (args.iter().any(|a| a.to_str().is_none())
                || std::env::vars_os().any(|(k, v)| k.to_str().is_none() || v.to_str().is_none()))
            .then(|| "non-UTF-8 invocation metadata cannot be paired".to_owned())
        });
        // A failed record leaves no completed directory; the forwarded bytes
        // and exit status are already final.
        let _ = record(&dir, &meta, &stdout, &stderr);
    }
    match code {
        Some(code) => ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1)),
        // Killed by a signal (or wait failed): report failure, as Cargo
        // would see for the driver itself.
        None => ExitCode::from(101),
    }
}
