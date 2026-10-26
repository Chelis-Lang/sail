//! Focused repair checks and complete validation.
//!
//! Explanations, baselines and comparisons are in [`crate::evidence`].

use serde::{Deserialize, Serialize};

use crate::project::ProjectResult;

/// Independent repair contract version.
pub const VERSION: u32 = 1;

/// Source-engine batch output; each phase retains its independent proof report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchResult {
    /// Complete selected-function phase, absent if it did not produce evidence.
    pub focus: Option<ProjectResult>,
    /// Independent complete requested-target phase. A standalone repair
    /// document (the repair worker's output, `sail repair-check --output json`)
    /// carries it. A job result omits it: the job's `result` is this phase,
    /// so readers receive the graph once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full: Option<ProjectResult>,
    /// Hash of the validated optional engine build manifest.
    pub manifest_sha256: String,
    /// Number of Verus driver subprocesses actually started by this batch.
    /// An engine batch can run ghost verification and final executable checks
    /// inside one driver; this is not the number of internal Rust sessions.
    pub compiler_invocations: u64,
    /// True only for engine-supported focus/full batching within compiler calls.
    pub batched: bool,
    /// Exact focus selection was unavailable or ambiguous; full evidence is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_error: Option<crate::output::ErrorInfo>,
    /// The engine faulted before any target could fall back individually
    /// (ADR 0027). `full` is then an independent stock verification, retained
    /// as evidence; it never accepts the repair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_fault: Option<crate::output::ErrorInfo>,
}

impl BatchResult {
    /// Whether selection succeeded and full validation established current complete evidence.
    /// Preliminary focus success cannot authorize a repair, and neither can a
    /// stock fallback after an engine fault. Without a full phase, false.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.full
            .as_ref()
            .is_some_and(|full| self.accepted_with(full))
    }

    /// [`Self::accepted`] with the full phase supplied separately, as in a job
    /// result whose `result` holds it.
    #[must_use]
    pub fn accepted_with(&self, full: &ProjectResult) -> bool {
        self.selection_error.is_none() && self.engine_fault.is_none() && accepted(full)
    }
}

/// Whether an independently completed full report authorizes a repair.
///
/// Summary counts are derived evidence, not acceptance authority: the report
/// must be internally consistent and every function must be confirmed verified.
#[must_use]
pub fn accepted(full: &ProjectResult) -> bool {
    crate::acceptance::complete_selection(full).is_some()
}

/// Explicit selected target/function; never guessed from semantic dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Cargo package selection.
    pub package: String,
    /// Explicit binary name; library selection is unique within the package.
    pub target: Option<String>,
    /// `lib` or `bin`; target names alone are ambiguous.
    pub kind: String,
    /// Exact crate-qualified function identifier as reported by Verus.
    pub function: String,
}

/// Repair submission uses the existing environment/revision request contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    /// Verification context, arguments and originating environment.
    pub verification: crate::jobs::Submission,
    /// Explicit target/function selection.
    pub selection: Selection,
}

/// Finished repair response; accepted is derived rather than trusted from storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultDocument {
    /// Independent contract version.
    pub version: u32,
    /// Complete phase evidence.
    pub batch: BatchResult,
    /// Derived from successful selection and the independently finalized full report.
    pub accepted: bool,
}

impl From<BatchResult> for ResultDocument {
    fn from(batch: BatchResult) -> Self {
        Self {
            version: VERSION,
            accepted: batch.accepted(),
            batch,
        }
    }
}
