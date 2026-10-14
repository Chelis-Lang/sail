//! `sail verify`: `cargo verus verify` with sail in front of the solver.
//!
//! The user's own `cargo verus` and Verus run unchanged, with the same
//! arguments. sail only sets three environment variables:
//! - `VERUS_Z3_PATH` names `sail-z3`;
//! - `SAIL_Z3_PATH` names the Z3 the user's Verus would have used;
//! - `SAIL_DAEMON_SOCKET` names the cache daemon, which is started on demand.
//!
//! If the daemon cannot be reached, `sail-z3` passes everything through to
//! that Z3 unchanged, and one `warning[SAIL0003]` line on stderr says why.
//! Switching back means running `cargo verus verify` again.
//!
//! `sail verify --output json [args…]` (ADR 0014) also asks Verus for
//! machine-readable results and logs that leave Z3's input unchanged, and
//! prints one schema-v1 document (`docs/output-schema.md`) on stdout.
//! Verus's diagnostics still go to stderr, verbatim. Without `--output
//! json`, nothing is added and, while the daemon is available, the output is
//! exactly `cargo verus verify`'s.

use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use sail_verus::attribution::{self, Provenance};

/// Exit code when `sail verify` cannot set up (stable, `SAIL01xx`).
const EXIT_SETUP: u8 = 2;

pub(crate) fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The `cargo-verus` to run: `$SAIL_CARGO_VERUS`, else the one on `PATH`.
pub(crate) fn cargo_verus() -> Option<PathBuf> {
    std::env::var_os("SAIL_CARGO_VERUS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| on_path("cargo-verus"))
}

/// The Z3 the user's Verus would use, following the Verus launcher's own
/// rules: `$VERUS_Z3_PATH` if set (and not already `sail-z3`), else `z3`
/// next to the Verus binaries, else `z3` on `PATH`.
pub(crate) fn real_z3(cargo_verus: &Path, shim: &Path) -> Option<PathBuf> {
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
/// listening. Returns whether a daemon is available. When it is not, one
/// `warning[SAIL0003]` per process says why on stderr; verification then
/// runs against the pinned Z3 without the cache, with the same results.
#[cfg(unix)]
pub(crate) fn ensure_daemon(socket: &Path) -> bool {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    match start_daemon(socket) {
        Ok(()) => true,
        Err(why) => {
            if !WARNED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                eprintln!(
                    "warning[SAIL0003]: the Sail cache daemon is unavailable: {why}; \
                     verifying without the cache (same results, no reuse)"
                );
            }
            false
        }
    }
}

#[cfg(unix)]
fn start_daemon(socket: &Path) -> Result<(), String> {
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt as _;
    if UnixStream::connect(socket).is_ok() {
        return Ok(());
    }
    if !sail_api::jobs::socket_fits(socket) {
        return Err(format!(
            "its socket path {} (or the job socket beside it) is longer than the {} bytes \
             Unix sockets allow; set SAIL_CACHE_DIR or SAIL_DAEMON_SOCKET to a shorter path",
            socket.display(),
            sail_api::jobs::MAX_SOCKET_PATH
        ));
    }
    let me = std::env::current_exe().map_err(|e| format!("cannot locate sail: {e}"))?;
    let mut child = Command::new(me)
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("cannot run `sail serve`: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            // Another client's daemon may have won the race to bind.
            if UnixStream::connect(socket).is_ok() {
                return Ok(());
            }
            return Err(format!(
                "`sail serve --socket {}` exited ({status}); run it to see why",
                socket.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "no daemon answered on {} within 5 s; run `sail serve` to see why",
        socket.display()
    ))
}

#[cfg(not(unix))]
pub(crate) fn ensure_daemon(_socket: &Path) -> bool {
    false
}

/// A setup failure: a stable code and a message.
struct SetupError(&'static str, String);

/// What `sail verify` runs.
struct Setup {
    mode: sail_api::protocol::VerificationMode,
    measure: Option<u64>,
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
            .map_err(|e| SetupError(e.code, e.message))?,
    };
    Ok(Setup {
        mode: sail_api::protocol::VerificationMode::Default,
        measure: None,
        cargo_verus,
        shim,
        z3,
        socket,
    })
}

/// `cargo verus verify <args>` with sail's environment; the daemon is
/// started on demand. Returns the command and whether the daemon is in use.
fn direct_source(args: &[String]) -> Option<&str> {
    let mut it = args.iter().take_while(|a| *a != "--");
    while let Some(arg) = it.next() {
        if arg == "--verus-source" {
            return it.next().map(String::as_str);
        }
    }
    None
}

fn command(s: &Setup, args: &[String]) -> (Command, bool) {
    let mut cmd = Command::new(&s.cargo_verus);
    cmd.arg("verus")
        .arg("verify")
        .args(args.iter().map(OsString::from))
        .env("VERUS_Z3_PATH", &s.shim)
        .env("SAIL_Z3_PATH", &s.z3)
        .env("SAIL_SOLVER_MODE", s.mode.name());
    if crate::portfolio::partial(args) {
        // Partial selection is explicitly requested; cargo-verus requires focus.
        cmd = {
            let mut focus = Command::new(&s.cargo_verus);
            focus
                .arg("verus")
                .arg("focus")
                .args(args)
                .env("VERUS_Z3_PATH", &s.shim)
                .env("SAIL_Z3_PATH", &s.z3)
                .env("SAIL_SOLVER_MODE", s.mode.name());
            focus
        };
    }
    if s.mode == sail_api::protocol::VerificationMode::AcceptFirstProof {
        cmd.stdin(Stdio::null());
    }
    if let Some(source) = direct_source(args) {
        cmd = Command::new(s.cargo_verus.with_file_name("verus"));
        cmd.arg(source)
            .args(args.iter().skip_while(|a| *a != "--").skip(1))
            .env("VERUS_Z3_PATH", &s.shim)
            .env("SAIL_Z3_PATH", &s.z3)
            .env("SAIL_SOLVER_MODE", s.mode.name())
            .stdin(Stdio::null());
    }
    let daemon = s.measure.is_none() && ensure_daemon(&s.socket);
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
    if let Some(exit) = crate::managed_cli::verify(args, out, err) {
        return exit;
    }
    let fail = |err: &mut dyn std::io::Write, e: &SetupError| {
        let _ = writeln!(err, "error[{}]: sail verify: {}", e.0, e.1);
        ExitCode::from(EXIT_SETUP)
    };
    let (args, measure) = match crate::fragility_join::extract(args) {
        Ok(v) => v,
        Err(message) => return fail(err, &SetupError("SAIL0110", message)),
    };
    let args = args.as_slice();
    match crate::portfolio::parse(args) {
        Ok(Some(_)) if measure.is_some() => {
            return fail(
                err,
                &SetupError(
                    "SAIL0110",
                    "measure fragility separately from a portfolio".to_owned(),
                ),
            );
        }
        Ok(Some(options)) => return crate::portfolio::run(options, out, err),
        Ok(None) => {}
        Err(message) => {
            return fail(err, &SetupError("SAIL0110", message));
        }
    }
    let (json, args) = match split_args(args) {
        Ok(split) => split,
        Err(e) => return fail(err, &e),
    };
    let mut setup = match setup() {
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
    setup.measure = measure;
    if measure.is_some() {
        let (measured_args, _ownership) =
            match crate::measurement::prepare(&args, crate::portfolio::partial(&args)) {
                Ok(v) => v,
                Err(message) => {
                    return json_failure(&args, &SetupError("SAIL0110", message), out, err);
                }
            };
        return verify_json(&setup, &measured_args, out, err);
    }
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

/// Internal independent portfolio worker. Closing its parent's input pipe
/// kills the worker's entire process group, even when the parent was killed.
pub(crate) fn portfolio_attempt(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    crate::portfolio::guard_parent();
    let Some(seed) = args.first().and_then(|s| s.parse::<u32>().ok()) else {
        let _ = writeln!(err, "error[SAIL0110]: invalid portfolio attempt seed");
        return ExitCode::from(2);
    };
    let mut setup = match setup() {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err, "error[{}]: {}", e.0, e.1);
            return ExitCode::from(2);
        }
    };
    setup.mode = sail_api::protocol::VerificationMode::AcceptFirstProof;
    let pinned = sail_verus::toolchain::Pins::embedded()
        .ok()
        .and_then(|p| p.assets.get(sail_verus::toolchain::host()).cloned())
        .is_some_and(|a| file_sha256(&setup.z3).as_deref() == Some(a.z3_sha256.as_str()));
    if !pinned {
        let _ = writeln!(
            err,
            "error[SAIL0115]: portfolio requires the pinned Z3 binary"
        );
        return ExitCode::from(2);
    }
    let Some(target) = std::env::var_os("SAIL_PORTFOLIO_TARGET") else {
        let _ = writeln!(err, "error[SAIL0111]: portfolio worker has no owned target");
        return ExitCode::from(2);
    };
    let expected = if direct_source(args).is_some() {
        None
    } else {
        match crate::portfolio::force_verification(
            args.get(1..).unwrap_or_default(),
            Path::new(&target),
            crate::portfolio::partial(args),
        ) {
            Ok(names) => Some(names),
            Err(e) => {
                let _ = writeln!(err, "error[SAIL0111]: {e}");
                return ExitCode::from(2);
            }
        }
    };
    let mut bytes = Vec::new();
    let status = verify_json(&setup, args.get(1..).unwrap_or_default(), &mut bytes, err);
    if let Ok(mut report) = serde_json::from_slice::<sail_api::output::Report>(&bytes) {
        if expected.as_ref().is_some_and(|names| {
            names.iter().any(|required| {
                !report
                    .crates
                    .iter()
                    .any(|c| c.name.as_ref() == Some(required))
            })
        }) {
            report.error = Some(sail_api::output::ErrorInfo {
                code: "SAIL0113".to_owned(),
                message: "not every requested verification target produced a result".to_owned(),
            });
        }
        setup.mode.name().clone_into(&mut report.mode);
        for f in report.crates.iter_mut().flat_map(|c| &mut c.functions) {
            f.provenance.mode.clone_from(&report.mode);
            f.provenance.configuration = Some(sail_api::output::SolverConfiguration { seed });
        }
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string(&report).unwrap_or_default()
        );
    }
    status
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
pub(crate) fn logs_unavailable(args: &[String]) -> Option<String> {
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
pub(crate) fn file_sha256(path: &Path) -> Option<String> {
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

pub(crate) fn logged_args(args: &[String], logs: bool, logs_dir: &Path) -> Vec<String> {
    let mut extra: Vec<String> = Vec::new();
    for flag in ["--output-json", "--time"] {
        if !has_verus_flag(args, flag) {
            extra.push(flag.to_owned());
        }
    }
    if logs {
        extra.extend(["--log", "smt-transcript", "--log", "air", "--log-dir"].map(str::to_owned));
        extra.push(logs_dir.to_string_lossy().into_owned());
    }
    let mut full: Vec<String> = args.to_vec();
    if !full.iter().any(|a| a == "--") {
        full.push("--".to_owned());
    }
    full.extend(extra);
    full
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
    let unavailable = if direct_source(args).is_some() {
        None
    } else {
        logs_unavailable(args)
    };
    let full = logged_args(args, unavailable.is_none(), &logs_dir);
    let (mut cmd, daemon) = command(setup, &full);
    cmd.env("SAIL_PROVENANCE_DIR", scratch.0.join("provenance"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if setup.measure.is_some() {
        cmd.env("SAIL_TRACE_DIR", scratch.0.join("traces"));
    }
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
    let mut report = crate::report::build(&crate::report::Inputs {
        args,
        exit_code: status,
        solver_path: Some(setup.z3.to_string_lossy().into_owned()),
        solver_sha256: file_sha256(&setup.z3),
        daemon,
        attribution: &result,
        diagnostics: &diagnostics,
    });
    if let Some(seeds) = setup.measure {
        crate::fragility_join::measure(
            &mut report,
            &setup.z3,
            &scratch.0.join("traces"),
            logs.as_deref().unwrap_or_default(),
            seeds,
        );
    }
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

    /// A socket path the kernel cannot bind is reported at once, with the
    /// limit and the fix, instead of a silent 5 s wait.
    #[cfg(unix)]
    #[test]
    fn an_overlong_socket_is_reported_without_waiting() {
        let socket = std::path::Path::new("/tmp")
            .join("s".repeat(200))
            .join("d.sock");
        let started = std::time::Instant::now();
        let why = super::start_daemon(&socket).err().unwrap_or_default();
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(
            why.contains("longer than") && why.contains("SAIL_CACHE_DIR"),
            "{why}"
        );
    }

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
