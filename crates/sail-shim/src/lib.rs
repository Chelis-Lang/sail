//! The Z3-compatible solver executable Verus invokes (`sail-z3`).
//!
//! Status (M2): pass-through only. The shim starts the pinned Z3 named by
//! `SAIL_Z3_PATH` with the arguments it was given, and copies bytes
//! unchanged in both directions: stdin to the solver and the solver's stdout
//! to stdout. Stderr is inherited. The solver's exit status becomes the
//! shim's.
//!
//! When `SAIL_TRACE_DIR` is set, an observation tap records each session:
//! - the raw input and output streams;
//! - a JSON-lines trace with one record per satisfiability check: the state
//!   key, the answer, and the timing.
//!
//! The tap runs on its own thread and is fed through a bounded channel with
//! non-blocking sends. Nothing it does (parsing, hashing, file I/O, falling
//! behind, failing) can delay, alter or stop the forwarded bytes. If it
//! falls behind, it stops and the trace records that it is incomplete.
//! See ADR 0005.
#![forbid(unsafe_code)]

#[cfg(unix)]
mod cached;
#[cfg(unix)]
mod cached_stderr;
mod trace;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::thread;
use std::time::Instant;

/// Environment variable naming the real solver executable.
pub const ENV_Z3_PATH: &str = "SAIL_Z3_PATH";
/// Environment variable naming the daemon socket. When set, sessions are
/// served from the daemon's cache with a lazily started solver (ADR 0011).
pub const ENV_DAEMON_SOCKET: &str = "SAIL_DAEMON_SOCKET";
/// Environment variable overriding the daemon client timeout, in
/// milliseconds (default: 10,000). Each daemon exchange that takes longer is
/// a daemon failure, which falls back to the pinned solver.
pub const ENV_DAEMON_TIMEOUT_MS: &str = "SAIL_DAEMON_TIMEOUT_MS";
/// Environment variable naming a directory for session traces.
pub const ENV_TRACE_DIR: &str = "SAIL_TRACE_DIR";
/// Environment variable naming a directory where cached sessions record how
/// many replies the cache served (ADR 0014). Set by `sail verify --output
/// json`.
pub const ENV_PROVENANCE_DIR: &str = "SAIL_PROVENANCE_DIR";

/// Exit code when the shim cannot start the solver (stable, `SAIL2xxx`).
const EXIT_SETUP: u8 = 125;

/// Chunk size for copying. Each read is forwarded as soon as it returns, so
/// this bounds the copy buffer, not latency.
const CHUNK: usize = 64 * 1024;

/// Tap channel capacity in chunks. If the tap falls this far behind, it is
/// abandoned rather than ever slowing the forwarding threads.
const TAP_CAPACITY: usize = 4096;

/// Messages from the forwarding threads to the tap.
#[derive(Debug)]
pub(crate) enum TapEvent {
    /// Bytes sent to the solver, and when.
    Input(Vec<u8>, Instant),
    /// Bytes the solver produced, and when.
    Output(Vec<u8>, Instant),
    /// The solver exited with this status code (`None` if killed).
    End(Option<i32>),
}

/// A sender that never blocks: once the channel is full it drops the tap.
#[derive(Clone)]
struct TapSender {
    tx: Option<SyncSender<TapEvent>>,
    overflowed: Arc<AtomicBool>,
}

impl TapSender {
    fn send(&mut self, event: TapEvent) {
        if self.overflowed.load(Ordering::Relaxed) {
            self.tx = None;
        }
        let Some(tx) = &self.tx else { return };
        match tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.overflowed.store(true, Ordering::Relaxed);
                self.tx = None;
            }
            Err(TrySendError::Disconnected(_)) => self.tx = None,
        }
    }
}

fn setup_error(code: &str, message: &str) -> ExitCode {
    eprintln!("error[{code}]: sail-z3: {message}");
    ExitCode::from(EXIT_SETUP)
}

/// Resolves the real solver from the environment, refusing to run itself.
fn solver_path() -> Result<PathBuf, ExitCode> {
    let Some(raw) = std::env::var_os(ENV_Z3_PATH).filter(|v| !v.is_empty()) else {
        return Err(setup_error(
            "SAIL2001",
            &format!("{ENV_Z3_PATH} is not set; it must name the pinned z3 executable"),
        ));
    };
    let path = PathBuf::from(raw);
    let canonical = path.canonicalize().map_err(|e| {
        setup_error(
            "SAIL2002",
            &format!("cannot resolve {ENV_Z3_PATH}={}: {e}", path.display()),
        )
    })?;
    let me = std::env::current_exe().and_then(|p| p.canonicalize());
    if me.is_ok_and(|me| me == canonical) {
        return Err(setup_error(
            "SAIL2003",
            &format!("{ENV_Z3_PATH} points at sail-z3 itself"),
        ));
    }
    Ok(canonical)
}

/// Copies `from` to `to` chunk by chunk, flushing after each chunk and
/// copying each chunk to the tap. Returns when `from` ends or `to` fails.
fn pump(
    mut from: impl Read,
    mut to: impl Write,
    mut tap: TapSender,
    wrap: fn(Vec<u8>, Instant) -> TapEvent,
) {
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let chunk = buf.get(..n).unwrap_or_default();
        // Hand the chunk to the tap before forwarding it. Otherwise the
        // solver's reply to an input chunk can reach the tap (from the other
        // pump) before the chunk itself does, and the tap loses pairing.
        // The send never blocks, so forwarding is not delayed.
        tap.send(wrap(chunk.to_vec(), Instant::now()));
        if to.write_all(chunk).and_then(|()| to.flush()).is_err() {
            return;
        }
    }
}

fn spawn_solver(path: &Path, args: &[OsString]) -> Result<Child, ExitCode> {
    Command::new(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| setup_error("SAIL2004", &format!("cannot start {}: {e}", path.display())))
}

#[cfg(unix)]
pub(crate) fn status_code(status: std::process::ExitStatus) -> (Option<i32>, u8) {
    use std::os::unix::process::ExitStatusExt as _;
    match (status.code(), status.signal()) {
        (Some(code), _) => (Some(code), u8::try_from(code & 0xff).unwrap_or(1)),
        // Shell convention for death by signal.
        (None, Some(sig)) => (None, u8::try_from((128 + sig) & 0xff).unwrap_or(1)),
        (None, None) => (None, 1),
    }
}

#[cfg(not(unix))]
pub(crate) fn status_code(status: std::process::ExitStatus) -> (Option<i32>, u8) {
    let code = status.code();
    (
        code,
        code.and_then(|c| u8::try_from(c & 0xff).ok()).unwrap_or(1),
    )
}

/// Runs the shim with the process's own arguments, stdio and environment.
#[must_use]
pub fn run() -> ExitCode {
    let started = Instant::now();
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let solver = match solver_path() {
        Ok(path) => path,
        Err(code) => return code,
    };
    #[cfg(unix)]
    if let Some(socket) = std::env::var_os(ENV_DAEMON_SOCKET).filter(|v| !v.is_empty())
        && args
            .iter()
            .map(|a| a.to_str())
            .eq([Some("-smt2"), Some("-in")])
        && let Some(code) = cached::run(std::path::Path::new(&socket), &solver, &args)
    {
        return code;
    }
    let mut child = match spawn_solver(&solver, &args) {
        Ok(child) => child,
        Err(code) => return code,
    };
    let (Some(child_in), Some(child_out)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        return setup_error("SAIL2005", "solver started without piped stdio");
    };

    let overflowed = Arc::new(AtomicBool::new(false));
    let (tap, tap_thread) = match std::env::var_os(ENV_TRACE_DIR).filter(|v| !v.is_empty()) {
        Some(dir) => {
            let (tx, rx) = std::sync::mpsc::sync_channel(TAP_CAPACITY);
            let ctx = trace::Context {
                mode: if std::env::var("SAIL_SOLVER_MODE").as_deref() == Ok("accept-first-proof") {
                    sail_key::Mode::OptIn("accept-first-proof".to_owned())
                } else {
                    sail_key::Mode::Default
                },
                dir: PathBuf::from(dir),
                solver: solver.clone(),
                args: args
                    .iter()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect(),
                started,
                overflowed: Arc::clone(&overflowed),
            };
            let handle = thread::Builder::new()
                .name("sail-z3-tap".to_owned())
                .spawn(move || trace::run(&ctx, &rx))
                .ok();
            (Some(tx), handle)
        }
        None => (None, None),
    };
    let sender = TapSender {
        tx: tap.clone(),
        overflowed: Arc::clone(&overflowed),
    };

    // stdin -> solver. Not joined: if the solver exits first, this thread may
    // stay blocked reading our stdin, and process exit ends it.
    let to_solver = sender.clone();
    let _ = thread::Builder::new()
        .name("sail-z3-in".to_owned())
        .spawn(move || {
            pump(
                std::io::stdin().lock(),
                child_in,
                to_solver,
                TapEvent::Input,
            );
            // Dropping `child_in` here closes the solver's stdin.
        });

    // solver -> stdout. Joined, so every byte is delivered before we exit.
    let from_solver = sender;
    let out_thread = thread::Builder::new()
        .name("sail-z3-out".to_owned())
        .spawn(move || {
            pump(
                child_out,
                std::io::stdout().lock(),
                from_solver,
                TapEvent::Output,
            );
        });

    let status = child.wait();
    if let Ok(handle) = out_thread {
        let _ = handle.join();
    }
    let (code, exit) = match status {
        Ok(status) => status_code(status),
        Err(e) => {
            eprintln!("error[SAIL2006]: sail-z3: lost track of the solver process: {e}");
            (None, 1)
        }
    };
    if let (Some(tx), Some(handle)) = (tap, tap_thread) {
        // A blocking send is safe: the tap is draining, or it has exited and
        // the send fails at once.
        let _ = tx.send(TapEvent::End(code));
        drop(tx);
        let _ = handle.join();
    }
    ExitCode::from(exit)
}
