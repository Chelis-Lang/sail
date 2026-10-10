//! Bounded freshness projections: mutable contents are rehashed on every read;
//! expensive runtime digests are reused only under Linux kernel read leases.

use super::{InputSnapshot, runner};
#[cfg(target_os = "linux")]
use super::{lease::ReadLease, precise::Observer};
#[cfg(target_os = "linux")]
use crate::Error;
use crate::Result;
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(target_os = "linux")]
const MAX_CONTEXTS: usize = 32;
#[cfg(target_os = "linux")]
const MAX_REVALIDATIONS: usize = 2;

#[cfg(target_os = "linux")]
struct RuntimeValidation {
    identity: String,
    lease: ReadLease,
}
#[cfg(target_os = "linux")]
struct RuntimeSlot {
    runtime: Option<RuntimeValidation>,
    coherent: Option<(std::time::Instant, InputSnapshot)>,
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
    fn refresh(&self, runtime: &Mutex<RuntimeSlot>) -> Option<InputSnapshot> {
        let started = std::time::Instant::now();
        let deadline = started + std::time::Duration::from_millis(100);
        let mut validation = loop {
            let mut slot = read_projection(runtime)?;
            // Coalescing is linearizable within this reader's own interval.
            // A sequential reader never inherits a previous read's timestamp.
            if let Some((completed, snapshot)) = &slot.coherent
                && *completed >= started
            {
                return Some(snapshot.clone());
            }
            if let Some(validation) = slot.runtime.take() {
                break validation;
            }
            drop(slot);
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        // Expensive work owns the lease but holds no shared projection mutex.
        let valid = |validation: &mut RuntimeValidation| {
            let (sha, _) = runner::runtime_identity(&self.cache, &self.context)?;
            (sha == validation.identity && validation.lease.clean()).then_some(())
        };
        let result = (|| {
            valid(&mut validation)?;
            let snapshot = runner::refresh_context_validated(
                &self.cache,
                &self.workspace,
                &self.context,
                &self.environment,
                || true,
            )
            .ok()?;
            valid(&mut validation)?;
            Some(snapshot)
        })();
        let mut slot = read_projection(runtime)?;
        if let Some(snapshot) = &result {
            slot.runtime = Some(validation);
            slot.coherent = snapshot
                .complete
                .then(|| (std::time::Instant::now(), snapshot.clone()));
        } else {
            slot.coherent = None;
        }
        result
    }
}

/// A process-local bounded freshness index. Mutable inputs are content-hashed
/// for every read. Only continuously held kernel read leases authorize runtime
/// digest reuse; inotify alone cannot observe existing writable mmap mappings.
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
    /// Reads mutable contents after a synchronous native event barrier, reusing
    /// only leased runtime digests. Uncertain coverage returns `None` and starts
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
            let request = Self::request(cache, workspace, context, environment).ok()?;
            let entry = self.entry(&request.key())?;
            let (generation, runtime) = {
                let mut projection = read_projection(&entry)?;
                let runtime = if projection.observer.as_mut().is_some_and(Observer::clean) {
                    projection.runtime.clone()
                } else {
                    None
                };
                (projection.generation, runtime)
            };
            if let Some(runtime) = runtime
                && let Some(snapshot) = request.refresh(&runtime)
            {
                let mut projection = read_projection(&entry)?;
                if projection.generation == generation
                    && projection
                        .runtime
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &runtime))
                    && projection.observer.as_mut().is_some_and(Observer::clean)
                {
                    return Some(snapshot);
                }
            }
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
            if let Some(runtime) = runtime
                && let Some(snapshot) = request.refresh(&runtime)
            {
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
        let result: Result<_> = (|| {
            let plan = runner::observation_plan(
                &request.cache,
                &request.workspace,
                &request.context,
                &request.environment,
            )?;
            let mut observer = Observer::begin(&plan.0, &plan.1);
            let runtime = runner::runtime_identity(&request.cache, &request.context);
            let mut lease = observer.as_ref().and_then(|observer| {
                let mut paths = observer.files();
                paths.extend(runtime.as_ref()?.1.iter().cloned());
                paths.sort();
                paths.dedup();
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
            snapshot.complete &= plan == after;
            let cacheable = snapshot.complete
                && observer.as_mut().is_some_and(Observer::clean)
                && lease.as_mut().is_some_and(ReadLease::clean)
                && runtime == runner::runtime_identity(&request.cache, &request.context);
            snapshot.complete &= cacheable;
            let runtime = if cacheable {
                runtime.zip(lease).map(|((identity, _), lease)| {
                    Arc::new(Mutex::new(RuntimeSlot {
                        runtime: Some(RuntimeValidation { identity, lease }),
                        coherent: None,
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
#[cfg(target_os = "linux")]
fn read_projection<T>(entry: &Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
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
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::{Duration, Instant};
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
