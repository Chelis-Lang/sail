//! Complete final driver stdout through Cargo before allowing the driver to exit.
//!
//! Stderr always uses its original pipe, including Cargo's diagnostic rendering.
//! The private socket carries acknowledgments only; it never carries user bytes
//! or authorizes metadata, verdicts, freshness or proof reuse.

use std::io::{self, Write};

/// Private managed plumbing, excluded from compiler/proof environment keys.
pub(crate) const ENV: &str = "SAIL_MANAGED_VISIBLE_OUTPUT";

/// The original destination of a driver write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

/// Preserve the original pipe, acknowledging only final driver stdout.
pub(crate) fn emit(kind: Stream, bytes: &[u8]) -> io::Result<()> {
    match kind {
        Stream::Stdout => {
            #[cfg(unix)]
            if let Some(path) = super::transport::var(ENV) {
                return unix::send(std::path::Path::new(&path), bytes, &mut io::stdout());
            }
            io::stdout()
                .write_all(bytes)
                .and_then(|()| io::stdout().flush())
        }
        Stream::Stderr => io::stderr()
            .write_all(bytes)
            .and_then(|()| io::stderr().flush()),
    }
}

#[cfg(unix)]
pub(crate) use unix::Receiver;

/// Exercise the complete production sender through an owned original-pipe writer.
#[cfg(all(test, unix))]
pub(crate) fn send_test(
    path: &std::path::Path,
    bytes: &[u8],
    writer: &mut dyn Write,
) -> io::Result<()> {
    unix::send(path, bytes, writer)
}

#[cfg(unix)]
mod unix {
    use std::fs::{File, OpenOptions, TryLockError};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::DirBuilderExt as _;
    use std::os::unix::fs::FileTypeExt as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
    use std::os::unix::net::UnixDatagram;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(30);
    const LOCK: &str = "writer.lock";
    const CAPABILITY: &str = "capability";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Client(PathBuf);
    impl Drop for Client {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn open_private(path: &Path) -> io::Result<File> {
        let file = File::from(
            rustix::fs::open(
                path,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(io::Error::other(
                "managed output file is not private and regular",
            ));
        }
        Ok(file)
    }

    fn prefix(parent: &Path) -> io::Result<Vec<u8>> {
        let mut capability = String::new();
        open_private(&parent.join(CAPABILITY))?
            .take(65)
            .read_to_string(&mut capability)?;
        if capability.len() != 64
            || !capability
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(io::Error::other("invalid managed output capability"));
        }
        Ok(format!("[[sail-stdout:{capability}:").into_bytes())
    }

    pub(super) fn send(path: &Path, bytes: &[u8], writer: &mut dyn Write) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing output directory"))?;
        let prefix = prefix(parent)?;
        if bytes.windows(prefix.len()).any(|window| window == prefix) {
            return Err(io::Error::other(
                "driver stdout contains its private completion marker",
            ));
        }
        let lock = open_private(&parent.join(LOCK))?;
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
        let name = format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let client = Client(parent.join(&name));
        let socket = UnixDatagram::bind(&client.0)?;
        socket.connect(path)?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        let mut marker = prefix;
        marker.extend_from_slice(name.as_bytes());
        marker.extend_from_slice(b"]]\n");
        // The bytes and completion travel through the SAME original Cargo pipe.
        // Complete the original final record exactly as Cargo does at EOF.
        // Keep valid Cargo JSON separate from the private marker so its
        // original parsing/filtering cannot change when the marker is added.
        writer.write_all(bytes)?;
        if !bytes.ends_with(b"\n") {
            writer.write_all(b"\n")?;
        }
        writer.write_all(&marker)?;
        writer.flush()?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "output delivery deadline"))?;
        socket.set_read_timeout(Some(remaining))?;
        let mut ack = [0; 2];
        if socket.recv(&mut ack)? != 1 || ack[0] != 1 {
            return Err(io::Error::other("managed stdout was not acknowledged"));
        }
        Ok(())
    }

    /// An acknowledgment is created only after Cargo forwards a complete marker.
    pub(crate) struct Acknowledgment {
        peer: PathBuf,
        socket: UnixDatagram,
        acknowledged: bool,
    }
    impl Acknowledgment {
        pub(crate) fn acknowledge(mut self) -> io::Result<()> {
            if self.socket.send_to(&[1], &self.peer)? != 1 {
                return Err(io::Error::other("partial stdout acknowledgment"));
            }
            self.acknowledged = true;
            Ok(())
        }
    }
    impl Drop for Acknowledgment {
        fn drop(&mut self) {
            if !self.acknowledged {
                let _ = self.socket.send_to(&[0], &self.peer);
            }
        }
    }

    /// One original Cargo stdout record with only this run's marker removed.
    pub(crate) struct Decoded {
        pub(crate) bytes: Vec<u8>,
        pub(crate) completion: Option<Acknowledgment>,
    }

    /// Owned acknowledgment endpoint and fresh, private marker capability.
    pub(crate) struct Receiver {
        directory: PathBuf,
        path: PathBuf,
        socket: UnixDatagram,
        writer: File,
        capability: File,
        prefix: Vec<u8>,
    }
    impl Receiver {
        pub(crate) fn bind() -> io::Result<Self> {
            use sha2::Digest as _;
            let directory = Path::new("/tmp").join(format!(
                "sail-output-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
            let result = (|| {
                let path = directory.join("output.sock");
                let socket = UnixDatagram::bind(&path)?;
                socket.set_write_timeout(Some(TIMEOUT))?;
                let writer = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(directory.join(LOCK))?;
                let mut capability = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(directory.join(CAPABILITY))?;
                let mut entropy = [0; 32];
                File::open("/dev/urandom")?.read_exact(&mut entropy)?;
                write!(capability, "{:x}", sha2::Sha256::digest(entropy))?;
                capability.flush()?;
                let prefix = prefix(&directory)?;
                Ok(Self {
                    directory: directory.clone(),
                    path,
                    socket,
                    writer,
                    capability,
                    prefix,
                })
            })();
            if result.is_err() {
                for name in ["output.sock", LOCK, CAPABILITY] {
                    let _ = std::fs::remove_file(directory.join(name));
                }
                let _ = std::fs::remove_dir(directory);
            }
            result
        }
        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
        pub(crate) fn decode(&self, line: &[u8]) -> io::Result<Decoded> {
            let Some(start) = line
                .windows(self.prefix.len())
                .position(|window| window == self.prefix)
            else {
                return Ok(Decoded {
                    bytes: line.to_vec(),
                    completion: None,
                });
            };
            let marker = &line[start + self.prefix.len()..];
            let name = marker
                .strip_suffix(b"]]\n")
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .filter(|name| name.len() <= 41)
                .and_then(|name| {
                    let (pid, sequence) = name.split_once('-')?;
                    (pid.parse::<u32>().ok()? != 0 && sequence.parse::<u64>().is_ok())
                        .then_some(name)
                })
                .ok_or_else(|| io::Error::other("invalid driver stdout completion marker"))?;
            let peer = self.directory.join(name);
            if !std::fs::symlink_metadata(&peer)?.file_type().is_socket() {
                return Err(io::Error::other("stdout completion peer is not a socket"));
            }
            let mut bytes = line[..start].to_vec();
            if !bytes.is_empty() {
                // Cargo adds a newline to a final unterminated stdout record.
                // Preserve that observed original-pipe byte when removing the marker.
                bytes.push(b'\n');
            }
            Ok(Decoded {
                bytes,
                completion: Some(Acknowledgment {
                    peer,
                    socket: self.socket.try_clone()?,
                    acknowledged: false,
                }),
            })
        }
        fn remove_file(&self, name: &str, expected: &File) -> io::Result<()> {
            let path = self.directory.join(name);
            let actual = std::fs::symlink_metadata(&path)?;
            let expected = expected.metadata()?;
            if !actual.is_file() || actual.dev() != expected.dev() || actual.ino() != expected.ino()
            {
                return Err(io::Error::other("owned output file changed"));
            }
            std::fs::remove_file(path)
        }
        fn close(&mut self) -> io::Result<()> {
            // Refuse even clients whose marker never reached the original pipe.
            // Unknown non-socket files are left in place and fail final removal.
            let clients = (|| {
                for entry in std::fs::read_dir(&self.directory)? {
                    let entry = entry?;
                    if entry.path() == self.path || !entry.file_type()?.is_socket() {
                        continue;
                    }
                    let _ = self.socket.send_to(&[0], entry.path());
                    match std::fs::remove_file(entry.path()) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            })();
            let socket = std::fs::remove_file(&self.path);
            let lock = self.remove_file(LOCK, &self.writer);
            let capability = self.remove_file(CAPABILITY, &self.capability);
            let directory = std::fs::remove_dir(&self.directory);
            clients.and(socket).and(lock).and(capability).and(directory)
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
