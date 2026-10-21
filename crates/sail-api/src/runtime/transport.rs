//! Owner-private runtime RPC, independent of cache and Cargo-job protocol versions.

mod client;
#[cfg(test)]
mod tests;
pub use client::Client;

use super::{Capabilities, Error, Request as RuntimeRequest, Response as RuntimeResponse};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fmt::Write as _,
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt as _, FileTypeExt as _, MetadataExt as _},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Transport framing version, independent of the payload's version.
pub const VERSION: u32 = 1;
/// Maximum encoded request frame, checked before allocation.
pub const MAX_REQUEST_BYTES: usize = 128 * 1024 * 1024;
/// Maximum encoded response frame, including byte-array JSON overhead.
pub const MAX_RESPONSE_BYTES: usize = 640 * 1024 * 1024;
/// Absolute deadline for control requests and frame input/output.
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// One bounded local request; each connection carries exactly one request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Negotiate actual runtime readiness before dispatch.
    Capabilities,
    /// Execute the unchanged public runtime request.
    Execute {
        /// Unique active transport identity, excluded from solver evidence.
        id: String,
        /// Fully versioned runtime request.
        request: RuntimeRequest,
    },
    /// Cancel one active execution using a separate responsive control connection.
    Cancel {
        /// Exact active transport identity.
        id: String,
    },
}

/// One complete local response; partial frames never establish a solver result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    /// Actual implementation and installation readiness.
    Capabilities {
        /// Same capabilities as the embedded runtime.
        capabilities: Capabilities,
    },
    /// Complete execution, including unavailable or failed query outcomes.
    Completed {
        /// Exact transport identity of the request.
        id: String,
        /// Unchanged public runtime response.
        response: RuntimeResponse,
    },
    /// Cancellation acknowledgement, never a theorem verdict.
    Cancelled {
        /// Requested transport identity.
        id: String,
        /// Whether an active execution was found and cancelled.
        found: bool,
    },
    /// Explicit admission, overload or service failure.
    Error {
        /// Stable structured diagnostic.
        error: Error,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u32,
    body: T,
}

struct Bounded(Vec<u8>, usize);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.1.saturating_sub(self.0.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SAIL6201: runtime frame exceeds its bound",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Write one independently versioned frame with a hard serialization bound.
///
/// # Errors
/// Returns encoding, bound or output errors before any partial JSON is accepted.
pub fn write_frame<T: Serialize>(
    writer: &mut impl Write,
    body: &T,
    bound: usize,
) -> io::Result<()> {
    let mut encoded = Bounded(Vec::new(), bound);
    serde_json::to_writer(
        &mut encoded,
        &Envelope {
            version: VERSION,
            body,
        },
    )
    .map_err(|e| io::Error::new(e.io_error_kind().unwrap_or(io::ErrorKind::InvalidData), e))?;
    let length = u32::try_from(encoded.0.len()).map_err(io::Error::other)?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&encoded.0)?;
    writer.flush()
}

/// Read one bounded fresh frame, rejecting unknown versions and malformed payloads.
///
/// # Errors
/// Returns incomplete, oversized, malformed or incompatible frame errors.
pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read, bound: usize) -> io::Result<T> {
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let length = usize::try_from(u32::from_be_bytes(header)).map_err(io::Error::other)?;
    if length == 0 || length > bound {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SAIL6201: runtime frame exceeds its bound",
        ));
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes)?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if envelope.version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SAIL6201: incompatible runtime endpoint; restart sail serve",
        ));
    }
    Ok(envelope.body)
}

/// The runtime endpoint derived from the same canonical cache namespace as jobs.
#[must_use]
pub fn socket_path(cache_socket: &Path) -> PathBuf {
    let jobs = crate::jobs::socket_path(cache_socket);
    let hash = sail_key::sha256(jobs.as_os_str().as_encoded_bytes());
    let mut suffix = String::new();
    for byte in hash.iter().take(8) {
        let _ = write!(suffix, "{byte:02x}");
    }
    jobs.with_file_name(format!("rt-{suffix}")).join("v1.sock")
}

/// Verify or create the immediate owner-private directory of a runtime socket.
///
/// # Errors
/// Refuses symlinks, foreign ownership and any group or other permission bits.
pub fn private_parent(socket: &Path, create: bool) -> io::Result<()> {
    let parent = socket
        .parent()
        .ok_or_else(|| io::Error::other("SAIL6201: runtime endpoint has no parent"))?;
    if create {
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        if let Err(e) = builder.create(parent)
            && e.kind() != io::ErrorKind::AlreadyExists
        {
            return Err(e);
        }
    }
    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SAIL6201: runtime parent must be an owned real directory with mode 0700",
        ));
    }
    Ok(())
}

/// Authenticate the local filesystem ownership boundary before connecting.
///
/// # Errors
/// Returns explicit unavailable or permission diagnostics, never a solver fallback.
pub fn validate_socket(socket: &Path) -> io::Result<()> {
    private_parent(socket, false)?;
    let metadata = std::fs::symlink_metadata(socket)?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SAIL6201: runtime endpoint must be an owned socket with mode 0600",
        ));
    }
    Ok(())
}

/// One Unix stream with an absolute read/write deadline, including trickle input.
pub struct Deadline<'a> {
    stream: &'a std::os::unix::net::UnixStream,
    until: Instant,
    prepared: bool,
}
impl<'a> Deadline<'a> {
    /// Apply one absolute deadline to an entire frame exchange.
    #[must_use]
    pub fn new(stream: &'a std::os::unix::net::UnixStream, duration: Duration) -> Self {
        Self {
            stream,
            until: Instant::now() + duration,
            prepared: false,
        }
    }
    fn remaining(&self) -> io::Result<Duration> {
        self.until
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "SAIL6201: runtime frame deadline expired",
                )
            })
    }
}
impl Deadline<'_> {
    fn prepare(&mut self) -> io::Result<()> {
        if !self.prepared {
            self.stream.set_nonblocking(true)?;
            self.prepared = true;
        }
        Ok(())
    }
    fn pause(&self) -> io::Result<()> {
        std::thread::sleep(self.remaining()?.min(Duration::from_millis(2)));
        Ok(())
    }
}
impl Read for Deadline<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.prepare()?;
        loop {
            self.remaining()?;
            match self.stream.read(bytes) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => self.pause()?,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
}
impl Write for Deadline<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.prepare()?;
        loop {
            self.remaining()?;
            match self.stream.write(bytes) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => self.pause()?,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
