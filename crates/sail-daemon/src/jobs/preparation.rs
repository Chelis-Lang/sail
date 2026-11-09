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

struct Task {
    request: Submission,
    key: PathBuf,
    bytes: usize,
    after: Instant,
}
struct Active {
    key: PathBuf,
    cancel: Arc<AtomicBool>,
    /// The origin's unchanged-call key, which an identical foreground call adopts.
    origin: Option<String>,
}
/// Spike (#558): the longest a foreground call waits for an adopted preparation.
const ADOPT_LIMIT: Duration = Duration::from_secs(30);
#[derive(Default)]
struct Inner {
    pending: VecDeque<Task>,
    active: Option<Active>,
    foreground: usize,
    status: Status,
}
impl Inner {
    fn preempt(&mut self, key: &Path) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::SeqCst);
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
            active.cancel.store(true, Ordering::SeqCst);
        }
        let delay = if request.origin.is_some() {
            // Spike (#558): the worker binds unsettled files by content, so
            // it no longer waits for the origin's outputs to settle.
            Duration::ZERO
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
    adopt: Option<&str>,
) -> io::Result<()> {
    if version != VERSION {
        return Err(io::Error::other(
            "SAIL0802: unsupported preparation version",
        ));
    }
    let key = super::schedule::workspace_key(project, args);
    if let Some(adopt) = adopt {
        let outcome = await_adopted(state, &key, adopt);
        adopt_trace(state, outcome);
    }
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
/// Wait while a preparation of this workspace with the caller's exact key is
/// queued or running. Returns immediately when there is none; whatever
/// happened to it, the caller then takes the ordinary path (replay or run).
fn await_adopted(state: &State, key: &Path, adopt: &str) -> (&'static str, Duration) {
    let started = Instant::now();
    let mut seen = false;
    loop {
        let matched = {
            let Ok(mut inner) = state.preparation.0.lock() else {
                return ("lock", started.elapsed());
            };
            let running = inner.active.as_ref().is_some_and(|active| {
                active.key == key
                    && active.origin.as_deref() == Some(adopt)
                    && !active.cancel.load(Ordering::SeqCst)
            });
            let mut queued = false;
            for task in &mut inner.pending {
                if task.key == key && task.request.origin.as_ref().is_some_and(|o| o.key == adopt) {
                    task.after = Instant::now();
                    queued = true;
                }
            }
            running || queued
        };
        if !matched {
            return (if seen { "finished" } else { "none" }, started.elapsed());
        }
        seen = true;
        if state.stop.load(Ordering::SeqCst) || started.elapsed() >= ADOPT_LIMIT {
            return ("limit", started.elapsed());
        }
        state.ready.notify_all();
        std::thread::sleep(Duration::from_millis(2));
    }
}
/// Spike diagnostics only: one line per adoption decision.
fn adopt_trace(state: &State, (outcome, waited): (&'static str, Duration)) {
    use std::io::Write as _;
    let directory = state.root.join("preparation");
    if super::store::private_dir(&directory).is_ok()
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("adopt.log"))
    {
        let _ = writeln!(file, "{outcome} {:.1}", waited.as_secs_f64() * 1000.0);
    }
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
        if let Some((task, cancel)) = task {
            state.activity.fetch_add(1, Ordering::SeqCst);
            let outcome = execute(state, &task.request, &cancel);
            if let Ok(mut inner) = state.preparation.0.lock() {
                inner.active = None;
                if cancel.load(Ordering::SeqCst) || state.stop.load(Ordering::SeqCst) {
                    inner.status.superseded = inner.status.superseded.saturating_add(1);
                } else if outcome.as_ref().is_ok_and(|outcome| outcome.ready) {
                    inner.status.ready = inner.status.ready.saturating_add(1);
                } else {
                    inner.status.failed = inner.status.failed.saturating_add(1);
                }
            }
            if task.request.origin.is_none()
                && !cancel.load(Ordering::SeqCst)
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
        let _ = state.ready.wait_timeout(queue, Duration::from_millis(20));
    }
}

fn take(coordinator: &Coordinator) -> Option<(Task, Arc<AtomicBool>)> {
    let mut inner = coordinator.0.lock().ok()?;
    if inner.foreground != 0 || inner.active.is_some() {
        return None;
    }
    let index = inner
        .pending
        .iter()
        .position(|task| Instant::now() >= task.after)?;
    let task = inner.pending.remove(index)?;
    let cancel = Arc::new(AtomicBool::new(false));
    inner.active = Some(Active {
        key: task.key.clone(),
        cancel: Arc::clone(&cancel),
        origin: task
            .request
            .origin
            .as_ref()
            .map(|origin| origin.key.clone()),
    });
    Some((task, cancel))
}

fn execute(state: &State, task: &Submission, cancel: &AtomicBool) -> io::Result<Outcome> {
    use std::os::unix::process::CommandExt as _;
    if cancel.load(Ordering::SeqCst) || state.stop.load(Ordering::SeqCst) {
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
    ownership
        .try_lock()
        .map_err(|error| io::Error::other(error.to_string()))?;
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
    loop {
        if cancel.load(Ordering::SeqCst)
            || state.stop.load(Ordering::SeqCst)
            || Instant::now() >= deadline
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
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(result)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
