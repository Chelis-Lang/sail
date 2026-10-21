//! Bounded subprocess ownership, cancellation and complete I/O joining.

use crate::Cancellation;
use sail_api::runtime::{Limits, Outcome};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) struct Captured {
    pub(crate) outcome: Outcome,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) error: Option<String>,
}

fn kill(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn reader(mut source: impl Read, bound: usize, overflow: &AtomicBool) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let n = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let remaining = bound.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..n.min(remaining)]);
        if n > remaining {
            overflow.store(true, Ordering::SeqCst);
        }
    }
    Ok(output)
}

fn drain(
    source: impl Read + Send + 'static,
    bound: usize,
    overflow: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<std::io::Result<Vec<u8>>>> {
    thread::Builder::new()
        .name("sail-runtime-output".into())
        .spawn(move || {
            let result = reader(source, bound, &overflow);
            done.store(true, Ordering::SeqCst);
            result
        })
}

fn setup_failure(
    child: &mut Child,
    issue: impl std::fmt::Display,
    stdout: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
) -> Captured {
    kill(child);
    let output = |handle: Option<JoinHandle<std::io::Result<Vec<u8>>>>| {
        handle
            .and_then(|h| h.join().ok())
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    Captured {
        outcome: Outcome::Infrastructure,
        stdout: output(stdout),
        stderr: output(stderr),
        error: Some(issue.to_string()),
    }
}

/// Every exit path kills or reaps the owned child and joins all started I/O workers.
#[allow(
    clippy::too_many_lines,
    reason = "One owner keeps all child and I/O cleanup paths explicit."
)]
pub(crate) fn run(
    path: &Path,
    args: &[String],
    input: &[u8],
    limits: &Limits,
    cancellation: &Cancellation,
) -> Captured {
    if cancellation.is_cancelled() {
        return Captured {
            outcome: Outcome::Cancelled,
            stdout: Vec::new(),
            stderr: Vec::new(),
            error: Some("execution cancelled before spawn".into()),
        };
    }
    let started = Instant::now();
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return Captured {
                outcome: Outcome::Infrastructure,
                stdout: Vec::new(),
                stderr: Vec::new(),
                error: Some(format!("cannot spawn backend: {e}")),
            };
        }
    };
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return setup_failure(&mut child, "backend pipes unavailable", None, None);
    };
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout_done = Arc::new(AtomicBool::new(false));
    let stderr_done = Arc::new(AtomicBool::new(false));
    let writer_done = Arc::new(AtomicBool::new(false));
    let stdout = match drain(
        stdout,
        limits.output_bytes,
        Arc::clone(&overflow),
        Arc::clone(&stdout_done),
    ) {
        Ok(handle) => handle,
        Err(e) => return setup_failure(&mut child, e, None, None),
    };
    let stderr = match drain(
        stderr,
        limits.output_bytes,
        Arc::clone(&overflow),
        Arc::clone(&stderr_done),
    ) {
        Ok(handle) => handle,
        Err(e) => return setup_failure(&mut child, e, Some(stdout), None),
    };
    let bytes = input.to_vec();
    let finished = Arc::clone(&writer_done);
    let writer = match thread::Builder::new()
        .name("sail-runtime-input".into())
        .spawn(move || {
            let result = stdin.write_all(&bytes);
            drop(stdin);
            finished.store(true, Ordering::SeqCst);
            result
        }) {
        Ok(handle) => handle,
        Err(e) => return setup_failure(&mut child, e, Some(stdout), Some(stderr)),
    };
    let mut exit = None;
    let mut failure = None;
    let outcome = loop {
        if cancellation.is_cancelled() {
            break Outcome::Cancelled;
        }
        if overflow.load(Ordering::SeqCst) {
            break Outcome::OutputLimit;
        }
        if started.elapsed() >= Duration::from_millis(limits.wall_time_ms) {
            break Outcome::Timeout;
        }
        if exit.is_none() {
            match child.try_wait() {
                Ok(status) => exit = status,
                Err(e) => {
                    failure = Some(format!("cannot wait for backend: {e}"));
                    break Outcome::Infrastructure;
                }
            }
        }
        if exit.is_some()
            && stdout_done.load(Ordering::SeqCst)
            && stderr_done.load(Ordering::SeqCst)
            && writer_done.load(Ordering::SeqCst)
        {
            break Outcome::Completed;
        }
        thread::sleep(Duration::from_millis(2));
    };
    // Also close any descendants left after the parent exits before joining pipes.
    kill(&mut child);
    let stdout = stdout
        .join()
        .map_err(|_| "stdout worker panicked".to_owned())
        .and_then(|r| r.map_err(|e| e.to_string()));
    let stderr = stderr
        .join()
        .map_err(|_| "stderr worker panicked".to_owned())
        .and_then(|r| r.map_err(|e| e.to_string()));
    let wrote = writer
        .join()
        .map_err(|_| "input worker panicked".to_owned())
        .and_then(|r| r.map_err(|e| e.to_string()));
    let mut outcome = outcome;
    if outcome == Outcome::Completed
        && (!exit.is_some_and(|status| status.success())
            || wrote.is_err()
            || stdout.is_err()
            || stderr.is_err())
    {
        outcome = Outcome::Infrastructure;
        failure = Some("backend did not complete with successful exit and complete I/O".into());
    }
    Captured {
        outcome,
        stdout: stdout.unwrap_or_default(),
        stderr: stderr.unwrap_or_default(),
        error: failure,
    }
}
