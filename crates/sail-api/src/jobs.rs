//! Versioned, bounded local project-job protocol, independent of the SMT cache.

use std::collections::BTreeMap;
use std::fmt::Write as _;
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
    /// Explicit backend/repair mode; old endpoints are never used for these requests.
    #[serde(default)]
    pub mode: Mode,
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

/// Verification execution mode, negotiated through the version-two job endpoint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "selection", rename_all = "snake_case")]
pub enum Mode {
    /// Existing stock pinned Verus verification.
    #[default]
    Stock,
    /// Optional explicitly selected Sail-managed interactive engine.
    Interactive,
    /// One engine batch containing focus and independent full validation.
    Repair(crate::repair::Selection),
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
    /// Independent focus/full repair evidence, when this was an engine batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<Box<crate::repair::ResultDocument>>,
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
    /// Fail-closed negotiation before any non-stock submission.
    Capabilities,
    /// Read bounded version-one proof observations using a resumable cursor.
    ProofEvents {
        /// Durable job identity.
        id: String,
        /// Last consumed event; stream identity must match on reconnect.
        cursor: crate::proof::Cursor,
        /// Maximum event count, clamped by the service.
        limit: usize,
    },
    /// Explain an exact reported target/function, preserving raw evidence.
    Explain {
        /// Durable completed job identity.
        id: String,
        /// Complete Cargo target identity.
        target: crate::proof::Target,
        /// Exact crate-qualified function.
        function: String,
        /// Optional exact obligation id.
        obligation: Option<String>,
    },
    /// Save independent immutable proof evidence outside job retention.
    SaveBaseline {
        /// Completed evidence source.
        id: String,
        /// Immutable owner-selected name.
        name: String,
    },
    /// List retained immutable named baselines.
    Baselines,
    /// Read immutable evidence for its recorded revision, never current inputs.
    GetBaseline {
        /// Exact retained baseline name.
        name: String,
    },
    /// Compare explicit completed evidence with an earlier job or baseline.
    Compare {
        /// Earlier named baseline or durable job.
        before: crate::evidence::Reference,
        /// Later completed job identity.
        after: String,
    },
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
    /// Supported product contracts; does not claim an optional engine is installed.
    Capabilities {
        /// Independent proof stream version.
        proof_version: u32,
        /// Non-stock submission modes are understood and never ignored.
        /// Absent from services without the engine, which decode as false.
        #[serde(default)]
        repair: bool,
    },
    /// Ordered, resumable proof observations.
    ProofEvents(crate::proof::Page),
    /// Exact establishing evidence for a failure or proof.
    Explanation(Box<crate::evidence::Explanation>),
    /// Independently retained immutable evidence.
    Baseline(Box<crate::evidence::Baseline>),
    /// Immutable baseline names.
    Baselines(Vec<String>),
    /// Scoped, configuration-aware proof comparison.
    Comparison(crate::evidence::Comparison),
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

/// Resolve the configured cache root without accessing the filesystem.
///
/// # Errors
/// Returns `SAIL3001` when no cache or home environment setting is available.
pub fn cache_directory() -> io::Result<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    var("SAIL_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| var("XDG_CACHE_HOME").map(|root| PathBuf::from(root).join("sail")))
        .or_else(|| var("HOME").map(|root| PathBuf::from(root).join(".cache/sail")))
        .ok_or_else(|| {
            io::Error::other(
                "error[SAIL3001]: none of SAIL_CACHE_DIR, XDG_CACHE_HOME or HOME is set",
            )
        })
}

/// Default cache-service endpoint for the current protocol.
#[must_use]
pub fn default_cache_socket(cache: &Path) -> PathBuf {
    cache.join(format!(
        "daemon-v{}-workspace-v2-proof-v1.sock",
        crate::protocol::PROTOCOL_VERSION
    ))
}

/// Resolve a cache endpoint using the caller's working directory.
/// Relative cache roots and explicit overrides belong to that caller.
#[must_use]
pub fn cache_socket(cache: &Path, explicit: Option<&Path>, cwd: &Path) -> PathBuf {
    let socket = explicit.map_or_else(|| default_cache_socket(cache), Path::to_path_buf);
    if socket.is_absolute() {
        socket
    } else {
        cwd.join(socket)
    }
}

/// Resolve the job endpoint with the same environment semantics as the CLI.
///
/// # Errors
/// Returns the cache configuration error without starting a service.
pub fn configured_socket(cwd: &Path) -> io::Result<PathBuf> {
    let explicit = std::env::var_os("SAIL_DAEMON_SOCKET")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    Ok(socket_path(&cache_socket(
        &cache_directory()?,
        explicit.as_deref(),
        cwd,
    )))
}

/// The job endpoint alongside a cache endpoint, without changing cache protocol.
/// Relative paths and existing filesystem aliases identify the same endpoint.
#[must_use]
pub fn socket_path(cache_socket: &Path) -> PathBuf {
    let cache_socket = endpoint_path(cache_socket);
    let digest = sail_key::sha256(cache_socket.as_os_str().as_encoded_bytes());
    let suffix = digest
        .iter()
        .take(4)
        .fold(String::new(), |mut value, byte| {
            let _ = write!(value, "{byte:02x}");
            value
        });
    cache_socket.with_file_name(format!("jobs-v1-proof-v1-{suffix}.sock"))
}

fn endpoint_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    // Resolve the socket itself when it exists, including a symlink to a live
    // endpoint. Before startup, resolve its nearest existing ancestor instead.
    // This also covers aliases whose socket parent has not been created yet.
    for ancestor in absolute.ancestors() {
        if let Ok(resolved) = ancestor.canonicalize()
            && let Ok(suffix) = absolute.strip_prefix(ancestor)
        {
            return if suffix.as_os_str().is_empty() {
                resolved
            } else {
                resolved.join(suffix)
            };
        }
    }
    absolute
}

/// A timeout-bounded local client. Every call receives a complete fresh frame.
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
        self.call_with(request, |_| {})
    }
    /// Like [`Client::call`], first handing `connected` a clone of the
    /// connected stream. Another thread can shut that clone down to abandon
    /// the call; `connected` is not called when the connection fails.
    ///
    /// # Errors
    /// Returns connection, framing, timeout, or server errors.
    pub fn call_with(
        &self,
        request: &Request,
        connected: impl FnOnce(std::os::unix::net::UnixStream),
    ) -> io::Result<Response> {
        use std::os::unix::net::UnixStream;
        if matches!(request, Request::Submit { submission } if submission.mode != Mode::Stock)
            && !matches!(
                self.call(&Request::Capabilities)?,
                Response::Capabilities {
                    proof_version: crate::proof::VERSION,
                    repair: true
                }
            )
        {
            return Err(io::Error::other(
                "SAIL0903: endpoint does not support proof/repair contracts",
            ));
        }
        let mut stream = UnixStream::connect(&self.socket)?;
        connected(stream.try_clone()?);
        let timeout = Some(std::time::Duration::from_secs(5));
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        write_frame(&mut stream, request)?;
        match read_frame(&mut stream)? {
            Response::Error(message) => Err(io::Error::other(message)),
            response => Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_read_has_exact_additive_wire_identity() {
        let request = Request::GetBaseline {
            name: "--json".into(),
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap_or_else(|error| unreachable!("{error}")),
            serde_json::json!({"operation":"get_baseline","name":"--json"})
        );
        let mut bytes = Vec::new();
        assert!(write_frame(&mut bytes, &request).is_ok());
        assert!(matches!(read_frame::<Request>(&mut bytes.as_slice()),
            Ok(Request::GetBaseline { name }) if name == "--json"));
        assert!(
            serde_json::from_value::<Request>(
                serde_json::json!({"operation":"unknown_baseline","name":"--json"})
            )
            .is_err()
        );
    }

    #[test]
    fn endpoint_resolution_anchors_only_relative_settings_to_each_caller() {
        let a = Path::new("/workspace/a");
        let b = Path::new("/workspace/b");
        assert_ne!(
            cache_socket(Path::new("cache"), None, a),
            cache_socket(Path::new("cache"), None, b)
        );
        assert_eq!(
            cache_socket(Path::new("/shared/cache"), None, a),
            cache_socket(Path::new("/shared/cache"), None, b)
        );
        assert_eq!(
            cache_socket(
                Path::new("ignored"),
                Some(Path::new("sockets/service.sock")),
                a
            ),
            a.join("sockets/service.sock")
        );
        assert_eq!(
            cache_socket(
                Path::new("ignored"),
                Some(Path::new("/explicit/service.sock")),
                b
            ),
            Path::new("/explicit/service.sock")
        );
        assert_eq!(
            cache_socket(Path::new("cache"), None, a),
            a.join(default_cache_socket(Path::new("cache")))
        );
    }

    #[cfg(unix)]
    #[test]
    fn endpoint_aliases_share_the_same_namespace_before_and_after_startup() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("sail-job-socket-{}", std::process::id()));
        let canonical = root.join("actual");
        assert!(std::fs::create_dir_all(&canonical).is_ok());
        let canonical = canonical
            .canonicalize()
            .unwrap_or_else(|e| unreachable!("{e}"));
        let alias = root.join("alias");
        assert!(symlink(&canonical, &alias).is_ok());
        let endpoint = canonical.join("nested/cache.sock");
        assert_eq!(
            socket_path(&endpoint),
            socket_path(&alias.join("nested/cache.sock"))
        );
        assert!(std::fs::create_dir_all(endpoint.parent().unwrap_or(&canonical)).is_ok());
        let current = std::env::current_dir().unwrap_or_else(|e| unreachable!("{e}"));
        let relative = PathBuf::from("../".repeat(current.components().count().saturating_sub(1)))
            .join(endpoint.strip_prefix("/").unwrap_or(&endpoint));
        assert_eq!(socket_path(&endpoint), socket_path(&relative));
        let listener = std::os::unix::net::UnixListener::bind(&endpoint)
            .unwrap_or_else(|e| unreachable!("{e}"));
        let socket_alias = root.join("cache-alias.sock");
        assert!(symlink(&endpoint, &socket_alias).is_ok());
        assert_eq!(socket_path(&endpoint), socket_path(&socket_alias));
        let hash = sail_key::sha256(endpoint.as_os_str().as_encoded_bytes());
        let suffix = hash.iter().take(4).fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        });
        assert_eq!(
            socket_path(&endpoint),
            endpoint.with_file_name(format!("jobs-v1-proof-v1-{suffix}.sock"))
        );
        drop(listener);
        let _ = std::fs::remove_dir_all(root);
    }
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
