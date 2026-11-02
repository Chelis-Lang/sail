//! Optional daemon-owned preparation. These requests never authorize a proof.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Preparation protocol version, negotiated independently of ordinary jobs.
pub const VERSION: u32 = 2;

/// The successful invocation whose next-call recording is being prepared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    /// Exact unchanged-call key, including output and environment settings.
    pub key: String,
    /// Original verification context.
    pub context: String,
    /// Complete closing input revision of the successful origin, including
    /// generated inputs discovered by that run. Published proof revisions and
    /// establishing provenance are independent and remain unchanged.
    pub revision: String,
    /// Canonical workspace of the origin's Current read-back record.
    pub workspace: PathBuf,
    /// Public establishing revision identified by that read-back record.
    /// This can differ from the closing revision after generated-input discovery.
    pub readback_revision: String,
}

/// One best-effort preparation request, owned by the receiving service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Explicit protocol version; unknown versions are refused.
    pub version: u32,
    /// Ordinary verification inputs and originating environment.
    pub verification: crate::jobs::Submission,
    /// Successful origin, or `None` for initial new-project preparation.
    pub origin: Option<Origin>,
    /// Whether the originating caller requested JSON output.
    pub json: bool,
    /// Resolved Cargo colour decision, already bound by the replay key.
    pub colour: bool,
}

/// Observable preparation state; none of these counters establishes proof.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// Supported protocol version.
    pub version: u32,
    /// Number of queued preparations.
    pub queued: usize,
    /// Workspace currently being prepared, if any.
    pub running: Option<PathBuf>,
    /// Completed actual unchanged-call recordings.
    pub ready: u64,
    /// Replaced or foreground-preempted preparations.
    pub superseded: u64,
    /// Failed, declined or interrupted preparations.
    pub failed: u64,
}

/// Private worker outcome. The service uses this for scheduling, never admission.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    /// Whether the worker committed an actual unchanged-call recording.
    pub ready: bool,
    /// New-project verification may schedule one later unchanged capture.
    pub origin: Option<Origin>,
}
