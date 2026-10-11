//! Fixed-slot rendezvous while the compiler thread holds its real namespace guard.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
// Fixed mutex/condvar/Arc controls and one boxed queue request. No variable-size
// data, waiter queue, source file, span, String, or compiler context is owned here.
const CONTROL_BYTES: usize = 16 * 1024;
pub(super) const QUEUE_RESERVE: usize = 64 * 1024;
static QUEUE_USED: AtomicBool = AtomicBool::new(false);
static OWNED: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
type DropPause = (
    std::sync::mpsc::SyncSender<()>,
    std::sync::mpsc::Receiver<()>,
);
#[cfg(test)]
static DROP_PAUSE: Mutex<Option<DropPause>> = Mutex::new(None);
#[cfg(test)]
static RETIREMENT_PAUSE: Mutex<Option<DropPause>> = Mutex::new(None);
struct Owner;
impl Drop for Owner {
    fn drop(&mut self) {
        OWNED.store(0, Ordering::Release);
        #[cfg(test)]
        {
            let pause = DROP_PAUSE.lock().ok().and_then(|mut value| value.take());
            if let Some((started, resume)) = pause {
                let _ = started.send(());
                let _ = resume.recv();
            }
        }
    }
}
#[derive(Default)]
struct State {
    ready: bool,
    requester: bool,
    service: bool,
    command: Option<[u64; 2]>,
    reply: Option<bool>,
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    _owner: Owner,
}
#[derive(Default)]
struct Completion {
    retired: Mutex<bool>,
    changed: Condvar,
}
/// Compiler-only lexical guard, installed before its borrowed namespace guard.
/// This is a lifetime acknowledgement, never source authority.
pub(super) struct Retirement {
    completion: Arc<Completion>,
}
impl Drop for Retirement {
    fn drop(&mut self) {
        // Recover poison only to notify disposal. No source validator, Ready
        // state or proof authority is accessed through this separate control.
        let mut retired = match self.completion.retired.lock() {
            Ok(retired) => retired,
            Err(error) => error.into_inner(),
        };
        *retired = true;
        self.completion.changed.notify_all();
        drop(retired);
        #[cfg(test)]
        {
            let pause = RETIREMENT_PAUSE.lock().ok().and_then(|mut value| value.take());
            if let Some((started, resume)) = pause {
                let _ = started.send(());
                let _ = resume.recv();
            }
        }
    }
}
pub(super) fn retained_bytes() -> usize {
    OWNED.load(Ordering::Acquire)
        + if QUEUE_USED.load(Ordering::Acquire) {
            QUEUE_RESERVE
        } else {
            0
        }
}
#[cfg(test)]
pub(super) fn active_bytes() -> usize {
    OWNED.load(Ordering::Acquire)
}
pub(super) struct Request {
    shared: Arc<Shared>,
    completion: Arc<Completion>,
}
pub(super) struct Lease {
    shared: Arc<Shared>,
    completion: Arc<Completion>,
}
impl Lease {
    pub(super) fn pair() -> Option<(Self, Request)> {
        QUEUE_USED.store(true, Ordering::Release);
        OWNED
            .compare_exchange(0, CONTROL_BYTES, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                requester: true,
                service: true,
                ..State::default()
            }),
            changed: Condvar::new(),
            _owner: Owner,
        });
        // The existing active reservation precedes BOTH fixed allocations.
        let completion = Arc::new(Completion::default());
        Some((
            Self {
                shared: Arc::clone(&shared),
                completion: Arc::clone(&completion),
            },
            Request { shared, completion },
        ))
    }
    pub(super) fn ready(&self) -> bool {
        let Ok(mut state) = self.shared.state.lock() else {
            return false;
        };
        while !state.ready && state.service {
            let Ok(next) = self.shared.changed.wait(state) else {
                return false;
            };
            state = next;
        }
        state.ready && state.service
    }
    pub(super) fn validate(&self, words: [u64; 2]) -> bool {
        let Ok(mut state) = self.shared.state.lock() else {
            return false;
        };
        if !state.ready || !state.service || state.command.is_some() || state.reply.is_some() {
            return false;
        }
        state.command = Some(words);
        self.shared.changed.notify_all();
        while state.reply.is_none() && state.service {
            let Ok(next) = self.shared.changed.wait(state) else {
                return false;
            };
            state = next;
        }
        state.reply.take().unwrap_or(false)
    }
    /// Normal-path retirement, after Ready and the complete worker operation.
    /// The caller retains PREPARATION_GATE until this returns and this Lease
    /// physically drops. Drop itself remains a non-waiting disconnect.
    pub(super) fn finish(&self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.requester = false;
        }
        self.shared.changed.notify_all();
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
        if let Ok(mut state) = self.shared.state.lock() {
            state.requester = false;
        }
        self.shared.changed.notify_all();
    }
}
impl Request {
    /// Install before map.files(), then dispose after service and that borrow.
    /// On unwind lexical drop order performs the same lifetime notification.
    pub(super) fn retirement_guard(&self) -> Retirement {
        Retirement { completion: Arc::clone(&self.completion) }
    }
    pub(super) fn serve(self, mut validate: impl FnMut([u64; 2]) -> bool) {
        let Ok(mut state) = self.shared.state.lock() else {
            return;
        };
        state.ready = true;
        self.shared.changed.notify_all();
        loop {
            while state.command.is_none() && state.requester {
                let Ok(next) = self.shared.changed.wait(state) else {
                    return;
                };
                state = next;
            }
            let Some(words) = state.command.take() else {
                return;
            };
            drop(state);
            let answer = validate(words);
            let Ok(next) = self.shared.state.lock() else {
                return;
            };
            state = next;
            state.reply = Some(answer);
            self.shared.changed.notify_all();
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.service = false;
        }
        self.shared.changed.notify_all();
    }
}

#[cfg(test)]
pub(super) fn shared_allocation_bytes() -> usize {
    std::mem::size_of::<Shared>() + 2 * std::mem::size_of::<usize>()
}
#[cfg(test)]
pub(super) fn completion_allocation_bytes() -> usize {
    std::mem::size_of::<Completion>() + 2 * std::mem::size_of::<usize>()
}
#[cfg(test)]
pub(super) fn retirement_handle_bytes() -> usize {
    std::mem::size_of::<Retirement>()
}
#[cfg(test)]
mod tests {
    use super::Lease;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn actual_default_rustc_namespace_stays_on_compiler_thread() {
        rustc_data_structures::sync::set_dyn_thread_safe_mode(false);
        assert!(rustc_data_structures::sync::check_dyn_thread_safe().is_none());
        let map = rustc_span::source_map::SourceMap::new(
            rustc_span::source_map::FilePathMapping::empty(),
        );
        map.new_source_file(
            rustc_span::FileName::Custom("first".into()),
            "abc".to_owned(),
        );
        let (lease, request) = Lease::pair().unwrap();
        let published = Arc::new(AtomicBool::new(false));
        let signal = published.clone();
        let (diag_send, diag_receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            assert!(lease.validate([7, 8]));
            assert!(!lease.validate([9, 10]));
            diag_send.send("ordinary queued diagnostic").unwrap();
            signal.store(true, Ordering::Release);
            drop(lease);
        });
        {
            let files = map.files();
            request.serve(|words| {
                assert_eq!(files.len(), 1);
                assert!(!published.load(Ordering::Acquire));
                words == [7, 8]
            });
            assert!(published.load(Ordering::Acquire));
            // Deliberately do not dequeue/render diagnostics while files is held.
        }
        assert_eq!(diag_receive.recv().unwrap(), "ordinary queued diagnostic");
        // Actual SourceMap mutation succeeds only after service and guard disposal.
        map.new_source_file(
            rustc_span::FileName::Custom("after".into()),
            "xyz".to_owned(),
        );
        assert_eq!(map.files().len(), 2);
        worker.join().unwrap();
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn cancellation_before_ready_releases_service_and_owner() {
        let (lease, request) = Lease::pair().unwrap();
        drop(lease);
        request.serve(|_| panic!("cancelled request validated"));
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn dropped_service_refuses_ready_without_waiting() {
        let (lease, request) = Lease::pair().unwrap();
        drop(request);
        assert!(!lease.ready());
        drop(lease);
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn validator_unwind_wakes_waiting_worker() {
        let (lease, request) = Lease::pair().unwrap();
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            assert!(!lease.validate([1, 2]));
        });
        assert!(
            std::panic::catch_unwind(|| request.serve(|_| panic!("injected validator panic")))
                .is_err()
        );
        worker.join().unwrap();
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn worker_unwind_disconnects_compiler_service() {
        let (lease, request) = Lease::pair().unwrap();
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            panic!("injected worker panic");
        });
        request.serve(|_| false);
        assert!(worker.join().is_err());
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn last_peer_owns_budget_until_physical_control_disposal() {
        let (lease, request) = Lease::pair().unwrap();
        assert!(Lease::pair().is_none());
        drop(lease);
        assert!(Lease::pair().is_none());
        assert_eq!(super::active_bytes(), 16384);
        drop(request);
        assert_eq!(super::active_bytes(), 0);
        let pair = Lease::pair().unwrap();
        drop(pair);
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn repeated_validations_use_one_fixed_slot_and_exact_words() {
        let (lease, request) = Lease::pair().unwrap();
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            for i in 0..1000 {
                assert_eq!(lease.validate([i, u64::MAX - i]), i % 2 == 0);
            }
        });
        request.serve(|words| {
            assert_eq!(words[1], u64::MAX - words[0]);
            assert_eq!(super::active_bytes(), 16384);
            words[0] % 2 == 0
        });
        worker.join().unwrap();
        assert_eq!(super::active_bytes(), 0);
    }
    #[test]
    fn paused_retiring_shared_shell_is_covered_by_fixed_margin() {
        let pair = Lease::pair().unwrap();
        let (started, notice) = std::sync::mpsc::sync_channel(0);
        let (resume, continue_drop) = std::sync::mpsc::sync_channel(0);
        *super::DROP_PAUSE.lock().unwrap() = Some((started, continue_drop));
        let retiring = std::thread::spawn(move || drop(pair));
        notice.recv().unwrap();
        // Old Shared's fields have disposed, but Arc's allocation is physically
        // live until the paused Owner::drop returns. New admission is permitted
        // only because a permanent retirement reserve covers this second shell.
        assert_eq!(super::active_bytes(), 0);
        assert_eq!(super::retained_bytes(), super::QUEUE_RESERVE);
        let new = Lease::pair().unwrap();
        assert_eq!(super::retained_bytes(), super::QUEUE_RESERVE + 16384);
        drop(new);
        resume.send(()).unwrap();
        retiring.join().unwrap();
        assert_eq!(super::retained_bytes(), super::QUEUE_RESERVE);
    }
    #[test]
    fn inline_shared_and_two_retiring_controls_fit_explicit_fixed_margin() {
        let shared = super::shared_allocation_bytes();
        // Pinned Unix pthread lazy initialization: at most two candidates each
        // for one Mutex and Condvar. Each native object has a256-byte audited
        // ceiling on the supported Linux/Darwin pins. Kernel state is not a
        // Rust heap/source allocation; no variable-size user queue exists.
        assert!(2 * shared + 4 * 256 + 512 <= 4096);
        assert!(std::mem::size_of::<super::Request>() + std::mem::size_of::<super::Lease>() <= 128);
    }
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn map() -> rustc_span::source_map::SourceMap {
        rustc_data_structures::sync::set_dyn_thread_safe_mode(false);
        assert!(rustc_data_structures::sync::check_dyn_thread_safe().is_none());
        let map = rustc_span::source_map::SourceMap::new(
            rustc_span::source_map::FilePathMapping::empty(),
        );
        map.new_source_file(rustc_span::FileName::Custom("first".into()), "abc".to_owned());
        map
    }

    #[test]
    fn old_disconnect_under_live_compiler_owner_refuses_next_admission() {
        let map = map();
        let (lease, request) = Lease::pair().unwrap();
        // Force the legal receiver scheduling window: Ready was published,
        // but the compiler still owns Request and has not handled disconnect.
        request.shared.state.lock().unwrap().ready = true;
        let files = map.files();
        assert_eq!(files.len(), 1);
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            drop(lease); // The unchanged old normal-return disconnect.
        });
        worker.join().unwrap();
        assert_eq!(active_bytes(), CONTROL_BYTES);
        assert!(Lease::pair().is_none());
        let retirement = request.retirement_guard();
        request.serve(|_| panic!("disconnected request must not validate"));
        drop(files);
        drop(retirement);
        assert_eq!(active_bytes(), 0);
    }

    #[test]
    fn normal_finish_waits_for_actual_compiler_guard_and_owner_retirement() {
        let map = map();
        let (lease, request) = Lease::pair().unwrap();
        let retirement = request.retirement_guard();
        let files = map.files();
        let (finished, result) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            assert!(lease.validate([1, 2]));
            lease.finish();
            drop(lease);
            finished.send(()).unwrap();
        });
        request.serve(|words| { assert_eq!(files.len(), 1); words == [1, 2] });
        // The service Request is gone, but the real borrowed namespace and
        // lifetime token remain. The worker is still the charged last owner.
        assert_eq!(active_bytes(), CONTROL_BYTES);
        assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
        drop(files);
        map.new_source_file(rustc_span::FileName::Custom("after".into()), "xyz".to_owned());
        drop(retirement);
        result.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        assert_eq!(active_bytes(), 0);
        drop(Lease::pair().unwrap());
    }

    #[test]
    fn retirement_before_waiter_registration_is_not_lost() {
        let (lease, request) = Lease::pair().unwrap();
        let retirement = request.retirement_guard();
        drop(request); // Peer loss: no source authority or Ready is granted.
        assert!(!lease.ready());
        drop(retirement);
        lease.finish(); // Predicate is already true, without an extra signal.
        drop(lease);
        assert_eq!(active_bytes(), 0);
    }

    #[test]
    fn validator_unwind_retires_guard_and_wakes_normal_finish() {
        let map = map();
        let (lease, request) = Lease::pair().unwrap();
        let worker = std::thread::spawn(move || {
            assert!(lease.ready());
            assert!(!lease.validate([1, 2]));
            lease.finish();
        });
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _retirement = request.retirement_guard();
            let _files = map.files();
            request.serve(|_| panic!("controlled validator unwind"));
        })).is_err());
        worker.join().unwrap();
        map.new_source_file(rustc_span::FileName::Custom("after unwind".into()), "xyz".to_owned());
        assert_eq!(active_bytes(), 0);
    }

    #[test]
    fn acknowledged_old_completion_shell_remains_covered_after_active_refund() {
        let (lease, request) = Lease::pair().unwrap();
        let retirement = request.retirement_guard();
        let old_completion = Arc::downgrade(&retirement.completion);
        drop(request);
        let (started, notice) = mpsc::sync_channel(0);
        let (resume, continue_drop) = mpsc::sync_channel(0);
        *RETIREMENT_PAUSE.lock().unwrap() = Some((started, continue_drop));
        let retiring = std::thread::spawn(move || drop(retirement));
        notice.recv_timeout(Duration::from_secs(1)).unwrap();
        lease.finish();
        drop(lease);
        assert_eq!(active_bytes(), 0);
        assert!(old_completion.upgrade().is_some());
        assert_eq!(retained_bytes(), QUEUE_RESERVE);
        let next = Lease::pair().unwrap();
        assert_eq!(retained_bytes(), QUEUE_RESERVE + CONTROL_BYTES);
        drop(next);
        resume.send(()).unwrap();
        retiring.join().unwrap();
        assert!(old_completion.upgrade().is_none());
        assert_eq!(retained_bytes(), QUEUE_RESERVE);
    }

    #[test]
    fn fixed_completion_and_retiring_native_candidates_fit_old_margin() {
        assert!(2 * shared_allocation_bytes() + 2 * completion_allocation_bytes()
            + 2 * std::mem::size_of::<Request>() + 2 * std::mem::size_of::<Lease>()
            + 2 * retirement_handle_bytes() + 10 * 256 + 512 <= 4096);
    }
}
