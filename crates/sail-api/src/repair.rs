//! Focused repair checks and complete validation.
//!
//! Explanations, baselines and comparisons are in [`crate::evidence`].

use serde::{Deserialize, Serialize};

use crate::project::{Currentness, ProjectResult};
use crate::proof::Target;

/// Independent repair contract version.
pub const VERSION: u32 = 1;

/// Source-reported preparation counters for one observed Verus driver.
///
/// These are preparation observations, not proof evidence or process RSS.
/// Records remain separate: peaks and budgets from different drivers are not
/// summed into a claimed aggregate. An older engine supplies no record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationStats {
    /// Supported source telemetry contract version (currently one).
    #[serde(deserialize_with = "preparation_version")]
    pub version: u32,
    /// Whether preparation reuse was enabled for this driver.
    pub enabled: bool,
    /// Hits restored from the in-memory preparation entry inside this batch.
    pub batch_hits: u64,
    /// Hits restored from validated persistent files. A file may have been
    /// written earlier in this same batch; this is not an across-edit counter.
    pub persistent_hits: u64,
    /// Source-reported preparation misses, including rejected admission.
    pub misses: u64,
    /// Peak bytes charged to this driver's retained preparation state.
    /// This counter is not operating-system resident memory.
    pub peak_retained_bytes: u64,
    /// Configured retained preparation memory budget, in bytes.
    pub memory_budget_bytes: u64,
    /// Configured persisted preparation storage budget, in bytes.
    pub disk_budget_bytes: u64,
    /// Exact driver target when independently bound to observed inventory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// Phase when the source counter applies to only that phase.
    /// Absent for a summary spanning focused and full verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<crate::proof::Phase>,
}

fn preparation_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(deserializer)?;
    if version == VERSION {
        Ok(version)
    } else {
        Err(serde::de::Error::custom(
            "unsupported preparation telemetry version",
        ))
    }
}

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
    /// Exact source-reported summaries, one per observed driver.
    /// `None` means unavailable, never an invented collection of zero counters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation: Option<Vec<PreparationStats>>,
}

impl BatchResult {
    /// Whether selection succeeded and full validation established current complete evidence.
    /// Preliminary focus success cannot authorize a repair.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.selection_error.is_none() && accepted(&self.full)
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
