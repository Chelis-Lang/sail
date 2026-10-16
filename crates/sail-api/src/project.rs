//! Managed workspace verification results, independently versioned from reports.

use serde::{Deserialize, Serialize};

use crate::output::Report;

/// Current version of the managed project-result envelope.
pub const SCHEMA_VERSION: u32 = 2;

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

/// The validated source engine of a project run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineIdentity {
    /// The engine name (`interactive`).
    pub name: String,
    /// SHA-256 of the validated engine build manifest, as lowercase hex.
    pub manifest_sha256: String,
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
    /// Invocation which actually established the embedded report. A shipped
    /// proof uses the nonlocal namespace `shipped:<build identity>/<entry key>`;
    /// it does not name a local replay run.
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
    /// Set when this run placed the target's compiled, verified artifacts in a
    /// new Cargo target from Sail's dependency store (ADR 0045): the key of the
    /// store entry. Plain Cargo-Verus would have verified this dependency in
    /// this invocation, so a renderer that reproduces plain output prints its
    /// recorded result here. `establishing_run` names the run, in another
    /// workspace, that verified it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_store: Option<String>,
    /// Set when the target's proof originated in the Sail release's shipped
    /// dependency build (ADR 0048), including later receipt/store reuse:
    /// `<build identity>/<entry key>`. The release build compiled and
    /// verified the target with the same Verus runtime from the same sources,
    /// arguments and environment; plain Cargo-Verus would have verified it in
    /// this invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipped: Option<String>,
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
