//! Acknowledged visible driver output, separate from Cargo's JSON pipe.
//!
//! Only the runner's public writer acknowledges delivery. A driver therefore
//! cannot exit before its visible bytes, and subsequent Cargo errors cannot
//! overtake them. A per-emit lock and final-chunk marker preserve whole writes
//! across datagrams, including concurrent drivers. Frames never authorize
//! metadata, verdicts or proof reuse.

use std::io::{self, Write};

/// Private managed plumbing, excluded from compiler/proof environment keys.
pub(crate) const ENV: &str = "SAIL_MANAGED_VISIBLE_OUTPUT";

/// The original public destination of an exact driver write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

/// Write exact bytes, waiting for public delivery in a managed invocation.
pub(crate) fn emit(kind: Stream, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    if let Some(path) = std::env::var_os(ENV) {
        return unix::send(std::path::Path::new(&path), kind, bytes);
    }
    match kind {
        Stream::Stdout => io::stdout()
            .write_all(bytes)
            .and_then(|()| io::stdout().flush()),
        Stream::Stderr => io::stderr()
            .write_all(bytes)
            .and_then(|()| io::stderr().flush()),
    }
}

#[cfg(unix)]
pub(crate) use unix::{Frame, Receiver};

/// Exercise the production sender without changing process-global environment.
#[cfg(all(test, unix))]
pub(crate) fn send_test(path: &std::path::Path, kind: Stream, bytes: &[u8]) -> io::Result<()> {
    unix::send(path, kind, bytes)
}

#[cfg(unix)]
mod unix {
    use super::Stream;
    use std::fs::{File, OpenOptions, TryLockError};
    use std::io;
    use std::os::unix::fs::DirBuilderExt as _;
    use std::os::unix::fs::FileTypeExt as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    use std::os::unix::net::UnixDatagram;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    const PAYLOAD: usize = 1024;
    const TIMEOUT: Duration = Duration::from_secs(30);
    const LOCK: &str = "writer.lock";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Client(PathBuf);
    impl Drop for Client {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn configure(socket: &UnixDatagram) -> io::Result<()> {
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))
    }

    pub(super) fn send(path: &Path, kind: Stream, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing output directory"))?;
        let lock = File::from(
            rustix::fs::open(
                parent.join(LOCK),
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        if !lock.metadata()?.is_file() {
            return Err(io::Error::other(
                "managed output lock is not a regular file",
            ));
        }
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "managed output writer remained busy",
                    ));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
        let client = Client(parent.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let socket = UnixDatagram::bind(&client.0)?;
        configure(&socket)?;
        socket.connect(path)?;
        let mut chunks = bytes.chunks(PAYLOAD).peekable();
        while let Some(bytes) = chunks.next() {
            let mut frame = Vec::with_capacity(bytes.len() + 1);
            let stream = match kind {
                Stream::Stdout => 1,
                Stream::Stderr => 2,
            };
            frame.push(stream | if chunks.peek().is_none() { 0x80 } else { 0 });
            frame.extend_from_slice(bytes);
            if socket.send(&frame)? != frame.len() {
                return Err(io::Error::other("partial managed output frame"));
            }
            let mut ack = [0; 2];
            if socket.recv(&mut ack)? != 1 || ack[0] != 1 {
                return Err(io::Error::other("managed output was not acknowledged"));
            }
        }
        Ok(())
    }

    /// Exact visible bytes and the private peer waiting for their delivery.
    pub(crate) struct Frame {
        pub kind: Stream,
        pub bytes: Vec<u8>,
        pub final_chunk: bool,
        peer: PathBuf,
        socket: UnixDatagram,
        acknowledged: bool,
    }
    impl Frame {
        pub(crate) fn sender(&self) -> &Path {
            &self.peer
        }
        pub(crate) fn acknowledge(mut self) -> io::Result<()> {
            if self.socket.send_to(&[1], &self.peer)? != 1 {
                return Err(io::Error::other("partial managed output acknowledgment"));
            }
            self.acknowledged = true;
            Ok(())
        }
    }
    impl Drop for Frame {
        fn drop(&mut self) {
            if !self.acknowledged {
                let _ = self.socket.send_to(&[0], &self.peer);
            }
        }
    }

    /// One owned endpoint; the receiver never writes public stdout or stderr.
    pub(crate) struct Receiver {
        directory: PathBuf,
        path: PathBuf,
        socket: UnixDatagram,
        writer: File,
        stopping: Arc<AtomicBool>,
        thread: Option<JoinHandle<io::Result<()>>>,
    }
    impl Receiver {
        pub(crate) fn bind() -> io::Result<Self> {
            // Short even for deep caches; existing names grant no ownership.
            let directory = Path::new("/tmp").join(format!(
                "sail-output-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
            let path = directory.join("output.sock");
            let socket = match UnixDatagram::bind(&path) {
                Ok(socket) => socket,
                Err(error) => {
                    let _ = std::fs::remove_dir(&directory);
                    return Err(error);
                }
            };
            let writer = match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(directory.join(LOCK))
            {
                Ok(writer) => writer,
                Err(error) => {
                    let _ = std::fs::remove_file(&path);
                    let _ = std::fs::remove_dir(&directory);
                    return Err(error);
                }
            };
            let receiver = Self {
                directory,
                path,
                socket,
                writer,
                stopping: Arc::new(AtomicBool::new(false)),
                thread: None,
            };
            configure(&receiver.socket)?;
            Ok(receiver)
        }
        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
        pub(crate) fn start(
            &mut self,
            forward: impl Fn(io::Result<Frame>) -> bool + Send + 'static,
        ) -> io::Result<()> {
            if self.thread.is_some() {
                return Err(io::Error::other("output receiver already started"));
            }
            let socket = self.socket.try_clone()?;
            socket.set_read_timeout(Some(Duration::from_millis(100)))?;
            let stopping = Arc::clone(&self.stopping);
            self.thread = Some(
                std::thread::Builder::new()
                    .name("sail-visible-output".into())
                    .spawn(move || {
                        let mut bytes = [0; PAYLOAD + 2];
                        while !stopping.load(Ordering::Acquire) {
                            let (count, peer) = match socket.recv_from(&mut bytes) {
                                Ok(received) => received,
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        io::ErrorKind::WouldBlock
                                            | io::ErrorKind::TimedOut
                                            | io::ErrorKind::Interrupted
                                    ) =>
                                {
                                    continue;
                                }
                                Err(error) => {
                                    forward(Err(error));
                                    break;
                                }
                            };
                            if stopping.load(Ordering::Acquire) {
                                break;
                            }
                            let received = (|| {
                                if !(2..=PAYLOAD + 1).contains(&count) {
                                    return Err(io::Error::other(
                                        "invalid managed output frame length",
                                    ));
                                }
                                let kind = match bytes[0] & 0x7f {
                                    1 => Stream::Stdout,
                                    2 => Stream::Stderr,
                                    _ => {
                                        return Err(io::Error::other(
                                            "invalid managed output stream",
                                        ));
                                    }
                                };
                                Ok(Frame {
                                    kind,
                                    final_chunk: bytes[0] & 0x80 != 0,
                                    bytes: bytes[1..count].to_vec(),
                                    peer: peer
                                        .as_pathname()
                                        .ok_or_else(|| io::Error::other("unnamed output sender"))?
                                        .to_owned(),
                                    socket: socket.try_clone()?,
                                    acknowledged: false,
                                })
                            })();
                            let failed = received.is_err();
                            if !forward(received) || failed {
                                break;
                            }
                        }
                        Ok(())
                    })?,
            );
            Ok(())
        }
        fn close(&mut self) -> io::Result<()> {
            self.stopping.store(true, Ordering::Release);
            // Wake a blocked receive. The 100ms receive bound also covers a full queue.
            if let Ok(wake) = UnixDatagram::unbound() {
                let _ = wake.set_write_timeout(Some(Duration::from_millis(100)));
                let _ = wake.send_to(&[0], &self.path);
            }
            let joined = self.thread.take().map_or(Ok(()), |thread| {
                thread
                    .join()
                    .map_err(|_| io::Error::other("output receiver failed"))?
            });
            let socket = std::fs::remove_file(&self.path);
            // On a broken public sink, waiting senders receive NACK while the
            // runner unwinds. Remove their socket names as well: a late client
            // drop tolerates an already unlinked endpoint. Unknown files are
            // left alone and make directory removal fail visibly.
            let clients = (|| {
                for entry in std::fs::read_dir(&self.directory)? {
                    let entry = entry?;
                    if entry.file_type()?.is_socket() {
                        match std::fs::remove_file(entry.path()) {
                            Ok(()) => {}
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error),
                        }
                    }
                }
                Ok(())
            })();
            let lock = (|| {
                let path = self.directory.join(LOCK);
                let actual = std::fs::symlink_metadata(&path)?;
                let expected = self.writer.metadata()?;
                if !actual.is_file()
                    || actual.dev() != expected.dev()
                    || actual.ino() != expected.ino()
                {
                    return Err(io::Error::other("owned output lock changed"));
                }
                std::fs::remove_file(path)
            })();
            let directory = std::fs::remove_dir(&self.directory);
            joined.and(socket).and(clients).and(lock).and(directory)
        }
        pub(crate) fn finish(mut self) -> io::Result<()> {
            let result = self.close();
            self.path.clear();
            result
        }
    }
    impl Drop for Receiver {
        fn drop(&mut self) {
            if !self.path.as_os_str().is_empty() {
                let _ = self.close();
            }
        }
    }
}
