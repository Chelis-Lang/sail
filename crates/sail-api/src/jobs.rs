//! Versioned, bounded local project-job protocol, independent of the SMT cache.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Project job protocol version.
pub const VERSION: u32 = 1;
/// Maximum request or response frame (32 MiB).
pub const MAX_FRAME: usize = 32 * 1024 * 1024;

/// A verification request. The environment belongs to its submitting client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    /// Absolute Cargo project directory.
    pub project_dir: PathBuf,
    /// Verification arguments, without the `verify` subcommand or output flags.
    pub args: Vec<String>,
    /// Force fresh verification even when a proof receipt exists.
    pub force: bool,
    /// UTF-8 process environment, captured by the submitting client.
    pub environment: BTreeMap<String, String>,
    /// Content revision observed by the submitting client.
    pub input_revision: Option<String>,
}

/// Durable lifecycle, independent of the proof outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    /// Waiting for the single verification worker.
    Queued,
    /// The verification process is running.
    Running,
    /// A complete verification result is available (including failed proofs).
    Completed,
    /// Explicitly cancelled by a client.
    Cancelled,
    /// Cancelled because a newer input revision was requested.
    Superseded,
    /// Startup, persistence, process, or restart failure.
    InfrastructureError,
}
impl Lifecycle {
    /// Whether no more worker events are expected.
    #[must_use]
    pub const fn terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

/// How the result's input revision relates to current inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Currentness {
    /// The runner established a coherent current revision.
    Current,
    /// Inputs changed during or since verification.
    Historical,
    /// Input coverage or result attribution could not be established.
    Unknown,
}

/// Persistent status, safe to inspect while verification runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    /// Protocol/store format version.
    pub version: u32,
    /// Durable job id.
    pub id: String,
    /// Absolute project directory.
    pub project_dir: PathBuf,
    /// Requested arguments; never broadened silently.
    pub args: Vec<String>,
    /// Lifecycle, distinct from proof verdict.
    pub state: Lifecycle,
    /// Submission time, milliseconds since the Unix epoch.
    pub submitted_at_ms: u64,
    /// Start time, if started.
    pub started_at_ms: Option<u64>,
    /// Completion time, if terminal.
    pub finished_at_ms: Option<u64>,
    /// Process exit status, if available.
    pub exit_code: Option<i32>,
    /// Input revision before the run.
    pub input_revision: Option<String>,
    /// Result currentness.
    pub currentness: Currentness,
    /// Stable infrastructure error code and explanatory text, when present.
    pub error: Option<String>,
}

/// A job result preserving the managed runner's versioned document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultDocument {
    /// Job status and provenance.
    pub job: Status,
    /// Managed project result, only after a complete worker output.
    pub result: Option<Value>,
}

/// A bounded slice of a job's diagnostics or target events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Byte offset of the next slice.
    pub next: u64,
    /// UTF-8 text (lossy only for diagnostics from non-UTF-8 subprocesses).
    pub text: String,
    /// Whether this job has reached a terminal state.
    pub complete: bool,
}

/// One local RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Request {
    /// Submit or coalesce an identical in-flight request.
    Submit {
        /// Requested verification.
        submission: Submission,
    },
    /// List retained jobs, optionally for one canonical project directory.
    List {
        /// Project filter.
        project_dir: Option<PathBuf>,
    },
    /// Inspect one job without waiting for verification.
    Status {
        /// Job id.
        id: String,
    },
    /// Read one completed or in-progress result.
    Result {
        /// Job id.
        id: String,
    },
    /// Cancel one job and its entire process group.
    Cancel {
        /// Job id.
        id: String,
        /// New edits supersede this run.
        superseded: bool,
    },
    /// Read bounded stderr diagnostics from a byte offset.
    Logs {
        /// Job id.
        id: String,
        /// Byte offset.
        offset: u64,
    },
    /// Read bounded JSON-lines target events from a byte offset.
    Events {
        /// Job id.
        id: String,
        /// Byte offset.
        offset: u64,
    },
}

/// A response or a stable user-facing error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response", content = "value", rename_all = "snake_case")]
pub enum Response {
    /// One status.
    Status(Status),
    /// Retained jobs, oldest first.
    List(Vec<Status>),
    /// A result with its lifecycle and provenance.
    Result(ResultDocument),
    /// Diagnostics or target events.
    Chunk(Chunk),
    /// Stable error text, prefixed with a `SAIL` code.
    Error(String),
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    version: u32,
    body: T,
}

/// Writes a bounded versioned frame.
///
/// # Errors
/// Returns invalid data for oversized frames or serialization errors, or an I/O error.
pub fn write_frame<T: Serialize>(writer: &mut impl Write, body: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(&Envelope {
        version: VERSION,
        body,
    })
    .map_err(io::Error::other)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::other("SAIL0801: job frame exceeds limit"));
    }
    let size = u32::try_from(bytes.len()).map_err(io::Error::other)?;
    writer.write_all(&size.to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Reads a bounded frame, refusing unsupported versions before dispatch.
///
/// # Errors
/// Returns invalid data for malformed, oversized, or unsupported frames, or an I/O error.
pub fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let size = usize::try_from(u32::from_be_bytes(length)).map_err(io::Error::other)?;
    if size > MAX_FRAME {
        return Err(io::Error::other("SAIL0801: job frame exceeds limit"));
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if envelope.version != VERSION {
        return Err(io::Error::other("SAIL0802: incompatible job protocol"));
    }
    Ok(envelope.body)
}

/// The job endpoint alongside a cache endpoint, without changing cache protocol.
#[must_use]
pub fn socket_path(cache_socket: &Path) -> PathBuf {
    cache_socket.with_file_name(format!(
        "jobs-v1-{}.sock",
        cache_socket
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
    ))
}

/// A short-lived, timeout-bounded local client. Every call has its own connection.
#[cfg(unix)]
#[derive(Debug, Clone)]
pub struct Client {
    socket: PathBuf,
}
#[cfg(unix)]
impl Client {
    /// A client for `socket`; no connection or side effect until a call.
    #[must_use]
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }
    /// Sends one RPC with five-second I/O deadlines.
    ///
    /// # Errors
    /// Returns connection, framing, timeout, or server errors.
    pub fn call(&self, request: &Request) -> io::Result<Response> {
        use std::os::unix::net::UnixStream;
        let mut stream = UnixStream::connect(&self.socket)?;
        let timeout = Some(std::time::Duration::from_secs(5));
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        write_frame(&mut stream, request)?;
        let response = read_frame(&mut stream)?;
        match response {
            Response::Error(message) => Err(io::Error::other(message)),
            response => Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_and_size_are_checked() {
        let mut bytes = Vec::new();
        assert!(write_frame(&mut bytes, &Request::List { project_dir: None }).is_ok());
        assert!(matches!(
            read_frame::<Request>(&mut bytes.as_slice()),
            Ok(Request::List { .. })
        ));
        let huge = u32::MAX.to_be_bytes();
        assert!(read_frame::<Request>(&mut huge.as_slice()).is_err());
        let body = br#"{"version":2,"body":{"operation":"list","project_dir":null}}"#;
        let mut wrong = u32::try_from(body.len())
            .unwrap_or(0)
            .to_be_bytes()
            .to_vec();
        wrong.extend_from_slice(body);
        assert!(read_frame::<Request>(&mut wrong.as_slice()).is_err());
    }
}
