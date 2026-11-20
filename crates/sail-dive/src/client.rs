//! Parent-owned isolated execution. Dropping a future is not cancellation.
use crate::edits::{Batch, BatchResult};
use crate::{DIVE_VERSION, MAX_DOCUMENT_BYTES, Request, Response, VERSION};
use sail_runtime::temporal::image::{self, Image};
use std::io::{Read, Write as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// The job's own private working and temporary directory, removed on drop.
/// The verified worker copy itself may be shared with later jobs
/// ([`sail_runtime::temporal::image`]); its directory never is.
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Scratch {
    fn new() -> Result<Self, String> {
        let directory = std::env::temp_dir().join(format!(
            "sail-runtime-job-{}-{}",
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
        Ok(Self(directory))
    }
}

/// Process execution result, separate from every model verdict.
#[derive(Debug)]
pub enum RunResult<T = Response> {
    /// Complete version-checked evidence from a successful worker.
    Completed(Box<T>),
    /// The deadline expired. Any running child was killed and reaped.
    TimedOut {
        /// Whether this request started a child before termination.
        worker_started: bool,
    },
    /// Cancellation was requested. Any running child was killed and reaped.
    Cancelled {
        /// Whether this request started a child before termination.
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
    match run_in(
        worker,
        worker_sha256,
        request,
        deadline,
        || cancel.load(Ordering::Acquire),
        started,
    ) {
        Ok(result) => result,
        Err(message) => RunResult::Failed(format!("SAIL4212: {message}")),
    }
}

/// Dispatch an ordered group through one authenticated native worker.
///
/// The caller records `started` before reading the batch. The deadline and
/// cancellation remain in force through input transport and result validation.
#[must_use]
pub fn run_edit_session(
    worker: &Path,
    worker_sha256: &str,
    batch: &Batch,
    started: Instant,
    cancel: &AtomicBool,
) -> RunResult<BatchResult> {
    if let Err(error) = batch.admit() {
        return RunResult::Failed(error);
    }
    run_in(
        worker,
        worker_sha256,
        batch,
        Duration::from_millis(batch.wall_time_ms),
        || cancel.load(Ordering::Acquire),
        started,
    )
    .unwrap_or_else(|e| RunResult::Failed(format!("SAIL4212: {e}")))
}

/// Qualify two requests with the same authenticated child and parent-owned deadline as an ordinary check.
/// A successful comparison supplies a conditional relation and carries no model verdict.
#[must_use]
pub fn project(
    worker: &Path,
    sha: &str,
    query: &crate::edits::projection::Query,
    deadline: Duration,
    started: Instant,
    cancel: &sail_runtime::Cancellation,
) -> RunResult<crate::edits::projection::Relation> {
    run_in(
        worker,
        sha,
        query,
        deadline,
        || cancel.is_cancelled(),
        started,
    )
    .unwrap_or_else(|e| RunResult::Failed(format!("SAIL4212: {e}")))
}

/// Check a conditional first-claim relation in the authenticated worker under the shared deadline.
#[must_use]
pub fn project_first_claim(
    worker: &Path,
    sha: &str,
    query: &crate::edits::first_claim::Query,
    deadline: Duration,
    started: Instant,
    cancel: &sail_runtime::Cancellation,
) -> RunResult<crate::edits::first_claim::Relation> {
    run_in(
        worker,
        sha,
        query,
        deadline,
        || cancel.is_cancelled(),
        started,
    )
    .unwrap_or_else(|e| RunResult::Failed(format!("SAIL4212: {e}")))
}

trait WorkerRequest: serde::Serialize {
    type Reply;
    const ARG: Option<&'static str>;
    fn checked_reply(&self, bytes: &[u8]) -> Result<Self::Reply, String>;
}

impl WorkerRequest for Request {
    type Reply = Response;
    const ARG: Option<&'static str> = None;

    fn checked_reply(&self, bytes: &[u8]) -> Result<Self::Reply, String> {
        let response = crate::decode_response(bytes).map_err(|e| e.to_string())?;
        if response.version != VERSION || response.backend_revision != DIVE_VERSION {
            return Err("worker response version or backend identity differs".into());
        }
        if response.request_sha256 != crate::request_sha256(self).map_err(|e| e.to_string())? {
            return Err("worker response belongs to another request".into());
        }
        crate::validate_response(self, &response)?;
        Ok(response)
    }
}

impl WorkerRequest for crate::edits::projection::Query {
    type Reply = crate::edits::projection::Relation;
    const ARG: Option<&'static str> = Some("--project-invariants");

    fn checked_reply(&self, bytes: &[u8]) -> Result<Self::Reply, String> {
        let relation = sail_api::temporal::decode(bytes).map_err(|e| e.to_string())?;
        self.validate(&relation)?;
        Ok(relation)
    }
}

impl WorkerRequest for crate::edits::first_claim::Query {
    type Reply = crate::edits::first_claim::Relation;
    const ARG: Option<&'static str> = Some("--project-first-temporal-claim");

    fn checked_reply(&self, bytes: &[u8]) -> Result<Self::Reply, String> {
        let relation = sail_api::temporal::decode(bytes).map_err(|e| e.to_string())?;
        self.validate(&relation)?;
        Ok(relation)
    }
}

impl WorkerRequest for Batch {
    type Reply = BatchResult;
    const ARG: Option<&'static str> = Some("--edit-session");

    fn checked_reply(&self, bytes: &[u8]) -> Result<Self::Reply, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            backend_source: String,
            result: BatchResult,
        }
        let reply: Reply = sail_api::temporal::decode(bytes).map_err(|e| e.to_string())?;
        if reply.backend_source != crate::edits::BACKEND_SOURCE {
            return Err("edit worker backend source differs".into());
        }
        reply.result.validate(self)?;
        Ok(reply.result)
    }
}

fn spawn(image: &Image, scratch: &Scratch, argument: Option<&str>) -> std::io::Result<Child> {
    let mut command = Command::new(image.executable());
    command
        .args(argument)
        .env_clear()
        .env("TMPDIR", &scratch.0)
        .current_dir(&scratch.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    command.spawn()
}

#[allow(
    clippy::too_many_lines,
    reason = "One scope owns child lifetime, all transport workers and their complete drain."
)]
fn run_in<R: WorkerRequest>(
    worker: &Path,
    worker_sha256: &str,
    request: &R,
    deadline: Duration,
    cancelled: impl Fn() -> bool,
    started: Instant,
) -> Result<RunResult<R::Reply>, String> {
    let interruption = |worker_started| {
        if cancelled() {
            Some(RunResult::Cancelled { worker_started })
        } else if started.elapsed() >= deadline {
            Some(RunResult::TimedOut { worker_started })
        } else {
            None
        }
    };
    if let Some(reason) = interruption(false) {
        return Ok(reason);
    }
    let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err("worker request exceeds byte bound".into());
    }
    if let Some(reason) = interruption(false) {
        return Ok(reason);
    }
    let image = image::image(worker, worker_sha256)?;
    if let Some(reason) = interruption(false) {
        return Ok(reason);
    }
    let scratch = Scratch::new()?;
    // Held, like `image`, until the worker is reaped.
    let mut _retried = None;
    let child = match spawn(&image, &scratch, R::ARG) {
        Ok(child) => child,
        // A reused copy retired by a concurrent reinstall between lookup and
        // spawn: verify the installed bytes again for this job alone.
        Err(_) if image.reused() => {
            let fresh = image::fresh(worker, worker_sha256)?;
            if let Some(reason) = interruption(false) {
                return Ok(reason);
            }
            let child = spawn(&fresh, &scratch, R::ARG).map_err(|e| e.to_string())?;
            _retried = Some(fresh);
            child
        }
        Err(error) => return Err(error.to_string()),
    };
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
            if let Some(reason) = interruption(true) {
                termination = Some(reason);
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
        let response = request.checked_reply(&bytes)?;
        if let Some(reason) = interruption(true) {
            return Ok(reason);
        }
        Ok(RunResult::Completed(Box::new(response)))
    })
}
