//! Owned, bounded Unix socket fixtures; failed assertions retire their actors.
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Fixture I/O has an owned cancellation flag and a finite deadline. Keeping
/// the socket nonblocking avoids relying on cross-thread shutdown to wake a
/// Darwin recv call; that call has remained blocked despite `SO_RCVTIMEO`.
pub(crate) struct Connection {
    stream: UnixStream,
    stop: Arc<AtomicBool>,
    read_timeout: Cell<Option<Duration>>,
    write_timeout: Option<Duration>,
}
impl Connection {
    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if timeout == Some(Duration::ZERO) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "zero socket timeout",
            ));
        }
        self.read_timeout.set(timeout);
        Ok(())
    }

    fn transfer(
        &self,
        timeout: Option<Duration>,
        retiring: impl FnOnce() -> io::Result<usize>,
        mut operation: impl FnMut(&UnixStream) -> io::Result<usize>,
    ) -> io::Result<usize> {
        let started = Instant::now();
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return retiring();
            }
            match operation(&self.stream) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let remaining = timeout
                        .unwrap_or(Duration::from_secs(2))
                        .saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "fixture I/O expired",
                        ));
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(5)));
                }
                result => return result,
            }
        }
    }
}
impl Read for Connection {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.transfer(
            self.read_timeout.get(),
            || Ok(0),
            |mut stream| stream.read(bytes),
        )
    }
}
impl Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.transfer(
            self.write_timeout,
            || Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture retired")),
            |mut stream| stream.write(bytes),
        )
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct Server {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub(crate) fn spawn(
        listener: UnixListener,
        requests: usize,
        mut handle: impl FnMut(&mut Connection) + Send + 'static,
    ) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            for _ in 0..requests {
                let started = Instant::now();
                let stream = loop {
                    if worker_stop.load(Ordering::SeqCst) {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            assert!(
                                started.elapsed() < Duration::from_secs(2),
                                "fixture accept expired"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => unreachable!("fixture accept failed: {error}"),
                    }
                };
                assert!(stream.set_nonblocking(true).is_ok());
                let mut stream = Connection {
                    stream,
                    stop: Arc::clone(&worker_stop),
                    read_timeout: Cell::new(Some(Duration::from_secs(2))),
                    write_timeout: Some(Duration::from_secs(2)),
                };
                if worker_stop.load(Ordering::SeqCst) {
                    return;
                }
                handle(&mut stream);
            }
        });
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    fn retire(&mut self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        self.worker
            .take()
            .is_none_or(|worker| worker.join().is_ok())
    }
    pub(crate) fn finish(mut self) -> bool {
        self.retire()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}

pub(crate) struct Thread {
    worker: Option<std::thread::JoinHandle<()>>,
    cleanup: Option<Box<dyn FnOnce()>>,
}
impl Thread {
    pub(crate) fn spawn(
        work: impl FnOnce() + Send + 'static,
        cleanup: impl FnOnce() + 'static,
    ) -> Self {
        Self {
            worker: Some(std::thread::spawn(work)),
            cleanup: Some(Box::new(cleanup)),
        }
    }
    fn retire(&mut self) -> bool {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
        self.worker
            .take()
            .is_none_or(|worker| worker.join().is_ok())
    }
    pub(crate) fn finish(mut self) -> bool {
        self.retire()
    }
}
impl Drop for Thread {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_open_silent_peer_times_out_and_retired_io_returns_without_peer_shutdown() {
        let (stream, mut peer) = UnixStream::pair().unwrap_or_else(|error| unreachable!("{error}"));
        assert!(stream.set_nonblocking(true).is_ok());
        let stop = Arc::new(AtomicBool::new(false));
        let mut connection = Connection {
            stream,
            stop: Arc::clone(&stop),
            read_timeout: Cell::new(Some(Duration::from_millis(20))),
            write_timeout: Some(Duration::from_millis(20)),
        };
        let started = Instant::now();
        assert_eq!(
            connection.read(&mut [0]).map_err(|error| error.kind()),
            Err(io::ErrorKind::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(connection.read(&mut [0]).ok(), Some(0));
        assert_eq!(
            connection.write(b"retired").map_err(|error| error.kind()),
            Err(io::ErrorKind::BrokenPipe)
        );
        drop(connection);
        assert_eq!(peer.read(&mut [0]).ok(), Some(0));
    }

    #[test]
    fn early_fixture_retirement_closes_accept_and_active_reads_before_join() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "sail-mcp-fixture-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap_or_else(|error| unreachable!("{error}"));
        let server = Server::spawn(listener, 1, |_| unreachable!("no client connected"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let started = Instant::now();
        drop(server);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(std::fs::remove_file(&path).is_ok());
        let listener = UnixListener::bind(&path).unwrap_or_else(|error| unreachable!("{error}"));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let server = Server::spawn(listener, 1, move |stream| {
            assert!(ready_tx.send(()).is_ok());
            assert_eq!(stream.read(&mut [0]).ok(), Some(0));
        })
        .unwrap_or_else(|error| unreachable!("{error}"));
        let mut client = UnixStream::connect(&path).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .is_ok()
        );
        assert!(ready_rx.recv_timeout(Duration::from_secs(2)).is_ok());
        let started = Instant::now();
        drop(server);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(client.read(&mut [0]).ok(), Some(0));
        assert!(std::fs::remove_file(path).is_ok());
    }
}
