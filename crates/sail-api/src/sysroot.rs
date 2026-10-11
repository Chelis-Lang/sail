//! Optional local guarded sysroot-digest handoff, separate from proof/cache keys.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Protocol version; an old endpoint can never silently grant reuse.
pub const VERSION: u32 = 1;
/// Maximum complete encoded frame, including its envelope: one MiB.
pub const MAX_FRAME: usize = 1024 * 1024;
/// Aggregate connect, write, read and decode budget for optional reuse.
pub const BUDGET: Duration = Duration::from_millis(100);
/// Maximum installed roots in one exact request.
pub const MAX_ROOTS: usize = 8;
/// Maximum guarded files in one handoff.
pub const MAX_FILES: usize = 4096;

/// Exact caller runtime selection and Linux mount namespace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Sorted, distinct absolute native sysroot paths; unsupported encodings decline reuse.
    #[serde(deserialize_with = "roots")]
    pub roots: Vec<PathBuf>,
    /// Actual canonical device/inode identities of every root, in the same order.
    #[serde(deserialize_with = "bindings")]
    pub bindings: Vec<Binding>,
    /// Actual device/inode identity of the process filesystem root.
    pub view_root: Binding,
    /// Exact pre-sysroot runtime-selection content digest.
    pub selection: String,
    /// Kernel mount-namespace link identity.
    pub namespace: String,
    /// SHA-256 of the caller's complete mount inventory.
    pub mounts: String,
}

impl Scope {
    /// Whether this scope fits the bounded, exact wire contract.
    #[must_use]
    pub fn valid(&self) -> bool {
        !self.roots.is_empty()
            && self.roots.len() <= MAX_ROOTS
            && self.roots.windows(2).all(|paths| paths[0] < paths[1])
            && self.roots.iter().all(|path| {
                path.is_absolute()
                    && path.to_str().is_some_and(|s| !s.contains('\0'))
                    && path.as_os_str().len() <= 4096
            })
            && self
                .roots
                .iter()
                .map(|p| p.as_os_str().len())
                .sum::<usize>()
                <= 32768
            && self.bindings.len() == self.roots.len()
            && self
                .bindings
                .iter()
                .zip(&self.roots)
                .all(|(b, p)| b.path == *p && b.valid())
            && self.view_root.valid()
            && self.view_root.path == Path::new("/")
            && self.view_root.canonical == Path::new("/")
            && digest(&self.selection)
            && digest(&self.mounts)
            && self.namespace.len() <= 128
            && self.namespace.starts_with("mnt:[")
            && self.namespace.ends_with(']')
    }
}

/// Exact namespace binding, combined with guarded content authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// Original absolute path.
    pub path: PathBuf,
    /// Actual canonical target path.
    pub canonical: PathBuf,
    /// Kernel device identity.
    pub device: u64,
    /// Kernel inode identity.
    pub inode: u64,
}
impl Binding {
    /// Whether both exact paths have supported bounded encodings.
    #[must_use]
    pub fn valid(&self) -> bool {
        [&self.path, &self.canonical].iter().all(|p| {
            p.is_absolute()
                && p.to_str().is_some_and(|s| !s.contains('\0'))
                && p.as_os_str().len() <= 4096
        })
    }
}

/// One file independently read under the daemon's continuous guard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Exact inventory path, with its original path encoding.
    pub path: PathBuf,
    /// Exact canonical path guarded by both owners during handoff.
    pub canonical: PathBuf,
    /// Kernel device identity; never sufficient authority on its own.
    pub device: u64,
    /// Kernel inode identity; never sufficient authority on its own.
    pub inode: u64,
    /// SHA-256 of independently guarded content reads.
    pub sha256: String,
}

/// Complete entry; caller-local guards must bracket its import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Exact echoed request scope.
    pub scope: Scope,
    /// Nonrenewable daemon owner generation, for provenance only.
    pub generation: String,
    /// Sorted complete original inventory, without metadata-only substitutions.
    #[serde(deserialize_with = "rows")]
    pub rows: Vec<Row>,
}

impl Snapshot {
    /// Refuses unsupported, incomplete, oversized or reordered inventory shapes.
    #[must_use]
    pub fn valid(&self, scope: &Scope) -> bool {
        self.scope == *scope
            && scope.valid()
            && digest(&self.generation)
            && !self.rows.is_empty()
            && self.rows.len() <= MAX_FILES
            && self.rows.windows(2).all(|rows| rows[0].path < rows[1].path)
            && self.rows.iter().all(|row| {
                [row.path.as_path(), row.canonical.as_path()]
                    .iter()
                    .all(|p| {
                        p.is_absolute()
                            && p.to_str().is_some_and(|s| !s.contains('\0'))
                            && p.as_os_str().len() <= 4096
                    })
                    && digest(&row.sha256)
            })
            && self
                .rows
                .iter()
                .map(|r| r.path.as_os_str().len() + r.canonical.as_os_str().len())
                .sum::<usize>()
                <= 512 * 1024
    }
}

fn bounded<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visitor<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> serde::de::Visitor<'de> for Visitor<T, MAX> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "at most {MAX} inventory elements")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            if sequence.size_hint().is_some_and(|size| size > MAX) {
                return Err(serde::de::Error::custom(
                    "SAIL0833: inventory count exceeds limit",
                ));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAX));
            while values.len() < MAX {
                let Some(value) = sequence.next_element()? else {
                    return Ok(values);
                };
                values.push(value);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(
                    "SAIL0833: inventory count exceeds limit",
                ));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Visitor::<T, MAX>(std::marker::PhantomData))
}
fn roots<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<PathBuf>, D::Error> {
    bounded::<D, PathBuf, MAX_ROOTS>(deserializer)
}
fn bindings<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Binding>, D::Error> {
    bounded::<D, Binding, MAX_ROOTS>(deserializer)
}
fn rows<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Row>, D::Error> {
    bounded::<D, Row, MAX_FILES>(deserializer)
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A short lookup or a bounded maintenance hint; no caller hashes are accepted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Returns an existing validated owner or immediately declines reuse.
    Lookup {
        /// Exact runtime scope.
        scope: Scope,
    },
    /// Queues independent construction after the caller's original first hash barrier.
    Prepare {
        /// Exact runtime scope.
        scope: Scope,
    },
}

/// Optional handoff response; all refusals retain original local work.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "response",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    /// Complete continuously protected inventory, or a miss.
    Lookup(Option<Box<Snapshot>>),
    /// Whether a bounded maintenance hint was accepted.
    Prepared(bool),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u32,
    body: T,
}

/// Derives the local optional endpoint without changing existing socket/key encoding.
#[must_use]
pub fn socket_path(cache: &Path) -> PathBuf {
    let mut socket = cache.as_os_str().to_owned();
    socket.push(".sysroots-v1");
    PathBuf::from(socket)
}

struct Limited<'a>(&'a mut Vec<u8>);
impl Write for Limited<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FRAME.saturating_sub(self.0.len()) {
            return Err(io::Error::other("SAIL0830: sysroot frame exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Writes one bounded versioned frame.
///
/// # Errors
/// Refuses serialization, framing limits and I/O failures.
pub fn write_frame<T: Serialize>(writer: &mut impl Write, body: &T) -> io::Result<()> {
    let mut bytes = Vec::new();
    serde_json::to_writer(
        Limited(&mut bytes),
        &Envelope {
            version: VERSION,
            body,
        },
    )
    .map_err(io::Error::other)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::other("SAIL0830: sysroot frame exceeds limit"));
    }
    writer.write_all(
        &u32::try_from(bytes.len())
            .map_err(io::Error::other)?
            .to_be_bytes(),
    )?;
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Reads a complete bounded versioned frame; trailing JSON is refused.
///
/// # Errors
/// Refuses malformed, oversized, truncated, unsupported and I/O failures.
pub fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let size = usize::try_from(u32::from_be_bytes(length)).map_err(io::Error::other)?;
    if size > MAX_FRAME {
        return Err(io::Error::other("SAIL0830: sysroot frame exceeds limit"));
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if envelope.version != VERSION {
        return Err(io::Error::other("SAIL0831: incompatible sysroot protocol"));
    }
    Ok(envelope.body)
}

#[cfg(unix)]
mod transport {
    use super::{
        BUDGET, Duration, Instant, Path, Read, Request, Response, Write, io, read_frame,
        socket_path, write_frame,
    };
    use std::os::unix::net::UnixStream;

    /// Nonblocking connected I/O with a single absolute optional-reuse budget.
    pub struct Bounded<'a> {
        stream: &'a mut UnixStream,
        started: Instant,
    }
    impl<'a> Bounded<'a> {
        /// Keeps a caller-owned stream and the original request-entry clock.
        ///
        /// # Errors
        /// Returns socket-mode errors.
        pub fn new(stream: &'a mut UnixStream, started: Instant) -> io::Result<Self> {
            stream.set_nonblocking(true)?;
            Ok(Self { stream, started })
        }
        fn check(&self) -> io::Result<()> {
            if self.started.elapsed() >= BUDGET {
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "SAIL0832: optional sysroot budget elapsed",
                ))
            } else {
                Ok(())
            }
        }
        fn operation<T>(
            &mut self,
            mut run: impl FnMut(&mut UnixStream) -> io::Result<T>,
        ) -> io::Result<T> {
            loop {
                self.check()?;
                match run(self.stream) {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    result => {
                        self.check()?;
                        return result;
                    }
                }
            }
        }
    }
    impl Read for Bounded<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.operation(|s| s.read(bytes))
        }
    }
    impl Write for Bounded<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.operation(|s| s.write(bytes))
        }
        fn flush(&mut self) -> io::Result<()> {
            self.operation(Write::flush)
        }
    }

    /// Sends one optional reuse RPC; no response after its budget is admitted.
    ///
    /// # Errors
    /// Returns connect, framing, unsupported-version and whole-transport budget errors.
    pub fn call(cache_socket: &Path, request: &Request) -> io::Result<Response> {
        let started = Instant::now();
        let scope = match request {
            Request::Lookup { scope } | Request::Prepare { scope } => scope,
        };
        if !scope.valid() {
            return Err(io::Error::other("SAIL0833: unsupported sysroot scope"));
        }
        let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        let endpoint = socket2::SockAddr::unix(socket_path(cache_socket))?;
        let remaining = BUDGET
            .checked_sub(started.elapsed())
            .filter(|time| !time.is_zero())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "SAIL0832: optional sysroot budget elapsed",
                )
            })?;
        socket.connect_timeout(&endpoint, remaining)?;
        socket.peer_addr()?;
        let fd: std::os::fd::OwnedFd = socket.into();
        let mut stream = UnixStream::from(fd);
        let mut bounded = Bounded::new(&mut stream, started)?;
        write_frame(&mut bounded, request)?;
        let result = read_frame(&mut bounded)?;
        bounded.check()?;
        match (&result, request) {
            (Response::Lookup(None), Request::Lookup { .. })
            | (Response::Prepared(_), Request::Prepare { .. }) => {}
            (Response::Lookup(Some(snapshot)), Request::Lookup { .. }) if snapshot.valid(scope) => {
            }
            _ => return Err(io::Error::other("SAIL0834: invalid sysroot response")),
        }
        bounded.check()?;
        Ok(result)
    }
}
#[cfg(unix)]
pub use transport::{Bounded, call};

#[cfg(test)]
mod tests {
    use super::{
        Binding, Envelope, MAX_FRAME, MAX_ROOTS, Request, Response, Scope, Snapshot, read_frame,
        write_frame,
    };
    use std::io::{self, Cursor};
    use std::path::PathBuf;

    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn scope() -> Scope {
        Scope {
            roots: vec![PathBuf::from("/sysroot")],
            bindings: vec![Binding {
                path: "/sysroot".into(),
                canonical: "/sysroot".into(),
                device: 1,
                inode: 2,
            }],
            view_root: Binding {
                path: "/".into(),
                canonical: "/".into(),
                device: 1,
                inode: 2,
            },
            selection: "a".repeat(64),
            namespace: "mnt:[1]".to_owned(),
            mounts: "b".repeat(64),
        }
    }
    #[test]
    fn exact_scope_rejects_reordered_relative_unbound_and_unsupported_roots() {
        let good = scope();
        assert!(good.valid());
        let mut bad = good.clone();
        bad.roots.push("/aaa".into());
        assert!(!bad.valid());
        let mut bad = good.clone();
        bad.roots[0] = "relative".into();
        assert!(!bad.valid());
        let mut bad = good.clone();
        bad.bindings.clear();
        assert!(!bad.valid());
        let mut bad = good.clone();
        bad.bindings[0].path = "/other".into();
        assert!(!bad.valid());
        let mut bad = good.clone();
        bad.mounts = "x".repeat(64);
        assert!(!bad.valid());
        let mut bad = good;
        bad.selection.push('0');
        assert!(!bad.valid());
    }
    #[test]
    fn frames_reject_old_version_truncation_trailing_and_oversize_before_payload() -> TestResult {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Request::Lookup { scope: scope() })?;
        let _: Request = read_frame(&mut Cursor::new(&bytes))?;
        assert!(read_frame::<Request>(&mut Cursor::new(&bytes[..bytes.len() - 1])).is_err());
        let payload = serde_json::to_vec(&Envelope {
            version: 0,
            body: Request::Lookup { scope: scope() },
        })?;
        let mut old = u32::try_from(payload.len())?.to_be_bytes().to_vec();
        old.extend(payload);
        assert!(read_frame::<Request>(&mut Cursor::new(old)).is_err());
        let mut too_large = u32::try_from(MAX_FRAME + 1)?.to_be_bytes().to_vec();
        too_large.extend(b"{}");
        assert!(read_frame::<Request>(&mut Cursor::new(too_large)).is_err());
        let mut payload = serde_json::to_vec(&Envelope {
            version: super::VERSION,
            body: Request::Lookup { scope: scope() },
        })?;
        payload.extend(b" true");
        let mut trailing = u32::try_from(payload.len())?.to_be_bytes().to_vec();
        trailing.extend(payload);
        assert!(read_frame::<Request>(&mut Cursor::new(trailing)).is_err());
        let mut sink = Vec::new();
        let large = "x".repeat(MAX_FRAME + 1);
        assert!(write_frame(&mut sink, &large).is_err());
        assert_eq!(sink, [] as [u8; 0]);
        Ok(())
    }
    #[test]
    fn decoder_rejects_excess_elements_before_collecting_the_inventory() -> io::Result<()> {
        let mut value = serde_json::to_value(scope())?;
        value["roots"] = serde_json::json!(vec!["/root"; MAX_ROOTS + 1]);
        let bytes = serde_json::to_vec(&value)?;
        assert!(serde_json::from_slice::<Scope>(&bytes).is_err());
        let mut value = serde_json::to_value(scope())?;
        value["bindings"] = serde_json::json!(vec![
            Binding {
                path: "/root".into(),
                canonical: "/root".into(),
                device: 1,
                inode: 1,
            };
            MAX_ROOTS + 1
        ]);
        assert!(serde_json::from_slice::<Scope>(&serde_json::to_vec(&value)?).is_err());
        Ok(())
    }

    #[test]
    fn snapshot_never_accepts_partial_bad_digest_or_different_scope() {
        let scope = scope();
        let mut snapshot = Snapshot {
            scope: scope.clone(),
            generation: "c".repeat(64),
            rows: vec![super::Row {
                path: "/sysroot/bin/rustc".into(),
                canonical: "/sysroot/bin/rustc".into(),
                device: 1,
                inode: 3,
                sha256: "d".repeat(64),
            }],
        };
        assert!(snapshot.valid(&scope));
        let mut other = scope.clone();
        other.selection = "e".repeat(64);
        assert!(!snapshot.valid(&other));
        snapshot.rows[0].sha256 = "not-a-digest".to_owned();
        assert!(!snapshot.valid(&scope));
        snapshot.rows.clear();
        assert!(!snapshot.valid(&scope));
    }
    #[cfg(unix)]
    #[test]
    fn late_whole_response_and_wrong_variant_never_grant_reuse() -> TestResult {
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        for late in [false, true] {
            let cache = std::path::Path::new("/tmp").join(format!(
                "sail-sysroot-rpc-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let endpoint = super::socket_path(&cache);
            let listener = UnixListener::bind(&endpoint)?;
            let worker = std::thread::spawn(move || -> std::io::Result<()> {
                let (mut stream, _) = listener.accept()?;
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
                let _: Request = read_frame(&mut stream)?;
                if late {
                    std::thread::sleep(super::BUDGET + std::time::Duration::from_millis(20));
                }
                // A successful wrong operation is not a lookup grant.
                let _ = write_frame(&mut stream, &Response::Prepared(true));
                Ok(())
            });
            assert!(super::call(&cache, &Request::Lookup { scope: scope() }).is_err());
            worker.join().map_err(|_| "owned fixture thread failed")??;
            std::fs::remove_file(endpoint)?;
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn complete_snapshot_is_exact_but_disconnect_and_partial_trickle_decline() -> TestResult {
        use std::io::Write as _;
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        for behavior in [0, 1, 2] {
            let cache = std::path::Path::new("/tmp").join(format!(
                "sail-sr-wire-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let endpoint = super::socket_path(&cache);
            let listener = UnixListener::bind(&endpoint)?;
            let expected = Box::new(Snapshot {
                scope: scope(),
                generation: "8".repeat(64),
                rows: vec![super::Row {
                    path: "/sysroot/lib/std.rlib".into(),
                    canonical: "/sysroot/lib/std.rlib".into(),
                    device: 1,
                    inode: 3,
                    sha256: "1".repeat(64),
                }],
            });
            let returned = expected.clone();
            let worker = std::thread::spawn(move || -> std::io::Result<()> {
                let (mut stream, _) = listener.accept()?;
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
                let _: Request = read_frame(&mut stream)?;
                if behavior == 1 {
                    return Ok(());
                }
                let response = Response::Lookup(Some(returned));
                if behavior == 0 {
                    write_frame(&mut stream, &response)?;
                } else {
                    let mut bytes = Vec::new();
                    write_frame(&mut bytes, &response)?;
                    for chunk in bytes.chunks(20) {
                        if stream.write_all(chunk).is_err() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(25));
                    }
                }
                Ok(())
            });
            let result = super::call(&cache, &Request::Lookup { scope: scope() });
            if behavior == 0 {
                assert!(matches!(result?, Response::Lookup(Some(actual)) if actual == expected));
            } else {
                assert!(result.is_err());
            }
            worker.join().map_err(|_| "owned fixture thread failed")??;
            std::fs::remove_file(endpoint)?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn full_unix_backlog_cannot_wait_or_false_connect_into_reuse() -> TestResult {
        let cache =
            std::path::Path::new("/tmp").join(format!("sail-sr-backlog-{}", std::process::id()));
        let endpoint = super::socket_path(&cache);
        let listener = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        let address = socket2::SockAddr::unix(&endpoint)?;
        listener.bind(&address)?;
        listener.listen(1)?;
        let mut filling = Vec::new();
        for _ in 0..2 {
            let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
            socket.connect_timeout(&address, std::time::Duration::from_millis(20))?;
            socket.peer_addr()?;
            filling.push(socket);
        }
        let started = std::time::Instant::now();
        assert!(super::call(&cache, &Request::Lookup { scope: scope() }).is_err());
        assert!(started.elapsed() < std::time::Duration::from_millis(250));
        drop(filling);
        drop(listener);
        std::fs::remove_file(endpoint)?;
        Ok(())
    }
}
