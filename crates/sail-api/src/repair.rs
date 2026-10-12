//! Focused repair checks and complete validation.
//!
//! Explanations, baselines and comparisons are in [`crate::evidence`].

use serde::{Deserialize, Serialize};

use crate::project::{Currentness, ProjectResult};

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
    /// stock fallback after an engine fault.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.selection_error.is_none() && self.engine_fault.is_none() && accepted(&self.full)
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
                && !target.report.crates.is_empty()
                && target.report.engine.as_ref().is_none_or(|engine| {
                    engine.established_by == crate::output::EngineEstablishment::Engine
                })
                && target.report.crates.iter().all(|report| {
                    report.verus.success == Some(true)
                        && report.verus.encountered_vir_error != Some(true)
                })
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
