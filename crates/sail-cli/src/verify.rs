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
//!
//! `sail verify --output json [args…]` (ADR 0014) also asks Verus for
//! machine-readable results and logs that leave Z3's input unchanged, and
//! prints one schema-v1 document (`docs/output-schema.md`) on stdout.
//! Verus's diagnostics still go to stderr, verbatim. Without `--output
//! json`, nothing is added and the output is exactly `cargo verus verify`'s.

use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use sail_verus::attribution::{self, Provenance};

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

/// A setup failure: a stable code and a message.
struct SetupError(&'static str, String);

/// What `sail verify` runs.
struct Setup {
    cargo_verus: PathBuf,
    shim: PathBuf,
    z3: PathBuf,
    socket: PathBuf,
}

fn setup() -> Result<Setup, SetupError> {
    let cargo_verus = cargo_verus().ok_or_else(|| {
        SetupError(
            "SAIL0101",
            "`cargo-verus` is not on PATH (or set SAIL_CARGO_VERUS)".to_owned(),
        )
    })?;
    let shim = std::env::current_exe()
        .map(|me| me.with_file_name("sail-z3"))
        .map_err(|e| SetupError("SAIL0102", format!("cannot locate sail: {e}")))?;
    if !shim.is_file() {
        return Err(SetupError(
            "SAIL0102",
            format!("sail-z3 not found next to sail at {}", shim.display()),
        ));
    }
    let z3 = real_z3(&cargo_verus, &shim).ok_or_else(|| {
        SetupError(
            "SAIL0103",
            "cannot find the Z3 Verus uses (set VERUS_Z3_PATH)".to_owned(),
        )
    })?;
    let socket = match std::env::var_os("SAIL_DAEMON_SOCKET").filter(|v| !v.is_empty()) {
        Some(s) => PathBuf::from(s),
        None => sail_verus::cache_dir()
            .map(|c| crate::default_socket(&c))
            .map_err(|e| SetupError("SAIL0104", e.to_string()))?,
    };
    Ok(Setup {
        cargo_verus,
        shim,
        z3,
        socket,
    })
}

/// `cargo verus verify <args>` with sail's environment; the daemon is
/// started on demand. Returns the command and whether the daemon is in use.
fn command(s: &Setup, args: &[String]) -> (Command, bool) {
    let mut cmd = Command::new(&s.cargo_verus);
    cmd.arg("verus")
        .arg("verify")
        .args(args.iter().map(OsString::from))
        .env("VERUS_Z3_PATH", &s.shim)
        .env("SAIL_Z3_PATH", &s.z3);
    let daemon = ensure_daemon(&s.socket);
    if daemon {
        cmd.env("SAIL_DAEMON_SOCKET", &s.socket);
    } else {
        cmd.env_remove("SAIL_DAEMON_SOCKET");
    }
    (cmd, daemon)
}

fn exit_code(code: Option<i32>) -> ExitCode {
    ExitCode::from(code.and_then(|c| u8::try_from(c & 0xff).ok()).unwrap_or(1))
}

/// Splits sail's own flags from the arguments passed through. Only
/// `--output json` (or `--output=json`) is sail's, and only before a `--`.
/// Returns `(json, passed_through)`.
fn split_args(args: &[String]) -> Result<(bool, Vec<String>), SetupError> {
    let mut json = false;
    let mut rest = Vec::with_capacity(args.len());
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--" {
            rest.push(arg.clone());
            rest.extend(it.by_ref().cloned());
            break;
        }
        let value = if arg == "--output" {
            Some(it.next().cloned().unwrap_or_default())
        } else {
            arg.strip_prefix("--output=").map(str::to_owned)
        };
        match value.as_deref() {
            Some("json") => json = true,
            Some(other) => {
                return Err(SetupError(
                    "SAIL0106",
                    format!("unknown output format `{other}` (expected `--output json`)"),
                ));
            }
            None => rest.push(arg.clone()),
        }
    }
    Ok((json, rest))
}

/// Runs `cargo verus verify <args>` through sail.
pub fn verify(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let fail = |err: &mut dyn std::io::Write, e: &SetupError| {
        let _ = writeln!(err, "error[{}]: sail verify: {}", e.0, e.1);
        ExitCode::from(EXIT_SETUP)
    };
    let (json, args) = match split_args(args) {
        Ok(split) => split,
        Err(e) => return fail(err, &e),
    };
    let setup = match setup() {
        Ok(s) => s,
        Err(e) if json => {
            let report = crate::report::setup_failure(&args, e.0, &e.1);
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
            return fail(err, &e);
        }
        Err(e) => return fail(err, &e),
    };
    if json {
        return verify_json(&setup, &args, out, err);
    }
    let (mut cmd, _) = command(&setup, &args);
    match cmd.status() {
        Ok(status) => exit_code(status.code()),
        Err(e) => fail(
            err,
            &SetupError(
                "SAIL0105",
                format!("cannot run {}: {e}", setup.cargo_verus.display()),
            ),
        ),
    }
}

/// Whether the arguments already pass Verus `flag` (after `--`).
fn has_verus_flag(args: &[String], flag: &str) -> bool {
    args.iter()
        .skip_while(|a| *a != "--")
        .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}

/// Why sail cannot ask this run for Verus logs, or `None` if it can.
///
/// Verus deletes every file in its log directory when it first writes there,
/// and `cargo verus` forwards the same `--log-dir` to every package it
/// verifies. Logs are therefore requested only when one Verus process will
/// write them: a single workspace member with at most one lib or bin target
/// (ADR 0014, `docs/verus-issues/VI-0004`).
fn logs_unavailable(args: &[String]) -> Option<String> {
    for flag in ["--log", "--log-all", "--log-dir"] {
        if has_verus_flag(args, flag) {
            return Some(format!(
                "attribution needs sail's own Verus logs, and the arguments already pass {flag}"
            ));
        }
    }
    let mut cmd = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cmd.args(["metadata", "--no-deps", "--format-version", "1"]);
    let mut it = args.iter().take_while(|a| *a != "--");
    while let Some(a) = it.next() {
        if a == "--manifest-path" {
            if let Some(path) = it.next() {
                cmd.arg("--manifest-path").arg(path);
            }
        } else if a.starts_with("--manifest-path=") {
            cmd.arg(a);
        }
    }
    let output = cmd.stdin(Stdio::null()).stderr(Stdio::null()).output();
    let metadata: serde_json::Value =
        match output {
            Ok(o) if o.status.success() => serde_json::from_slice(&o.stdout).unwrap_or_default(),
            _ => return Some(
                "`cargo metadata` failed, so sail cannot tell how many packages Verus will verify"
                    .to_owned(),
            ),
        };
    let members: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let [member] = members.as_slice() else {
        return Some(format!(
            "the workspace has {} members; Verus logs are shared by every package in a run, so sail does not request them",
            members.len()
        ));
    };
    let targets = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["id"].as_str() == Some(*member))
        .flat_map(|p| p["targets"].as_array().into_iter().flatten())
        .filter(|t| {
            t["kind"].as_array().into_iter().flatten().any(|k| {
                matches!(
                    k.as_str(),
                    Some("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" | "bin")
                )
            })
        })
        .count();
    (targets > 1).then(|| {
        format!(
            "the package has {targets} lib and bin targets; Verus logs are shared by every target in a run, so sail does not request them"
        )
    })
}

/// A scratch directory for one run, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> std::io::Result<Scratch> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("sail-verify-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("provenance"))?;
    Ok(Scratch(dir))
}

/// SHA-256 of a file, as lowercase hex.
fn file_sha256(path: &Path) -> Option<String> {
    use sha2::Digest as _;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(buf.get(..n)?),
            Err(_) => return None,
        }
    }
    Some(hasher.finalize().iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    }))
}

/// Reports a setup failure in JSON mode: the document on `out`, the message
/// on `err`.
fn json_failure(
    args: &[String],
    e: &SetupError,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let report = crate::report::setup_failure(args, e.0, &e.1);
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    let _ = writeln!(err, "error[{}]: sail verify: {}", e.0, e.1);
    ExitCode::from(EXIT_SETUP)
}

/// Copies `from` to `to` as it arrives, and returns a copy of everything.
fn relay(mut from: impl std::io::Read, to: &mut dyn std::io::Write) -> Vec<u8> {
    let mut copy = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = buf.get(..n).unwrap_or_default();
                let _ = to.write_all(chunk);
                let _ = to.flush();
                copy.extend_from_slice(chunk);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    copy
}

/// `sail verify --output json`: runs with the extra Verus flags, relays
/// stderr verbatim while keeping a copy, and prints the report on stdout.
fn verify_json(
    setup: &Setup,
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let scratch = match scratch() {
        Ok(s) => s,
        Err(e) => {
            let e = SetupError(
                "SAIL0107",
                format!("cannot create a scratch directory: {e}"),
            );
            return json_failure(args, &e, out, err);
        }
    };
    let logs_dir = scratch.0.join("logs");
    let unavailable = logs_unavailable(args);
    let mut extra: Vec<String> = Vec::new();
    for flag in ["--output-json", "--time"] {
        if !has_verus_flag(args, flag) {
            extra.push(flag.to_owned());
        }
    }
    if unavailable.is_none() {
        extra.extend(["--log", "smt-transcript", "--log", "air", "--log-dir"].map(str::to_owned));
        extra.push(logs_dir.to_string_lossy().into_owned());
    }
    let mut full: Vec<String> = args.to_vec();
    if !full.iter().any(|a| a == "--") {
        full.push("--".to_owned());
    }
    full.extend(extra);
    let (mut cmd, daemon) = command(setup, &full);
    cmd.env("SAIL_PROVENANCE_DIR", scratch.0.join("provenance"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let e = SetupError(
                "SAIL0105",
                format!("cannot run {}: {e}", setup.cargo_verus.display()),
            );
            return json_failure(args, &e, out, err);
        }
    };
    let stdout = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            buf
        })
    });
    let stderr_copy = child
        .stderr
        .take()
        .map(|s| relay(s, err))
        .unwrap_or_default();
    let status = child.wait().ok().and_then(|s| s.code());
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    let (docs, rest) = attribution::json_documents(&stdout);
    // Anything on stdout that is not Verus's JSON goes to stderr, so stdout
    // holds only sail's document.
    let _ = err.write_all(&rest);
    let logs = if unavailable.is_none() {
        Some(attribution::read_logs(&logs_dir).unwrap_or_default())
    } else {
        None
    };
    let provenance = if daemon {
        Provenance::Records(attribution::read_session_records(
            &scratch.0.join("provenance"),
        ))
    } else {
        Provenance::NoCache
    };
    let mut result =
        attribution::attribute(&docs, logs.as_deref(), &provenance, unavailable.as_deref());
    if docs.is_empty() {
        result.status = attribution::Status::Unavailable;
        result.notes.insert(
            0,
            "Verus produced no results: Cargo found every package up to date, or the build failed before verification".to_owned(),
        );
    }
    let diagnostics = sail_verus::diagnostics::parse(&String::from_utf8_lossy(&stderr_copy));
    let report = crate::report::build(&crate::report::Inputs {
        args,
        exit_code: status,
        solver_path: Some(setup.z3.to_string_lossy().into_owned()),
        solver_sha256: file_sha256(&setup.z3),
        daemon,
        attribution: &result,
        diagnostics: &diagnostics,
    });
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    let _ = out.flush();
    exit_code(status)
}

#[cfg(test)]
mod tests {
    use super::{has_verus_flag, split_args};

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn output_json_is_sails_flag() {
        let split = |v: &[&str]| split_args(&strings(v)).map_err(|e| e.0);
        assert_eq!(split(&["-p", "x"]), Ok((false, strings(&["-p", "x"]))));
        assert_eq!(
            split(&["--output", "json", "-p", "x"]),
            Ok((true, strings(&["-p", "x"])))
        );
        assert_eq!(
            split(&["-p", "x", "--output=json"]),
            Ok((true, strings(&["-p", "x"])))
        );
        // After `--`, everything belongs to Verus.
        assert_eq!(
            split(&["--", "--output", "json"]),
            Ok((false, strings(&["--", "--output", "json"])))
        );
        assert_eq!(split(&["--output", "yaml"]), Err("SAIL0106"));
        assert_eq!(split(&["--output"]), Err("SAIL0106"));
    }

    #[test]
    fn verus_flags_are_found_after_the_separator() {
        let a = strings(&["--time", "--", "--rlimit", "5", "--log-dir=x"]);
        assert!(!has_verus_flag(&a, "--time"));
        assert!(has_verus_flag(&a, "--rlimit"));
        assert!(has_verus_flag(&a, "--log-dir"));
    }
}
