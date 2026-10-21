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
    /// Present when Sail could not bind this invocation's setup to a proof
    /// receipt (`SAIL3112`, ADR 0046). Such a run reuses and records no proof
    /// receipts, and its result is never `current`. Absent for runs that
    /// can reuse proofs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_reuse: Option<NoReuse>,
}

/// Name of the one target of a run verified as plain `cargo verus verify`
/// ([`VerifiedAs::PlainCargoVerus`]): its report covers the whole run.
pub const PLAIN_TARGET: &str = "cargo verus verify";

/// Why and how a run was verified without proof reuse (`SAIL3112`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoReuse {
    /// Always `SAIL3112`.
    pub code: String,
    /// The setup Sail could not bind to a proof receipt.
    pub reason: String,
    /// How the run was verified.
    pub verified_as: VerifiedAs,
}

/// How a run without proof reuse was verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerifiedAs {
    /// Exactly as plain `cargo verus verify` verifies it, in the invocation's
    /// own Cargo target, with Sail's solver cache. `targets` holds one
    /// requested target named [`PLAIN_TARGET`] whose report covers every
    /// crate Verus verified; a crate Cargo found up to date has no report.
    /// `context_sha256` and `input_revision` are empty.
    PlainCargoVerus,
    /// By managed verification in Sail's own target, with per-target
    /// reports (a process-loader variable is set).
    Managed,
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

/// Where a compiler-boundary dependency proof was established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyBuildKind {
    /// A verified dependency packaged with a release.
    Release,
    /// An earlier full verification on this machine, after fresh native compilation.
    MachineLocal,
}

/// Provenance retained when a dependency invocation replays a complete proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyBuildOrigin {
    /// Scope of the dependency build.
    pub kind: DependencyBuildKind,
    /// Runtime identity and exact compiler-boundary entry key.
    pub identity: String,
    /// Invocation which actually established the proof, rather than its replay.
    pub establishing_run: String,
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
    /// Exact invocation arguments when an unchanged partial-scope report was
    /// reused. This never establishes a complete target proof and does not
    /// contribute to `Execution::reused_targets`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_reuse: Option<Vec<String>>,
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
    /// Origin of a replayed dependency proof (ADR 0048), preserved through
    /// later receipt reuse. Native prerequisites execute before boundary lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_build: Option<DependencyBuildOrigin>,
}

/// Current invocation's execution counters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    /// Number of target verification subprocesses started in this invocation.
    /// For [`VerifiedAs::PlainCargoVerus`], this counts the one aggregate
    /// Cargo-Verus driver result, even if Cargo verifies zero or several crates;
    /// it is zero when setup or the driver fails without an aggregate result.
    /// It does not measure per-crate verifier or solver work in that mode.
    pub verifier_invocations: u64,
    /// Number of targets whose reports were reused from complete receipts.
    pub reused_targets: u64,
    /// Total wall-clock time including Cargo and input validation.
    pub wall_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::{NoReuse, VerifiedAs};

    /// `no_reuse` matches its JSON Schema, and the schema refuses a
    /// document that drops how the run was verified.
    #[test]
    fn no_reuse_matches_its_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../schema/project-result-v2.json"))
                .unwrap_or_default();
        let schema = &schema["properties"]["no_reuse"];
        for verified_as in [VerifiedAs::PlainCargoVerus, VerifiedAs::Managed] {
            let value = serde_json::to_value(NoReuse {
                code: "SAIL3112".to_owned(),
                reason: "`--tests` is not bound to a proof receipt".to_owned(),
                verified_as,
            })
            .unwrap_or_default();
            assert_eq!(
                crate::schema::validate(schema, &value),
                Vec::<String>::new()
            );
            let mut missing = value.clone();
            if let Some(object) = missing.as_object_mut() {
                object.remove("verified_as");
            }
            assert_ne!(
                crate::schema::validate(schema, &missing),
                Vec::<String>::new()
            );
        }
        assert_eq!(
            serde_json::to_value(VerifiedAs::PlainCargoVerus).ok(),
            Some(serde_json::json!("plain-cargo-verus"))
        );
    }
}
