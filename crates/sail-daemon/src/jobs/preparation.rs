//! One bounded, preemptible worker for optional preparation and cache maintenance.

use std::collections::VecDeque;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sail_api::jobs::Response;
use sail_api::preparation::{Outcome, Status, Submission, VERSION};

use super::State;

const MAX_PENDING: usize = 64;
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// Another daemon thread's child can retain the previous worker's lock until
/// it execs. Preparation is optional, so bound the wait even if that child
/// never reaches exec.
const OWNERSHIP_WAIT: Duration = Duration::from_secs(1);

struct Task {
    request: Submission,
    key: PathBuf,
    bytes: usize,
    after: Instant,
}
struct Active {
    key: PathBuf,
    /// Whether this preparation re-runs a verification that just succeeded
    /// (it has an origin), rather than preparing a new project.
    origin: bool,
    control: Run,
}

/// How long a preempted origin preparation of the caller's own workspace may
/// still run before it is killed. Its own deadline, 60 seconds from its start,
/// is the tighter bound by default.
///
/// A preparation is an ordinary managed run: it consumes its compiler
/// target's build-script export when it begins and republishes it only when
/// it finishes. Killed in between, it leaves the caller's run (the next run
/// of that target) without the export, so that run cleans every build-script
/// package, vstd included, rebuilds the dependency graph and cannot retain
/// what it compiles (#595). An origin preparation re-runs a verification that
/// just succeeded, so letting it finish costs the caller at most that
/// preparation's remaining time, while killing it costs the caller that
/// rebuild.
const PREEMPT_GRACE: Duration = Duration::from_secs(60);

struct Inner {
    pending: VecDeque<Task>,
    active: Option<Active>,
    foreground: usize,
    status: Status,
    grace: Duration,
}
impl Default for Inner {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            active: None,
            foreground: 0,
            status: Status::default(),
            grace: PREEMPT_GRACE,
        }
    }
}
impl Inner {
    /// Gives foreground priority to work on `key`: drops its queued
    /// preparation and stops the running one. A running origin preparation of
    /// the same workspace is only preempted, so it may finish within
    /// [`PREEMPT_GRACE`] (the caller waits for it on the target lock); every
    /// other running preparation, including any for another workspace, is
    /// killed at once.
    fn preempt(&mut self, key: &Path) {
        if let Some(active) = &self.active {
            if active.origin && active.key == key {
                active.control.preempted.store(true, Ordering::SeqCst);
            } else {
                active.control.cancel.store(true, Ordering::SeqCst);
            }
        }
        let before = self.pending.len();
        self.pending.retain(|task| task.key != key);
        self.status.superseded = self
            .status
            .superseded
            .saturating_add((before - self.pending.len()) as u64);
    }
}
#[derive(Default)]
pub(super) struct Coordinator(Mutex<Inner>);

impl Coordinator {
    /// Replaces [`PREEMPT_GRACE`].
    pub(super) fn set_grace(&self, grace: Duration) {
        if let Ok(mut inner) = self.0.lock() {
            inner.grace = grace;
        }
    }
    pub(super) fn status(&self) -> io::Result<Status> {
        let inner = self.0.lock().map_err(super::lock_error)?;
        let mut status = inner.status.clone();
        status.version = VERSION;
        status.queued = inner.pending.len();
        status.running = inner.active.as_ref().map(|active| active.key.clone());
        Ok(status)
    }
    pub(super) fn preempt(&self, key: &Path) {
        if let Ok(mut inner) = self.0.lock() {
            inner.preempt(key);
        }
    }
    pub(super) fn enqueue(&self, mut request: Submission) -> io::Result<Status> {
        if request.version != VERSION
            || request.verification.force
            || request.verification.mode != sail_api::jobs::Mode::Stock
            || !request.verification.project_dir.is_absolute()
            || request
                .verification
                .args
                .iter()
                .any(|arg| arg.contains('\0'))
        {
            return Err(io::Error::other("SAIL0806: invalid preparation request"));
        }
        request.verification.project_dir = request.verification.project_dir.canonicalize()?;
        if !request.verification.project_dir.is_dir() {
            return Err(io::Error::other(
                "SAIL0806: preparation directory is not a directory",
            ));
        }
        let bytes = serde_json::to_vec(&request)
            .map_err(io::Error::other)?
            .len();
        let key = super::schedule::workspace_key(
            &request.verification.project_dir,
            &request.verification.args,
        );
        let mut inner = self.0.lock().map_err(super::lock_error)?;
        let existing = inner.pending.iter().position(|task| task.key == key);
        let pending_bytes = inner
            .pending
            .iter()
            .filter(|task| task.key != key)
            .fold(0usize, |sum, task| sum.saturating_add(task.bytes));
        if bytes > MAX_BYTES
            || pending_bytes.saturating_add(bytes) > MAX_BYTES
            || (existing.is_none() && inner.pending.len() >= MAX_PENDING)
        {
            return Err(io::Error::other("SAIL0815: preparation queue is full"));
        }
        if let Some(index) = existing {
            inner.pending.remove(index);
            inner.status.superseded = inner.status.superseded.saturating_add(1);
        }
        if let Some(active) = &inner.active
            && active.key == key
        {
            active.control.cancel.store(true, Ordering::SeqCst);
        }
        let delay = if request.origin.is_some() {
            Duration::from_secs(2)
        } else {
            Duration::from_millis(500)
        };
        inner.pending.push_back(Task {
            request,
            key,
            bytes,
            after: Instant::now() + delay,
        });
        drop(inner);
        self.status()
    }
}

/// Foreground connections own priority until EOF, even outside the job queue.
pub(super) fn foreground(
    state: &Arc<State>,
    stream: &mut std::os::unix::net::UnixStream,
    version: u32,
    project: &Path,
    args: &[String],
) -> io::Result<()> {
    if version != VERSION {
        return Err(io::Error::other(
            "SAIL0802: unsupported preparation version",
        ));
    }
    let key = super::schedule::workspace_key(project, args);
    {
        let mut inner = state.preparation.0.lock().map_err(super::lock_error)?;
        // Cancellation and admission must share the lock with `take`: no
        // preparation may start between preemption and acquiring this lease.
        inner.preempt(&key);
        inner.foreground = inner.foreground.saturating_add(1);
    }
    state.activity.fetch_add(1, Ordering::SeqCst);
    let _lease = Foreground(state);
    sail_api::jobs::write_frame(stream, &Response::Preparation(state.preparation.status()?))?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut byte = [0];
    while !state.stop.load(Ordering::SeqCst) {
        match stream.read(&mut byte) {
            Ok(_) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
struct Foreground<'a>(&'a State);
impl Drop for Foreground<'_> {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.0.preparation.0.lock() {
            inner.foreground = inner.foreground.saturating_sub(1);
        }
        self.0.activity.fetch_sub(1, Ordering::SeqCst);
        self.0.ready.notify_all();
    }
}

pub(super) fn run(state: &State, project_cache: Option<&Path>) {
    let mut budget_after = Instant::now();
    while !state.stop.load(Ordering::SeqCst) {
        let (idle, task) = match state.queue.lock() {
            Ok(queue) => {
                let idle = queue.idle();
                (idle, if idle { take(&state.preparation) } else { None })
            }
            Err(_) => break,
        };
        if let Some((task, control, grace)) = task {
            state.activity.fetch_add(1, Ordering::SeqCst);
            let outcome = execute(state, &task.request, &control, grace);
            if let Ok(mut inner) = state.preparation.0.lock() {
                inner.active = None;
                let ready = outcome.as_ref().is_ok_and(|outcome| outcome.ready);
                if (control.superseded() && !ready) || state.stop.load(Ordering::SeqCst) {
                    inner.status.superseded = inner.status.superseded.saturating_add(1);
                } else if ready {
                    inner.status.ready = inner.status.ready.saturating_add(1);
                } else {
                    inner.status.failed = inner.status.failed.saturating_add(1);
                }
            }
            if task.request.origin.is_none()
                && !control.superseded()
                && !state.stop.load(Ordering::SeqCst)
                && let Ok(Outcome {
                    origin: Some(origin),
                    ..
                }) = outcome
            {
                let mut next = task.request;
                next.origin = Some(origin);
                let _ = state.preparation.enqueue(next);
            }
            state.activity.fetch_sub(1, Ordering::SeqCst);
        } else if idle
            && Instant::now() >= budget_after
            && state
                .preparation
                .0
                .lock()
                .is_ok_and(|inner| inner.foreground == 0 && inner.active.is_none())
        {
            budget_after = Instant::now() + Duration::from_secs(60);
            if let Some(cache) = project_cache
                && let Ok(Some(limit)) = sail_verus::managed::projects::budget::load(cache)
            {
                let _ = sail_verus::managed::projects::budget::enforce(cache, limit, false);
            }
        }
        let Ok(queue) = state.queue.lock() else { break };
        let _ = state.ready.wait_timeout(queue, Duration::from_millis(100));
    }
}

/// The controls of one running preparation: `cancel` kills it at once;
/// `preempted` kills it once the grace has passed.
#[derive(Clone, Default)]
struct Run {
    cancel: Arc<AtomicBool>,
    preempted: Arc<AtomicBool>,
}
impl Run {
    /// Whether the preparation was cancelled or preempted, whether or not it
    /// was then allowed to finish.
    fn superseded(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || self.preempted.load(Ordering::SeqCst)
    }
}

fn take(coordinator: &Coordinator) -> Option<(Task, Run, Duration)> {
    let mut inner = coordinator.0.lock().ok()?;
    if inner.foreground != 0 || inner.active.is_some() {
        return None;
    }
    let index = inner
        .pending
        .iter()
        .position(|task| Instant::now() >= task.after)?;
    let task = inner.pending.remove(index)?;
    let control = Run::default();
    inner.active = Some(Active {
        key: task.key.clone(),
        origin: task.request.origin.is_some(),
        control: control.clone(),
    });
    Some((task, control, inner.grace))
}

fn execute(state: &State, task: &Submission, run: &Run, grace: Duration) -> io::Result<Outcome> {
    use std::os::unix::process::CommandExt as _;
    if run.superseded() || state.stop.load(Ordering::SeqCst) {
        return Err(io::Error::other("preparation cancelled"));
    }
    // The owned private file prevents a pipe-full deadlock on worker results.
    let directory = state.root.join("preparation");
    super::store::private_dir(&directory)?;
    let ownership = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("worker.lock"))?;
    lock_worker(
        &ownership,
        run,
        &state.stop,
        Instant::now() + OWNERSHIP_WAIT,
    )?;
    let request_file = directory.join("request.json");
    sail_api::jobs::write_frame(&mut super::store::output(&request_file)?, task)?;
    let result = directory.join("outcome.json");
    let stdout = super::store::output(&result)?;
    let stderr = super::store::output(&directory.join("stderr"))?;
    let mut process = super::worker::Process(
        Command::new(&state.executable)
            .arg("_prepare-worker")
            .arg(&request_file)
            .current_dir(&task.verification.project_dir)
            .env_clear()
            .envs(&task.verification.environment)
            .stdin(Stdio::piped())
            .stdout(stdout)
            .stderr(stderr)
            .process_group(0)
            .spawn()?,
    );
    let _parent = process
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("preparation input unavailable"))?;
    let deadline = Instant::now()
        + if task.origin.is_some() {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(600)
        };
    let mut preempted_at = None;
    loop {
        if run.preempted.load(Ordering::SeqCst) {
            preempted_at.get_or_insert_with(Instant::now);
        }
        if run.cancel.load(Ordering::SeqCst)
            || state.stop.load(Ordering::SeqCst)
            || Instant::now() >= deadline
            || preempted_at.is_some_and(|at: Instant| at.elapsed() >= grace)
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preparation cancelled or timed out",
            ));
        }
        if let Some(status) = process.try_wait()? {
            if !status.success() {
                return Err(io::Error::other("preparation worker declined"));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(result)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Keep the private request/result files owned across short inherited-lock
/// windows without delaying foreground admission or daemon shutdown.
fn lock_worker(
    ownership: &std::fs::File,
    run: &Run,
    stop: &AtomicBool,
    deadline: Instant,
) -> io::Result<()> {
    loop {
        // No worker has started yet, so even same-workspace preemption can
        // stop immediately without taking its build-script export.
        if run.superseded() || stop.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preparation cancelled while waiting for worker ownership",
            ));
        }
        match ownership.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "preparation worker ownership remained busy",
                    ));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(20)));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error),
        }
    }
}
