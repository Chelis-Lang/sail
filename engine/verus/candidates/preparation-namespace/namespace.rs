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
}
pub(super) struct Lease {
    shared: Arc<Shared>,
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
        Some((
            Self {
                shared: Arc::clone(&shared),
            },
            Request { shared },
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
