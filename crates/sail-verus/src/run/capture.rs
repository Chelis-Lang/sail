//! In-memory capture completion, independent of child environment, filesystem
//! writes and process groups. The listener sends its identity before accepting
//! a completion; stale aliases never issue commands to an unrelated endpoint.

use std::collections::BTreeSet;
use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Error, Result};

/// Owned frontend directory prefix for capture-control protocol version one.
pub const CONTROL_PREFIX: &str = "capture-frontend-c1-";
/// Exact server greeting prefix, checked before a delegate sends any data.
pub const CONTROL_GREETING: &str = "sail-compiler-capture-v1 ";

pub(super) struct Server {
    directory: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<std::result::Result<Vec<String>, String>>>,
}

fn capture_error(message: impl std::fmt::Display) -> Error {
    Error::new("SAIL3018", message.to_string())
}

impl Server {
    pub(super) fn start() -> Result<Self> {
        let listener =
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(capture_error)?;
        listener.set_nonblocking(true).map_err(capture_error)?;
        let port = listener.local_addr().map_err(capture_error)?.port();
        let nonce = format!(
            "{:016x}{:016x}",
            RandomState::new().build_hasher().finish(),
            RandomState::new().build_hasher().finish()
        );
        let directory = format!("{CONTROL_PREFIX}{port}-{nonce}");
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("sail-capture-control".to_owned())
            .spawn(move || serve(&listener, &nonce, &stopping))
            .map_err(capture_error)?;
        Ok(Self {
            directory,
            stop,
            thread: Some(thread),
        })
    }
    pub(super) fn directory(&self) -> &str {
        &self.directory
    }
    pub(super) fn finish(mut self) -> Result<Vec<String>> {
        self.stop.store(true, Ordering::Release);
        self.thread
            .take()
            .ok_or_else(|| capture_error("capture control thread is absent"))?
            .join()
            .map_err(|_| capture_error("capture control thread failed"))?
            .map_err(capture_error)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(
    listener: &TcpListener,
    nonce: &str,
    stop: &Arc<AtomicBool>,
) -> std::result::Result<Vec<String>, String> {
    let mut workers: Vec<JoinHandle<std::result::Result<String, String>>> = Vec::new();
    let mut error = None;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if workers.len() >= 4096
                    || workers.iter().filter(|w| !w.is_finished()).count() >= 64
                {
                    error = Some("capture control capacity exceeded".to_owned());
                    break;
                }
                let greeting = format!("{CONTROL_GREETING}{nonce}\n");
                match worker(stream, greeting, Arc::clone(stop)) {
                    Ok(worker) => workers.push(worker),
                    Err(failure) => {
                        error = Some(failure);
                        break;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => {
                error = Some(format!("capture control accept: {e}"));
                break;
            }
        }
    }
    stop.store(true, Ordering::Release);
    let mut names = BTreeSet::new();
    for worker in workers {
        match worker.join() {
            Ok(Ok(name)) => {
                if !names.insert(name) {
                    error.get_or_insert_with(|| "duplicate capture completion".to_owned());
                }
            }
            Ok(Err(failure)) => {
                error.get_or_insert(failure);
            }
            Err(_) => {
                error.get_or_insert_with(|| "capture control worker failed".to_owned());
            }
        }
    }
    error.map_or_else(|| Ok(names.into_iter().collect()), Err)
}

fn worker(
    mut stream: TcpStream,
    greeting: String,
    stop: Arc<AtomicBool>,
) -> std::result::Result<JoinHandle<std::result::Result<String, String>>, String> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name("sail-capture-request".to_owned())
        .spawn(move || {
            stream
                .write_all(greeting.as_bytes())
                .map_err(|e| format!("capture greeting: {e}"))?;
            let mut frame = Vec::new();
            let mut bytes = [0u8; 128];
            loop {
                match stream.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(count) => {
                        frame.extend_from_slice(&bytes[..count]);
                        if frame.len() > 128 {
                            return Err("oversized capture completion".to_owned());
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        if stop.load(Ordering::Acquire) {
                            return Err(
                                "compiler capture remained incomplete when Cargo ended".to_owned()
                            );
                        }
                    }
                    Err(e) => return Err(format!("capture completion: {e}")),
                }
            }
            let frame = std::str::from_utf8(&frame).map_err(|e| e.to_string())?;
            let name = frame
                .strip_prefix("complete:")
                .and_then(|s| s.strip_suffix('\n'))
                .filter(|s| valid_name(s))
                .ok_or_else(|| "compiler capture did not complete".to_owned())?;
            stream
                .write_all(b"ok\n")
                .map_err(|e| format!("capture acknowledgment: {e}"))?;
            Ok(name.to_owned())
        })
        .map_err(|e| format!("cannot start capture control worker: {e}"))
}

fn valid_name(name: &str) -> bool {
    let Some((time, pid)) = name.split_once('-') else {
        return false;
    };
    time.len() == 32
        && time.bytes().all(|b| b.is_ascii_digit())
        && !pid.is_empty()
        && pid.len() <= 10
        && pid.bytes().all(|b| b.is_ascii_digit())
}

/// An invocation-owned connection to the differential harness. Its lifetime
/// starts before any metadata or environment access. Closing it without a
/// completion makes the owning run an infrastructure refusal.
pub struct CaptureControl(TcpStream);

impl CaptureControl {
    /// Connects when this executable belongs to a staged capture frontend.
    /// Standalone delegate fixtures have no parent control connection.
    ///
    /// # Errors
    /// `SAIL3018` for an invalid staged identity, unavailable parent, or a
    /// greeting that does not match this exact staging namespace.
    pub fn connect() -> Result<Option<Self>> {
        let executable = std::env::current_exe().map_err(capture_error)?;
        Self::at(&executable)
    }

    fn at(executable: &std::path::Path) -> Result<Option<Self>> {
        let Some(directory) = executable
            .parent()
            .and_then(std::path::Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
        else {
            return Err(capture_error("cannot identify capture frontend directory"));
        };
        let Some(identity) = directory.strip_prefix(CONTROL_PREFIX) else {
            return Ok(None);
        };
        let (port, nonce) = identity
            .split_once('-')
            .ok_or_else(|| capture_error("invalid capture control identity"))?;
        if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(capture_error("invalid capture control nonce"));
        }
        let port = port.parse::<u16>().map_err(capture_error)?;
        let address = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port));
        let mut stream =
            TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(capture_error)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(capture_error)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(capture_error)?;
        // Read and verify before sending anything. A retained alias can never
        // issue a command to an unrelated process that later owns this port.
        let expected = format!("{CONTROL_GREETING}{nonce}\n");
        let mut greeting = vec![0; expected.len()];
        stream.read_exact(&mut greeting).map_err(capture_error)?;
        if greeting != expected.as_bytes() {
            return Err(capture_error("capture parent identity mismatch"));
        }
        Ok(Some(Self(stream)))
    }

    /// Acknowledges one fully written and forwarded invocation record.
    ///
    /// # Errors
    /// `SAIL3018` for an invalid record identifier or a missing parent acknowledgment.
    pub fn complete(mut self, name: &str) -> Result<()> {
        if !valid_name(name) {
            return Err(capture_error("invalid capture completion identifier"));
        }
        self.0
            .write_all(format!("complete:{name}\n").as_bytes())
            .map_err(capture_error)?;
        self.0
            .shutdown(std::net::Shutdown::Write)
            .map_err(capture_error)?;
        let mut acknowledgment = [0; 3];
        self.0
            .read_exact(&mut acknowledgment)
            .map_err(capture_error)?;
        if acknowledgment != *b"ok\n" {
            return Err(capture_error("invalid capture acknowledgment"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "assertions in bounded control-protocol tests"
)]
mod tests {
    use super::*;

    fn client(server: &Server) -> CaptureControl {
        CaptureControl::at(
            &std::path::Path::new("/tmp")
                .join(server.directory())
                .join("verus"),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn concurrent_completions_and_incomplete_connections_are_distinct() {
        let server = Server::start().unwrap();
        let mut threads = Vec::new();
        for pid in 1..=8 {
            let client = client(&server);
            threads.push(std::thread::spawn(move || {
                client.complete(&format!("{:032}-{pid}", 123)).unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(server.finish().unwrap().len(), 8);
        let server = Server::start().unwrap();
        drop(client(&server));
        assert!(server.finish().is_err());
        let server = Server::start().unwrap();
        let _held = client(&server);
        let start = std::time::Instant::now();
        assert!(server.finish().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn retained_alias_sends_nothing_to_a_reused_endpoint() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .write_all(b"sail-compiler-capture-v1 ffffffffffffffffffffffffffffffff\n")
                .unwrap();
            let mut received = Vec::new();
            stream.read_to_end(&mut received).unwrap();
            assert_eq!(received, [] as [u8; 0]);
        });
        let path = format!("/tmp/{CONTROL_PREFIX}{port}-00000000000000000000000000000000/verus");
        assert!(CaptureControl::at(std::path::Path::new(&path)).is_err());
        peer.join().unwrap();
    }
}
