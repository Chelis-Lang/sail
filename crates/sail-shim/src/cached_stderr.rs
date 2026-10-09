//! Forward solver stderr and certify silence only after solver shutdown.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

struct State {
    stream: UnixStream,
    seen: bool,
    eof: bool,
    failed: bool,
}

impl State {
    fn drain(&mut self) -> bool {
        let mut buffer = [0; 8192];
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => {
                    self.seen = true;
                    let mut stderr = std::io::stderr().lock();
                    if stderr
                        .write_all(&buffer[..n])
                        .and_then(|()| stderr.flush())
                        .is_err()
                    {
                        self.failed = true;
                        // Release the child's writer even when our stderr sink
                        // is gone. Keeping a full unread socket would deadlock
                        // a solver before it can produce stdout or exit.
                        let _ = self.stream.shutdown(Shutdown::Read);
                        self.eof = true;
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.failed = true;
                    let _ = self.stream.shutdown(Shutdown::Read);
                    self.eof = true;
                    break;
                }
            }
        }
        !self.failed
    }
}

pub(super) struct Monitor {
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Monitor {
    pub(super) fn new() -> std::io::Result<(Self, Stdio)> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        let state = Arc::new(Mutex::new(State {
            stream: reader,
            seen: false,
            eof: false,
            failed: false,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&state);
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("sail-z3-stderr".to_owned())
            .spawn(move || {
                while !stopping.load(Ordering::SeqCst) {
                    let ended = match shared.lock() {
                        Ok(mut state) => !state.drain() || state.eof,
                        Err(_) => true,
                    };
                    if ended {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })?;
        Ok((
            Self {
                state,
                stop,
                thread: Some(thread),
            },
            Stdio::from(OwnedFd::from(writer)),
        ))
    }

    /// Drain bytes already written before forwarding the next stdout chunk.
    /// A separate pump also prevents a full stderr socket blocking the solver.
    pub(super) fn flush(&self) -> bool {
        self.state.lock().is_ok_and(|mut state| state.drain())
    }

    pub(super) fn failed(&self) -> bool {
        self.state.lock().map_or(true, |state| state.failed)
    }

    /// Only EOF with no stderr bytes and no forwarding error permits caching.
    pub(super) fn quiet(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|mut state| state.drain() && state.eof && !state.seen)
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
