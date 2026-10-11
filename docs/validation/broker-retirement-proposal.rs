//! Isolated protocol model, not a production broker or compiler API control.
#![forbid(unsafe_code)]

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

const ACTIVE: usize = 16 * 1024;
struct Quota(Arc<AtomicUsize>);
impl Drop for Quota {
    fn drop(&mut self) {
        self.0.store(0, Ordering::Release);
    }
}
struct Shared {
    requester: Mutex<bool>,
    changed: Condvar,
    _quota: Quota,
}
#[derive(Default)]
struct Completion {
    retired: Mutex<bool>,
    changed: Condvar,
}
// Install before borrowing the compiler namespace. On unwind, the later
// namespace guard drops before this fixed completion guard signals retirement.
struct Retire(Arc<Completion>);
impl Drop for Retire {
    fn drop(&mut self) {
        // Poison recovery is for lifetime completion only, never authority.
        let mut retired = self.0.retired.lock().unwrap_or_else(|e| e.into_inner());
        *retired = true;
        self.0.changed.notify_all();
    }
}
struct Lease {
    shared: Arc<Shared>,
    completion: Arc<Completion>,
}
struct Request {
    shared: Arc<Shared>,
    completion: Arc<Completion>,
}
impl Lease {
    fn disconnect(&self) {
        if let Ok(mut requester) = self.shared.requester.lock() {
            *requester = false;
        }
        self.shared.changed.notify_all();
    }
    fn finish(&self) {
        self.disconnect();
        let Ok(mut retired) = self.completion.retired.lock() else {
            return;
        };
        while !*retired {
            let Ok(next) = self.completion.changed.wait(retired) else {
                return;
            };
            retired = next;
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.disconnect();
    }
}
fn pair(quota: &Arc<AtomicUsize>) -> Option<(Lease, Request)> {
    quota
        .compare_exchange(0, ACTIVE, Ordering::AcqRel, Ordering::Acquire)
        .ok()?;
    let shared = Arc::new(Shared {
        requester: Mutex::new(true),
        changed: Condvar::new(),
        _quota: Quota(Arc::clone(quota)),
    });
    let completion = Arc::new(Completion::default());
    Some((
        Lease {
            shared: Arc::clone(&shared),
            completion: Arc::clone(&completion),
        },
        Request { shared, completion },
    ))
}
struct BorrowedNamespace(Arc<AtomicUsize>);
impl Drop for BorrowedNamespace {
    fn drop(&mut self) {
        self.0.store(0, Ordering::Release);
    }
}
fn serve(
    request: Request,
    guarded: Arc<AtomicUsize>,
    paused: mpsc::SyncSender<()>,
    resume: mpsc::Receiver<()>,
) {
    let Request { shared, completion } = request;
    let retirement = Retire(completion);
    guarded.store(1, Ordering::Release);
    let guard = BorrowedNamespace(guarded);
    let mut requester = shared.requester.lock().unwrap();
    while *requester {
        requester = shared.changed.wait(requester).unwrap();
    }
    drop(requester);
    paused.send(()).unwrap();
    resume.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(shared);
    drop(guard);
    drop(retirement);
}

#[test]
fn old_disconnect_allows_gate_to_unlock_before_service_retirement() {
    let quota = Arc::new(AtomicUsize::new(0));
    let guarded = Arc::new(AtomicUsize::new(0));
    let (lease, request) = pair(&quota).unwrap();
    let (paused, pause) = mpsc::sync_channel(1);
    let (resume, resumed) = mpsc::sync_channel(1);
    let main_guarded = Arc::clone(&guarded);
    let main = std::thread::spawn(move || serve(request, main_guarded, paused, resumed));
    // Models ordinary with_namespace returning and releasing PREPARATION_GATE.
    drop(lease);
    pause.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(guarded.load(Ordering::Acquire), 1);
    assert_eq!(quota.load(Ordering::Acquire), ACTIVE);
    assert!(pair(&quota).is_none()); // Counterfactual: next valid operation misses.
    resume.send(()).unwrap();
    main.join().unwrap();
    assert_eq!(quota.load(Ordering::Acquire), 0);
}

#[test]
fn normal_finish_waits_for_real_retirement_then_drops_last_owner_under_gate() {
    let quota = Arc::new(AtomicUsize::new(0));
    let guarded = Arc::new(AtomicUsize::new(0));
    let (lease, request) = pair(&quota).unwrap();
    let weak = Arc::downgrade(&lease.shared);
    let (paused, pause) = mpsc::sync_channel(1);
    let (resume, resumed) = mpsc::sync_channel(1);
    let (released, release) = mpsc::sync_channel(1);
    let main_guarded = Arc::clone(&guarded);
    let main = std::thread::spawn(move || serve(request, main_guarded, paused, resumed));
    let worker = std::thread::spawn(move || {
        lease.finish();
        drop(lease); // Must complete before the existing gate guard drops.
        released.send(()).unwrap();
    });
    pause.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(release.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert_eq!(quota.load(Ordering::Acquire), ACTIVE);
    assert_eq!(guarded.load(Ordering::Acquire), 1);
    resume.send(()).unwrap();
    release.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.join().unwrap();
    main.join().unwrap();
    assert_eq!(guarded.load(Ordering::Acquire), 0);
    assert!(weak.upgrade().is_none());
    assert_eq!(quota.load(Ordering::Acquire), 0);
    let next = pair(&quota).unwrap();
    drop(next);
}

#[test]
fn worker_unwind_drop_still_disconnects_without_acknowledgement_wait() {
    let quota = Arc::new(AtomicUsize::new(0));
    let guarded = Arc::new(AtomicUsize::new(0));
    let (lease, request) = pair(&quota).unwrap();
    let (paused, pause) = mpsc::sync_channel(1);
    let (resume, resumed) = mpsc::sync_channel(1);
    let main = std::thread::spawn(move || serve(request, guarded, paused, resumed));
    assert!(
        std::panic::catch_unwind(move || {
            let _lease = lease;
            panic!("controlled worker unwind");
        })
        .is_err()
    );
    pause.recv_timeout(Duration::from_secs(1)).unwrap();
    resume.send(()).unwrap();
    main.join().unwrap();
    assert_eq!(quota.load(Ordering::Acquire), 0);
}

#[test]
fn model_fixed_controls_fit_existing_small_retirement_margin() {
    let shared = std::mem::size_of::<Shared>() + 2 * std::mem::size_of::<usize>();
    let completion = std::mem::size_of::<Completion>() + 2 * std::mem::size_of::<usize>();
    let handles = 2 * std::mem::size_of::<Request>() + 2 * std::mem::size_of::<Lease>();
    // Eight initialization candidates for the new Shared/Completion and two
    // native objects of a retiring Completion. The old Shared's fields drop
    // before its quota refund. Native256B ceilings need the actual pinned check.
    assert!(2 * shared + 2 * completion + handles + 10 * 256 + 512 <= 4096);
}
