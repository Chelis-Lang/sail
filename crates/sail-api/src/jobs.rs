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
    /// Qualified contract snapshot metadata; absent for ordinary project jobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contracts: Option<Box<crate::contracts::Evidence>>,
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
    /// Start a bounded owned mutation campaign over an authenticated source job.
    SubmitMutation(crate::mutation::Submission),
    /// Prepare separate exact scalar literal-contract witness queries; starts no verifier.
    PrepareLiteralWitness {
        /// Complete original source with its exact content identity.
        source: crate::contracts::Submission,
        /// Maximum selected literal candidates, between one and eight.
        max_mutants: usize,
    },
    /// Rebuild an original-source witness plan and validate one concrete model.
    ValidateLiteralWitness {
        /// Full exact original source-bound plan.
        plan: Box<crate::literal_witness::Plan>,
        /// Exact deterministic original candidate identity.
        candidate_id: String,
        /// Complete original SAT model response from the pinned runtime.
        result: Box<crate::runtime::QueryResult>,
    },
    /// Reauthenticate and read a retained mutation campaign.
    Mutation {
        /// Exact retained campaign identity.
        id: String,
    },
    /// Cancel actual source jobs in one campaign.
    CancelMutation {
        /// Exact retained campaign identity.
        id: String,
    },
    /// Read requirement-aware assurance for an authenticated neutral source job.
    Assurance {
        /// Exact durable source-job identity.
        id: String,
    },
    /// Submit exact neutral source with Sail-owned execution controls.
    SubmitContracts(crate::contracts::Submission),
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
        /// Neutral source submission version; absent on older services.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        neutral_contracts_version: Option<u32>,
        /// Explicit supported source profiles; older endpoints advertise none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        neutral_contract_profiles: Vec<u32>,
        /// Requirement assurance view version; absent on older services.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assurance_version: Option<u32>,
        /// Bounded mutation protocol, absent on older services.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mutation_version: Option<u32>,
        /// Non-stock submission modes are understood and never ignored.
        /// Absent from services without the engine, which decode as false.
        #[serde(default)]
        repair: bool,
    },
    /// Authenticated bounded mutation evidence, independent of proof tiers.
    Mutation(Box<crate::mutation::View>),
    /// Deterministic source-admitted witness queries, separate from native mutation.
    LiteralWitness(Box<crate::literal_witness::Plan>),
    /// Concrete original-source counterexample; never a native Verus kill.
    LiteralWitnessValidated(Box<crate::literal_witness::Validated>),
    /// Ordered, resumable proof observations.
    ProofEvents(crate::proof::Page),
    /// Exact establishing evidence for a failure or proof.
    Explanation(Box<crate::evidence::Explanation>),
    /// Snapshot-scoped contract-to-code evidence grouped by requirement identity.
    Assurance(Box<crate::assurance::View>),
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
        return Err(io::Error::other("SAIL0802: job frame exceeds limit"));
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
        return Err(io::Error::other("SAIL0802: job frame exceeds limit"));
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

/// The longest Unix socket path, in bytes, this platform accepts
/// (`sun_path` less its terminating NUL: 108 bytes on Linux, 104 on macOS
/// and the BSDs).
pub const MAX_SOCKET_PATH: usize = if cfg!(target_os = "linux") { 107 } else { 103 };

/// Default cache-service endpoint for the current protocol:
/// `<cache>/daemon-v<N>-workspace-v2-proof-v1.sock`.
///
/// When that path, or the job endpoint beside it, would exceed
/// [`MAX_SOCKET_PATH`] (a deep `HOME` or a long `SAIL_CACHE_DIR`), the
/// endpoint moves to a private per-cache directory `/tmp/sail-<hash>/`,
/// where `<hash>` names the cache. That directory is created mode 0700 and
/// used only when it is a real directory that no other user can access and
/// this user can write; otherwise the long path is returned unchanged, and
/// starting the daemon reports that it is too long.
#[must_use]
pub fn default_cache_socket(cache: &Path) -> PathBuf {
    let socket = cache.join(format!(
        "daemon-v{}-workspace-v2-proof-v1.sock",
        crate::protocol::PROTOCOL_VERSION
    ));
    if socket_fits(&socket) {
        return socket;
    }
    short_socket(&socket).unwrap_or(socket)
}

/// Whether both the cache endpoint and its job endpoint fit in `sun_path`.
#[must_use]
pub fn socket_fits(cache_socket: &Path) -> bool {
    let absolute = std::path::absolute(cache_socket).unwrap_or_else(|_| cache_socket.to_owned());
    absolute.as_os_str().len() <= MAX_SOCKET_PATH
        && socket_path(cache_socket).as_os_str().len() <= MAX_SOCKET_PATH
}

#[cfg(unix)]
fn short_socket(socket: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
    let name = socket.file_name()?;
    let digest = sail_key::sha256(endpoint_path(socket).as_os_str().as_encoded_bytes());
    let hash = digest
        .iter()
        .take(8)
        .fold(String::new(), |mut value, byte| {
            let _ = write!(value, "{byte:02x}");
            value
        });
    let directory = Path::new("/tmp").join(format!("sail-{hash}"));
    // An existing directory is accepted only when it is private.
    let _ = std::fs::DirBuilder::new().mode(0o700).create(&directory);
    let metadata = std::fs::symlink_metadata(&directory).ok()?;
    if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return None;
    }
    // Only its owner can create a file in a 0700 directory, so this proves
    // the directory is this user's and no other user's daemon can be there.
    let probe = directory.join(format!(".probe-{}", std::process::id()));
    std::fs::File::create(&probe).ok()?;
    std::fs::remove_file(&probe).ok()?;
    let short = directory.join(name);
    socket_fits(&short).then_some(short)
}

#[cfg(not(unix))]
fn short_socket(_socket: &Path) -> Option<PathBuf> {
    None
}

/// Resolve a cache endpoint using the caller's working directory.
/// Relative cache roots and explicit overrides belong to that caller.
#[must_use]
pub fn cache_socket(cache: &Path, explicit: Option<&Path>, cwd: &Path) -> PathBuf {
    // Anchor a relative cache first, so the length check sees the caller's path.
    let socket = explicit.map_or_else(|| default_cache_socket(&cwd.join(cache)), Path::to_path_buf);
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
    /// Independently evaluate a complete scalar model through the source owner.
    /// # Errors
    /// Refuses older services, changed plans, models and invalid counterexamples.
    pub fn validate_literal_witness(
        &self,
        plan: crate::literal_witness::Plan,
        candidate_id: String,
        result: &crate::runtime::QueryResult,
    ) -> io::Result<crate::literal_witness::Validated> {
        let prepared = plan
            .prepared
            .iter()
            .find(|p| p.candidate.id == candidate_id)
            .cloned()
            .ok_or_else(|| io::Error::other("SAIL3145: missing literal witness candidate"))?;
        match self.call(&Request::ValidateLiteralWitness {
            plan: Box::new(plan),
            candidate_id,
            result: Box::new(result.clone()),
        })? {
            Response::LiteralWitnessValidated(value)
                if value.prepared == prepared && value.model_result == *result =>
            {
                Ok(*value)
            }
            Response::Error(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::other(
                "SAIL3145: incompatible concrete witness response",
            )),
        }
    }
    /// Prepare separate source-admitted scalar literal witness queries.
    /// This starts no verification or mutation campaign and has no local fallback.
    /// # Errors
    /// Refuses unavailable or older services, changed input and invalid query links.
    pub fn literal_witnesses(
        &self,
        source: &crate::contracts::Submission,
        max_mutants: usize,
    ) -> io::Result<crate::literal_witness::Plan> {
        source.validate().map_err(io::Error::other)?;
        match self.call(&Request::PrepareLiteralWitness {
            source: source.clone(),
            max_mutants,
        })? {
            Response::LiteralWitness(plan)
                if plan.baseline == *source && plan.max_mutants == max_mutants =>
            {
                plan.validate().map_err(io::Error::other)?;
                Ok(*plan)
            }
            Response::Error(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::other(
                "SAIL3145: incompatible literal witness response",
            )),
        }
    }
    /// Submit a bounded owned mutation campaign through the existing job service.
    /// # Errors
    /// Refuses incompatible services, invalid baselines or invalid budgets.
    pub fn submit_mutation(
        &self,
        submission: crate::mutation::Submission,
    ) -> io::Result<crate::mutation::View> {
        if !matches!(
            self.call(&Request::Capabilities)?,
            Response::Capabilities {
                mutation_version: Some(crate::mutation::VERSION),
                ..
            }
        ) {
            return Err(io::Error::other(
                "SAIL3144: running service has no compatible mutation API",
            ));
        }
        let expected = submission.clone();
        let result = self.mutation_call(&Request::SubmitMutation(submission))?;
        if result.submission != expected {
            return Err(io::Error::other(
                "SAIL3144: mutation submission attribution differs",
            ));
        }
        Ok(result)
    }
    /// Reauthenticate an exact durable campaign and all original result links.
    /// # Errors
    /// Missing, changed or incompatible evidence is an explicit error.
    pub fn mutation(&self, id: &str) -> io::Result<crate::mutation::View> {
        let result = self.mutation_call(&Request::Mutation { id: id.into() })?;
        if result.id != id {
            return Err(io::Error::other(
                "SAIL3144: mutation campaign attribution differs",
            ));
        }
        Ok(result)
    }
    /// Request cancellation of actual campaign jobs; inspect the returned lifecycle.
    /// # Errors
    /// Missing campaigns and service failures remain explicit.
    pub fn cancel_mutation(&self, id: &str) -> io::Result<crate::mutation::View> {
        let result = self.mutation_call(&Request::CancelMutation { id: id.into() })?;
        if result.id != id {
            return Err(io::Error::other(
                "SAIL3144: mutation cancellation attribution differs",
            ));
        }
        Ok(result)
    }
    fn mutation_call(&self, request: &Request) -> io::Result<crate::mutation::View> {
        match self.call(request)? {
            Response::Mutation(view) if view.version == crate::mutation::VERSION => {
                crate::mutation::validate_view(&view).map_err(io::Error::other)?;
                Ok(*view)
            }
            Response::Error(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::other("SAIL3144: incompatible mutation response")),
        }
    }
    /// Read source-snapshot requirement assurance after version negotiation.
    ///
    /// # Errors
    /// Refuses unsupported services, unknown jobs and invalid stored evidence.
    pub fn requirement_assurance(&self, id: &str) -> io::Result<crate::assurance::View> {
        if id.is_empty() || id.len() > 256 || id.contains('\0') {
            return Err(io::Error::other("SAIL3143: invalid assurance job identity"));
        }
        if !matches!(
            self.call(&Request::Capabilities)?,
            Response::Capabilities {
                assurance_version: Some(crate::assurance::VERSION),
                ..
            }
        ) {
            return Err(io::Error::other(
                "SAIL3143: endpoint does not support requirement assurance",
            ));
        }
        match self.call(&Request::Assurance { id: id.into() })? {
            Response::Assurance(view)
                if view.version == crate::assurance::VERSION && view.job_id == id =>
            {
                Ok(*view)
            }
            Response::Error(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::other(
                "SAIL3143: invalid assurance response attribution",
            )),
        }
    }

    /// Submit neutral source only after explicit capability negotiation.
    /// # Errors
    /// Refuses older services, invalid source or transport failures without fallback.
    pub fn submit_contracts(&self, submission: crate::contracts::Submission) -> io::Result<Status> {
        submission.validate().map_err(io::Error::other)?;
        let supported = match self.call(&Request::Capabilities)? {
            Response::Capabilities {
                neutral_contracts_version: Some(crate::contracts::VERSION),
                neutral_contract_profiles,
                ..
            } => submission.profile == 1 || neutral_contract_profiles.contains(&submission.profile),
            _ => false,
        };
        if !supported {
            return Err(io::Error::other(
                "SAIL3142: endpoint does not support the selected neutral source profile",
            ));
        }
        match self.call(&Request::SubmitContracts(submission))? {
            Response::Status(status) => Ok(status),
            Response::Error(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::other(
                "SAIL3142: unexpected neutral submission response",
            )),
        }
    }

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
                    repair: true,
                    ..
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
            default_cache_socket(&a.join("cache"))
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
    /// A deep cache directory (as under a long `HOME` or `SAIL_CACHE_DIR`)
    /// gets a short, private, per-cache endpoint; a short one is unchanged.
    #[cfg(unix)]
    #[test]
    fn long_cache_paths_get_a_short_private_socket() {
        use std::os::unix::fs::MetadataExt as _;
        let short = Path::new("/tmp/c");
        assert_eq!(
            default_cache_socket(short),
            short.join(format!(
                "daemon-v{}-workspace-v2-proof-v1.sock",
                crate::protocol::PROTOCOL_VERSION
            ))
        );
        let deep = Path::new("/nonexistent").join("d".repeat(120));
        let socket = default_cache_socket(&deep);
        assert!(socket.starts_with("/tmp"), "{}", socket.display());
        assert!(socket_fits(&socket));
        assert_eq!(socket, default_cache_socket(&deep), "deterministic");
        let directory = socket.parent().unwrap_or(Path::new("/"));
        let mode = std::fs::metadata(directory).map(|m| m.mode() & 0o777);
        assert!(matches!(mode, Ok(0o700)), "{mode:?}");
        assert_ne!(
            socket,
            default_cache_socket(&Path::new("/nonexistent").join("e".repeat(120))),
            "one directory per cache"
        );
        // Another user's (here: a non-private) directory is never used.
        assert!(
            std::fs::set_permissions(
                directory,
                std::os::unix::fs::PermissionsExt::from_mode(0o755)
            )
            .is_ok()
        );
        assert!(!default_cache_socket(&deep).starts_with("/tmp"));
        let _ = std::fs::remove_dir_all(directory);
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
