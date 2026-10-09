//! Durable, FIFO project verification jobs on a separate local endpoint.

mod store;
mod worker;

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sail_api::jobs::{Currentness, Lifecycle, Request, Response, Status, Submission};

use store::Stored;

struct State {
    root: PathBuf,
    executable: PathBuf,
    jobs: Mutex<BTreeMap<String, Stored>>,
    queue: Mutex<VecDeque<String>>,
    ready: Condvar,
    stop: AtomicBool,
    activity: Arc<AtomicU64>,
}

/// A bound project endpoint; it owns one FIFO verification worker.
pub struct JobServer {
    socket: PathBuf,
    listener: UnixListener,
    state: Arc<State>,
}

/// Lifetime and idle-activity handle for a running project service.
pub struct JobHandle {
    socket: PathBuf,
    state: Arc<State>,
    listener: Option<JoinHandle<()>>,
    worker: Option<JoinHandle<()>>,
}

/// Binds an endpoint and recovers retained job records. Unfinished records
/// become infrastructure errors rather than silently resuming.
///
/// # Errors
/// Returns socket, ownership, persistence, or thread setup errors.
pub fn bind(socket: &Path, root: &Path, executable: &Path) -> io::Result<JobServer> {
    store::private_dir(root)?;
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket.exists() {
        if UnixStream::connect(socket).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "SAIL0803: job endpoint already running",
            ));
        }
        std::fs::remove_file(socket)?;
    }
    let jobs = store::load(root)?;
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(JobServer {
        socket: socket.to_path_buf(),
        listener,
        state: Arc::new(State {
            root: root.to_path_buf(),
            executable: executable.to_path_buf(),
            jobs: Mutex::new(jobs),
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            stop: AtomicBool::new(false),
            activity: Arc::new(AtomicU64::new(0)),
        }),
    })
}

impl JobServer {
    /// Starts the endpoint and single verifier worker.
    ///
    /// # Errors
    /// Returns thread creation errors.
    pub fn spawn(self) -> io::Result<JobHandle> {
        let worker_state = Arc::clone(&self.state);
        let worker = std::thread::Builder::new()
            .name("sail-project-worker".to_owned())
            .spawn(move || worker::run(&worker_state))?;
        let endpoint_state = Arc::clone(&self.state);
        let listener = self.listener;
        let endpoint = std::thread::Builder::new()
            .name("sail-project-endpoint".to_owned())
            .spawn(move || endpoint(&listener, &endpoint_state));
        match endpoint {
            Ok(listener) => Ok(JobHandle {
                socket: self.socket,
                state: self.state,
                listener: Some(listener),
                worker: Some(worker),
            }),
            Err(error) => {
                self.state.stop.store(true, Ordering::SeqCst);
                self.state.ready.notify_all();
                let _ = worker.join();
                Err(error)
            }
        }
    }
}
impl JobHandle {
    /// Number of queued and running jobs, for the cache daemon's idle guard.
    #[must_use]
    pub fn activity(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.state.activity)
    }
    /// Cancels active work, joins both service threads, and removes the endpoint.
    pub fn shutdown(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        self.state.ready.notify_all();
        if let Ok(mut jobs) = self.state.jobs.lock() {
            for job in jobs.values_mut() {
                job.cancel.store(true, Ordering::SeqCst);
                if !job.status.state.terminal() {
                    job.status.state = Lifecycle::Cancelled;
                    job.status.finished_at_ms = Some(store::now());
                    let _ = store::save(&self.state.root, job);
                }
            }
        }
        if let Some(thread) = self.listener.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.worker.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}
impl Drop for JobHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn endpoint(listener: &UnixListener, state: &Arc<State>) {
    let clients = Arc::new(AtomicU64::new(0));
    while !state.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if clients.fetch_add(1, Ordering::SeqCst) >= 32 {
                    clients.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let state = Arc::clone(state);
                let clients = Arc::clone(&clients);
                let _ = std::thread::spawn(move || {
                    let timeout = Some(Duration::from_secs(5));
                    let _ = stream.set_read_timeout(timeout);
                    let _ = stream.set_write_timeout(timeout);
                    let response = sail_api::jobs::read_frame(&mut stream).map_or_else(
                        |error| Response::Error(format!("SAIL0801: {error}")),
                        |request| dispatch(&state, request),
                    );
                    if sail_api::jobs::write_frame(&mut stream, &response).is_err() {
                        let _ = sail_api::jobs::write_frame(&mut stream, &Response::Error("SAIL0814: result exceeds RPC limit; retained files are available in the private job store".to_owned()));
                    }
                    clients.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn dispatch(state: &State, request: Request) -> Response {
    let result = match request {
        Request::Submit { submission } => submit(state, submission).map(Response::Status),
        Request::List { project_dir } => state.jobs.lock().map_err(lock_error).map(|jobs| {
            let mut statuses: Vec<_> = jobs
                .values()
                .filter(|job| {
                    project_dir
                        .as_ref()
                        .is_none_or(|dir| *dir == job.status.project_dir)
                })
                .map(|job| {
                    let mut status = job.status.clone();
                    if status.state == Lifecycle::Completed {
                        status.currentness = Currentness::Unknown;
                    }
                    status
                })
                .collect();
            statuses.sort_by_key(|status| status.submitted_at_ms);
            Response::List(statuses)
        }),
        Request::Status { id } => get(state, &id).and_then(|job| {
            if job.status.state == Lifecycle::Completed {
                store::result(state, &job).map(|document| Response::Status(document.job))
            } else {
                Ok(Response::Status(job.status))
            }
        }),
        Request::Result { id } => get(state, &id)
            .and_then(|job| store::result(state, &job))
            .map(Response::Result),
        Request::Cancel { id, superseded } => cancel(state, &id, superseded).map(Response::Status),
        Request::Logs { id, offset } => get(state, &id)
            .and_then(|job| store::chunk(state, &job, "stderr", offset))
            .map(Response::Chunk),
        Request::Events { id, offset } => get(state, &id)
            .and_then(|job| store::chunk(state, &job, "events.jsonl", offset))
            .map(Response::Chunk),
    };
    result.unwrap_or_else(|error| Response::Error(error.to_string()))
}
fn lock_error<T>(_: std::sync::PoisonError<T>) -> io::Error {
    io::Error::other("SAIL0804: job state lock poisoned")
}
fn get(state: &State, id: &str) -> io::Result<Stored> {
    state
        .jobs
        .lock()
        .map_err(lock_error)?
        .get(id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SAIL0805: unknown job"))
}
fn submit(state: &State, mut submission: Submission) -> io::Result<Status> {
    if state.stop.load(Ordering::SeqCst) {
        return Err(io::Error::other("SAIL0815: job service is stopping"));
    }
    submission.project_dir = submission.project_dir.canonicalize()?;
    if !submission.project_dir.is_dir() || submission.args.iter().any(|arg| arg.contains('\0')) {
        return Err(io::Error::other("SAIL0806: invalid project or arguments"));
    }
    let mut jobs = state.jobs.lock().map_err(lock_error)?;
    if let Some(job) = jobs.values().find(|job| {
        !job.status.state.terminal()
            && submission.input_revision.is_some()
            && job.submission == submission
    }) {
        return Ok(job.status.clone());
    }
    if jobs
        .values()
        .filter(|job| !job.status.state.terminal())
        .count()
        >= 256
    {
        return Err(io::Error::other("SAIL0815: job queue is full"));
    }
    let job = Stored::new(submission);
    let id = job.status.id.clone();
    store::save(&state.root, &job)?;
    let status = job.status.clone();
    jobs.insert(id.clone(), job);
    state.activity.fetch_add(1, Ordering::SeqCst);
    state.queue.lock().map_err(lock_error)?.push_back(id);
    state.ready.notify_one();
    Ok(status)
}
fn cancel(state: &State, id: &str, superseded: bool) -> io::Result<Status> {
    let mut jobs = state.jobs.lock().map_err(lock_error)?;
    let job = jobs
        .get_mut(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SAIL0805: unknown job"))?;
    if !job.status.state.terminal() {
        job.cancel.store(true, Ordering::SeqCst);
        job.status.state = if superseded {
            Lifecycle::Superseded
        } else {
            Lifecycle::Cancelled
        };
        job.status.finished_at_ms = Some(store::now());
        store::save(&state.root, job)?;
    }
    Ok(job.status.clone())
}
