//! Optional bounded local sysroot handoff endpoint (ADR0031).
//! One accepted connection and one maintenance owner are serviced at a time;
//! there is no verifier worker, HTTP route or shared-store export here.

use std::io;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sail_api::sysroot::{Bounded, Request, Response, read_frame, socket_path, write_frame};
use sail_verus::managed::SysrootIndex;

struct SocketFile {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl Drop for SocketFile {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.path).is_ok_and(|m| {
            m.dev() == self.device && m.ino() == self.inode && m.file_type().is_socket()
        }) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// A bound optional private local endpoint; construction does not hash content.
pub struct Server {
    listener: UnixListener,
    _file: SocketFile,
}

/// Fully owned endpoint/maintenance lifetime. Shutdown never publishes a builder
/// after cancellation and joins owned workers outside every foreground RPC.
pub struct Handle {
    index: Arc<SysrootIndex>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

fn current_uid() -> Option<u32> {
    // Linux-only endpoint: the bounded kernel status report avoids trusting
    // USER/HOME, a path owner unrelated to this process or an unsafe uid call.
    use std::io::Read as _;
    let mut status = String::new();
    std::fs::File::open("/proc/self/status")
        .ok()?
        .take(16385)
        .read_to_string(&mut status)
        .ok()?;
    (status.len() <= 16384).then_some(())?;
    let line = status.lines().find_map(|line| line.strip_prefix("Uid:"))?;
    let ids = line
        .split_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (ids.len() == 4 && ids.iter().all(|id| *id == ids[0])).then_some(ids[0])
}

/// Binds the private endpoint adjacent to an already bound local cache socket.
///
/// # Errors
/// Refuses unsupported platforms, unsafe existing files and active endpoints.
/// Optional service failure must leave original local hashing available.
pub fn bind(cache_socket: &Path) -> io::Result<Server> {
    if !cfg!(target_os = "linux") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SAIL0835: guarded sysroot endpoint unsupported",
        ));
    }
    let cache = std::fs::symlink_metadata(cache_socket)?;
    if !cache.file_type().is_socket()
        || cache.mode() & 0o077 != 0
        || current_uid() != Some(cache.uid())
    {
        return Err(io::Error::other(
            "SAIL0835: sysroot endpoint needs a private local cache socket",
        ));
    }
    let path = socket_path(cache_socket);
    if let Ok(previous) = std::fs::symlink_metadata(&path) {
        if !previous.file_type().is_socket()
            || previous.uid() != cache.uid()
            || previous.mode() & 0o077 != 0
        {
            return Err(io::Error::other(
                "SAIL0835: existing sysroot endpoint is not owned private socket",
            ));
        }
        let probe = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        let error = match probe
            .connect_timeout(&socket2::SockAddr::unix(&path)?, sail_api::sysroot::BUDGET)
        {
            Ok(()) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "SAIL0835: sysroot endpoint already active",
                ));
            }
            Err(error) => error,
        };
        if error.kind() != io::ErrorKind::ConnectionRefused {
            return Err(error);
        }
        let current = std::fs::symlink_metadata(&path)?;
        if current.dev() != previous.dev() || current.ino() != previous.ino() {
            return Err(io::Error::other(
                "SAIL0835: stale sysroot endpoint changed during bind",
            ));
        }
        std::fs::remove_file(&path)?;
    }
    let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
    socket.bind(&socket2::SockAddr::unix(&path)?)?;
    let metadata = std::fs::symlink_metadata(&path)?;
    let file = SocketFile {
        path,
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    std::fs::set_permissions(&file.path, std::fs::Permissions::from_mode(0o600))?;
    // Explicitly finite kernel backlog; only one accepted connection/body is
    // ever owned by the endpoint thread. No connection thread is spawned.
    socket.listen(4)?;
    socket.set_nonblocking(true)?;
    let descriptor: std::os::fd::OwnedFd = socket.into();
    Ok(Server {
        listener: UnixListener::from(descriptor),
        _file: file,
    })
}

impl Server {
    /// Starts the endpoint and its single independent maintenance worker.
    ///
    /// # Errors
    /// Reports thread/owner creation errors without changing proof behavior.
    pub fn spawn(self) -> io::Result<Handle> {
        let index = Arc::new(SysrootIndex::new()?);
        let service = Arc::clone(&index);
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("sail-sysroot-endpoint".into())
            .spawn(move || self.run(&cancelled, &service))?;
        Ok(Handle {
            index,
            stop,
            worker: Some(worker),
        })
    }
    fn run(&self, stop: &AtomicBool, index: &SysrootIndex) {
        // Keep the socket's inode ownership alive for the whole endpoint.
        while !stop.load(Ordering::Acquire) {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    let started = Instant::now();
                    let response = (|| {
                        let mut transport = Bounded::new(&mut stream, started)?;
                        let request: Request = read_frame(&mut transport)?;
                        let response = match request {
                            Request::Lookup { scope } if scope.valid() => {
                                Response::Lookup(index.lookup(&scope))
                            }
                            Request::Prepare { scope } if scope.valid() => {
                                Response::Prepared(index.prepare(&scope))
                            }
                            _ => return Err(io::Error::other("SAIL0833: invalid sysroot scope")),
                        };
                        if stop.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        write_frame(&mut transport, &response)
                    })();
                    // Refusal, timeout and disconnect are cache misses; never
                    // echo path/runtime inputs or secrets into daemon logs.
                    let _ = response;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    }
}
impl Handle {
    /// Cancels and joins this endpoint and its maintenance owner. All join work
    /// is outside lookup/verification RPCs; cancellation is cooperative between
    /// bounded construction operations, not an interruptible kernel-I/O promise.
    pub fn shutdown(&mut self) {
        self.index.cancel();
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.index.shutdown();
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    fn fixture() -> (PathBuf, UnixListener) {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "sail-sysroot-endpoint-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir(&directory).is_ok());
        let path = directory.join("cache.sock");
        let listener = UnixListener::bind(&path).unwrap_or_else(|_| unreachable!("cache fixture"));
        assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).is_ok());
        (directory, listener)
    }
    #[test]
    fn private_endpoint_refuses_foreign_files_and_active_replacement() {
        let (root, cache) = fixture();
        let cache_path = root.join("cache.sock");
        let endpoint = socket_path(&cache_path);
        assert!(std::fs::write(&endpoint, b"foreign file").is_ok());
        assert!(bind(&cache_path).is_err());
        assert_eq!(
            std::fs::read(&endpoint).unwrap_or_default(),
            b"foreign file"
        );
        assert!(std::fs::remove_file(&endpoint).is_ok());
        let server = bind(&cache_path).unwrap_or_else(|_| unreachable!("server fixture"));
        assert!(bind(&cache_path).is_err());
        drop(server);
        assert!(!endpoint.exists());
        drop(cache);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn partial_peer_shutdown_joins_endpoint_and_maintenance_without_late_socket_unlink() {
        let (root, cache) = fixture();
        let cache_path = root.join("cache.sock");
        let endpoint = socket_path(&cache_path);
        let mut service = bind(&cache_path)
            .and_then(Server::spawn)
            .unwrap_or_else(|_| unreachable!("service fixture"));
        let mut peer =
            UnixStream::connect(&endpoint).unwrap_or_else(|_| unreachable!("peer fixture"));
        assert!(peer.write_all(&100u32.to_be_bytes()).is_ok());
        let clock = Instant::now();
        service.shutdown();
        assert!(clock.elapsed() < Duration::from_millis(250));
        assert!(!endpoint.exists());
        assert!(std::fs::write(&endpoint, b"replacement").is_ok());
        drop(service);
        assert_eq!(std::fs::read(&endpoint).unwrap_or_default(), b"replacement");
        drop(peer);
        drop(cache);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
