//! Joined cancellation of a complete fresh-frame RPC.

use std::fmt;
use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{Client, Mode, Request, Response, read_frame, write_frame};

const TRANSPORT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);

/// Failure origin for a cancellable RPC; only a pre-RPC connect failure is retryable.
#[derive(Debug)]
pub enum CancellableError {
    /// No RPC has been sent: opening the initial socket failed.
    Connect(io::Error),
    /// Cancellation, timeout, negotiation, or I/O after opening a socket.
    Operation(io::Error),
}
impl CancellableError {
    /// Whether the initial connect failed before any RPC, including negotiation.
    #[must_use]
    pub fn is_pre_rpc_connect_failure(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
    /// The original I/O error, including its cancellation/timeout kind.
    #[must_use]
    pub fn io_error(&self) -> &io::Error {
        match self {
            Self::Connect(error) | Self::Operation(error) => error,
        }
    }
}
impl fmt::Display for CancellableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io_error().fmt(formatter)
    }
}
impl std::error::Error for CancellableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.io_error())
    }
}

#[derive(Clone, Copy)]
enum Stop {
    Cancelled,
    Deadline,
}
impl Stop {
    fn error(self) -> io::Error {
        match self {
            Self::Cancelled => io::Error::new(
                io::ErrorKind::Interrupted,
                "SAIL0804: job transport cancelled",
            ),
            Self::Deadline => io::Error::new(
                io::ErrorKind::TimedOut,
                "SAIL0805: job transport deadline elapsed",
            ),
        }
    }
}
#[derive(Default)]
struct Completion {
    finished: bool,
    stopped: Option<Stop>,
}

fn check(cancelled: &impl Fn() -> bool, started: Instant) -> Option<Stop> {
    if cancelled() {
        Some(Stop::Cancelled)
    } else if started.elapsed() >= TRANSPORT {
        Some(Stop::Deadline)
    } else {
        None
    }
}

impl Client {
    /// Sends a fresh RPC with joined cancellation and an absolute five-second transport budget.
    ///
    /// The callback must return promptly; it is polled by one owned monitor.
    /// Cancellation shuts down the socket, and every exit joins that monitor.
    /// Normal completion wakes it immediately. The complete frame and optional
    /// decoding cache follow [`Client::call`]'s unchanged rules. A late response
    /// is refused after cancellation or expiry, including a byte-identical cache hit.
    ///
    /// Unlike `call`'s per-I/O limits, this path bounds the full transport
    /// interval, including fragmentation and capability negotiation. Kernel I/O
    /// and OS scheduling are not interruptible deadline guarantees.
    ///
    /// # Errors
    /// Returns [`CancellableError::Connect`] only before any RPC. Startup fallback
    /// may inspect that category and its I/O kind; errors after opening a socket,
    /// cancellation, and timeout must never trigger another RPC attempt.
    pub fn call_cancellable(
        &self,
        request: &Request,
        cancelled: impl Fn() -> bool + Sync,
    ) -> Result<Response, CancellableError> {
        self.call_cancellable_at(request, &cancelled, Instant::now())
    }

    fn call_cancellable_at(
        &self,
        request: &Request,
        cancelled: &(impl Fn() -> bool + Sync),
        started: Instant,
    ) -> Result<Response, CancellableError> {
        if let Some(stop) = check(cancelled, started) {
            return Err(CancellableError::Operation(stop.error()));
        }
        let negotiated =
            matches!(request, Request::Submit { submission } if submission.mode != Mode::Stock);
        if negotiated
            && !matches!(
                self.call_cancellable_at(&Request::Capabilities, cancelled, started)?,
                Response::Capabilities {
                    proof_version: crate::proof::VERSION,
                    repair: true
                }
            )
        {
            return Err(CancellableError::Operation(io::Error::other(
                "SAIL0903: endpoint does not support proof/repair contracts",
            )));
        }
        let mut stream = UnixStream::connect(&self.socket).map_err(|error| {
            if negotiated {
                CancellableError::Operation(error)
            } else {
                CancellableError::Connect(error)
            }
        })?;
        stream
            .set_read_timeout(Some(TRANSPORT))
            .map_err(CancellableError::Operation)?;
        stream
            .set_write_timeout(Some(TRANSPORT))
            .map_err(CancellableError::Operation)?;
        let shutdown = stream.try_clone().map_err(CancellableError::Operation)?;
        if let Some(stop) = check(cancelled, started) {
            return Err(CancellableError::Operation(stop.error()));
        }
        let completion = (Mutex::new(Completion::default()), Condvar::new());
        std::thread::scope(|scope| {
            let monitor = scope.spawn(|| -> io::Result<()> {
                loop {
                    // Callback and shutdown run outside the completion mutex.
                    let stopped = check(cancelled, started);
                    let (lock, ready) = &completion;
                    let mut state = lock
                        .lock()
                        .map_err(|_| io::Error::other("SAIL0806: cancellation monitor failed"))?;
                    if let Some(stop) = stopped {
                        state.stopped = Some(stop);
                        drop(state);
                        let _ = shutdown.shutdown(Shutdown::Both);
                        return Ok(());
                    }
                    if state.finished {
                        return Ok(());
                    }
                    let wait = POLL.min(TRANSPORT.saturating_sub(started.elapsed()));
                    let _ = ready
                        .wait_timeout(state, wait)
                        .map_err(|_| io::Error::other("SAIL0806: cancellation monitor failed"))?;
                }
            });
            let result = write_frame(&mut stream, request).and_then(|()| match &self.decoded {
                Some(cache) => cache.read(&mut stream),
                None => read_frame(&mut stream),
            });
            let (lock, ready) = &completion;
            let signalled = lock.lock().map(|mut state| {
                state.finished = true;
                ready.notify_all();
            });
            let joined = monitor
                .join()
                .map_err(|_| io::Error::other("SAIL0806: cancellation monitor failed"));
            signalled.map_err(|_| {
                CancellableError::Operation(io::Error::other(
                    "SAIL0806: cancellation monitor failed",
                ))
            })?;
            joined
                .map_err(CancellableError::Operation)?
                .map_err(CancellableError::Operation)?;
            let stopped = lock
                .lock()
                .map_err(|_| {
                    CancellableError::Operation(io::Error::other(
                        "SAIL0806: cancellation monitor failed",
                    ))
                })?
                .stopped;
            if let Some(stop) = stopped.or_else(|| check(cancelled, started)) {
                return Err(CancellableError::Operation(stop.error()));
            }
            match result.map_err(CancellableError::Operation)? {
                Response::Error(message) => {
                    Err(CancellableError::Operation(io::Error::other(message)))
                }
                response => Ok(response),
            }
        })
    }
}

#[cfg(test)]
#[path = "cancellable_tests.rs"]
mod tests;
