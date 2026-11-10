//! Durable project verification jobs on a separate local endpoint. Jobs run
//! concurrently on a small worker pool, one at a time per Cargo workspace.

mod contracts;
mod mutation;
mod preparation;
mod product;
mod result_cache;
mod schedule;
mod store;
mod worker;

use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
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
    mutations: Mutex<mutation::Controllers>,
    queue: Mutex<schedule::Queue>,
    ready: Condvar,
    stop: AtomicBool,
    activity: Arc<AtomicU64>,
    proofs: crate::proof_journal::Journals,
    freshness: sail_verus::managed::FreshnessIndex,
    result_cache: result_cache::ResultCache,
    preparation: preparation::Coordinator,
}

/// A bound project endpoint; it owns a bounded pool of verification workers.
pub struct JobServer {
    socket: PathBuf,
    listener: UnixListener,
    state: Arc<State>,
    workers: usize,
    project_cache: Option<PathBuf>,
}

/// Lifetime and idle-activity handle for a running project service.
pub struct JobHandle {
    socket: PathBuf,
    state: Arc<State>,
    listener: Option<JoinHandle<()>>,
    workers: Vec<JoinHandle<()>>,
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
        if sail_api::client::probe_listener(&socket)? {
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
        workers: schedule::workers(),
        project_cache: None,
        state: Arc::new(State {
            root,
            executable,
            jobs: Mutex::new(jobs),
            mutations: Mutex::new(BTreeMap::new()),
            queue: Mutex::new(schedule::Queue::default()),
            ready: Condvar::new(),
            stop: AtomicBool::new(false),
            activity: Arc::new(AtomicU64::new(0)),
            proofs: crate::proof_journal::Journals::default(),
            freshness: sail_verus::managed::FreshnessIndex,
            result_cache,
            preparation: preparation::Coordinator::default(),
        }),
    })
}

impl JobServer {
    /// Select the managed-project cache for optional idle budget maintenance.
    ///
    /// The job store and solver-reply store do not identify this namespace.
    /// A server without an explicit project cache performs no budget pruning.
    ///
    /// # Errors
    /// Returns an error if the cache path cannot be made absolute.
    pub fn with_project_cache(mut self, cache: &Path) -> io::Result<Self> {
        self.project_cache = Some(sail_verus::managed::canonical_cache_root(
            std::path::absolute(cache)?,
        ));
        Ok(self)
    }

    /// Queue optional preparation before starting this service's workers.
    ///
    /// # Errors
    /// Returns invalid request or queue-capacity errors; this grants no proof.
    pub fn enqueue_preparation(
        &self,
        submission: sail_api::preparation::Submission,
    ) -> io::Result<()> {
        self.state.preparation.enqueue(submission).map(|_| ())
    }
    /// Overrides how long a preempted background preparation of the caller's
    /// own workspace may still finish before it is killed (60 seconds by
    /// default; its own deadline also applies). Zero kills it at once.
    #[must_use]
    pub fn with_preemption_grace(self, grace: Duration) -> Self {
        self.state.preparation.set_grace(grace);
        self
    }
    /// Overrides the number of concurrent verifier workers (at least one).
    #[must_use]
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers.max(1);
        self
    }
    /// Starts the endpoint and `SAIL_JOB_WORKERS` verifier workers.
    ///
    /// # Errors
    /// Returns thread creation errors.
    pub fn spawn(self) -> io::Result<JobHandle> {
        let mut workers = Vec::new();
        for index in 0..self.workers {
            let worker_state = Arc::clone(&self.state);
            match std::thread::Builder::new()
                .name(format!("sail-project-worker-{index}"))
                .spawn(move || worker::run(&worker_state))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    self.state.stop.store(true, Ordering::SeqCst);
                    self.state.ready.notify_all();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error);
                }
            }
        }
        let preparation_state = Arc::clone(&self.state);
        let project_cache = self.project_cache;
        match std::thread::Builder::new()
            .name("sail-preparation".to_owned())
            .spawn(move || preparation::run(&preparation_state, project_cache.as_deref()))
        {
            Ok(worker) => workers.push(worker),
            Err(error) => {
                self.state.stop.store(true, Ordering::SeqCst);
                self.state.ready.notify_all();
                for worker in workers {
                    let _ = worker.join();
                }
                return Err(error);
            }
        }
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
                workers,
            }),
            Err(error) => {
                self.state.stop.store(true, Ordering::SeqCst);
                self.state.ready.notify_all();
                for worker in workers {
                    let _ = worker.join();
                }
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
            let _ = sail_api::client::connect_stream(&self.socket, sail_api::client::PROBE_TIMEOUT);
            let _ = thread.join();
        }
        for thread in self.workers.drain(..) {
            let _ = thread.join();
        }
        if let Ok(mut controllers) = self.state.mutations.lock() {
            for (_, (_, handle)) in std::mem::take(&mut *controllers) {
                let _ = handle.join();
            }
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
                        Ok(Request::Foreground {
                            version,
                            project_dir,
                            args,
                        }) => {
                            let _ = preparation::foreground(
                                &state,
                                &mut stream,
                                version,
                                &project_dir,
                                &args,
                            );
                            clients.fetch_sub(1, Ordering::SeqCst);
                            return;
                        }
                        Ok(Request::Result { id }) => get(&state, &id)
                            .and_then(|job| store::result_frame(&state, &job))
                            .map(|frame| (None, Some(frame))),
                        Ok(request) => Ok((Some(dispatch(&state, request)), None)),
                        Err(error) => {
                            Ok((Some(Response::Error(format!("SAIL0802: {error}"))), None))
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
                        Ok((None, None)) => Err(io::Error::other(
                            "SAIL0899: internal error: missing RPC response; please report it",
                        )),
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

#[allow(
    clippy::too_many_lines,
    reason = "Explicit RPC routing keeps each operation's authority and capabilities visible."
)]
fn dispatch(state: &Arc<State>, request: Request) -> Response {
    let result = match request {
        Request::PreparationStatus => state.preparation.status().map(Response::Preparation),
        Request::Prepare(submission) => {
            let result = state
                .preparation
                .enqueue(submission)
                .map(Response::Preparation);
            state.ready.notify_all();
            result
        }
        Request::Foreground { .. } => Err(io::Error::other(
            "SAIL0802: foreground requires a lease connection",
        )),
        Request::Capabilities => Ok(Response::Capabilities {
            proof_version: sail_api::proof::VERSION,
            repair: true,
            standalone: true,
            standalone_repair: true,
            neutral_contracts_version: Some(sail_api::contracts::VERSION),
            neutral_contract_profiles: vec![1, 2, 3],
            assurance_version: Some(sail_api::assurance::VERSION),
            attribution_version: Some(sail_api::attribution::VERSION),
            counterexample_version: Some(sail_api::counterexample::VERSION),
            mutation_version: Some(sail_api::mutation::VERSION),
        }),
        Request::SubmitMutation(submission) => {
            mutation::submit(state, &submission).map(|view| Response::Mutation(Box::new(view)))
        }
        Request::PrepareLiteralWitness {
            source,
            max_mutants,
        } => sail_verus::contracts::literal_witness::prepare(&source, max_mutants)
            .map(|plan| Response::LiteralWitness(Box::new(plan)))
            .map_err(io::Error::other),
        Request::ValidateLiteralWitness {
            plan,
            candidate_id,
            result,
        } => sail_verus::contracts::literal_witness::validate(&plan, &candidate_id, &result)
            .map(|value| Response::LiteralWitnessValidated(Box::new(value)))
            .map_err(io::Error::other),
        Request::Mutation { id } => {
            mutation::read(state, &id).map(|view| Response::Mutation(Box::new(view)))
        }
        Request::CancelMutation { id } => {
            mutation::cancel(state, &id).map(|view| Response::Mutation(Box::new(view)))
        }
        Request::Assurance { id } => get(state, &id)
            .and_then(|job| store::result(state, &job))
            .and_then(|document| {
                sail_api::assurance::from_contract_result(&document).map_err(io::Error::other)
            })
            .and_then(|mut view| {
                mutation::attach(state, &id, &mut view)?;
                Ok(Response::Assurance(Box::new(view)))
            }),
        Request::AssuranceInputs { id } => get(state, &id).and_then(|job| {
            let document = store::result(state, &job)?;
            // Decoded struct fields take their names from the retained source.
            let fields = contracts::source(state, &job)
                .map(|source| sail_verus::contracts::struct_field_names(&source.source))
                .unwrap_or_default();
            sail_api::assurance::decoded_inputs(&document, &fields)
                .map(|view| Response::AssuranceInputs(Box::new(view)))
                .map_err(io::Error::other)
        }),
        Request::AttributedAssurance { id } => get(state, &id).and_then(|job| {
            if job.contracts.is_some() || job.requirements.is_empty() {
                return Err(io::Error::other(
                    "SAIL3160: job was not submitted with requirement attributions",
                ));
            }
            let document = store::result(state, &job)?;
            sail_api::attribution::from_project_result(&document, &job.requirements)
                .map(|view| Response::AttributedAssurance(Box::new(view)))
                .map_err(io::Error::other)
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
        Request::SubmitContracts(submission) => {
            contracts::submit(state, &submission).map(Response::Status)
        }
        Request::Submit { submission } => {
            submit(state, submission, Vec::new()).map(Response::Status)
        }
        Request::SubmitAttributed {
            submission,
            requirements,
        } => sail_api::attribution::canonical(requirements)
            .map_err(io::Error::other)
            .and_then(|requirements| {
                if sail_api::attribution::mode_supported(&submission.mode) {
                    submit(state, submission, requirements)
                } else {
                    Err(io::Error::other(
                        "SAIL3160: requirement attribution applies only to stock and standalone jobs",
                    ))
                }
            })
            .map(Response::Status),
        Request::List { project_dir } => list(state, project_dir.as_deref()).map(Response::List),
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
    io::Error::other("SAIL0899: internal error: job state lock poisoned; please report it")
}
/// Every retained job (or one project's), oldest first. Each completed job
/// carries the same currentness projection as `Status`, computed outside the
/// job lock; a record without a validated projection, or one that fails
/// validation, stays Unknown.
fn list(state: &State, project_dir: Option<&std::path::Path>) -> io::Result<Vec<Status>> {
    let selected: Vec<Stored> = state
        .jobs
        .lock()
        .map_err(lock_error)?
        .values()
        .filter(|job| project_dir.is_none_or(|dir| dir == job.status.project_dir))
        .cloned()
        .collect();
    let mut statuses: Vec<_> = selected
        .iter()
        .map(|job| {
            let mut status = job.status.clone();
            if status.state == Lifecycle::Completed {
                status.currentness = job
                    .projection
                    .as_ref()
                    .and_then(|_| store::status(state, job).ok())
                    .map_or(Currentness::Unknown, |current| current.currentness);
            }
            status
        })
        .collect();
    statuses.sort_by_key(|status| status.submitted_at_ms);
    Ok(statuses)
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
fn submit(
    state: &State,
    mut submission: Submission,
    requirements: Vec<sail_api::attribution::RequirementAttribution>,
) -> io::Result<Status> {
    if state.stop.load(Ordering::SeqCst) {
        return Err(io::Error::other("SAIL0815: job service is stopping"));
    }
    if matches!(
        submission.mode,
        sail_api::jobs::Mode::Standalone | sail_api::jobs::Mode::StandaloneRepair { .. }
    ) != sail_verus::managed::standalone::source(&submission.args).is_some()
    {
        return Err(io::Error::other(
            "SAIL3150: standalone job mode and source argument disagree",
        ));
    }
    submission.project_dir = submission.project_dir.canonicalize()?;
    if !submission.project_dir.is_dir() || submission.args.iter().any(|arg| arg.contains('\0')) {
        return Err(io::Error::other("SAIL0806: invalid project or arguments"));
    }
    if submission.input_revision.is_none() {
        establish_revision(state, &mut submission)?;
    }
    let key = schedule::workspace_key(&submission.project_dir, &submission.args);
    state.preparation.preempt(&key);
    let mut jobs = state.jobs.lock().map_err(lock_error)?;
    if let Some(job) = jobs.values().find(|job| {
        !job.status.state.terminal()
            && job.contracts.is_none()
            && submission.input_revision.is_some()
            && job.submission == submission
            && job.requirements == requirements
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
    let mut job = Stored::new(submission);
    job.requirements = requirements;
    let id = job.status.id.clone();
    store::save(&state.root, &job)?;
    let status = job.status.clone();
    jobs.insert(id.clone(), job);
    state.activity.fetch_add(1, Ordering::SeqCst);
    queue_job(state, id, key)?;
    Ok(status)
}

fn queue_job(state: &State, id: String, key: PathBuf) -> io::Result<()> {
    // Preparation takes work under this same queue owner. Once admitted, an
    // ordinary job prevents another preparation from starting and stops one
    // that started before admission: work for another workspace is killed at
    // once, and an origin preparation of this workspace is preempted and may
    // finish within its grace (ADR 0096 amendment, #595).
    let mut queue = state.queue.lock().map_err(lock_error)?;
    state.preparation.preempt(&key);
    queue.push(id, key);
    drop(queue);
    state.ready.notify_all();
    Ok(())
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
