//! Fixed-slot rendezvous while the compiler thread holds its real namespace guard.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
// Fixed mutex/condvar/Arc controls and one boxed queue request. No variable-size
// data, waiter queue, source file, span, String, or compiler context is owned here.
const CONTROL_BYTES: usize = 16 * 1024;
static OWNED: AtomicUsize = AtomicUsize::new(0);
struct Owner;
impl Drop for Owner {
    fn drop(&mut self) {
        OWNED.store(0, Ordering::Release);
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
}
pub(super) struct Request {
    shared: Arc<Shared>,
}
pub(super) struct Lease {
    shared: Arc<Shared>,
}
impl Lease {
    pub(super) fn pair() -> Option<(Self, Request)> {
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
