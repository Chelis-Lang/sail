//! Version-one exact SMT runtime requests and attributed responses.

use serde::{Deserialize, Serialize};

/// Private local runtime transport, independent of cache and project-job RPC.
#[cfg(unix)]
pub mod transport;

/// Public runtime protocol version.
pub const VERSION: u32 = 1;
/// Maximum number of independently executed queries in one request.
pub const MAX_QUERIES: usize = 64;
/// Maximum total admitted script bytes.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum retained bytes in each output stream of one query.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
/// Aggregate stdout, stderr and duplicated product byte budget for one batch.
pub const MAX_RETAINED_OUTPUT_BYTES: usize = 128 * 1024 * 1024;
/// Maximum per-query process safety deadline.
pub const MAX_WALL_TIME_MS: u64 = 300_000;

/// One versioned runtime invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Must equal [`VERSION`].
    pub version: u32,
    /// Explicit input family.
    pub input: Input,
}

/// Implemented runtime input families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    /// One attributed native temporal job.
    Temporal {
        /// Immutable payload and bounded deadline.
        job: Box<crate::temporal::Job>,
    },
    /// Independent complete SMT transcripts, in caller order.
    Smt {
        /// Bounded independent queries.
        queries: Vec<Query>,
    },
}

/// One complete SMT-LIB transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Stable, nonempty identity chosen by the caller.
    pub obligation_id: String,
    /// Strictly sorted, unique nonempty requirement identities.
    pub requirement_ids: Vec<String>,
    /// Explicit solver family; this slice implements `z3`.
    pub backend: String,
    /// Exact unmodified UTF-8 SMT-LIB input.
    pub script: String,
    /// Execution limits, independent of solver answers.
    #[serde(default)]
    pub limits: Limits,
}

/// Bounded process execution and deterministic solver effort.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Z3 resource units; zero is deliberately not admitted.
    pub resource_limit: u64,
    /// Whole-process safety deadline in milliseconds.
    pub wall_time_ms: u64,
    /// Maximum exact input bytes for this query.
    pub input_bytes: usize,
    /// Maximum bytes retained from each output stream.
    pub output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            resource_limit: 2_000_000,
            wall_time_ms: 30_000,
            input_bytes: MAX_INPUT_BYTES,
            output_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

/// A complete versioned invocation response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// Equals [`VERSION`].
    pub version: u32,
    /// Explicit result family.
    pub output: Output,
}

/// Runtime outputs, sharing the request-family discriminator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Output {
    /// One native temporal execution, including original evidence.
    Temporal {
        /// Exact caller attribution and separate execution/native outcomes.
        result: Box<crate::temporal::Result>,
    },
    /// Query results in original caller order.
    Smt {
        /// One result for every admitted query.
        results: Vec<QueryResult>,
    },
}

/// Solver answers are separate from process outcomes and proposition validity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// A satisfying assignment exists.
    Sat,
    /// No satisfying assignment exists.
    Unsat,
    /// The solver did not decide.
    Unknown,
}

/// Whole-transcript execution outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every expected response was validated; checks may still be unknown.
    Completed,
    /// Requested backend or exact pinned executable is unavailable.
    Unavailable,
    /// Process safety deadline expired without a valid transcript.
    Timeout,
    /// Explicit caller cancellation stopped execution.
    Cancelled,
    /// An output stream exceeded its retained-byte bound.
    OutputLimit,
    /// Process or response integrity failed.
    Infrastructure,
}

/// One check and its explicitly requested products.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// Zero-based index among original top-level commands.
    pub command_index: usize,
    /// Raw satisfiability answer.
    pub answer: Answer,
    /// Products belonging to this check, in command order.
    pub products: Vec<Product>,
}

/// Requested response product.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    /// Zero-based original command index.
    pub command_index: usize,
    /// Product interpretation.
    pub kind: ProductKind,
    /// Exact response expression bytes; complete stdout remains alongside it.
    pub raw: Vec<u8>,
}

/// Explicitly supported solver response products.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductKind {
    /// A satisfying model.
    Model,
    /// Values for the requested terms.
    Values,
    /// Named assertions forming an unsatisfiable core.
    UnsatCore,
    /// Solver's reason for its unknown answer.
    ReasonUnknown,
}

/// Exact executable establishing solver responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolverIdentity {
    /// Explicit solver family.
    pub backend: String,
    /// Exact admitted version.
    pub version: String,
    /// SHA-256 of executable bytes.
    pub sha256: String,
    /// OS and architecture class.
    pub platform: String,
}

/// One attributed transcript result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Caller identity, preserved exactly.
    pub obligation_id: String,
    /// Caller requirement identities, preserved exactly.
    pub requirement_ids: Vec<String>,
    /// Requested backend, preserved even when unavailable.
    pub backend: String,
    /// Process outcome; never inferred from a partial output prefix.
    pub outcome: Outcome,
    /// Empty unless the complete response plan was validated.
    pub checks: Vec<Check>,
    /// Exact bounded standard output bytes.
    pub stdout: Vec<u8>,
    /// Exact bounded standard error bytes.
    pub stderr: Vec<u8>,
    /// Present only after actual executable identity verification.
    pub solver: Option<SolverIdentity>,
    /// Content identity of the exact script.
    pub script_sha256: String,
    /// Effective admitted limits.
    pub limits: Limits,
    /// True only when complete original solver evidence was reused.
    pub cache_hit: bool,
    /// Explicit failure diagnostic, absent for completed transcripts.
    pub error: Option<Error>,
}

/// Versioned runtime capabilities; readiness is not a theorem verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Native temporal readiness; absent on older SMT-only services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<crate::temporal::Capability>,
    /// Equals [`VERSION`].
    pub version: u32,
    /// Backends implemented by this build.
    pub backends: Vec<BackendCapability>,
    /// Complete transcripts support push/pop and multiple checks.
    pub incremental_transcripts: bool,
    /// Bounded independent-query submission is implemented.
    pub batches: bool,
    /// Maximum query count.
    pub max_queries: usize,
}

/// Implementation and actual installation are separate facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapability {
    /// Backend family.
    pub backend: String,
    /// Backend code is implemented.
    pub implemented: bool,
    /// Exact usable pinned executable was verified.
    pub installed: bool,
    /// Actual verified identity, when available.
    pub solver: Option<SolverIdentity>,
    /// Explicit unavailability diagnostic.
    pub error: Option<Error>,
}

/// Stable machine-readable error with a human diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    /// Stable `SAIL6101` admission or `SAIL6102` execution code.
    pub code: String,
    /// Stable specific category such as `invalid_request` or `unsupported_command`.
    pub kind: String,
    /// Actionable diagnostic.
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}]: {}: {}", self.code, self.kind, self.message)
    }
}

impl std::error::Error for Error {}
