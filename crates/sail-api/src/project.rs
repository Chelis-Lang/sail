//! Managed workspace verification results, independently versioned from reports.

use serde::{Deserialize, Serialize};

use crate::output::Report;

/// Current version of the managed project-result envelope.
pub const SCHEMA_VERSION: u32 = 1;

/// A complete invocation of managed Cargo verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectResult {
    /// Document discriminator, always `sail-project-result`.
    pub kind: String,
    /// Envelope version; embedded reports retain their own schema version.
    pub schema_version: u32,
    /// Unique establishing invocation identifier.
    pub run_id: String,
    /// Canonical Cargo workspace root.
    pub workspace: String,
    /// Verification configuration and executable identity digest.
    pub context_sha256: String,
    /// Content digest of the declared input revision when the run started.
    pub input_revision: String,
    /// Whether inputs remained unchanged while the invocation ran.
    pub currentness: Currentness,
    /// Cargo-Verus process exit status, absent on signal termination.
    pub exit_code: Option<i32>,
    /// Reports for every selected verification target observed by Cargo.
    pub targets: Vec<TargetResult>,
    /// Measurements of this invocation, never replayed historical counters.
    pub execution: Execution,
    /// Coverage limitations or recovery notes.
    pub notes: Vec<String>,
    /// Setup or infrastructure failure, never a solver verdict.
    pub error: Option<crate::output::ErrorInfo>,
    /// Explicitly selected source engine (ADR 0027); absent for stock runs.
    /// Each target report records whether the engine or its stock fallback
    /// established it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineIdentity>,
}

/// The validated source engine and preparation-reuse policy of a project run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineIdentity {
    /// The engine name (`interactive`).
    pub name: String,
    /// SHA-256 of the validated engine build manifest, as lowercase hex.
    pub manifest_sha256: String,
    /// Whether opt-in persistent cross-edit prepared-lowering reuse was enabled.
    pub persistent_preparation: bool,
}

/// Relationship of a result to the workspace's current declared inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Currentness {
    /// Declared inputs did not change during verification.
    Current,
    /// Inputs changed; the establishing result is historical.
    Historical,
    /// Input coverage could not be established.
    Unknown,
}

/// One Cargo target's proof report, with an explicit establishing invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetResult {
    /// Cargo's package ID, including version and source.
    pub package_id: String,
    /// Cargo target name.
    pub target: String,
    /// Target kind (`lib`, `bin`, and so on).
    pub kind: Vec<String>,
    /// Invocation which actually established the embedded report.
    pub establishing_run: String,
    /// True when Cargo's fresh artifact and its complete proof receipt matched.
    pub reused: bool,
    /// True for an explicitly selected verification target; false for dependencies.
    #[serde(default)]
    pub requested: bool,
    /// True only for successful, complete target verification.
    pub complete: bool,
    /// Original report; its timing and provenance belong to `establishing_run`.
    pub report: Report,
}

/// Current invocation's execution counters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    /// Number of target verification subprocesses started in this invocation.
    pub verifier_invocations: u64,
    /// Number of targets whose reports were reused from complete receipts.
    pub reused_targets: u64,
    /// Total wall-clock time including Cargo and input validation.
    pub wall_ms: f64,
}
