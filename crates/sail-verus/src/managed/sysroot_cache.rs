//! Daemon-local nonrenewable owners; snapshots hand off to an overlapping caller
//! owner. Neither stored metadata nor caller-submitted digests grant authority.

use sail_api::sysroot::{Scope, Snapshot};
use std::io;

/// Bounded optional daemon-local guarded sysroot owners.
///
/// Lookup never constructs or hashes an owner. Each accepted snapshot still
/// requires an independently acquired caller-local Baseline and exact import
/// validation. Unsupported platforms decline all reuse.
pub struct SysrootIndex {
    #[cfg(target_os = "linux")]
    inner: linux::Index,
}
impl SysrootIndex {
    /// Starts one owned maintenance worker on supported Linux systems.
    ///
    /// # Errors
    /// Reports worker creation or generation-entropy failure. Optional callers
    /// must continue through their original local hashing path.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            #[cfg(target_os = "linux")]
            inner: linux::Index::new()?,
        })
    }

    /// Immediately cancels queued and constructing owners without waiting.
    /// Every bounded construction phase observes this same cancellation source.
    pub fn cancel(&self) {
        #[cfg(target_os = "linux")]
        self.inner.cancel();
    }

    /// Cancels and joins the single owned maintenance worker outside RPCs.
    /// Cancellation is cooperative between bounded operations; kernel reads
    /// are not claimed to be interruptible.
    pub fn shutdown(&self) {
        #[cfg(target_os = "linux")]
        self.inner.shutdown();
    }

    /// Returns a complete guarded snapshot or immediately declines a busy/miss.
    /// This performs fresh guard validation, but never content hashing.
    #[must_use]
    pub fn lookup(&self, scope: &Scope) -> Option<Box<Snapshot>> {
        #[cfg(target_os = "linux")]
        {
            self.inner.lookup(scope)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = scope;
            None
        }
    }

    /// Queues a bounded hint after the caller's original local hash barrier.
    /// It never accepts submitted hashes or waits behind construction.
    #[must_use]
    pub fn prepare(&self, scope: &Scope) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.inner.prepare(scope)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = scope;
            false
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(
    dead_code,
    reason = "Runner activation follows independent handoff qualification"
)]
pub(super) fn scope(
    roots: &std::collections::BTreeSet<std::path::PathBuf>,
    selection: String,
) -> Option<Scope> {
    linux::scope(roots, selection)
}
#[cfg(target_os = "linux")]
#[allow(
    dead_code,
    reason = "Runner activation follows independent handoff qualification"
)]
pub(super) fn actual(scope: &Scope) -> bool {
    linux::actual(scope)
}
#[cfg(target_os = "linux")]
pub(super) fn binding(path: &std::path::Path) -> Option<sail_api::sysroot::Binding> {
    linux::binding(path)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Scope, Snapshot, io};
    use crate::managed::{files::digest, sysroot::Baseline};
    use sail_api::sysroot::{Binding, Row};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    use std::fs::File;
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    const MAX_READY: usize = 2;
    const MAX_QUEUE: usize = 2;
    const MAX_ENTRY: usize = 16 * 1024 * 1024;
    const CONSTRUCTION_RESERVATION: usize = 96 * 1024 * 1024;
    // Includes retained owners, one builder and conservative endpoint/queue
    // reserves. This is an owned-Rust charge, never physical RSS or kernel RAM.
    const MAX_RUST_CHARGE: usize = 160 * 1024 * 1024;
    const ENDPOINT_AND_QUEUE_RESERVATION: usize = 16 * 1024 * 1024;
    const MAX_CONTENT: u64 = 4 * 1024 * 1024 * 1024;
    const BUILD_WORK: Duration = Duration::from_secs(120);

    struct Entry {
        snapshot: Box<Snapshot>,
        owner: Baseline,
        invalid: bool,
    }
    struct State {
        ready: Vec<Entry>,
        pending: VecDeque<Scope>,
        constructing: Option<Scope>,
        generation: u64,
        reserved: usize,
    }
    struct Shared {
        state: Mutex<State>,
        changed: Condvar,
        stop: Arc<AtomicBool>,
        nonce: [u8; 32],
    }
    pub(super) struct Index {
        shared: Arc<Shared>,
        worker: Mutex<Option<JoinHandle<()>>>,
    }

    pub(super) fn binding(path: &Path) -> Option<Binding> {
        let mut canonical = path.canonicalize().ok()?;
        canonical.shrink_to_fit();
        (canonical.capacity() <= 8192).then_some(())?;
        let metadata = std::fs::metadata(&canonical).ok()?;
        let binding = Binding {
            path: path.to_owned(),
            canonical,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        binding.valid().then_some(binding)
    }
    pub(super) fn scope(roots: &BTreeSet<PathBuf>, selection: String) -> Option<Scope> {
        if roots.is_empty() || roots.len() > sail_api::sysroot::MAX_ROOTS {
            return None;
        }
        let scope = Scope {
            roots: roots.iter().cloned().collect(),
            bindings: roots
                .iter()
                .map(|path| binding(path))
                .collect::<Option<_>>()?,
            view_root: binding(Path::new("/"))?,
            selection,
            namespace: std::fs::read_link("/proc/self/ns/mnt")
                .ok()?
                .to_str()?
                .to_owned(),
            mounts: digest(mounts()?.as_bytes()),
        };
        bounded_scope(&scope).then_some(scope)
    }
    pub(super) fn actual(scope: &Scope) -> bool {
        if !bounded_scope(scope) || binding(Path::new("/")).as_ref() != Some(&scope.view_root) {
            return false;
        }
        let Ok(namespace) = std::fs::read_link("/proc/self/ns/mnt") else {
            return false;
        };
        if namespace.to_str() != Some(scope.namespace.as_str()) {
            return false;
        }
        let Some(mounts) = mounts() else {
            return false;
        };
        if digest(mounts.as_bytes()) != scope.mounts {
            return false;
        }
        scope
            .bindings
            .iter()
            .all(|expected| binding(&expected.path).as_ref() == Some(expected))
    }
    pub(super) fn mounts() -> Option<String> {
        let mut text = String::new();
        File::open("/proc/self/mountinfo")
            .ok()?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut text)
            .ok()?;
        (!text.is_empty() && text.len() <= 1024 * 1024).then_some(text)
    }
    fn binding_heap(binding: &Binding) -> Option<usize> {
        let Binding {
            path,
            canonical,
            device: _,
            inode: _,
        } = binding;
        path.capacity().checked_add(canonical.capacity())
    }
    fn scope_heap(scope: &Scope) -> Option<usize> {
        let Scope {
            roots,
            bindings,
            view_root,
            selection,
            namespace,
            mounts,
        } = scope;
        let mut used = std::mem::size_of::<Scope>()
            .checked_add(
                roots
                    .capacity()
                    .checked_mul(std::mem::size_of::<PathBuf>())?,
            )?
            .checked_add(
                bindings
                    .capacity()
                    .checked_mul(std::mem::size_of::<Binding>())?,
            )?
            .checked_add(binding_heap(view_root)?)?
            .checked_add(selection.capacity())?
            .checked_add(namespace.capacity())?
            .checked_add(mounts.capacity())?;
        for root in roots {
            used = used.checked_add(root.capacity())?;
        }
        for binding in bindings {
            used = used.checked_add(binding_heap(binding)?)?;
        }
        Some(used)
    }
    fn bounded_scope(scope: &Scope) -> bool {
        scope.valid() && scope_heap(scope).is_some_and(|bytes| bytes <= 256 * 1024)
    }
    fn snapshot_heap(snapshot: &Snapshot) -> Option<usize> {
        let Snapshot {
            scope,
            generation,
            rows,
        } = snapshot;
        let mut used = std::mem::size_of::<Snapshot>()
            .checked_sub(std::mem::size_of::<Scope>())?
            .checked_add(scope_heap(scope)?)?
            .checked_add(generation.capacity())?
            .checked_add(rows.capacity().checked_mul(std::mem::size_of::<Row>())?)?;
        for row in rows {
            let Row {
                path,
                canonical,
                device: _,
                inode: _,
                sha256,
            } = row;
            used = used
                .checked_add(path.capacity())?
                .checked_add(canonical.capacity())?
                .checked_add(sha256.capacity())?;
        }
        Some(used)
    }
    impl Index {
        pub(super) fn new() -> io::Result<Self> {
            let mut nonce = [0u8; 32];
            File::open("/dev/urandom")?.read_exact(&mut nonce)?;
            let shared = Arc::new(Shared {
                state: Mutex::new(State {
                    ready: Vec::with_capacity(MAX_READY),
                    pending: VecDeque::with_capacity(MAX_QUEUE),
                    constructing: None,
                    generation: 0,
                    reserved: 0,
                }),
                changed: Condvar::new(),
                stop: Arc::new(AtomicBool::new(false)),
                nonce,
            });
            let owner = Arc::clone(&shared);
            let worker = std::thread::Builder::new()
                .name("sail-sysroot-maintenance".into())
                .spawn(move || maintenance(&owner))?;
            Ok(Self {
                shared,
                worker: Mutex::new(Some(worker)),
            })
        }
        pub(super) fn lookup(&self, scope: &Scope) -> Option<Box<Snapshot>> {
            if !bounded_scope(scope) || self.shared.stop.load(Ordering::Acquire) {
                return None;
            }
            let mut state = self.shared.state.try_lock().ok()?;
            let entry = state
                .ready
                .iter_mut()
                .find(|entry| entry.snapshot.scope == *scope)?;
            if entry.invalid || !actual(scope) || !entry.owner.clean() {
                entry.invalid = true;
                return None;
            }
            let snapshot = entry.snapshot.clone();
            if !actual(scope) || !entry.owner.clean() {
                entry.invalid = true;
                return None;
            }
            Some(snapshot)
        }
        pub(super) fn prepare(&self, scope: &Scope) -> bool {
            if !bounded_scope(scope) || self.shared.stop.load(Ordering::Acquire) {
                return false;
            }
            let Ok(mut state) = self.shared.state.try_lock() else {
                return false;
            };
            if state
                .ready
                .iter()
                .any(|entry| !entry.invalid && entry.snapshot.scope == *scope)
                || state.constructing.as_ref() == Some(scope)
                || state.pending.contains(scope)
            {
                return true;
            }
            if state.pending.len() >= MAX_QUEUE {
                return false;
            }
            state.pending.push_back(scope.clone());
            self.shared.changed.notify_one();
            true
        }
    }
    impl Index {
        pub(super) fn cancel(&self) {
            self.shared.stop.store(true, Ordering::Release);
            // Pair notification with the wait mutex: stop alone can notify
            // between the predicate and wait registration.
            let _state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.shared.changed.notify_one();
        }
        pub(super) fn shutdown(&self) {
            self.cancel();
            if let Ok(mut worker) = self.worker.lock()
                && let Some(worker) = worker.take()
            {
                let _ = worker.join();
            }
        }
    }
    impl Drop for Index {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
    fn maintenance(shared: &Shared) {
        loop {
            let Ok(mut state) = shared.state.lock() else {
                return;
            };
            while state.pending.is_empty() && !shared.stop.load(Ordering::Acquire) {
                let Ok(next) = shared.changed.wait(state) else {
                    return;
                };
                state = next;
            }
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            let Some(scope) = state.pending.pop_front() else {
                continue;
            };
            // Retirement is owned by this worker, never a foreground lookup.
            // Release a retired guard before constructing another helper.
            if state
                .ready
                .iter()
                .any(|entry| !entry.invalid && entry.snapshot.scope == scope)
            {
                continue;
            }
            let mut retired = Vec::with_capacity(MAX_READY);
            let mut index = 0;
            while index < state.ready.len() {
                if state.ready[index].invalid {
                    retired.push(state.ready.remove(index));
                } else {
                    index += 1;
                }
            }
            if state.ready.len() == MAX_READY {
                retired.push(state.ready.remove(0));
            }
            let Some(generation) = state.generation.checked_add(1) else {
                return;
            };
            let retained = state.ready.len().checked_mul(MAX_ENTRY);
            if state.reserved != 0
                || retained
                    .and_then(|n| n.checked_add(CONSTRUCTION_RESERVATION))
                    .and_then(|n| n.checked_add(ENDPOINT_AND_QUEUE_RESERVATION))
                    .is_none_or(|n| n > MAX_RUST_CHARGE)
            {
                return;
            }
            // Reserve the entire finite construction ledger before any owner,
            // helper, inventory or hashing collection is created. Per-container
            // limits below enforce this conservative reservation's components.
            state.reserved = CONSTRUCTION_RESERVATION;
            state.generation = generation;
            state.constructing = Some(scope.clone());
            drop(state);
            drop(retired);
            let entry = construct(shared, scope, generation);
            let Ok(mut state) = shared.state.lock() else {
                return;
            };
            state.constructing = None;
            state.reserved = 0;
            if !shared.stop.load(Ordering::Acquire)
                && let Some(entry) = entry
            {
                state.ready.push(entry);
            }
        }
        // Cleanup is outside every lookup and runs only after construction has
        // noticed cancellation. Index::drop owns and joins this worker.
        if let Ok(mut state) = shared.state.lock() {
            state.ready.clear();
            state.pending.clear();
        }
    }
    fn cancelled(shared: &Shared, started: Instant) -> bool {
        shared.stop.load(Ordering::Acquire) || started.elapsed() >= BUILD_WORK
    }
    fn hash(path: &Path, shared: &Shared, started: Instant, read: &mut u64) -> Option<String> {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .ok()?;
        let mut file = File::from(descriptor);
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 65536].into_boxed_slice();
        loop {
            if cancelled(shared, started) {
                return None;
            }
            let count = file.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            *read = read.checked_add(u64::try_from(count).ok()?)?;
            if *read > MAX_CONTENT {
                return None;
            }
            hasher.update(&buffer[..count]);
        }
        Some(format!("{:x}", hasher.finalize()))
    }
    fn construct(shared: &Shared, scope: Scope, generation: u64) -> Option<Entry> {
        let started = Instant::now();
        if !actual(&scope) || cancelled(shared, started) {
            return None;
        }
        let roots: BTreeSet<_> = scope.roots.iter().cloned().collect();
        let mut owner = Baseline::begin_bounded(&roots, Arc::clone(&shared.stop), started)?;
        if !actual(&scope) || cancelled(shared, started) {
            return None;
        }
        let mut rows = Vec::with_capacity(owner.paths().len());
        let mut hashes = BTreeMap::new();
        let mut read = 0;
        let mut path_capacity = 0usize;
        for path in owner.paths().to_vec() {
            if cancelled(shared, started) {
                return None;
            }
            let binding = binding(&path)?;
            path_capacity = path_capacity
                .checked_add(path.capacity())?
                .checked_add(binding.canonical.capacity())?;
            (path_capacity <= 512 * 1024).then_some(())?;
            let sha256 = if let Some(hash) = hashes.get(&binding.canonical) {
                String::clone(hash)
            } else {
                let hash = hash(&path, shared, started, &mut read)?;
                hashes.insert(binding.canonical.clone(), hash.clone());
                hash
            };
            owner.capture(&path, &sha256);
            rows.push(Row {
                path,
                canonical: binding.canonical,
                device: binding.device,
                inode: binding.inode,
                sha256,
            });
        }
        let generation = digest(&[shared.nonce.as_slice(), &generation.to_be_bytes()].concat());
        let snapshot = Box::new(Snapshot {
            scope,
            generation,
            rows,
        });
        if !snapshot.valid(&snapshot.scope)
            || snapshot_heap(&snapshot)?.checked_add(owner.retained_bytes()?)? > MAX_ENTRY
            || cancelled(shared, started)
            || !actual(&snapshot.scope)
            || !owner.complete()
        {
            return None;
        }
        // Enforce the same complete encoded cap before retaining a new owner.
        sail_api::sysroot::write_frame(
            &mut io::sink(),
            &sail_api::sysroot::Response::Lookup(Some(snapshot.clone())),
        )
        .ok()?;
        owner.complete().then_some(Entry {
            snapshot,
            owner,
            invalid: false,
        })
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::mpsc;

        #[test]
        fn poisoned_waiting_state_cancellation_still_joins_owned_worker() {
            let index = Index::new().unwrap_or_else(|_| unreachable!("worker fixture"));
            let shared = Arc::clone(&index.shared);
            let poison = std::thread::spawn(move || {
                let _guard = shared
                    .state
                    .lock()
                    .unwrap_or_else(|_| unreachable!("fixture lock"));
                // An explicit panic is isolated to this poison-injection worker.
                unreachable!("injected lock poison");
            });
            assert!(poison.join().is_err());
            let (done, receive) = mpsc::channel();
            let shutdown = std::thread::spawn(move || {
                index.shutdown();
                assert!(done.send(()).is_ok());
            });
            assert!(receive.recv_timeout(Duration::from_secs(2)).is_ok());
            assert!(shutdown.join().is_ok());
        }

        fn fixture() -> (PathBuf, Scope) {
            static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "sail-sysroot-index-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            assert!(std::fs::create_dir_all(root.join("lib")).is_ok());
            assert!(std::fs::write(root.join("lib/first.rlib"), b"first exact bytes").is_ok());
            assert!(std::fs::write(root.join("lib/second.rlib"), b"second exact bytes").is_ok());
            let scope = scope(&BTreeSet::from([root.clone()]), digest(b"selected runtime"))
                .unwrap_or_else(|| unreachable!("scope fixture"));
            (root, scope)
        }
        fn await_ready(index: &Index, scope: &Scope) -> Box<Snapshot> {
            assert!(index.prepare(scope));
            let started = Instant::now();
            loop {
                if let Some(snapshot) = index.lookup(scope) {
                    return snapshot;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "independent guarded construction failed"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        #[test]
        fn independent_handoff_is_exact_and_survives_daemon_loss_after_transfer() {
            let (root, scope) = fixture();
            let index = Index::new().unwrap_or_else(|_| unreachable!("index fixture"));
            assert!(
                index.lookup(&scope).is_none(),
                "cold lookup never waits for construction"
            );
            let _ready = await_ready(&index, &scope);
            let roots = BTreeSet::from([root.clone()]);
            let ineligible = root.join("lib/first.rlib");
            let mut caller = Baseline::begin(&roots, |path| path == ineligible);
            assert!(caller.clean());
            let snapshot = index
                .lookup(&scope)
                .unwrap_or_else(|| unreachable!("established guarded snapshot"));
            assert_eq!(snapshot.rows.len(), 2);
            for row in &snapshot.rows {
                assert_eq!(
                    row.sha256,
                    crate::managed::files::file_digest(&row.path).unwrap_or_default()
                );
            }
            assert!(caller.import(&scope, &snapshot));
            assert!(
                caller.captured(&ineligible).is_none(),
                "pre-guard aliases never receive remote authority"
            );
            index.shutdown();
            assert!(
                caller.complete(),
                "overlapping caller keeps its own original authority"
            );
            assert!(caller.captured(&root.join("lib/second.rlib")).is_some());
            drop(caller);
            assert!(std::fs::remove_dir_all(root).is_ok());
        }
        #[test]
        fn complete_inventory_scope_and_dirty_caller_refuse_stale_handoff() {
            let (root, scope) = fixture();
            let index = Index::new().unwrap_or_else(|_| unreachable!("index fixture"));
            let _ready = await_ready(&index, &scope);
            let mut caller = Baseline::begin(&BTreeSet::from([root.clone()]), |_| false);
            let mut snapshot = index
                .lookup(&scope)
                .unwrap_or_else(|| unreachable!("ready fixture"));
            let row = snapshot
                .rows
                .pop()
                .unwrap_or_else(|| unreachable!("row fixture"));
            assert!(
                !caller.import(&scope, &snapshot),
                "shape-valid partial inventory cannot grant reuse"
            );
            snapshot.rows.push(row);
            let mut other = scope.clone();
            other.selection = digest(b"different runtime");
            assert!(!caller.import(&other, &snapshot));
            assert!(index.lookup(&other).is_none());
            assert!(std::fs::write(root.join("lib/second.rlib"), b"second exact bytes").is_ok());
            assert!(
                !caller.import(&scope, &snapshot),
                "same final bytes do not revive a broken generation"
            );
            assert!(caller.captured(&root.join("lib/second.rlib")).is_none());
            assert!(index.lookup(&scope).is_none());
            drop(caller);
            index.shutdown();
            assert!(std::fs::remove_dir_all(root).is_ok());
        }

        #[test]
        fn outside_hardlink_mmap_restore_and_namespace_replacement_break_authority() {
            for change in ["hardlink", "mmap", "replacement", "new-path"] {
                let (root, scope) = fixture();
                let file = root.join("lib/second.rlib");
                let outside = root.with_extension("outside-link");
                if change == "hardlink" {
                    assert!(std::fs::hard_link(&file, &outside).is_ok());
                }
                let index = Index::new().unwrap_or_else(|_| unreachable!("index fixture"));
                let _ready = await_ready(&index, &scope);
                let mut caller = Baseline::begin(&BTreeSet::from([root.clone()]), |_| false);
                let snapshot = index
                    .lookup(&scope)
                    .unwrap_or_else(|| unreachable!("ready fixture"));
                match change {
                    "hardlink" => assert!(std::fs::write(&outside, b"second exact bytes").is_ok()),
                    "mmap" => assert!(std::process::Command::new("python3")
                        .args(["-I", "-c", "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); old=m[:]; m[:]=b'x'*len(old); m.flush(); m[:]=old; m.flush(); m.close()"])
                        .arg(&file).status().is_ok_and(|s| s.success())),
                    "replacement" => {
                        let other = file.with_extension("new");
                        assert!(std::fs::write(&other, b"second exact bytes").is_ok());
                        assert!(std::fs::rename(other, &file).is_ok());
                    }
                    "new-path" => assert!(std::fs::write(root.join("lib/new.rlib"), b"new input").is_ok()),
                    _ => unreachable!("bounded fixture variant"),
                }
                assert!(
                    !caller.import(&scope, &snapshot),
                    "{change} cannot revive cached bytes"
                );
                assert!(
                    index.lookup(&scope).is_none(),
                    "{change} invalidates daemon authority"
                );
                drop(caller);
                if change == "hardlink" {
                    assert!(std::fs::remove_file(outside).is_ok());
                }
                let replacement = await_ready(&index, &scope);
                assert_ne!(replacement.generation, snapshot.generation);
                index.shutdown();
                assert!(std::fs::remove_dir_all(root).is_ok());
            }
        }

        #[test]
        fn two_owner_bound_eviction_and_restart_preserve_only_local_overlap() {
            let (root, first) = fixture();
            let index = Index::new().unwrap_or_else(|_| unreachable!("index fixture"));
            let _first = await_ready(&index, &first);
            let mut caller = Baseline::begin(&BTreeSet::from([root.clone()]), |_| false);
            let snapshot = index
                .lookup(&first)
                .unwrap_or_else(|| unreachable!("first fixture"));
            assert!(caller.import(&first, &snapshot));
            let mut second = first.clone();
            second.selection = digest(b"second exact selection");
            let _second = await_ready(&index, &second);
            let mut third = first.clone();
            third.selection = digest(b"third exact selection");
            let _third = await_ready(&index, &third);
            assert!(
                index.lookup(&first).is_none(),
                "oldest owner evicted rather than retained without charge"
            );
            let state = index
                .shared
                .state
                .lock()
                .unwrap_or_else(|_| unreachable!("state fixture"));
            assert_eq!(state.ready.len(), MAX_READY);
            assert_eq!(state.reserved, 0);
            drop(state);
            assert!(caller.complete(), "caller guard outlives daemon eviction");
            index.shutdown();
            let restarted = Index::new().unwrap_or_else(|_| unreachable!("restart fixture"));
            assert!(
                restarted.lookup(&third).is_none(),
                "restart cannot restore metadata-only authority"
            );
            assert!(caller.complete());
            restarted.shutdown();
            drop(caller);
            assert!(std::fs::remove_dir_all(root).is_ok());
        }

        #[test]
        fn in_process_scope_spare_capacity_is_rejected_before_clone_or_queue() {
            let (root, mut scope) = fixture();
            scope.selection.reserve(512 * 1024);
            assert!(
                scope.valid(),
                "wire shape does not bound owned spare capacity"
            );
            assert!(!bounded_scope(&scope));
            let index = Index::new().unwrap_or_else(|_| unreachable!("index fixture"));
            assert!(!index.prepare(&scope));
            assert!(index.lookup(&scope).is_none());
            let state = index
                .shared
                .state
                .lock()
                .unwrap_or_else(|_| unreachable!("state fixture"));
            assert!(state.pending.is_empty());
            assert_eq!(state.reserved, 0);
            drop(state);
            index.shutdown();
            assert!(std::fs::remove_dir_all(root).is_ok());
        }
    }
}
