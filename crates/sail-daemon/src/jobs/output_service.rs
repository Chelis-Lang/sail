//! Optional output-only owners. Construction is single-threaded and never runs
//! in an RPC; uninterrupted output guards cannot authorize input Currentness.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use sail_verus::managed::{GUARDED_OUTPUT_RETAINED_BYTES, GuardedOutput};

use super::result_cache::{Budget, Charge, Frames};

const STACK_BYTES: usize = 1024 * 1024;
const CONTROL_BYTES: usize = 65_536;
const ARC_HEADER_BYTES: usize = 2 * std::mem::size_of::<usize>();
const OWNER_ALIGNMENT: usize = std::mem::align_of::<Mutex<OutputOwner>>();
const ARC_ALIGNMENT: usize = if OWNER_ALIGNMENT > std::mem::align_of::<usize>() {
    OWNER_ALIGNMENT
} else {
    std::mem::align_of::<usize>()
};
const OWNER_OFFSET: usize = ARC_HEADER_BYTES.div_ceil(OWNER_ALIGNMENT) * OWNER_ALIGNMENT;
const OWNER_ALLOCATION_BYTES: usize = (OWNER_OFFSET + std::mem::size_of::<Mutex<OutputOwner>>())
    .div_ceil(ARC_ALIGNMENT)
    * ARC_ALIGNMENT;
// Arc disposes OutputOwner fields before freeing its outer allocation. Count
// the mutex/header as well as each bounded endpoint destructor and the sole
// constructor's retiring shell, inside the original family ceiling.
pub(super) const FIXED_BYTES: usize =
    STACK_BYTES + CONTROL_BYTES + (super::MAX_CLIENTS + 1) * OWNER_ALLOCATION_BYTES;
const CONSTRUCTION_BYTES: usize = 64 * 1024 * 1024;
const INSPECTION_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct OutputOwner {
    guard: GuardedOutput,
    // Actual helper/observer disposal precedes reservation refund.
    _charge: Charge,
}
struct Task {
    frames: Arc<Frames>,
    path: PathBuf,
    expected: String,
    owner_charge: Charge,
    scratch_charge: Charge,
}

pub(super) struct Queue {
    task: Mutex<Option<Task>>,
    ready: Condvar,
    active: AtomicBool,
    cancelled: Arc<AtomicBool>,
    budget: Arc<Budget>,
}
impl Queue {
    pub(super) fn new(budget: Arc<Budget>) -> Arc<Self> {
        Arc::new(Self {
            task: Mutex::new(None),
            ready: Condvar::new(),
            active: AtomicBool::new(false),
            cancelled: Arc::new(AtomicBool::new(false)),
            budget,
        })
    }
    pub(super) fn spawn(queue: &Arc<Self>) -> io::Result<JoinHandle<()>> {
        let queue = Arc::clone(queue);
        std::thread::Builder::new()
            .name("sail-output-owner".to_owned())
            .stack_size(STACK_BYTES)
            .spawn(move || queue.run())
    }
    #[cfg(test)]
    pub(super) fn idle_for_test(&self) -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while self.active.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        !self.active.load(Ordering::Acquire)
    }
    pub(super) fn stop(&self) {
        self.cancelled.store(true, Ordering::Release);
        // Retire the queued Task outside its mutex. A running Task keeps its
        // independent reservations until all construction locals have dropped.
        let old = self.task.lock().ok().and_then(|mut task| task.take());
        let queued = old.is_some();
        drop(old);
        if queued {
            self.active.store(false, Ordering::Release);
        }
        self.ready.notify_all();
    }
    pub(super) fn offer(&self, frames: &Arc<Frames>, path: &Path, expected: &str) {
        if !frames.charged()
            || frames.offered.swap(true, Ordering::AcqRel)
            || self.cancelled.load(Ordering::Acquire)
            || path.as_os_str().len() > 8192
            || expected.len() != 64
            || !expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return;
        }
        // Both quota reservations precede any request allocation. Busy service
        // and unavailable quota leave this frame generation on original hashing.
        let Some(owner_charge) = self.budget.reserve(GUARDED_OUTPUT_RETAINED_BYTES) else {
            return;
        };
        let Some(scratch_charge) = self.budget.reserve(CONSTRUCTION_BYTES) else {
            return;
        };
        // The constructor holds this slot only to park or take a task. A bare
        // try-lock lost this one-shot offer whenever the two coincided, for
        // example just after the constructor thread started.
        let Some(mut slot) = lock_until(&self.task, std::time::Instant::now() + LOCK_WAIT) else {
            return;
        };
        if self.cancelled.load(Ordering::Acquire) || self.active.swap(true, Ordering::AcqRel) {
            return;
        }
        *slot = Some(Task {
            frames: Arc::clone(frames),
            path: path.to_path_buf(),
            expected: expected.to_owned(),
            owner_charge,
            scratch_charge,
        });
        drop(slot);
        self.ready.notify_one();
    }
    pub(super) fn validate<R>(
        &self,
        frames: &Frames,
        trace: &mut super::rpc_diagnostics::Trace<'_>,
        input: impl FnOnce(&mut super::rpc_diagnostics::Trace<'_>) -> R,
    ) -> Option<R> {
        // Scratch admission precedes all observer/kernel work. Retain the exact
        // charged owner across independent input validation, without holding
        // either output mutex while that separate operation runs. Concurrent
        // readers serialize only on these short barriers, each waiting at most
        // LOCK_WAIT; a longer wait or a poisoned lock falls back to the
        // complete read and hash.
        let _scratch = self.budget.reserve(INSPECTION_BYTES)?;
        let deadline = std::time::Instant::now() + LOCK_WAIT;
        let owner = {
            let cell = lock_until(&frames.output, deadline)?;
            Arc::clone(cell.as_ref()?)
        };
        {
            let mut checked = lock_until(&owner, deadline)?;
            if !trace.measure(super::rpc_diagnostics::Stage::OutputSeal, || {
                checked.guard.namespace_barrier()
            }) {
                return None;
            }
        }
        let input = input(trace);
        // Refuse owner removal/substitution, even if the substitute independently
        // hashed identical bytes. Index eviction alone does not invalidate a
        // continuously guarded owner retained by this exact frame family.
        let deadline = std::time::Instant::now() + LOCK_WAIT;
        let cell = lock_until(&frames.output, deadline)?;
        if !cell
            .as_ref()
            .is_some_and(|published| Arc::ptr_eq(published, &owner))
        {
            return None;
        }
        let mut checked = lock_until(&owner, deadline)?;
        trace
            .measure(super::rpc_diagnostics::Stage::OutputSeal, || {
                checked.guard.clean()
            })
            .then_some(input)
    }
    fn run(&self) {
        loop {
            let task = {
                let Ok(mut slot) = self.task.lock() else {
                    return;
                };
                while slot.is_none() && !self.cancelled.load(Ordering::Acquire) {
                    let Ok(next) = self.ready.wait(slot) else {
                        return;
                    };
                    slot = next;
                }
                if self.cancelled.load(Ordering::Acquire) {
                    return;
                }
                slot.take()
            };
            // Construct before moving Task into this scope, so Task/constructor
            // locals and scratch are retired before releasing active on unwind.
            let _retirement = Retirement(self);
            if let Some(task) = task {
                self.construct(task);
            }
        }
    }
    fn construct(&self, task: Task) {
        let Task {
            frames,
            path,
            expected,
            owner_charge,
            scratch_charge,
        } = task;
        let mut owner_charge = Some(owner_charge);
        let owner = GuardedOutput::begin(&path, &expected, Arc::clone(&self.cancelled));
        if let Some(guard) = owner
            && !self.cancelled.load(Ordering::Acquire)
            && guard
                .retained_bytes()
                .and_then(|n| n.checked_add(OWNER_ALLOCATION_BYTES))
                .is_some_and(|n| n <= GUARDED_OUTPUT_RETAINED_BYTES)
            && let Some(charge) = owner_charge.take()
        {
            let mut candidate = Some(Arc::new(Mutex::new(OutputOwner {
                guard,
                _charge: charge,
            })));
            if let Ok(mut cell) = frames.output.lock()
                && cell.is_none()
                && !self.cancelled.load(Ordering::Acquire)
            {
                *cell = candidate.take();
            }
            // A refused candidate disposes its helper and reservation off-lock.
            drop(candidate);
        }
        // Explicit disposal precedes scratch refund and the outer retirement
        // flag. Evicted frames still retain both charges throughout construction.
        drop(path);
        drop(expected);
        drop(frames);
        drop(owner_charge);
        drop(scratch_charge);
    }
}
/// Longest wait for one output barrier lock before the complete-hash fallback.
/// Barrier critical sections are short native checks, never input validation.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(50);

/// Acquires `mutex` before `deadline`, polling rather than blocking so the wait
/// stays bounded. Poison and expiry both decline, selecting the fallback.
fn lock_until<T>(
    mutex: &Mutex<T>,
    deadline: std::time::Instant,
) -> Option<std::sync::MutexGuard<'_, T>> {
    loop {
        match mutex.try_lock() {
            Ok(guard) => return Some(guard),
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_micros(50));
            }
            Err(_) => return None,
        }
    }
}

struct Retirement<'a>(&'a Queue);
impl Drop for Retirement<'_> {
    fn drop(&mut self) {
        self.0.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::super::result_cache::ResultCache;
    use super::*;

    fn frame(cache: &ResultCache, key: &str) -> Arc<Frames> {
        cache.insert(
            key.to_owned(),
            Frames::new(Arc::new(vec![1]), Arc::new(vec![2]), Arc::new(vec![3])),
        )
    }

    #[test]
    fn barrier_lock_waits_within_bound_then_declines() {
        use std::time::{Duration, Instant};
        let mutex = Arc::new(Mutex::new(()));
        let held = Arc::clone(&mutex);
        let (locked, release) = (
            Arc::new(std::sync::Barrier::new(2)),
            Duration::from_millis(5),
        );
        let signal = Arc::clone(&locked);
        let holder = std::thread::spawn(move || {
            let guard = held.lock();
            signal.wait();
            std::thread::sleep(release);
            drop(guard);
        });
        locked.wait();
        // Released well within the bound: the waiter acquires the lock.
        let acquired = lock_until(&mutex, Instant::now() + Duration::from_secs(5)).is_some();
        let joined = holder.join().is_ok();
        // Held beyond the bound: the waiter declines, selecting the fallback.
        let guard = mutex.lock();
        let started = Instant::now();
        let declined = lock_until(&mutex, started + Duration::from_millis(20)).is_none();
        let waited = started.elapsed();
        drop(guard);
        assert!(acquired && joined && declined);
        assert!(waited >= Duration::from_millis(20));
    }

    /// A constructor briefly holding its slot (parking or taking a task) must
    /// not silently consume a frame family's one-shot offer.
    #[test]
    fn offer_waits_for_a_briefly_held_constructor_slot() {
        let cache = ResultCache::default();
        let queue = Queue::new(cache.budget());
        let frames = frame(&cache, "briefly-busy");
        let initial = queue.budget.used_for_test();
        let locked = std::sync::Barrier::new(2);
        let queued = std::thread::scope(|scope| {
            let holder = scope.spawn(|| {
                let slot = queue.task.lock();
                locked.wait();
                std::thread::sleep(std::time::Duration::from_millis(5));
                drop(slot);
            });
            locked.wait();
            queue.offer(
                &frames,
                Path::new("/tmp/sail-output-not-created"),
                &"0".repeat(64),
            );
            let joined = holder.join().is_ok();
            joined && queue.task.lock().is_ok_and(|slot| slot.is_some())
        });
        queue.stop();
        assert!(frames.offered.load(Ordering::Acquire));
        assert!(queued);
        // Stopping retires the queued task and refunds both reservations.
        assert_eq!(queue.budget.used_for_test(), initial);
    }

    #[test]
    fn queued_construction_keeps_entire_evicted_family_charged_and_foreground_declines() {
        let cache = ResultCache::default();
        let queue = Queue::new(cache.budget());
        let frames = frame(&cache, "queued");
        let initial = queue.budget.used_for_test();
        queue.offer(
            &frames,
            Path::new("/tmp/sail-output-not-created"),
            &"0".repeat(64),
        );
        assert_eq!(
            queue.budget.used_for_test(),
            initial + GUARDED_OUTPUT_RETAINED_BYTES + CONSTRUCTION_BYTES
        );
        let mut input_called = false;
        assert!(
            queue
                .validate(
                    &frames,
                    &mut super::super::rpc_diagnostics::Trace::disabled(),
                    |_| {
                        input_called = true;
                    }
                )
                .is_none()
        );
        assert!(!input_called);
        cache.remove("queued");
        drop(frames);
        assert_eq!(
            queue.budget.used_for_test(),
            initial + GUARDED_OUTPUT_RETAINED_BYTES + CONSTRUCTION_BYTES
        );
        queue.stop();
        assert_eq!(queue.budget.used_for_test(), 0);
        assert!(!queue.active.load(Ordering::Acquire));
    }

    #[test]
    fn one_slot_and_one_shot_refusal_never_retry_or_overlap_constructors() {
        let cache = ResultCache::default();
        let queue = Queue::new(cache.budget());
        let first = frame(&cache, "first");
        let second = frame(&cache, "second");
        let path = Path::new("/tmp/sail-output-not-created");
        let expected = "0".repeat(64);
        let initial = queue.budget.used_for_test();
        queue.offer(&first, path, &expected);
        queue.offer(&second, path, &expected);
        assert!(first.offered.load(Ordering::Acquire));
        assert!(second.offered.load(Ordering::Acquire));
        assert!(queue.task.lock().is_ok_and(|slot| {
            slot.as_ref()
                .is_some_and(|task| Arc::ptr_eq(&task.frames, &first))
        }));
        assert_eq!(
            queue.budget.used_for_test(),
            initial + GUARDED_OUTPUT_RETAINED_BYTES + CONSTRUCTION_BYTES
        );
        queue.stop();
        assert_eq!(queue.budget.used_for_test(), initial);
        queue.offer(&second, path, &expected);
        assert!(queue.task.lock().is_ok_and(|slot| slot.is_none()));
        assert_eq!(queue.budget.used_for_test(), initial);
    }

    #[test]
    fn fixed_service_storage_and_single_stack_fit_separate_margin() {
        // The pinned standard library's ArcInner uses this C layout. Bind both
        // header-to-data and tail padding rather than assuming usize alignment
        // for the output mutex. No allocation or unsafe inspection is needed.
        #[repr(C)]
        struct AllocationLayout {
            _strong: std::sync::atomic::AtomicUsize,
            _weak: std::sync::atomic::AtomicUsize,
            _data: Mutex<OutputOwner>,
        }
        assert_eq!(
            OWNER_ALLOCATION_BYTES,
            std::mem::size_of::<AllocationLayout>()
        );
        assert_eq!(ARC_ALIGNMENT, std::mem::align_of::<AllocationLayout>());
        let fixed = std::mem::size_of::<Queue>()
            + 4 * std::mem::size_of::<usize>()
            + std::mem::size_of::<AtomicBool>()
            + std::mem::size_of::<JoinHandle<()>>()
            + std::mem::size_of::<Task>()
            + 8192
            + 64;
        assert!(fixed <= CONTROL_BYTES);
        assert_eq!(
            FIXED_BYTES,
            STACK_BYTES + CONTROL_BYTES + (super::super::MAX_CLIENTS + 1) * OWNER_ALLOCATION_BYTES
        );
        assert_eq!(
            CONSTRUCTION_BYTES + GUARDED_OUTPUT_RETAINED_BYTES,
            80 * 1024 * 1024
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_background_owner_brackets_input_and_cannot_survive_restored_output_write() {
        use std::time::{Duration, Instant};
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-output-service-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir(&root).is_ok());
        let path = root.join("stdout");
        let bytes = b"complete persisted output";
        assert!(std::fs::write(&path, bytes).is_ok());
        let cache = ResultCache::default();
        let queue = Queue::new(cache.budget());
        let frames = frame(&cache, "real");
        let initial = queue.budget.used_for_test();
        // Queue before spawning; see the overlap fixture.
        queue.offer(
            &frames,
            &path,
            &sail_key::QueryKey(sail_key::sha256(bytes)).to_string(),
        );
        let worker = Queue::spawn(&queue).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let deadline = Instant::now() + Duration::from_secs(30);
        while queue.active.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut trace = super::super::rpc_diagnostics::Trace::disabled();
        let healthy = queue.validate(&frames, &mut trace, |_| 17);
        let mut input_called = false;
        let mut writes_ok = false;
        let changed = queue.validate(&frames, &mut trace, |_| {
            input_called = true;
            writes_ok = std::fs::write(&path, b"changed output").is_ok()
                && std::fs::write(&path, bytes).is_ok();
            (writes_ok, 23)
        });
        let sticky = queue.validate(&frames, &mut trace, |_| 29);
        cache.remove("real");
        let in_flight = frames.select(sail_api::jobs::Currentness::Current);
        drop(frames);
        queue.stop();
        let joined = worker.join().is_ok();
        let held = queue.budget.used_for_test();
        drop(in_flight);
        let refunded = queue.budget.used_for_test();
        assert!(std::fs::remove_dir_all(&root).is_ok());
        // Cleanup and the actual constructor join precede every outcome assertion.
        assert_eq!(healthy, Some(17));
        assert!(input_called && writes_ok);
        assert!(changed.is_none());
        assert!(sticky.is_none());
        assert!(joined);
        assert_eq!(held, initial + GUARDED_OUTPUT_RETAINED_BYTES);
        assert_eq!(refunded, 0);
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "output_overlap_tests.rs"]
mod overlap_tests;
