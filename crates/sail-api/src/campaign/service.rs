//! Durable campaign inventory and progress, independent of ordinary job results.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Bounds, Classification, Mutation, Outcome, Summary};

/// Explicit request to plan or execute mutations in an existing Cargo workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Originating environment, source directory and exact verification arguments.
    /// Campaign execution requires full scope and uses fresh engine verification;
    /// it does not inherit a mode that would weaken confirmation.
    pub verification: crate::jobs::Submission,
    /// Requested source files relative to the workspace. The runner establishes
    /// target membership; paths alone confer no compiler attribution.
    pub files: Vec<PathBuf>,
    /// Seed controlling deterministic candidate ordering and operator variants.
    pub seed: u64,
    /// Explicit inventory, copy, repetition and total wall-time bounds.
    pub bounds: Bounds,
}

/// One planned exec-only edit, before or after authoritative target binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// Exact bytes, range, resulting closure and optional compiler identity.
    pub mutation: Mutation,
    /// Lexical name used only for discovery/display, never diagnostic attribution.
    pub function_hint: String,
    /// Generator's description of the edit.
    pub description: String,
}

/// A source function or shape excluded before campaign scheduling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    /// Workspace-relative source file.
    pub file: PathBuf,
    /// Lexical function name, if the mask identified one.
    pub function_hint: Option<String>,
    /// Explicit unsupported/excluded reason; no invented proof verdict.
    pub reason: String,
}

/// Immutable source inventory retained before verification scheduling begins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Independent campaign contract version.
    pub version: u32,
    /// Exact frozen source input identity.
    pub source_sha256: String,
    /// Generator identity, including exec-mask policy.
    pub generator: String,
    /// Seed used for candidate ordering.
    pub seed: u64,
    /// Actual requested resource bounds, without silent clipping.
    pub bounds: Bounds,
    /// Files from which the inventory was generated.
    pub files: Vec<PathBuf>,
    /// Distinct admitted source changes in execution order.
    pub candidates: Vec<Candidate>,
    /// Explicit exclusions, including functions that yielded no runnable edits.
    pub exclusions: Vec<Exclusion>,
    /// Additional valid, distinct edits omitted by the inventory budget.
    pub omitted: u64,
}

/// Campaign lifecycle; completion is separate from any verification verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Inputs and deterministic inventory are being established.
    Planning,
    /// Independent baseline repetitions are being established.
    Baseline,
    /// Mutant copies are running bounded repeated checks and confirmations.
    Running,
    /// Every admitted item reached its final disposition.
    Completed,
    /// A resource budget ended the campaign with retained partial observations.
    Partial,
    /// Explicitly cancelled by a client.
    Cancelled,
    /// Original source changed or a newer revision superseded the campaign.
    Superseded,
    /// Service restarted before this campaign finalized; never auto-promoted.
    Interrupted,
    /// Setup, execution or persistence failed; original observations remain.
    InfrastructureError,
}
impl State {
    /// Whether this lifecycle can no longer acquire new verification observations.
    #[must_use]
    pub const fn terminal(self) -> bool {
        !matches!(self, Self::Planning | Self::Baseline | Self::Running)
    }
}

/// One bounded item in the shared CLI, JSON and MCP progress view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Original exact candidate, never replaced by a summary-only edit.
    pub candidate: Candidate,
    /// Settled campaign classification, absent until sufficiently established.
    pub outcome: Option<Outcome>,
    /// Completed original observations retained for `show`.
    pub attempts: u64,
}

/// Shared bounded campaign status/list/survivor view. Detailed attempts are separate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// Always `sail-campaign`; retired neutral documents use no such authority.
    pub kind: String,
    /// Independent campaign version.
    pub version: u32,
    /// Durable unique campaign identifier.
    pub id: String,
    /// Original caller workspace.
    pub project_dir: PathBuf,
    /// Actual lifecycle, including incomplete/interrupted states.
    pub state: State,
    /// Whether original source still matches the retained input closure.
    pub currentness: crate::jobs::Currentness,
    /// Content identity of the immutable plan.
    pub plan_sha256: String,
    /// Requested bounds, retained even for partial work.
    pub bounds: Bounds,
    /// Counts and score with the complete ADR 0111 breakdown.
    pub summary: Summary,
    /// Selected items; status may return no items, survivors only survivors.
    pub items: Vec<Item>,
    /// Original campaign submission time in Unix milliseconds.
    pub submitted_at_ms: u64,
    /// Terminal completion time, absent while work remains active.
    pub finished_at_ms: Option<u64>,
    /// Stable infrastructure/setup error, if present.
    pub error: Option<crate::output::ErrorInfo>,
}

/// One mutant's complete original observations and derived campaign interpretation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    /// Independent campaign version.
    pub version: u32,
    /// Owning durable campaign.
    pub campaign_id: String,
    /// Every bound original observation supplied to the classifier.
    pub evidence: Classification,
    /// Derived from those observations, never accepted as proof authority.
    pub outcome: Outcome,
}
