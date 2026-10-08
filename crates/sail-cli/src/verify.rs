//! `sail verify`: `cargo verus verify` with sail in front of the solver.
//!
//! The user's own `cargo verus` and Verus run unchanged, with the same
//! arguments. sail only sets three environment variables:
//! - `VERUS_Z3_PATH` names `sail-z3`;
//! - `SAIL_Z3_PATH` names the Z3 the user's Verus would have used;
//! - `SAIL_DAEMON_SOCKET` names the cache daemon, which is started on demand.
//!
//! If the daemon cannot be reached, `sail-z3` passes everything through to
//! that Z3 unchanged. Switching back means running `cargo verus verify`
//! again.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

/// Exit code when `sail verify` cannot set up (stable, `SAIL01xx`).
const EXIT_SETUP: u8 = 2;

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The `cargo-verus` to run: `$SAIL_CARGO_VERUS`, else the one on `PATH`.
fn cargo_verus() -> Option<PathBuf> {
    std::env::var_os("SAIL_CARGO_VERUS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| on_path("cargo-verus"))
}

/// The Z3 the user's Verus would use, following the Verus launcher's own
/// rules: `$VERUS_Z3_PATH` if set (and not already `sail-z3`), else `z3`
/// next to the Verus binaries, else `z3` on `PATH`.
fn real_z3(cargo_verus: &Path, shim: &Path) -> Option<PathBuf> {
    let from_env = std::env::var_os("VERUS_Z3_PATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.canonicalize().ok() != shim.canonicalize().ok());
    from_env
        .or_else(|| {
            cargo_verus
                .parent()
                .map(|d| d.join("z3"))
                .filter(|p| p.is_file())
        })
        .or_else(|| on_path("z3"))
}

/// Connects to the daemon at `socket`, starting `sail serve` if nothing is
/// listening. Returns whether a daemon is available.
#[cfg(unix)]
fn ensure_daemon(socket: &Path) -> bool {
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt as _;
    if UnixStream::connect(socket).is_ok() {
        return true;
    }
    let Ok(me) = std::env::current_exe() else {
        return false;
    };
    let started = Command::new(me)
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    if started.is_err() {
        return false;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if UnixStream::connect(socket).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

#[cfg(not(unix))]
fn ensure_daemon(_socket: &Path) -> bool {
    false
}

/// Runs `cargo verus verify <args>` through sail.
pub fn verify(args: &[String], err: &mut impl std::io::Write) -> ExitCode {
    let fail = |err: &mut dyn std::io::Write, code: &str, msg: &str| {
        let _ = writeln!(err, "error[{code}]: sail verify: {msg}");
        ExitCode::from(EXIT_SETUP)
    };
    let Some(cargo_verus) = cargo_verus() else {
        return fail(
            err,
            "SAIL0101",
            "`cargo-verus` is not on PATH (or set SAIL_CARGO_VERUS)",
        );
    };
    let shim = match std::env::current_exe() {
        Ok(me) => me.with_file_name("sail-z3"),
        Err(e) => return fail(err, "SAIL0102", &format!("cannot locate sail: {e}")),
    };
    if !shim.is_file() {
        return fail(
            err,
            "SAIL0102",
            &format!("sail-z3 not found next to sail at {}", shim.display()),
        );
    }
    let Some(z3) = real_z3(&cargo_verus, &shim) else {
        return fail(
            err,
            "SAIL0103",
            "cannot find the Z3 Verus uses (set VERUS_Z3_PATH)",
        );
    };
    let socket = match std::env::var_os("SAIL_DAEMON_SOCKET").filter(|v| !v.is_empty()) {
        Some(s) => PathBuf::from(s),
        None => match sail_verus::cache_dir() {
            Ok(c) => crate::default_socket(&c),
            Err(e) => return fail(err, "SAIL0104", &e.to_string()),
        },
    };
    let mut cmd = Command::new(&cargo_verus);
    cmd.arg("verus")
        .arg("verify")
        .args(args.iter().map(OsString::from))
        .env("VERUS_Z3_PATH", &shim)
        .env("SAIL_Z3_PATH", &z3);
    if ensure_daemon(&socket) {
        cmd.env("SAIL_DAEMON_SOCKET", &socket);
    } else {
        cmd.env_remove("SAIL_DAEMON_SOCKET");
    }
    match cmd.status() {
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|c| u8::try_from(c & 0xff).ok())
                .unwrap_or(1),
        ),
        Err(e) => fail(
            err,
            "SAIL0105",
            &format!("cannot run {}: {e}", cargo_verus.display()),
        ),
    }
}
