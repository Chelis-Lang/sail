//! Running `sail` subcommands for a tool call, cancellably.
//!
//! The child runs in its own process group (on Unix), so cancelling a call
//! (`notifications/cancelled`) kills `sail verify`, `cargo verus`, Verus, the
//! shims and their solvers together, as `sail watch` does. Cancelling is
//! always safe: a killed shim stores only complete replies.

use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How much of the child's stderr a result keeps.
const STDERR_TAIL: usize = 8 * 1024;

/// A cancellation handle for one tool call.
#[derive(Debug, Default)]
pub struct Cancel {
    cancelled: AtomicBool,
    /// The running child's process group, while it runs.
    group: Mutex<Option<u32>>,
}

impl Cancel {
    /// Cancels the call: kills the running child's process group, if any,
    /// and makes any later [`run`] return immediately.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(group) = self.group.lock()
            && let Some(pgid) = *group
        {
            kill_group(pgid);
        }
    }

    /// Whether the call was cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// `kill -s KILL -- -PGID`: the unambiguous POSIX form.
fn kill_group(pgid: u32) {
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pgid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// What a child did.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Its exit code (`None` if killed by a signal).
    pub exit_code: Option<i32>,
    /// Everything it wrote on stdout.
    pub stdout: Vec<u8>,
    /// The last [`STDERR_TAIL`] bytes it wrote on stderr.
    pub stderr_tail: String,
    /// How long it ran.
    pub duration: Duration,
}

/// Runs `program args…` in `dir` with stdin closed, collecting its output.
/// The child never inherits the server's stdout, which carries the MCP
/// stream.
pub fn run(
    program: &Path,
    args: &[String],
    dir: &Path,
    cancel: &Cancel,
) -> std::io::Result<Outcome> {
    run_with_environment(program, args, dir, cancel, &[])
}

/// Runs a child with explicit extra environment settings.
///
/// # Errors
/// Returns startup, I/O, or cancellation errors.
pub fn run_with_environment(
    program: &Path,
    args: &[String],
    dir: &Path,
    cancel: &Cancel,
    environment: &[(&str, &str)],
) -> std::io::Result<Outcome> {
    if cancel.is_cancelled() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "cancelled",
        ));
    }
    let start = Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args)
        .envs(environment.iter().copied())
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    if let Ok(mut group) = cancel.group.lock() {
        *group = Some(child.id());
    }
    // A cancel that raced the spawn.
    if cancel.is_cancelled() {
        kill_group(child.id());
    }
    let stdout = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            buf
        })
    });
    let stderr = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            buf
        })
    });
    // Poll, holding the lock across reaping, so a cancel never signals a
    // process group whose leader has been reaped (its id could be reused).
    let status = loop {
        let mut group = cancel
            .group
            .lock()
            .map_err(|_| std::io::Error::other("cancel lock poisoned"))?;
        if let Some(status) = child.try_wait()? {
            *group = None;
            break status;
        }
        drop(group);
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    let tail_from = stderr.len().saturating_sub(STDERR_TAIL);
    Ok(Outcome {
        exit_code: status.code(),
        stdout,
        stderr_tail: String::from_utf8_lossy(stderr.get(tail_from..).unwrap_or_default())
            .into_owned(),
        duration: start.elapsed(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn collects_output_and_exit_code() {
        let cancel = Cancel::default();
        let out = run(
            Path::new("/bin/sh"),
            &["-c".to_owned(), "echo out; echo err >&2; exit 3".to_owned()],
            Path::new("/"),
            &cancel,
        );
        let Ok(out) = out else {
            unreachable!("cannot run sh")
        };
        assert_eq!(out.exit_code, Some(3));
        assert_eq!(out.stdout, b"out\n");
        assert_eq!(out.stderr_tail, "err\n");
    }

    #[test]
    fn cancel_kills_the_process_group() {
        let cancel = Arc::new(Cancel::default());
        let c = Arc::clone(&cancel);
        let handle = std::thread::spawn(move || {
            run(
                Path::new("/bin/sh"),
                &["-c".to_owned(), "sleep 30 & sleep 30; echo done".to_owned()],
                Path::new("/"),
                &c,
            )
        });
        std::thread::sleep(Duration::from_millis(300));
        cancel.cancel();
        let out = handle.join().ok().and_then(Result::ok);
        let Some(out) = out else {
            unreachable!("run failed")
        };
        assert_eq!(out.exit_code, None);
        assert!(out.duration < Duration::from_secs(10));
        assert_eq!(out.stdout, b"");
        // A cancelled handle refuses to start anything else.
        assert!(run(Path::new("/bin/true"), &[], Path::new("/"), &cancel).is_err());
    }
}
