//! Bounded freshness projections: exact input and runtime digests are retained
//! only within uninterrupted Linux kernel lease and namespace-owner generations.

use super::{InputSnapshot, runner};
#[cfg(target_os = "linux")]
use super::{lease::ReadLease, precise::Observer};
#[cfg(target_os = "linux")]
use crate::Error;
use crate::Result;
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::ops::ControlFlow;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(target_os = "linux")]
const MAX_CONTEXTS: usize = 32;
#[cfg(target_os = "linux")]
const MAX_REVALIDATIONS: usize = 2;

// Opt-in local diagnosis only: never used to admit a projection or establish a
// performance claim. Static labels carry no paths, environment or report data.
#[cfg(target_os = "linux")]
fn trace(stage: &'static str, started: std::time::Instant, reason: &'static str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("SAIL_FRESHNESS_TRACE").is_some_and(|v| v == "1")) {
        eprintln!(
            "sail-freshness thread={:?} elapsed_us={} stage={stage} reason={reason}",
            std::thread::current().id(),
            started.elapsed().as_micros()
        );
    }
}

#[cfg(target_os = "linux")]
struct RuntimeValidation {
    #[cfg(test)]
    identity_guarded: bool,
    identity: String,
    lease: ReadLease,
    inputs: InputSnapshot,
}
#[cfg(target_os = "linux")]
impl RuntimeValidation {
    fn valid(&mut self, request: &Request, started: std::time::Instant) -> Option<()> {
        // Candidate remains unselected in production pending whole-path
        // controls/profile. Exact raw sidecar membership was certified only
        // after the cold independent reads and all successful final barriers.
        #[cfg(test)]
        let guarded = self.identity_guarded && runtime_identity_guard::enabled();
        #[cfg(not(test))]
        let guarded = false;
        if !guarded {
            #[cfg(test)]
            runtime_identity_guard::record_parse();
            let Some((sha, _)) = runner::runtime_identity(&request.cache, &request.context) else {
                trace("runtime-identity", started, "unavailable");
                return None;
            };
            if sha != self.identity {
                trace("runtime-identity", started, "changed");
                return None;
            }
        }
        trace("runtime-identity", started, "ready");
        let clean = self.lease.clean();
        trace(
            "runtime-lease",
            started,
            if clean {
                "ready"
            } else {
                self.lease.failure_reason()
            },
        );
        clean.then_some(())
    }
}

#[cfg(target_os = "linux")]
struct RuntimeSlot {
    runtime: Option<RuntimeValidation>,
    coherent: Option<(std::time::Instant, InputSnapshot)>,
    validating: bool,
    published: Arc<Condvar>,
}
#[cfg(target_os = "linux")]
enum Refresh {
    Ready(InputSnapshot),
    Busy,
    Invalid,
}
#[cfg(target_os = "linux")]
#[derive(Default)]
struct Projection {
    observer: Option<Observer>,
    runtime: Option<Arc<Mutex<RuntimeSlot>>>,
    refreshing: bool,
    generation: u64,
    retry_after: Option<std::time::Instant>,
}
#[cfg(target_os = "linux")]
#[derive(Clone)]
struct Request {
    cache: PathBuf,
    workspace: PathBuf,
    context: String,
    environment: BTreeMap<String, String>,
}
#[cfg(target_os = "linux")]
impl Request {
    fn key(&self) -> String {
        // Paths can contain non-UTF-8 bytes. Hash their exact platform spelling
        // with lengths rather than falling back after JSON serialization errors.
        let mut bytes = b"sail-freshness-request-v1".to_vec();
        let mut append = |value: &[u8]| {
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value);
        };
        append(self.cache.as_os_str().as_encoded_bytes());
        append(self.workspace.as_os_str().as_encoded_bytes());
        append(self.context.as_bytes());
        for (name, value) in &self.environment {
            append(name.as_bytes());
            append(value.as_bytes());
        }
        super::files::digest(&bytes)
    }
    fn refresh(&self, runtime: &Mutex<RuntimeSlot>, started: std::time::Instant) -> Refresh {
        self.refresh_outcome(runtime, started, || {}, || {})
    }
    #[cfg(test)]
    fn refresh_started(
        &self,
        runtime: &Mutex<RuntimeSlot>,
        started: std::time::Instant,
        before_publish: impl FnOnce(),
    ) -> Option<InputSnapshot> {
        match self.refresh_outcome(runtime, started, || {}, before_publish) {
            Refresh::Ready(snapshot) => Some(snapshot),
            Refresh::Busy | Refresh::Invalid => None,
        }
    }
    fn await_validation(
        runtime: &Mutex<RuntimeSlot>,
        started: std::time::Instant,
    ) -> ControlFlow<Refresh, RuntimeValidation> {
        // The overlap boundary starts at caller entry; the existing bounded
        // owner-wait budget starts here, after the first namespace barrier.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        loop {
            let Some(mut slot) = read_projection_until(runtime, deadline) else {
                trace("runtime-lock", started, "busy");
                return ControlFlow::Break(Refresh::Busy);
            };
            // Coalescing is linearizable within this reader's own interval.
            // A sequential reader never inherits a previous read's timestamp.
            if let Some((completed, snapshot)) = &slot.coherent
                && *completed >= started
            {
                trace("runtime-cohort", started, "ready");
                return ControlFlow::Break(Refresh::Ready(snapshot.clone()));
            }
            if let Some(validation) = slot.runtime.take() {
                slot.validating = true;
                return ControlFlow::Continue(validation);
            }
            if !slot.validating {
                trace("runtime-owner", started, "missing");
                return ControlFlow::Break(Refresh::Invalid);
            }
            // Registration and publication use the same mutex. A notification
            // cannot occur between checking the state and atomically waiting.
            let published = Arc::clone(&slot.published);
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                trace("runtime-owner", started, "busy");
                return ControlFlow::Break(Refresh::Busy);
            }
            match published.wait_timeout(slot, remaining) {
                Ok((_, timeout))
                    if !timeout.timed_out() && std::time::Instant::now() < deadline => {}
                _ => {
                    trace("runtime-owner", started, "busy");
                    return ControlFlow::Break(Refresh::Busy);
                }
            }
        }
    }
    fn refresh_outcome(
        &self,
        runtime: &Mutex<RuntimeSlot>,
        started: std::time::Instant,
        before_validation: impl FnOnce(),
        before_publish: impl FnOnce(),
    ) -> Refresh {
        let mut validation = match Self::await_validation(runtime, started) {
            ControlFlow::Continue(validation) => validation,
            ControlFlow::Break(result) => return result,
        };
        // Expensive work owns the lease but holds no shared projection mutex.

        let result = (|| {
            // Cold validation bound this exact inventory, declaration files and
            // environment to this owner. Native namespace coverage spans every
            // package/configuration ancestor; the retained leases cover every
            // content inode. Any uncertainty discards the owner and recollects.
            // The cold fill acquired all leases before hashing these exact
            // input bytes. The final successful check of this nonrenewable
            // owner proves its leases survived throughout snapshot capture.
            // Outer observer barriers still bracket the namespace. Checking
            // these same leases before cloning would duplicate kernel work;
            // no timestamp or elapsed-time cache authorizes digest reuse.
            let snapshot = validation.inputs.clone();
            before_validation();
            // Record the conservative overlap boundary before final validation.
            // Publication can be delayed by mutex contention: its later time
            // must never admit a reader that started after this validation.
            let checkpoint = std::time::Instant::now();
            validation.valid(self, started)?;
            Some((checkpoint, snapshot))
        })();
        before_publish();
        let Some(mut slot) = read_projection(runtime) else {
            return Refresh::Invalid;
        };
        slot.validating = false;
        if let Some((checkpoint, snapshot)) = &result {
            slot.runtime = Some(validation);
            slot.coherent = snapshot.complete.then(|| (*checkpoint, snapshot.clone()));
        } else {
            slot.coherent = None;
        }
        // Notify both successful and invalid publications. Waking confers no
        // authority: each reader rechecks its own entry-time checkpoint.
        slot.published.notify_all();
        trace(
            "runtime-publish",
            started,
            if result.is_some() { "ready" } else { "invalid" },
        );
        result.map_or(Refresh::Invalid, |(_, snapshot)| Refresh::Ready(snapshot))
    }
}

#[cfg(target_os = "linux")]
fn namespace_reason(projection: &Projection) -> &'static str {
    projection
        .observer
        .as_ref()
        .map_or("namespace-owner-missing", Observer::failure_reason)
}

/// A process-local bounded freshness index. Inputs are content-hashed after
/// acquiring native coverage and kernel read leases. Only that uninterrupted
/// owner generation authorizes digest reuse; inotify alone cannot observe
/// existing writable mmap mappings.
/// Restart, unsupported coverage, helper loss and conflicting writers revalidate.
#[derive(Clone, Default)]
pub struct FreshnessIndex {
    #[cfg(target_os = "linux")]
    entries: Arc<Mutex<BTreeMap<String, Arc<Mutex<Projection>>>>>,
    #[cfg(target_os = "linux")]
    active: Arc<AtomicUsize>,
}
impl FreshnessIndex {
    #[cfg(target_os = "linux")]
    fn request(
        cache: &Path,
        workspace: &Path,
        context: &str,
        environment: &BTreeMap<String, String>,
    ) -> Result<Request> {
        if context.is_empty()
            || context.len() > 128
            || !context
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::new("SAIL3114", "invalid freshness context identity"));
        }
        Ok(Request {
            cache: std::path::absolute(cache).map_err(|e| Error::new("SAIL3114", e.to_string()))?,
            workspace: std::path::absolute(workspace)
                .map_err(|e| Error::new("SAIL3114", e.to_string()))?,
            context: context.to_owned(),
            environment: environment.clone(),
        })
    }
    #[cfg(target_os = "linux")]
    fn entry(&self, key: &str) -> Option<Arc<Mutex<Projection>>> {
        let mut entries = self.entries.lock().ok()?;
        if let Some(entry) = entries.get(key) {
            return Some(Arc::clone(entry));
        }
        if entries.len() >= MAX_CONTEXTS {
            let first = entries.keys().next()?.clone();
            entries.remove(&first);
        }
        let entry = Arc::new(Mutex::new(Projection::default()));
        entries.insert(key.to_owned(), Arc::clone(&entry));
        Some(entry)
    }
    /// Certifies bound input and runtime digests through synchronous native
    /// barriers and continuously held read leases. Uncertain coverage returns
    /// `None` and starts
    /// bounded background revalidation. Full runtime hashes never hold service
    /// request locks. Other platforms retain conservative content revalidation.
    #[must_use]
    pub fn inspect(
        &self,
        cache: &Path,
        workspace: &Path,
        context: &str,
        environment: &BTreeMap<String, String>,
    ) -> Option<InputSnapshot> {
        #[cfg(not(target_os = "linux"))]
        {
            runner::refresh_context(cache, workspace, context, environment).ok()
        }
        #[cfg(target_os = "linux")]
        {
            let started = std::time::Instant::now();
            let request = Self::request(cache, workspace, context, environment).ok()?;
            self.inspect_started(&request, started, || {})
        }
    }
    #[cfg(target_os = "linux")]
    fn inspect_started(
        &self,
        request: &Request,
        started: std::time::Instant,
        before_observer: impl FnOnce(),
    ) -> Option<InputSnapshot> {
        before_observer();
        let entry = self.entry(&request.key())?;
        let (generation, runtime) = {
            let mut projection = read_projection(&entry).or_else(|| {
                trace("projection-lock", started, "busy");
                None
            })?;
            let clean = projection.observer.as_mut().is_some_and(Observer::clean);
            trace(
                "namespace-before",
                started,
                if clean {
                    "ready"
                } else {
                    namespace_reason(&projection)
                },
            );
            let runtime = clean.then(|| projection.runtime.clone()).flatten();
            (projection.generation, runtime)
        };
        if let Some(runtime) = runtime {
            let snapshot = match request.refresh(&runtime, started) {
                Refresh::Ready(snapshot) => Some(snapshot),
                // A bounded waiter is unavailable for this read, but must
                // not discard the independent validation owner's leases.
                Refresh::Busy => return None,
                Refresh::Invalid => None,
            };
            if let Some(snapshot) = snapshot {
                let mut projection = read_projection(&entry).or_else(|| {
                    trace("projection-lock", started, "busy");
                    None
                })?;
                if projection.generation == generation
                    && projection
                        .runtime
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &runtime))
                    && projection.observer.as_mut().is_some_and(Observer::clean)
                {
                    trace("namespace-after", started, "ready");
                    return Some(snapshot);
                }
                let reason = if projection.generation != generation {
                    "projection-generation-changed"
                } else if !projection
                    .runtime
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &runtime))
                {
                    "projection-owner-changed"
                } else {
                    namespace_reason(&projection)
                };
                trace("namespace-after", started, reason);
            }
        }
        trace("projection-invalidate", started, "unavailable");
        let mut projection = read_projection(&entry)?;
        if projection.generation != generation {
            return None;
        }
        projection.observer = None;
        projection.runtime = None;
        if projection.refreshing
            || projection
                .retry_after
                .is_some_and(|time| std::time::Instant::now() < time)
        {
            return None;
        }
        let permit = Permit::acquire(&self.active)?;
        projection.refreshing = true;
        projection.generation = projection.generation.wrapping_add(1);
        let generation = projection.generation;
        drop(projection);
        let worker_entry = Arc::clone(&entry);
        let request = request.clone();
        if std::thread::Builder::new()
            .name("sail-freshness-revalidate".to_owned())
            .spawn(move || {
                let _permit = permit;
                let _ = Self::fill(&worker_entry, generation, &request);
            })
            .is_err()
            && let Ok(mut projection) = entry.lock()
        {
            projection.refreshing = false;
        }
        None
    }
    /// Establishes a projection before a worker publishes completion, without
    /// service locks. Returns a freshly content-validated snapshot even when the
    /// platform cannot safely retain its runtime digests for subsequent reads.
    ///
    /// # Errors
    /// Returns `SAIL3114` for invalid identities, capacity or poisoned state, or
    /// the original input-validation error. Incomplete declarations stay unknown.
    pub fn establish(
        &self,
        cache: &Path,
        workspace: &Path,
        context: &str,
        environment: &BTreeMap<String, String>,
    ) -> Result<InputSnapshot> {
        #[cfg(not(target_os = "linux"))]
        {
            runner::refresh_context(cache, workspace, context, environment)
        }
        #[cfg(target_os = "linux")]
        {
            let request = Self::request(cache, workspace, context, environment)?;
            let entry = self
                .entry(&request.key())
                .ok_or_else(|| Error::new("SAIL3114", "freshness state unavailable"))?;
            let (generation, runtime) = {
                let mut state = read_projection(&entry)
                    .ok_or_else(|| Error::new("SAIL3114", "freshness state busy"))?;
                let runtime = if state.observer.as_mut().is_some_and(Observer::clean) {
                    state.runtime.clone()
                } else {
                    None
                };
                (state.generation, runtime)
            };
            if let Some(runtime) = runtime {
                let snapshot = match request.refresh(&runtime, std::time::Instant::now()) {
                    Refresh::Ready(snapshot) => Some(snapshot),
                    Refresh::Busy => {
                        return Err(Error::new("SAIL3114", "freshness validation busy"));
                    }
                    Refresh::Invalid => None,
                };
                if let Some(snapshot) = snapshot {
                    let mut state = read_projection(&entry)
                        .ok_or_else(|| Error::new("SAIL3114", "freshness state busy"))?;
                    if state.generation == generation
                        && state
                            .runtime
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &runtime))
                        && state.observer.as_mut().is_some_and(Observer::clean)
                    {
                        return Ok(snapshot);
                    }
                }
            }
            let _permit = Permit::acquire(&self.active)
                .ok_or_else(|| Error::new("SAIL3114", "freshness validation capacity exhausted"))?;
            let generation = {
                let mut state = entry
                    .lock()
                    .map_err(|_| Error::new("SAIL3114", "freshness state unavailable"))?;
                state.generation = state.generation.wrapping_add(1);
                state.refreshing = true;
                state.observer = None;
                state.runtime = None;
                state.generation
            };
            Self::fill(&entry, generation, &request)
        }
    }
    #[cfg(target_os = "linux")]
    fn fill(
        entry: &Mutex<Projection>,
        generation: u64,
        request: &Request,
    ) -> Result<InputSnapshot> {
        let trace_started = std::time::Instant::now();
        let result: Result<_> = (|| {
            let plan = runner::observation_plan(
                &request.cache,
                &request.workspace,
                &request.context,
                &request.environment,
            )?;
            let mut observer = Observer::begin(&plan.0, &plan.1);
            let runtime = runner::runtime_identity(&request.cache, &request.context);
            let mut leased_paths = std::collections::BTreeSet::new();
            let mut lease = observer.as_ref().and_then(|observer| {
                let mut paths = observer.files();
                paths.extend(runtime.as_ref()?.1.iter().cloned());
                paths.sort();
                paths.dedup();
                leased_paths.extend(paths.iter().filter_map(|path| path.canonicalize().ok()));
                ReadLease::acquire(&paths)
            });
            let mut snapshot = runner::refresh_context_validated(
                &request.cache,
                &request.workspace,
                &request.context,
                &request.environment,
                || lease.is_some() && runner::runtime_current(&request.cache, &request.context),
            )?;
            let after = runner::observation_plan(
                &request.cache,
                &request.workspace,
                &request.context,
                &request.environment,
            )?;
            #[cfg(test)]
            let refresh_complete = snapshot.complete;
            let plan_matches = plan == after;
            snapshot.complete &= plan_matches;
            let cacheable = snapshot.complete
                && snapshot
                    .content_paths
                    .iter()
                    .all(|path| path.canonicalize().is_ok_and(|p| leased_paths.contains(&p)))
                && observer.as_mut().is_some_and(Observer::clean)
                && lease.as_mut().is_some_and(ReadLease::clean)
                && runtime == runner::runtime_identity(&request.cache, &request.context);
            snapshot.complete &= cacheable;
            #[cfg(test)]
            if !cacheable {
                trace_incomplete(
                    refresh_complete,
                    plan_matches,
                    observer.as_ref(),
                    lease.as_ref(),
                );
            }
            trace_fill(trace_started, cacheable, lease.as_ref(), observer.as_ref());
            let runtime = if cacheable {
                runtime.zip(lease).map(|((identity, _), lease)| {
                    Arc::new(Mutex::new(RuntimeSlot {
                        runtime: Some(RuntimeValidation {
                            #[cfg(test)]
                            identity_guarded: runtime_identity_guard::eligible(
                                &request.cache,
                                &request.context,
                                &identity,
                                &lease,
                            ),
                            identity,
                            lease,
                            inputs: snapshot.clone(),
                        }),
                        coherent: None,
                        validating: false,
                        published: Arc::new(Condvar::new()),
                    }))
                })
            } else {
                None
            };
            Ok((snapshot, observer, runtime))
        })();
        let mut state = entry
            .lock()
            .map_err(|_| Error::new("SAIL3114", "freshness state unavailable"))?;
        if state.generation == generation {
            state.refreshing = false;
            state.retry_after =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
        }
        match result {
            Ok((snapshot, observer, runtime)) => {
                if state.generation == generation && runtime.is_some() {
                    state.observer = observer;
                    state.runtime = runtime;
                    state.retry_after = None;
                }
                Ok(snapshot)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "freshness_identity_guard.rs"]
mod runtime_identity_guard;
#[cfg(target_os = "linux")]
fn trace_fill(
    trace_started: std::time::Instant,
    cacheable: bool,
    lease: Option<&ReadLease>,
    observer: Option<&Observer>,
) {
    trace(
        "fill-validation",
        trace_started,
        if cacheable {
            "ready"
        } else {
            lease.map_or("lease-owner-unavailable", ReadLease::failure_reason)
        },
    );
    trace(
        "fill-namespace",
        trace_started,
        if cacheable {
            "ready"
        } else {
            observer.map_or("namespace-owner-unavailable", Observer::failure_reason)
        },
    );
}
#[cfg(all(test, target_os = "linux"))]
fn trace_incomplete(
    refresh_complete: bool,
    plan_matches: bool,
    observer: Option<&Observer>,
    lease: Option<&ReadLease>,
) {
    eprintln!(
        "freshness establishment incomplete: refresh_complete_before_plan={refresh_complete} plan_matches={plan_matches} observer_present={} lease_present={} lease_reason={} observer_reason={}",
        observer.is_some(),
        lease.is_some(),
        lease.map_or("owner-unavailable", ReadLease::failure_reason),
        observer.map_or("owner-unavailable", Observer::failure_reason)
    );
}
#[cfg(target_os = "linux")]
fn read_projection<T>(entry: &Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
    read_projection_until(entry, deadline)
}
#[cfg(target_os = "linux")]
fn read_projection_until<T>(
    entry: &Mutex<T>,
    deadline: std::time::Instant,
) -> Option<std::sync::MutexGuard<'_, T>> {
    loop {
        match entry.try_lock() {
            Ok(projection) => return Some(projection),
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(_) => return None,
        }
    }
}
#[cfg(target_os = "linux")]
struct Permit(Arc<AtomicUsize>);
#[cfg(target_os = "linux")]
impl Permit {
    fn acquire(active: &Arc<AtomicUsize>) -> Option<Self> {
        active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_REVALIDATIONS).then_some(n + 1)
            })
            .ok()?;
        Some(Self(Arc::clone(active)))
    }
}
#[cfg(target_os = "linux")]
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
#[cfg(all(test, target_os = "linux"))]
mod tests {
    mod identity_guard_tests {
        include!("freshness_identity_guard_tests.rs");
    }
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::{Duration, Instant};
    // These controls exercise publication only, with no filesystem, proof or
    // Current evidence. Real lease/source controls remain separate below.
    fn waiting_slot() -> Mutex<RuntimeSlot> {
        Mutex::new(RuntimeSlot {
            runtime: None,
            coherent: None,
            validating: true,
            published: Arc::new(Condvar::new()),
        })
    }
    fn publication_snapshot() -> InputSnapshot {
        InputSnapshot {
            workspace: PathBuf::new(),
            revision: "protocol-control".to_owned(),
            roots: Vec::new(),
            excluded: Vec::new(),
            complete: false,
            observation_paths: std::collections::BTreeSet::default(),
            content_paths: std::collections::BTreeSet::default(),
        }
    }
    #[test]
    fn publication_before_waiter_registration_checks_exact_entry_checkpoint() {
        let runtime = waiting_slot();
        let entered = Instant::now();
        {
            let mut slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            slot.coherent = Some((Instant::now(), publication_snapshot()));
            slot.validating = false;
            slot.published.notify_all();
        }
        assert!(
            matches!(Request::await_validation(&runtime, entered), ControlFlow::Break(Refresh::Ready(snapshot)) if snapshot.revision == "protocol-control")
        );
        assert!(
            matches!(
                Request::await_validation(&runtime, Instant::now()),
                ControlFlow::Break(Refresh::Invalid)
            ),
            "later sequential reader cannot inherit earlier checkpoint"
        );
    }
    #[test]
    fn invalid_publication_wakes_waiter_without_authorizing_snapshot() {
        let runtime = Arc::new(waiting_slot());
        let (sent, received) = std::sync::mpsc::channel();
        let reader_runtime = Arc::clone(&runtime);
        let reader = std::thread::spawn(move || {
            assert!(sent.send(()).is_ok());
            Request::await_validation(&reader_runtime, Instant::now())
        });
        let entered = received.recv_timeout(Duration::from_secs(1));
        // Publication may precede registration; the same-mutex predicate makes
        // both this order and an already-waiting reader observe Invalid.
        {
            let mut slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            slot.validating = false;
            slot.coherent = None;
            slot.published.notify_all();
        }
        let result = reader
            .join()
            .unwrap_or_else(|e| std::panic::resume_unwind(e));
        assert!(entered.is_ok());
        assert!(matches!(result, ControlFlow::Break(Refresh::Invalid)));
    }
    #[test]
    fn spurious_notifications_keep_original_absolute_owner_wait_deadline() {
        let runtime = waiting_slot();
        std::thread::scope(|scope| {
            let notifications = scope.spawn(|| {
                for _ in 0..6 {
                    std::thread::sleep(Duration::from_millis(35));
                    let slot = runtime
                        .lock()
                        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
                    slot.published.notify_all();
                }
            });
            let started = Instant::now();
            let result = Request::await_validation(&runtime, started);
            let elapsed = started.elapsed();
            assert!(notifications.join().is_ok());
            assert!(matches!(result, ControlFlow::Break(Refresh::Busy)));
            assert!(elapsed >= Duration::from_millis(90));
            assert!(
                elapsed < Duration::from_millis(200),
                "spurious wake reset wait deadline: {elapsed:?}"
            );
            let slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            assert!(slot.validating, "Busy must preserve the independent owner");
            assert!(slot.coherent.is_none());
        });
    }
    #[test]
    fn poisoned_publication_state_never_returns_ready() {
        let runtime = waiting_slot();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            slot.coherent = Some((Instant::now(), publication_snapshot()));
            std::panic::resume_unwind(Box::new("intentional publication poison"));
        }));
        assert!(poisoned.is_err());
        assert!(matches!(
            Request::await_validation(&runtime, Instant::now()),
            ControlFlow::Break(Refresh::Busy)
        ));
    }
    struct Fixture {
        root: PathBuf,
        cache: PathBuf,
        workspace: PathBuf,
        context: String,
        environment: BTreeMap<String, String>,
        runtime: PathBuf,
    }
    impl Fixture {
        fn new(name: &str) -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "sail-freshness-{name}-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let workspace = root.join("project");
            let cache = root.join("cache");
            let context = "a".repeat(64);
            assert!(std::fs::create_dir_all(workspace.join("src")).is_ok());
            assert!(
                std::fs::write(
                    workspace.join("Cargo.toml"),
                    "[package]\nname='fixture'\nversion='0.0.0'\n"
                )
                .is_ok()
            );
            assert!(std::fs::write(workspace.join("src/lib.rs"), "original").is_ok());
            let directory = cache
                .join("projects")
                .join(super::super::files::digest(
                    workspace.as_os_str().as_encoded_bytes(),
                ))
                .join(&context);
            assert!(std::fs::create_dir_all(&directory).is_ok());
            let data = serde_json::json!({"workspace_root":workspace,"sail_invocation_directory":workspace,"sail_cache_directory":cache,"target_directory":workspace.join("target"),"packages":[{"manifest_path":workspace.join("Cargo.toml"),"source":null,"targets":[]}]});
            assert!(super::super::files::json(&directory.join("input-state.json"), &data).is_ok());
            assert!(
                super::super::files::json(
                    &directory.join("source-inputs.json"),
                    &serde_json::json!({"paths":[],"covered":true,"by_package":{}})
                )
                .is_ok()
            );
            let runtime = root.join("runtime.bin");
            assert!(std::fs::write(&runtime, vec![0; 8 * 1024 * 1024]).is_ok());
            let identity = vec![(
                runtime.to_string_lossy().into_owned(),
                super::super::files::file_digest(&runtime).unwrap_or_default(),
            )];
            let identity_sha =
                super::super::files::digest(&serde_json::to_vec(&identity).unwrap_or_default());
            assert!(std::fs::create_dir_all(cache.join("runtime-identities")).is_ok());
            assert!(
                super::super::files::json(
                    &cache
                        .join("runtime-identities")
                        .join(format!("{identity_sha}.json")),
                    &identity
                )
                .is_ok()
            );
            assert!(
                super::super::files::json(
                    &cache
                        .join("runtime-identities")
                        .join(format!("{context}.link")),
                    &identity_sha
                )
                .is_ok()
            );
            let environment =
                BTreeMap::from([("HOME".to_owned(), root.to_string_lossy().into_owned())]);
            Self {
                root,
                cache,
                workspace,
                context,
                environment,
                runtime,
            }
        }
        fn inspect(&self, index: &FreshnessIndex) -> Option<InputSnapshot> {
            index.inspect(
                &self.cache,
                &self.workspace,
                &self.context,
                &self.environment,
            )
        }
        fn establish(&self, index: &FreshnessIndex) -> InputSnapshot {
            index
                .establish(
                    &self.cache,
                    &self.workspace,
                    &self.context,
                    &self.environment,
                )
                .unwrap_or_else(|e| unreachable!("fixture:{e}"))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn non_utf8_path_spellings_and_environment_keep_requests_isolated() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt as _;
        let mut request = Request {
            cache: PathBuf::from(OsString::from_vec(b"/cache-\xff".to_vec())),
            workspace: PathBuf::from("/workspace"),
            context: "context".to_owned(),
            environment: BTreeMap::new(),
        };
        let initial = request.key();
        request.cache = PathBuf::from(OsString::from_vec(b"/cache-\xfe".to_vec()));
        assert_ne!(initial, request.key());
        let previous = request.key();
        request.workspace = PathBuf::from(OsString::from_vec(b"/workspace-\xff".to_vec()));
        assert_ne!(previous, request.key());
        let previous = request.key();
        request
            .environment
            .insert("PATH".to_owned(), "/other".to_owned());
        assert_ne!(previous, request.key());
    }
    #[test]
    fn established_reads_invalidate_true_content_changes_with_restored_mtime() {
        let fixture = Fixture::new("source");
        let index = FreshnessIndex::default();
        let original = fixture.establish(&index);
        assert!(original.complete);
        assert_eq!(
            fixture.inspect(&index).map(|s| s.revision),
            Some(original.revision.clone())
        );
        let source = fixture.workspace.join("src/lib.rs");
        let file = std::fs::File::open(&source).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(std::fs::write(&source, "different").is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        assert!(fixture.inspect(&index).is_none());
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if let Some(snapshot) = fixture.inspect(&index) {
                assert!(snapshot.complete);
                assert_ne!(snapshot.revision, original.revision);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "coalesced revalidation must finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn guarded_inventory_invalidates_namespace_and_alias_changes() {
        use std::os::unix::{ffi::OsStringExt as _, fs::symlink};
        for case in [
            "replace", "rename", "new", "remove", "config", "non-utf8", "alias",
        ] {
            let fixture = Fixture::new(case);
            let source = fixture.workspace.join("src/lib.rs");
            let outside = fixture.root.join("outside.rs");
            if case == "alias" {
                assert!(std::fs::write(&outside, b"same").is_ok());
                assert!(symlink(&outside, fixture.workspace.join("alias.rs")).is_ok());
            }
            let index = FreshnessIndex::default();
            let original = fixture.establish(&index);
            assert!(original.complete, "case:{case}");
            assert_eq!(
                fixture.inspect(&index).map(|s| s.revision),
                Some(original.revision.clone())
            );
            match case {
                "replace" => {
                    let new = fixture.workspace.join("replacement");
                    assert!(std::fs::write(&new, b"original").is_ok());
                    assert!(std::fs::rename(new, &source).is_ok());
                }
                "rename" => assert!(
                    std::fs::rename(&source, fixture.workspace.join("src/other.rs")).is_ok()
                ),
                "new" => {
                    assert!(std::fs::write(fixture.workspace.join("src/new.rs"), b"new").is_ok());
                }
                "remove" => assert!(std::fs::remove_file(&source).is_ok()),
                "config" => {
                    assert!(std::fs::create_dir_all(fixture.root.join(".cargo")).is_ok());
                    assert!(
                        std::fs::write(
                            fixture.root.join(".cargo/config.toml"),
                            b"[build]\njobs=1\n"
                        )
                        .is_ok()
                    );
                }
                "non-utf8" => assert!(
                    std::fs::write(
                        fixture
                            .workspace
                            .join(std::ffi::OsString::from_vec(b"source-\xff.rs".to_vec())),
                        b"new"
                    )
                    .is_ok()
                ),
                "alias" => {
                    let replacement = fixture.root.join("other.rs");
                    assert!(std::fs::write(&replacement, b"same").is_ok());
                    assert!(std::fs::remove_file(fixture.workspace.join("alias.rs")).is_ok());
                    assert!(symlink(replacement, fixture.workspace.join("alias.rs")).is_ok());
                }
                _ => unreachable!("fixture case"),
            }
            assert!(
                fixture.inspect(&index).is_none(),
                "old inventory must not certify {case}"
            );
            let recollected = fixture.establish(&index);
            assert!(recollected.complete, "case:{case}");
            if case != "replace" {
                assert_ne!(original.revision, recollected.revision, "case:{case}");
            }
            assert_eq!(
                fixture.inspect(&index).map(|s| s.revision),
                Some(recollected.revision)
            );
        }
    }
    #[test]
    fn retained_inventory_cannot_outlive_lost_declared_input_coverage() {
        let fixture = Fixture::new("coverage-record");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        assert!(fixture.inspect(&index).is_some());
        let source_state = fixture
            .cache
            .join("projects")
            .join(super::super::files::digest(
                fixture.workspace.as_os_str().as_encoded_bytes(),
            ))
            .join(&fixture.context)
            .join("source-inputs.json");
        assert!(
            super::super::files::json(
                &source_state,
                &serde_json::json!({"paths":[],"covered":false,"by_package":{}})
            )
            .is_ok()
        );
        assert!(fixture.inspect(&index).is_none());
        assert!(!fixture.establish(&index).complete);
        assert!(fixture.inspect(&index).is_none());
    }
    #[test]
    fn changed_runtime_cannot_be_current_and_restart_revalidates() {
        let fixture = Fixture::new("runtime");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let file =
            std::fs::File::open(&fixture.runtime).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(std::fs::write(&fixture.runtime, vec![1; 8 * 1024 * 1024]).is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        assert!(fixture.inspect(&index).is_none());
        let restarted = FreshnessIndex::default();
        assert!(!fixture.establish(&restarted).complete);
        assert!(fixture.inspect(&restarted).is_none());
    }
    #[test]
    fn existing_source_mapping_is_unknown_until_mapping_is_gone() {
        use std::io::{Read as _, Write as _};
        use std::process::{Command, Stdio};
        let fixture = Fixture::new("mapped-source");
        let source = fixture.workspace.join("src/lib.rs");
        let mut child=Command::new("python3").args(["-I","-u","-c",
            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); print('ready',flush=True); sys.stdin.readline(); m[:]=b'modified'; m.flush(); m.close()"])
            .arg(&source).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap_or_else(|e|unreachable!("fixture:{e}"));
        let mut ready = [0; 6];
        assert!(
            child
                .stdout
                .as_mut()
                .is_some_and(|o| o.read_exact(&mut ready).is_ok())
        );
        assert_eq!(&ready, b"ready\n");
        let index = FreshnessIndex::default();
        assert!(!fixture.establish(&index).complete);
        assert!(fixture.inspect(&index).is_none());
        assert!(
            child
                .stdin
                .as_mut()
                .is_some_and(|i| i.write_all(b"write\n").is_ok())
        );
        assert!(child.wait().is_ok_and(|s| s.success()));
        assert!(fixture.establish(&FreshnessIndex::default()).complete);
    }
    #[test]
    fn transient_source_mmap_restore_invalidates_retained_byte_digests() {
        let fixture = Fixture::new("transient-source-mmap");
        let index = FreshnessIndex::default();
        let original = fixture.establish(&index);
        assert!(original.complete);
        assert_eq!(
            fixture.inspect(&index).map(|s| s.revision),
            Some(original.revision.clone())
        );
        let source = fixture.workspace.join("src/lib.rs");
        let status = std::process::Command::new("python3")
            .args(["-I", "-c", "import mmap,os,sys; p=sys.argv[1]; st=os.stat(p); f=os.open(p,os.O_RDWR); m=mmap.mmap(f,0); os.close(f); original=m[:]; m[:]=b'modified'; m.flush(); m[:]=original; m.flush(); m.close(); os.utime(p,ns=(st.st_atime_ns,st.st_mtime_ns))"])
            .arg(&source).status().unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(status.success());
        assert_eq!(std::fs::read(&source).unwrap_or_default(), b"original");
        assert!(
            fixture.inspect(&index).is_none(),
            "restored bytes cannot revive the broken owner"
        );
        let fresh = fixture.establish(&index);
        assert!(fresh.complete);
        assert_eq!(
            fresh.revision, original.revision,
            "new independent owner hashes restored bytes"
        );
    }
    #[test]
    fn outside_hardlink_write_invalidates_retained_source_digest() {
        let fixture = Fixture::new("outside-hardlink");
        let source = fixture.workspace.join("src/lib.rs");
        let outside = fixture.root.with_extension("outside.rs");
        assert!(std::fs::hard_link(&source, &outside).is_ok());
        let index = FreshnessIndex::default();
        let original = fixture.establish(&index);
        assert!(original.complete);
        let file = std::fs::File::open(&source).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(std::fs::write(&outside, b"modified").is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        assert!(fixture.inspect(&index).is_none());
        let fresh = fixture.establish(&index);
        assert!(fresh.complete);
        assert_ne!(fresh.revision, original.revision);
        assert!(std::fs::remove_file(outside).is_ok());
    }
    #[test]
    fn helper_loss_never_revives_an_established_source_digest() {
        let fixture = Fixture::new("helper-loss");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        assert!(fixture.inspect(&index).is_some());
        let request = FreshnessIndex::request(
            &fixture.cache,
            &fixture.workspace,
            &fixture.context,
            &fixture.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let entry = index
            .entry(&request.key())
            .unwrap_or_else(|| unreachable!("fixture"));
        let runtime = entry
            .lock()
            .ok()
            .and_then(|s| s.runtime.clone())
            .unwrap_or_else(|| unreachable!("fixture"));
        {
            let mut slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            assert!(
                slot.runtime
                    .as_mut()
                    .is_some_and(|r| r.lease.stop_owner_for_test().is_ok())
            );
        }
        assert!(fixture.inspect(&index).is_none());
        let slot = runtime
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(slot.runtime.is_none());
        assert!(slot.coherent.is_none());
    }
    #[test]
    fn restored_write_while_readers_wait_never_reuses_previous_coherent_snapshot() {
        let fixture = Arc::new(Fixture::new("cohort-write"));
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        assert!(fixture.inspect(&index).is_some());
        let request = FreshnessIndex::request(
            &fixture.cache,
            &fixture.workspace,
            &fixture.context,
            &fixture.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let entry = index
            .entry(&request.key())
            .unwrap_or_else(|| unreachable!("fixture"));
        let runtime = entry
            .lock()
            .ok()
            .and_then(|s| s.runtime.clone())
            .unwrap_or_else(|| unreachable!("fixture"));
        let held = runtime
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let start = Arc::new(Barrier::new(5));
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let fixture = Arc::clone(&fixture);
                let index = index.clone();
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    fixture.inspect(&index)
                })
            })
            .collect();
        start.wait();
        std::thread::sleep(Duration::from_millis(10));
        let source = fixture.workspace.join("src/lib.rs");
        let file = std::fs::File::open(&source).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(std::fs::write(&source, "modified").is_ok());
        assert!(std::fs::write(&source, "original").is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        drop(held);
        for reader in readers {
            assert!(
                reader
                    .join()
                    .unwrap_or_else(|e| std::panic::resume_unwind(e))
                    .is_none()
            );
        }
    }
    #[test]
    fn busy_reader_preserves_independent_validation_owner() {
        let fixture = Fixture::new("busy-owner");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let request = FreshnessIndex::request(
            &fixture.cache,
            &fixture.workspace,
            &fixture.context,
            &fixture.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let entry = index
            .entry(&request.key())
            .unwrap_or_else(|| unreachable!("fixture"));
        let (generation, runtime) = {
            let state = entry.lock().unwrap_or_else(|e| unreachable!("fixture:{e}"));
            (
                state.generation,
                state
                    .runtime
                    .clone()
                    .unwrap_or_else(|| unreachable!("fixture")),
            )
        };
        let (validated_tx, validated_rx) = std::sync::mpsc::channel();
        let (publish_tx, publish_rx) = std::sync::mpsc::channel();
        let owner_runtime = Arc::clone(&runtime);
        let owner = std::thread::spawn(move || {
            request.refresh_started(&owner_runtime, Instant::now(), || {
                assert!(validated_tx.send(()).is_ok());
                assert!(publish_rx.recv_timeout(Duration::from_secs(5)).is_ok());
            })
        });
        assert!(validated_rx.recv_timeout(Duration::from_secs(5)).is_ok());
        assert!(
            fixture.inspect(&index).is_none(),
            "busy readers remain unavailable"
        );
        {
            let state = entry.lock().unwrap_or_else(|e| unreachable!("fixture:{e}"));
            assert_eq!(state.generation, generation, "busy is not invalidation");
            assert!(
                state
                    .runtime
                    .as_ref()
                    .is_some_and(|value| Arc::ptr_eq(value, &runtime))
            );
            assert!(!state.refreshing, "no redundant full runtime hash");
        }
        assert!(publish_tx.send(()).is_ok());
        assert!(
            owner
                .join()
                .unwrap_or_else(|e| std::panic::resume_unwind(e))
                .is_some()
        );
        assert!(
            fixture
                .inspect(&index)
                .is_some_and(|snapshot| snapshot.complete)
        );
    }
    #[test]
    fn final_validation_rejects_changes_after_snapshot_capture() {
        for changed in ["source", "runtime-alias", "runtime-identity"] {
            let fixture = Fixture::new(changed);
            let index = FreshnessIndex::default();
            assert!(fixture.establish(&index).complete);
            let request = FreshnessIndex::request(
                &fixture.cache,
                &fixture.workspace,
                &fixture.context,
                &fixture.environment,
            )
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            let runtime = index
                .entry(&request.key())
                .and_then(|entry| entry.lock().ok().and_then(|s| s.runtime.clone()))
                .unwrap_or_else(|| unreachable!("fixture"));
            let result = request.refresh_outcome(
                &runtime,
                Instant::now(),
                || match changed {
                    "source" => {
                        let source = fixture.workspace.join("src/lib.rs");
                        let modified = std::fs::metadata(&source)
                            .and_then(|m| m.modified())
                            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
                        assert!(std::fs::write(&source, "modified").is_ok());
                        let file = std::fs::File::open(source)
                            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
                        assert!(
                            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                                .is_ok()
                        );
                    }
                    "runtime-alias" => {
                        let moved = fixture.root.join("moved-runtime.bin");
                        assert!(std::fs::rename(&fixture.runtime, &moved).is_ok());
                        assert!(std::fs::copy(&moved, &fixture.runtime).is_ok());
                    }
                    _ => {
                        let link = fixture
                            .cache
                            .join("runtime-identities")
                            .join(format!("{}.link", fixture.context));
                        assert!(super::super::files::json(&link, &"e".repeat(64)).is_ok());
                    }
                },
                || {},
            );
            assert!(matches!(result, Refresh::Invalid), "{changed}");
            let slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            assert!(slot.runtime.is_none(), "invalidated lease is not revived");
            assert!(slot.coherent.is_none(), "no stale coalesced snapshot");
        }
    }
    #[test]
    fn late_publication_cannot_coalesce_a_reader_after_validation_and_mmap_break() {
        let fixture = Fixture::new("late-publication");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let request = FreshnessIndex::request(
            &fixture.cache,
            &fixture.workspace,
            &fixture.context,
            &fixture.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let runtime = index
            .entry(&request.key())
            .and_then(|entry| entry.lock().ok().and_then(|s| s.runtime.clone()))
            .unwrap_or_else(|| unreachable!("fixture"));
        let (validated_tx, validated_rx) = std::sync::mpsc::channel();
        let (publish_tx, publish_rx) = std::sync::mpsc::channel();
        let owner_runtime = Arc::clone(&runtime);
        let owner_request = request.clone();
        let owner = std::thread::spawn(move || {
            owner_request.refresh_started(&owner_runtime, Instant::now(), || {
                assert!(validated_tx.send(()).is_ok());
                assert!(publish_rx.recv_timeout(Duration::from_secs(5)).is_ok());
            })
        });
        assert!(validated_rx.recv_timeout(Duration::from_secs(5)).is_ok());
        // Final validation has completed, but publication is delayed. A new
        // writable mapping breaks the kernel lease before this reader enters.
        // inotify alone cannot observe the resulting mmap content mutation.
        assert!(
            std::process::Command::new("python3")
                .args([
                    "-I",
                    "-c",
                    "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); m[0]=1; m.flush(); m.close()",
                ])
                .arg(&fixture.runtime)
                .status()
                .is_ok_and(|s| s.success())
        );
        let late_started = Instant::now();
        assert!(publish_tx.send(()).is_ok());
        assert!(
            owner
                .join()
                .unwrap_or_else(|e| std::panic::resume_unwind(e))
                .is_some(),
            "the owner was valid before the subsequent write"
        );
        assert!(
            request
                .refresh_started(&runtime, late_started, || {})
                .is_none(),
            "a delayed publication must not admit a reader after validation"
        );
    }
    #[test]
    fn delayed_namespace_barrier_coalesces_only_valid_in_call_checkpoints() {
        for case in ["overlap", "sequential", "write-before", "write-after"] {
            let fixture = Arc::new(Fixture::new(case));
            let index = FreshnessIndex::default();
            assert!(fixture.establish(&index).complete);
            let request = FreshnessIndex::request(
                &fixture.cache,
                &fixture.workspace,
                &fixture.context,
                &fixture.environment,
            )
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            let runtime = index
                .entry(&request.key())
                .and_then(|entry| entry.lock().ok().and_then(|s| s.runtime.clone()))
                .unwrap_or_else(|| unreachable!("fixture"));
            if case == "sequential" {
                assert!(
                    request
                        .refresh_started(&runtime, Instant::now(), || {})
                        .is_some()
                );
            }
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (continue_tx, continue_rx) = std::sync::mpsc::channel();
            let reader_index = index.clone();
            let reader_request = request.clone();
            let reader = std::thread::spawn(move || {
                let entered = Instant::now();
                reader_index.inspect_started(&reader_request, entered, || {
                    assert!(entered_tx.send(()).is_ok());
                    assert!(continue_rx.recv_timeout(Duration::from_secs(5)).is_ok());
                })
            });
            assert!(entered_rx.recv_timeout(Duration::from_secs(5)).is_ok());
            let mutate = || {
                let source = fixture.workspace.join("src/lib.rs");
                let file =
                    std::fs::File::open(&source).unwrap_or_else(|e| unreachable!("fixture:{e}"));
                let modified = file
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or_else(|e| unreachable!("fixture:{e}"));
                assert!(std::fs::write(&source, b"changed").is_ok());
                assert!(
                    file.set_times(std::fs::FileTimes::new().set_modified(modified))
                        .is_ok()
                );
            };
            if case == "write-before" {
                mutate();
            }
            if case != "sequential" {
                let checkpoint = request.refresh_started(&runtime, Instant::now(), || {});
                assert_eq!(checkpoint.is_some(), case != "write-before");
            }
            if case == "write-after" {
                mutate();
            }
            // A different overlapping validator may hold the same continuous
            // lease owner. It has not published a new checkpoint; the queued
            // reader must use only one certified within its own call interval.
            let held = {
                let mut slot = runtime
                    .lock()
                    .unwrap_or_else(|e| unreachable!("fixture:{e}"));
                slot.validating = true;
                slot.runtime.take()
            };
            assert!(continue_tx.send(()).is_ok());
            let result = reader
                .join()
                .unwrap_or_else(|e| std::panic::resume_unwind(e));
            assert_eq!(
                result.is_some_and(|s| s.complete),
                case == "overlap",
                "{case}"
            );
            let mut slot = runtime
                .lock()
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
            slot.runtime = held;
            slot.validating = false;
        }
    }
    #[test]
    fn four_readers_do_not_wait_behind_unrelated_validation_or_context_locks() {
        let fixture = Arc::new(Fixture::new("readers"));
        let other = Fixture::new("isolated");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let request = FreshnessIndex::request(
            &other.cache,
            &other.workspace,
            &other.context,
            &other.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let busy = index
            .entry(&request.key())
            .unwrap_or_else(|| unreachable!("fixture"));
        let _held = busy.lock().unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let start = Arc::new(Barrier::new(5));
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let index = index.clone();
                let fixture = Arc::clone(&fixture);
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    let mut latencies = Vec::new();
                    for _ in 0..25 {
                        let time = Instant::now();
                        let read = fixture.inspect(&index);
                        assert!(
                            read.is_some_and(|s| s.complete),
                            "read at {:?}",
                            time.elapsed()
                        );
                        latencies.push(time.elapsed());
                    }
                    latencies
                })
            })
            .collect();
        start.wait();
        let mut latency: Vec<_> = readers
            .into_iter()
            .flat_map(|r| r.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect();
        latency.sort();
        eprintln!("100 complete freshness reads; p95={:?}", latency[94]);
        assert!(
            latency[94] < Duration::from_millis(200),
            "established read barrier p95 exceeds200ms: {:?}",
            latency[94]
        );
        assert!(other.inspect(&index).is_none());
    }
    #[test]
    fn dropped_observer_and_uncertain_source_declarations_never_reuse() {
        let fixture = Fixture::new("drop");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let request = FreshnessIndex::request(
            &fixture.cache,
            &fixture.workspace,
            &fixture.context,
            &fixture.environment,
        )
        .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let entry = index
            .entry(&request.key())
            .unwrap_or_else(|| unreachable!("fixture"));
        if let Ok(mut state) = entry.lock() {
            state.observer = None;
            state.refreshing = true;
        }
        assert!(fixture.inspect(&index).is_none());
        let directory = fixture
            .cache
            .join("projects")
            .join(super::super::files::digest(
                fixture.workspace.as_os_str().as_encoded_bytes(),
            ))
            .join(&fixture.context);
        assert!(
            super::super::files::json(
                &directory.join("source-inputs.json"),
                &serde_json::json!({"paths":[],"covered":false,"by_package":{}})
            )
            .is_ok()
        );
        assert!(!fixture.establish(&FreshnessIndex::default()).complete);
    }
}
