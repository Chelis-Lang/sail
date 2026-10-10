//! Focused repair checks, complete validation, evidence bundles and comparisons.

use serde::{Deserialize, Serialize};

use crate::output::{Diagnostic, FunctionReport, VerdictName};
use crate::project::{Currentness, ProjectResult};
use crate::proof::Target;

/// Independent repair contract version.
pub const VERSION: u32 = 1;

/// Source-engine batch output; each phase retains its independent proof report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchResult {
    /// Complete selected-function phase, absent if it did not produce evidence.
    pub focus: Option<ProjectResult>,
    /// Independent complete requested-target phase.
    pub full: ProjectResult,
    /// Hash of the validated optional engine build manifest.
    pub manifest_sha256: String,
    /// Number of compiler invocations actually started by this batch.
    pub compiler_invocations: u64,
    /// True only for engine-supported focus/full batching within compiler calls.
    pub batched: bool,
}

impl BatchResult {
    /// Whether full validation established successful current complete evidence.
    /// Preliminary focus success cannot authorize a repair.
    #[must_use]
    pub fn accepted(&self) -> bool {
        accepted(&self.full)
    }
}

/// Whether an independently completed full report authorizes a repair.
#[must_use]
pub fn accepted(full: &ProjectResult) -> bool {
    let selected: Vec<_> = full.targets.iter().filter(|t| t.requested).collect();
    full.currentness == Currentness::Current
        && full.exit_code == Some(0)
        && full.error.is_none()
        && !selected.is_empty()
        && selected.iter().all(|target| {
            target.complete
                && target.report.error.is_none()
                && target.report.exit_code == Some(0)
                && target.report.summary.failed == 0
                && target.report.summary.unknown == 0
                && target.report.summary.provisional == 0
        })
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
    /// Derived exclusively from the full report after independent finalization.
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

/// Explanation preserving exact verifier evidence instead of generating proof edits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    /// Independent contract version.
    pub version: u32,
    /// Complete target identity.
    pub target: Target,
    /// Establishing run, including reused historical reports.
    pub establishing_run: String,
    /// Currentness of the project evidence.
    pub currentness: Currentness,
    /// Complete unchanged function report, with obligations/models/provenance.
    pub function: FunctionReport,
    /// Diagnostics with explicit producer attribution, if available.
    pub attributed_diagnostics: Vec<Diagnostic>,
    /// Remaining run diagnostics preserved without line-window guesses.
    pub run_diagnostics: Vec<Diagnostic>,
    /// Whether diagnostic/function linkage is authoritative.
    pub diagnostic_attribution_complete: bool,
    /// Informational limitations, never an invented missing model or verdict.
    pub notes: Vec<String>,
}

/// An immutable independently retained baseline snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// Baseline contract version.
    pub version: u32,
    /// Owner-selected immutable name.
    pub name: String,
    /// Original job identity.
    pub job_id: String,
    /// Independently completed evidence, unchanged by later runs or job pruning.
    pub result: ProjectResult,
}

/// Classification of one comparison entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Same reported evidence class.
    Unchanged,
    /// A previously verified function now reports a settled failure.
    Regressed,
    /// A previously verified function no longer has a confirmed proof.
    BecameUnknown,
    /// A previously unproved function is now confirmed verified.
    Recovered,
    /// Newly observed function.
    Added,
    /// No matching evidence: partial scopes do not establish removal.
    NotObserved,
    /// Scope/runtime/mode differs; compare descriptively, not as a regression.
    Incomparable,
    /// Other non-proof transition.
    Changed,
}

/// One target/function transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Difference {
    /// Target identity including library/binary kind.
    pub target: Target,
    /// Exact function identity.
    pub function: String,
    /// Earlier verdict, absent when not observed.
    pub before: Option<VerdictName>,
    /// Later verdict, absent when not observed.
    pub after: Option<VerdictName>,
    /// Conservative comparison classification.
    pub change: Change,
}

/// Comparison of explicit completed evidence; not a new proof of current inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Independent contract version.
    pub version: u32,
    /// True only when recorded contexts, modes and requested scopes agree.
    pub compatible: bool,
    /// Earlier source revision.
    pub before_revision: String,
    /// Later source revision.
    pub after_revision: String,
    /// Later evidence currentness, never assumed from terminal lifecycle.
    pub after_currentness: Currentness,
    /// Differences and unchanged identities.
    pub differences: Vec<Difference>,
    /// Why comparisons cannot be interpreted as proof regressions.
    pub notes: Vec<String>,
}
