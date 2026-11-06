//! Parent-owned isolated execution. Dropping a future is not cancellation.
use crate::{DIVE_REVISION, MAX_DOCUMENT_BYTES, Request, Response, VERSION, sha256};
use std::io::{Read, Write as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct WorkerImage {
    directory: std::path::PathBuf,
    executable: std::path::PathBuf,
}
impl Drop for WorkerImage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl WorkerImage {
    fn freeze(path: &Path, expected: &str) -> Result<Self, String> {
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("worker requires a pinned lowercase SHA-256".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(512 * 1024 * 1024 + 1).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if bytes.len() > 512 * 1024 * 1024 {
            return Err("worker executable exceeds its 512 MiB admission bound".into());
        }
        if sha256(&bytes) != expected {
            return Err("worker executable differs from its pinned identity".into());
        }

        let directory = std::env::temp_dir().join(format!(
            "sail-runtime-image-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(|e| e.to_string())?;
        let image = Self {
            executable: directory.join("sail-runtime-worker"),
            directory,
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o500);
        }
        options
            .open(&image.executable)
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| e.to_string())?;
        Ok(image)
    }
}

/// Process execution result, separate from every model verdict.
#[derive(Debug)]
pub enum RunResult {
    /// Complete version-checked evidence from a successful worker.
    Completed(Box<Response>),
    /// Parent killed and reaped the worker at its deadline.
    TimedOut {
        /// Whether an actual child was killed and reaped.
        worker_started: bool,
    },
    /// Parent killed and reaped the worker after explicit cancellation.
    Cancelled {
        /// Whether an actual child was killed and reaped.
        worker_started: bool,
    },
    /// Startup, protocol or process failure, with no logical verdict.
    Failed(String),
}

struct OwnedChild(Child);
impl OwnedChild {
    fn stop(&mut self) {
        #[cfg(unix)]
        if let Ok(raw) = i32::try_from(self.0.id())
            && let Some(pid) = rustix::process::Pid::from_raw(raw)
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn bounded(mut input: impl Read, overflow: &AtomicBool) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(u64::try_from(MAX_DOCUMENT_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        overflow.store(true, Ordering::Release);
        return Err(std::io::Error::other(
            "SAIL4211: worker output exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

/// Runs the S.A.I.L.-owned worker, killing and reaping it on deadline/cancel.
///
/// The deadline includes serialization, startup, input transport, binding and
/// execution. No hard RAM limit is claimed. All started transport threads are
/// joined before return. No timeout, failed process or partial document is proof.
#[must_use]
pub fn run(
    worker: &Path,
    worker_sha256: &str,
    request: &Request,
    deadline: Duration,
    cancel: &AtomicBool,
) -> RunResult {
    let started = Instant::now();
    match run_in(worker, worker_sha256, request, deadline, cancel, started) {
        Ok(result) => result,
        Err(message) => RunResult::Failed(format!("SAIL4212: {message}")),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "One scope owns child lifetime, all transport workers and their complete drain."
)]
fn run_in(
    worker: &Path,
    worker_sha256: &str,
    request: &Request,
    deadline: Duration,
    cancel: &AtomicBool,
    started: Instant,
) -> Result<RunResult, String> {
    if cancel.load(Ordering::Acquire) {
        return Ok(RunResult::Cancelled {
            worker_started: false,
        });
    }
    if started.elapsed() >= deadline {
        return Ok(RunResult::TimedOut {
            worker_started: false,
        });
    }
    let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err("worker request exceeds byte bound".into());
    }
    let image = WorkerImage::freeze(worker, worker_sha256)?;
    if cancel.load(Ordering::Acquire) {
        return Ok(RunResult::Cancelled {
            worker_started: false,
        });
    }
    if started.elapsed() >= deadline {
        return Ok(RunResult::TimedOut {
            worker_started: false,
        });
    }
    let mut command = Command::new(&image.executable);
    command
        .env_clear()
        .env("TMPDIR", &image.directory)
        .current_dir(&image.directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let child = command.spawn().map_err(|e| e.to_string())?;
    let mut child = OwnedChild(child);
    let mut stdin = child.0.stdin.take().ok_or("worker stdin unavailable")?;
    let stdout = child.0.stdout.take().ok_or("worker stdout unavailable")?;
    let stderr = child.0.stderr.take().ok_or("worker stderr unavailable")?;
    let overflow = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let writer = std::thread::Builder::new()
            .name("sail-worker-input".into())
            .spawn_scoped(scope, move || stdin.write_all(&bytes))
            .map_err(|e| {
                child.stop();
                e.to_string()
            })?;
        let output = std::thread::Builder::new()
            .name("sail-worker-output".into())
            .spawn_scoped(scope, || bounded(stdout, &overflow))
            .map_err(|e| {
                child.stop();
                e.to_string()
            })?;
        let errors = std::thread::Builder::new()
            .name("sail-worker-errors".into())
            .spawn_scoped(scope, || bounded(stderr, &overflow))
            .map_err(|e| {
                child.stop();
                e.to_string()
            })?;
        let mut termination = None;
        let mut status = None;
        loop {
            if cancel.load(Ordering::Acquire) {
                termination = Some(RunResult::Cancelled {
                    worker_started: true,
                });
                child.stop();
                break;
            }
            if started.elapsed() >= deadline {
                termination = Some(RunResult::TimedOut {
                    worker_started: true,
                });
                child.stop();
                break;
            }
            if overflow.load(Ordering::Acquire) {
                termination = Some(RunResult::Failed(
                    "SAIL4211: worker output exceeds byte bound".into(),
                ));
                child.stop();
                break;
            }
            if status.is_none() {
                match child.0.try_wait() {
                    Ok(Some(exit)) => status = Some(exit),
                    Ok(None) => {}
                    Err(error) => {
                        termination = Some(RunResult::Failed(error.to_string()));
                        child.stop();
                        break;
                    }
                }
            }
            // Descendants can retain pipes after the leader exits. The deadline
            // remains active until every transport operation actually finishes.
            if status.is_some()
                && writer.is_finished()
                && output.is_finished()
                && errors.is_finished()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let written = writer
            .join()
            .map_err(|_| "worker writer panicked".to_owned());
        let output = output
            .join()
            .map_err(|_| "worker output reader panicked".to_owned());
        let errors = errors
            .join()
            .map_err(|_| "worker error reader panicked".to_owned());
        if let Some(termination) = termination {
            return Ok(termination);
        }
        let status = status.ok_or("worker status unavailable")?;
        let stderr = errors?.map_err(|e| e.to_string())?;
        if !status.success() || !stderr.is_empty() {
            return Err(format!(
                "worker exited {status} with stderr: {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        written?.map_err(|e| e.to_string())?;
        let bytes = output?.map_err(|e| e.to_string())?;
        let response = crate::decode_response(&bytes).map_err(|e| e.to_string())?;
        if response.version != VERSION || response.backend_revision != DIVE_REVISION {
            return Err("worker response version or backend identity differs".into());
        }
        if response.request_sha256 != crate::request_sha256(request).map_err(|e| e.to_string())? {
            return Err("worker response belongs to another request".into());
        }
        crate::validate_response(request, &response)?;
        Ok(RunResult::Completed(Box::new(response)))
    })
}
