//! Durable, FIFO project verification jobs on a separate local endpoint.

mod product;
mod result_cache;
mod store;
mod worker;

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sail_api::jobs::{Currentness, Lifecycle, Request, Response, Status, Submission};

use store::Stored;

const MAX_CLIENTS: usize = 32;

struct State {
    root: PathBuf,
    executable: PathBuf,
    jobs: Mutex<BTreeMap<String, Stored>>,
    queue: Mutex<VecDeque<String>>,
    ready: Condvar,
    stop: AtomicBool,
    activity: Arc<AtomicU64>,
    proofs: crate::proof_journal::Journals,
    freshness: sail_verus::managed::FreshnessIndex,
    result_cache: result_cache::ResultCache,
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
    let socket = std::path::absolute(socket)?;
    let root = std::path::absolute(root)?;
    let executable = std::path::absolute(executable)?;
    store::private_dir(&root)?;
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket.exists() {
        if UnixStream::connect(&socket).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "SAIL0803: job endpoint already running",
            ));
        }
        std::fs::remove_file(&socket)?;
    }
    let jobs = store::load(&root)?;
    let listener = UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    configure_accept(&listener)?;
    let result_cache = result_cache::ResultCache::default();
    Ok(JobServer {
        socket,
        listener,
        state: Arc::new(State {
            root,
            executable,
            jobs: Mutex::new(jobs),
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            stop: AtomicBool::new(false),
            activity: Arc::new(AtomicU64::new(0)),
            proofs: crate::proof_journal::Journals::default(),
            freshness: sail_verus::managed::FreshnessIndex::default(),
            result_cache,
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
            // Wake a blocked accept; its bounded wait remains the fallback.
            let _ = UnixStream::connect(&self.socket);
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

/// Bounded wait of one blocking `accept` before the endpoint rechecks `stop`.
#[cfg(target_os = "linux")]
const ACCEPT_WAIT: Duration = Duration::from_millis(100);

/// On Linux, a blocking `accept` returns as soon as a client connects; its
/// `SO_RCVTIMEO` bound lets the endpoint observe shutdown without polling.
/// Other platforms keep the original nonblocking poll.
fn configure_accept(listener: &UnixListener) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        listener.set_nonblocking(false)?;
        socket2::SockRef::from(listener).set_read_timeout(Some(ACCEPT_WAIT))
    }
    #[cfg(not(target_os = "linux"))]
    {
        listener.set_nonblocking(true)
    }
}

fn endpoint(listener: &UnixListener, state: &Arc<State>) {
    let clients = Arc::new(AtomicUsize::new(0));
    while !state.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok(_) if state.stop.load(Ordering::SeqCst) => break,
            Ok((mut stream, _)) => {
                if clients.fetch_add(1, Ordering::SeqCst) >= MAX_CLIENTS {
                    clients.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let state = Arc::clone(state);
                let clients = Arc::clone(&clients);
                let _ = std::thread::spawn(move || {
                    // Accepted sockets inherit nonblocking mode on macOS.
                    // RPC frames must use bounded blocking I/O, including large results.
                    let _ = stream.set_nonblocking(false);
                    let timeout = Some(Duration::from_secs(5));
                    let _ = stream.set_read_timeout(timeout);
                    let _ = stream.set_write_timeout(timeout);
                    let outcome = match sail_api::jobs::read_frame(&mut stream) {
                        Ok(Request::Result { id }) => get(&state, &id)
                            .and_then(|job| store::result_frame(&state, &job))
                            .map(|frame| (None, Some(frame))),
                        Ok(request) => Ok((Some(dispatch(&state, request)), None)),
                        Err(error) => {
                            Ok((Some(Response::Error(format!("SAIL0801: {error}"))), None))
                        }
                    };
                    let write = match outcome {
                        Ok((_, Some(frame))) => {
                            stream.write_all(&frame).and_then(|()| stream.flush())
                        }
                        Ok((Some(response), _)) => {
                            sail_api::jobs::write_frame(&mut stream, &response)
                        }
                        Err(error) => sail_api::jobs::write_frame(
                            &mut stream,
                            &Response::Error(error.to_string()),
                        ),
                        Ok((None, None)) => Err(io::Error::other("SAIL0814: missing RPC response")),
                    };
                    if write.is_err() {
                        let _ = sail_api::jobs::write_frame(&mut stream, &Response::Error("SAIL0814: result exceeds RPC limit; retained files are available in the private job store".to_owned()));
                    }
                    clients.fetch_sub(1, Ordering::SeqCst);
                });
            }
            #[cfg(target_os = "linux")]
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            #[cfg(not(target_os = "linux"))]
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn dispatch(state: &State, request: Request) -> Response {
    let result = match request {
        Request::Capabilities => Ok(Response::Capabilities {
            proof_version: sail_api::proof::VERSION,
            repair: true,
        }),
        Request::ProofEvents { id, cursor, limit } => get(state, &id)
            .and_then(|job| {
                state.proofs.page(
                    &state.root.join(&id),
                    &id,
                    &cursor,
                    limit,
                    job.status.state.terminal(),
                )
            })
            .map(Response::ProofEvents),
        Request::Explain {
            id,
            target,
            function,
            obligation,
        } => product::explain(state, &id, &target, &function, obligation.as_deref())
            .map(|value| Response::Explanation(Box::new(value))),
        Request::SaveBaseline { id, name } => product::save_baseline(state, &id, &name)
            .map(|value| Response::Baseline(Box::new(value))),
        Request::Baselines => product::baselines(state).map(Response::Baselines),
        Request::GetBaseline { name } => {
            product::baseline(state, &name).map(|value| Response::Baseline(Box::new(value)))
        }
        Request::Compare { before, after } => {
            product::compare(state, &before, &after).map(Response::Comparison)
        }
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
        Request::Status { id } => get(state, &id)
            .and_then(|job| store::status(state, &job))
            .map(Response::Status),
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
    if submission.input_revision.is_none() {
        establish_revision(state, &mut submission)?;
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
fn establish_revision(state: &State, submission: &mut Submission) -> io::Result<()> {
    // Refresh remembered inputs outside the lock and without Cargo/downloads.
    let matches: Vec<_> = state
        .jobs
        .lock()
        .map_err(lock_error)?
        .values()
        .filter(|job| {
            job.submission.project_dir == submission.project_dir
                && job.submission.args == submission.args
                && job.submission.mode == submission.mode
                && job.submission.force == submission.force
                && job.submission.environment == submission.environment
        })
        .cloned()
        .collect();
    for active in matches.iter().filter(|job| !job.status.state.terminal()) {
        if let Some((original, current)) = store::running_revision(state, active) {
            if let Some(job) = state
                .jobs
                .lock()
                .map_err(lock_error)?
                .get_mut(&active.status.id)
                && !job.status.state.terminal()
                && job.submission.input_revision.is_none()
            {
                job.submission.input_revision = Some(original.clone());
                job.status.input_revision = Some(original);
                store::save(&state.root, job)?;
            }
            submission.input_revision = Some(current);
            return Ok(());
        }
    }
    if let Some(completed) = matches
        .iter()
        .filter(|job| job.status.state == Lifecycle::Completed)
        .max_by_key(|job| job.status.submitted_at_ms)
    {
        submission.input_revision = store::known_revision(state, completed);
    }
    Ok(())
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

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    /// The endpoint serves while blocked in `accept`, and shutdown wakes it
    /// rather than leaving the listener thread waiting for another client.
    #[test]
    fn blocked_accept_serves_requests_and_wakes_for_shutdown() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-rpc-accept-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let outcome = (|| {
            let socket = root.join("jobs.sock");
            let mut handle =
                bind(&socket, &root.join("jobs"), &std::env::current_exe()?)?.spawn()?;
            let client = sail_api::jobs::Client::new(&socket);
            for _ in 0..3 {
                // Spaced beyond one bounded accept wait on Linux.
                std::thread::sleep(Duration::from_millis(150));
                assert!(matches!(
                    client.call(&Request::Capabilities)?,
                    Response::Capabilities { repair: true, .. }
                ));
            }
            let started = std::time::Instant::now();
            handle.shutdown();
            assert!(started.elapsed() < Duration::from_secs(5));
            assert!(!socket.exists());
            Ok(())
        })();
        std::fs::remove_dir_all(root)?;
        outcome
    }
}
