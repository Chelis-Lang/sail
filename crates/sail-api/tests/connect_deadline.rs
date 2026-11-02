//! Real listener backlog controls for the clients' connection deadline.
#![forbid(unsafe_code)]

use std::io;
use std::os::unix::fs::DirBuilderExt as _;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use sail_api::client::{Client, connect_stream};
use socket2::{Domain, SockAddr, Socket, Type};

struct Endpoint(PathBuf);
impl Endpoint {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = PathBuf::from("/tmp").join(format!(
            "sail-connect-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&root)?;
        Ok(Self(root))
    }
    fn socket(&self) -> PathBuf {
        self.0.join("s")
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn healthy_connection_keeps_the_io_timeouts_and_rejects_zero() -> io::Result<()> {
    let endpoint = Endpoint::new()?;
    let listener = UnixListener::bind(endpoint.socket())?;
    let timeout = Duration::from_millis(100);
    let client = connect_stream(&endpoint.socket(), timeout)?;
    let (_peer, _) = listener.accept()?;
    assert_eq!(client.read_timeout()?, Some(timeout));
    assert_eq!(client.write_timeout()?, Some(timeout));
    assert_eq!(
        connect_stream(&endpoint.socket(), Duration::ZERO)
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput)
    );
    Ok(())
}

#[test]
fn full_listener_backlog_cannot_block_the_cache_client_connect() -> io::Result<()> {
    let endpoint = Endpoint::new()?;
    let address = SockAddr::unix(endpoint.socket())?;
    let listener = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    listener.bind(&address)?;
    listener.listen(1)?;
    let mut queued = Vec::new();
    let mut full = false;
    for _ in 0..32 {
        let peer = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        peer.set_nonblocking(true)?;
        match peer.connect(&address) {
            Ok(()) => queued.push(peer),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionRefused
                ) =>
            {
                full = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    assert!(full && !queued.is_empty(), "listener was not saturated");
    let path = endpoint.socket();
    let (send, receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = Client::connect_with_timeout(&path, Duration::from_millis(25));
        let _ = send.send(result.map(|_| ()));
    });
    let result = receive.recv_timeout(Duration::from_secs(1));
    // Unblock the original implementation before asserting, so its regression
    // fails without leaving a blocked test thread or hanging the whole gate.
    drop(listener);
    drop(queued);
    worker
        .join()
        .map_err(|_| io::Error::other("connection worker failed"))?;
    let outcome = result.map_err(|error| io::Error::other(error.to_string()))?;
    assert!(outcome.is_err(), "a saturated listener was admitted");
    Ok(())
}
