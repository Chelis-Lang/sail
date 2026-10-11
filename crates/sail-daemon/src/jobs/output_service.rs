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
// Box disposes OutputOwner fields before freeing its outer allocation. Count
// each bounded endpoint destructor and the sole constructor's retiring shell.
pub(super) const FIXED_BYTES: usize =
    STACK_BYTES + CONTROL_BYTES + (super::MAX_CLIENTS + 1) * std::mem::size_of::<OutputOwner>();
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
        let Ok(mut slot) = self.task.try_lock() else {
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
        // Scratch admission happens before observer/kernel work. The same
        // nonblocking mutex protects both output barriers around independent
        // input/runtime freshness. No readiness or constructor wait is allowed.
        let _scratch = self.budget.reserve(INSPECTION_BYTES)?;
        let mut cell = frames.output.try_lock().ok()?;
        let owner = cell.as_mut()?;
        if !trace.measure(super::rpc_diagnostics::Stage::OutputSeal, || {
            owner.guard.namespace_barrier()
        }) {
            return None;
        }
        let input = input(trace);
        trace
            .measure(super::rpc_diagnostics::Stage::OutputSeal, || {
                owner.guard.clean()
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
                .and_then(|n| n.checked_add(std::mem::size_of::<OutputOwner>()))
                .is_some_and(|n| n <= GUARDED_OUTPUT_RETAINED_BYTES)
            && let Ok(mut cell) = frames.output.lock()
            && let Some(charge) = owner_charge.take()
        {
            *cell = Some(Box::new(OutputOwner {
                guard,
                _charge: charge,
            }));
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
            STACK_BYTES
                + CONTROL_BYTES
                + (super::super::MAX_CLIENTS + 1) * std::mem::size_of::<OutputOwner>()
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
        let worker = Queue::spawn(&queue).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        queue.offer(
            &frames,
            &path,
            &sail_key::QueryKey(sail_key::sha256(bytes)).to_string(),
        );
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
