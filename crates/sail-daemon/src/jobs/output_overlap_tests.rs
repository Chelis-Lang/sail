//! Real output ownership controls for concurrent independent input validation.

use super::super::result_cache::ResultCache;
use super::*;
use sail_api::jobs::Currentness;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

struct Fixture {
    root: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
    cache: ResultCache,
    queue: Arc<Queue>,
    frames: Option<Arc<Frames>>,
    worker: Option<JoinHandle<()>>,
}

impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-output-overlap-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir(&root).is_ok());
        let path = root.join("stdout");
        let bytes = b"complete continuously guarded persisted output".to_vec();
        assert!(std::fs::write(&path, &bytes).is_ok());
        let cache = ResultCache::default();
        let queue = Queue::new(cache.budget());
        let frames = cache.insert(
            "family".to_owned(),
            Frames::new(Arc::new(vec![1]), Arc::new(vec![2]), Arc::new(vec![3])),
        );
        // Queue the task before the worker exists: `offer` uses a nonblocking
        // try-lock, and a just-spawned worker can briefly hold the slot mutex.
        queue.offer(
            &frames,
            &path,
            &sail_key::QueryKey(sail_key::sha256(&bytes)).to_string(),
        );
        let worker = Queue::spawn(&queue).unwrap_or_else(|error| unreachable!("fixture: {error}"));
        let fixture = Self {
            root,
            path,
            bytes,
            cache,
            queue,
            frames: Some(frames),
            worker: Some(worker),
        };
        assert!(fixture.queue.idle_for_test());
        assert!(
            fixture
                .frames()
                .output
                .lock()
                .is_ok_and(|cell| cell.is_some())
        );
        fixture
    }

    fn frames(&self) -> &Arc<Frames> {
        self.frames
            .as_ref()
            .unwrap_or_else(|| unreachable!("fixture"))
    }

    fn owner(&self) -> Arc<Mutex<OutputOwner>> {
        self.frames()
            .output
            .lock()
            .ok()
            .and_then(|cell| cell.as_ref().map(Arc::clone))
            .unwrap_or_else(|| unreachable!("fixture owner"))
    }

    fn close(&mut self) -> (bool, bool) {
        self.queue.stop();
        let joined = self
            .worker
            .take()
            .is_none_or(|worker| worker.join().is_ok());
        self.cache.remove("family");
        self.cache.remove("substitute");
        drop(self.frames.take());
        let removed = !self.root.exists() || std::fs::remove_dir_all(&self.root).is_ok();
        (joined, removed)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[test]
fn independent_input_wait_does_not_hold_the_continuous_output_owner_lock() {
    let mut fixture = Fixture::new();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let queue = Arc::clone(&fixture.queue);
    let frames = Arc::clone(fixture.frames());
    let (entered, overlapping, first) = std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            queue.validate(
                &frames,
                &mut super::super::rpc_diagnostics::Trace::disabled(),
                |_| {
                    let sent = entered_tx.send(()).is_ok();
                    let released = release_rx.recv_timeout(Duration::from_secs(2)).is_ok();
                    (sent, released, 17)
                },
            )
        });
        let entered = entered_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        let overlapping = fixture.queue.validate(
            fixture.frames(),
            &mut super::super::rpc_diagnostics::Trace::disabled(),
            |_| 23,
        );
        // Release and join before asserting, even when the overlapping call
        // correctly exposes a regression. No production waiting is introduced.
        let _ = release_tx.send(());
        (entered, overlapping, worker.join().ok().flatten())
    });
    let cleanup = fixture.close();
    assert!(cleanup.0 && cleanup.1);
    assert!(entered);
    assert_eq!(overlapping, Some(23));
    assert_eq!(first, Some((true, true, 17)));
}

#[test]
fn restored_output_write_invalidates_both_overlapping_input_observations() {
    use std::fs::{File, FileTimes};
    let mut fixture = Fixture::new();
    let metadata =
        std::fs::metadata(&fixture.path).unwrap_or_else(|error| unreachable!("fixture: {error}"));
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let queue = Arc::clone(&fixture.queue);
    let frames = Arc::clone(fixture.frames());
    let (entered, writes, overlap, first) = std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            queue.validate(
                &frames,
                &mut super::super::rpc_diagnostics::Trace::disabled(),
                |_| {
                    let _ = entered_tx.send(());
                    release_rx.recv_timeout(Duration::from_secs(2)).is_ok()
                },
            )
        });
        let entered = entered_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        let mut writes = false;
        let overlap = fixture.queue.validate(
            fixture.frames(),
            &mut super::super::rpc_diagnostics::Trace::disabled(),
            |_| {
                writes = std::fs::write(&fixture.path, b"changed output").is_ok()
                    && std::fs::write(&fixture.path, &fixture.bytes).is_ok()
                    && File::options()
                        .write(true)
                        .open(&fixture.path)
                        .and_then(|file| {
                            file.set_times(FileTimes::new().set_modified(metadata.modified()?))
                        })
                        .is_ok();
                23
            },
        );
        let _ = release_tx.send(());
        (entered, writes, overlap, worker.join().ok().flatten())
    });
    let sticky = fixture.queue.validate(
        fixture.frames(),
        &mut super::super::rpc_diagnostics::Trace::disabled(),
        |_| 29,
    );
    let cleanup = fixture.close();
    assert!(cleanup.0 && cleanup.1 && entered && writes);
    assert!(overlap.is_none() && first.is_none() && sticky.is_none());
}

#[test]
fn missing_or_substituted_owner_cannot_complete_an_earlier_handoff() {
    let mut fixture = Fixture::new();
    let substitute = fixture.cache.insert(
        "substitute".to_owned(),
        Frames::new(Arc::new(vec![1]), Arc::new(vec![2]), Arc::new(vec![3])),
    );
    fixture.queue.offer(
        &substitute,
        &fixture.path,
        &sail_key::QueryKey(sail_key::sha256(&fixture.bytes)).to_string(),
    );
    let idle = fixture.queue.idle_for_test();
    let replacement = substitute
        .output
        .lock()
        .ok()
        .and_then(|cell| cell.as_ref().map(Arc::clone));
    let original = fixture.owner();
    let mut removed = None;
    let absent = fixture.queue.validate(
        fixture.frames(),
        &mut super::super::rpc_diagnostics::Trace::disabled(),
        |_| {
            removed = fixture.frames().output.try_lock().ok()?.take();
            Some(17)
        },
    );
    if let Ok(mut cell) = fixture.frames().output.lock() {
        *cell = removed.take();
    }
    let mut retired = None;
    let changed = fixture.queue.validate(
        fixture.frames(),
        &mut super::super::rpc_diagnostics::Trace::disabled(),
        |_| {
            let replacement = replacement.as_ref()?;
            let mut cell = fixture.frames().output.try_lock().ok()?;
            retired = cell.replace(Arc::clone(replacement));
            Some(23)
        },
    );
    let reserved_bytes = fixture.queue.budget.used_for_test();
    drop(retired);
    drop(original);
    drop(replacement);
    drop(substitute);
    let cleanup = fixture.close();
    assert!(cleanup.0 && cleanup.1 && idle);
    assert!(absent.is_none() && changed.is_none());
    assert!(reserved_bytes >= 2 * GUARDED_OUTPUT_RETAINED_BYTES);
    assert_eq!(fixture.queue.budget.used_for_test(), 0);
}

#[test]
fn evicted_family_keeps_exact_continuous_owner_charged_without_losing_validity() {
    let mut fixture = Fixture::new();
    let held = fixture.owner();
    let current = fixture.queue.validate(
        fixture.frames(),
        &mut super::super::rpc_diagnostics::Trace::disabled(),
        |_| {
            fixture.cache.remove("family");
            Currentness::Current
        },
    );
    let cleanup = fixture.close();
    let retained = fixture.queue.budget.used_for_test();
    let sole_reference = Arc::strong_count(&held) == 1;
    drop(held);
    let refunded = fixture.queue.budget.used_for_test();
    assert!(cleanup.0 && cleanup.1 && sole_reference);
    assert_eq!(current, Some(Currentness::Current));
    assert_eq!(retained, GUARDED_OUTPUT_RETAINED_BYTES);
    assert_eq!(refunded, 0);
}

#[test]
fn shutdown_between_barriers_refuses_even_with_a_retained_live_owner() {
    let mut fixture = Fixture::new();
    let held = fixture.owner();
    let result = fixture.queue.validate(
        fixture.frames(),
        &mut super::super::rpc_diagnostics::Trace::disabled(),
        |_| {
            fixture.queue.stop();
            Currentness::Current
        },
    );
    let cleanup = fixture.close();
    let retained = fixture.queue.budget.used_for_test();
    drop(held);
    assert!(cleanup.0 && cleanup.1);
    assert!(result.is_none());
    assert_eq!(retained, GUARDED_OUTPUT_RETAINED_BYTES);
    assert_eq!(fixture.queue.budget.used_for_test(), 0);
}

#[test]
fn actual_output_arc_shell_is_covered_after_its_charge_refunds() {
    let mut fixture = Fixture::new();
    let held = fixture.owner();
    let cleanup = fixture.close();
    let gate = Arc::new((std::sync::Barrier::new(2), std::sync::Barrier::new(2)));
    let (refunded, fits, joined) = std::thread::scope(|scope| {
        let worker_gate = Arc::clone(&gate);
        let worker = scope.spawn(move || {
            super::super::result_cache::REFUND_GATE
                .with(|slot| *slot.borrow_mut() = Some(worker_gate));
            drop(held);
            super::super::result_cache::REFUND_GATE.with(|slot| *slot.borrow_mut() = None);
        });
        gate.0.wait();
        let budget = Arc::clone(&fixture.queue.budget);
        let refunded = budget.used_for_test() == 0;
        // The same permanent margin still leaves the physical mutex/Arc shell
        // inside 128 MiB; the full ceiling can never be reserved by live owners.
        let fits = budget.reserve(128 * 1024 * 1024).is_none()
            && FIXED_BYTES
                >= STACK_BYTES
                    + CONTROL_BYTES
                    + (super::super::MAX_CLIENTS + 1) * OWNER_ALLOCATION_BYTES;
        gate.1.wait();
        (refunded, fits, worker.join().is_ok())
    });
    assert!(cleanup.0 && cleanup.1 && refunded && fits && joined);
}
