//! Owned, bounded Unix socket fixtures; failed assertions retire their actors.
use std::io;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct Server {
    stop: Arc<AtomicBool>,
    active: Arc<Mutex<Option<UnixStream>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub(crate) fn spawn(
        listener: UnixListener,
        requests: usize,
        mut handle: impl FnMut(&mut UnixStream) + Send + 'static,
    ) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(Mutex::new(None));
        let (worker_stop, worker_active) = (Arc::clone(&stop), Arc::clone(&active));
        let worker = std::thread::spawn(move || {
            for _ in 0..requests {
                let started = Instant::now();
                let mut stream = loop {
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
                assert!(stream.set_nonblocking(false).is_ok());
                assert!(
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .is_ok()
                );
                assert!(
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .is_ok()
                );
                let copy = stream
                    .try_clone()
                    .unwrap_or_else(|error| unreachable!("{error}"));
                *worker_active
                    .lock()
                    .unwrap_or_else(|error| unreachable!("{error}")) = Some(copy);
                if worker_stop.load(Ordering::SeqCst) {
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
                handle(&mut stream);
                if let Ok(mut active) = worker_active.lock() {
                    *active = None;
                }
            }
        });
        Ok(Self {
            stop,
            active,
            worker: Some(worker),
        })
    }
    fn retire(&mut self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut active) = self.active.lock()
            && let Some(stream) = active.take()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
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
    use std::io::Read as _;
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
