//! Version-one neutral source job inputs and evidence, without backend execution.
use serde::{Deserialize, Serialize};

/// Neutral contract job protocol version.
pub const VERSION: u32 = 1;
/// Maximum complete source file size.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
/// Fixed whole-process safety deadline for a neutral worker.
pub const WALL_TIME_SECS: u64 = 300;
/// Maximum retained exact worker stdout.
pub const MAX_STDOUT_BYTES: usize = 24 * 1024 * 1024;
/// Maximum retained exact worker stderr.
pub const MAX_STDERR_BYTES: usize = 8 * 1024 * 1024;

/// Complete caller input; execution controls belong exclusively to Sail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Explicit neutral source profile; omitted legacy submissions select profile one.
    #[serde(default = "legacy_profile", skip_serializing_if = "is_legacy_profile")]
    pub profile: u32,
    /// Exact UTF-8 original source snapshot.
    pub source: String,
    /// Required SHA-256 of the exact source bytes.
    pub source_sha256: String,
    /// Rust compiler crate identity, not a filesystem path.
    pub crate_name: String,
}
const fn legacy_profile() -> u32 {
    1
}
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde skip_serializing_if callbacks borrow the field."
)]
fn is_legacy_profile(profile: &u32) -> bool {
    *profile == 1
}

impl Submission {
    /// Validate bounded source, exact content identity and a conservative crate name.
    /// # Errors
    /// Returns a source admission diagnostic without starting any process.
    pub fn validate(&self) -> Result<(), String> {
        let expected = sail_key::QueryKey(sail_key::sha256(self.source.as_bytes())).to_string();
        if !matches!(self.profile, 1 | 2)
            || self.source.is_empty()
            || self.source.len() > MAX_SOURCE_BYTES
            || self.source_sha256 != expected
            || self.crate_name.is_empty()
            || self.crate_name.len() > 64
            || !self
                .crate_name
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
        {
            return Err("SAIL3142: invalid neutral source, source digest or crate identity".into());
        }
        Ok(())
    }
}

/// An original source range. Columns are zero-based, lines are one-based.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    /// First source line.
    pub line: usize,
    /// First source column.
    pub column: usize,
    /// Exclusive final source line.
    pub end_line: usize,
    /// Exclusive final source column.
    pub end_column: usize,
}

/// A requirement-to-guarantee binding, before actual backend obligation mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractBinding {
    /// Stable explicit owner requirement identity.
    pub requirement_id: String,
    /// Original fully qualified function identity.
    pub function: String,
    /// Exact generated Verus symbol; evidence must actually name this symbol.
    pub lowered_function: String,
    /// Hash of the exact parsed guarantee syntax in its canonical token rendering.
    pub guarantee_sha256: String,
    /// Original complete function range.
    pub source: SourceRange,
}

/// Content-bound, deterministic original-to-shadow mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringManifest {
    /// Lowering semantics version.
    pub version: u32,
    /// Neutral contract syntax version.
    pub contract_version: u32,
    /// Validated compiler crate name.
    pub crate_name: String,
    /// Exact original UTF-8 source digest.
    pub original_sha256: String,
    /// Exact emitted Verus source digest.
    pub lowered_sha256: String,
    /// Explicit supported arithmetic interpretation.
    pub arithmetic: String,
    /// Sorted one-guarantee function bindings; no invented solver obligation IDs.
    pub bindings: Vec<ContractBinding>,
}

/// Whether the submitted contracts were proved under their stated preconditions.
/// This does not establish requirement entailment, feasibility or a mutation kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofState {
    /// Complete attributed proof evidence for every bound function.
    Proved,
    /// Missing, failed, unknown, unstable or incomplete evidence; not a refutation.
    NotProved,
}

/// Underlying check counts, including advisory obligations and dependency targets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckCounts {
    /// Recorded unsatisfiable answers.
    pub unsat: usize,
    /// Recorded satisfiable answers; no source-level refutation is inferred.
    pub sat: usize,
    /// Recorded unknown answers, even when a legacy verdict says failed.
    pub unknown: usize,
    /// Checks with no recorded answer.
    pub missing_answer: usize,
    /// Checks with no recorded establishment source.
    pub missing_source: usize,
}

/// Exact-function proof projection; the complete original backend report is retained.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractProof {
    /// Conservative aggregate result, with no refuted state.
    pub state: ProofState,
    /// Every underlying recorded check, not just bound-function checks.
    pub checks: CheckCounts,
    /// Sorted distinct verbatim unknown reasons; null reasons remain in the backend.
    pub unknown_reasons: Vec<String>,
    /// Sorted component observations of complete nonadvisory function proofs.
    /// These are not standalone establishment or tiers when the aggregate is `NotProved`.
    pub proved_functions: Vec<String>,
}

/// Exact source-policy and resolution evidence for neutral translation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTrust {
    /// Independently qualified complete vstd and builtin macro source policy.
    pub source_policy_sha256: String,
    /// Actual Cargo metadata bytes used to admit all selected dependency features.
    pub metadata_sha256: String,
    /// Exact dependency lockfile used by the managed verification.
    pub cargo_lock_sha256: String,
}

/// Snapshot contract evidence accompanying the original managed project result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// Exact contract service format version.
    pub version: u32,
    /// Original source, guarantee and generated symbol relationships.
    pub manifest: LoweringManifest,
    /// Conservative snapshot proof; never live-source requirement establishment.
    pub proof: ContractProof,
    /// Actual qualified compiler-loaded source and resolution identity.
    pub translation_trust: TranslationTrust,
}

/// Complete output of Sail's own supervised contract worker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerResult {
    /// Equals VERSION.
    pub version: u32,
    /// Original complete managed report, including failures and unknowns.
    pub backend: crate::project::ProjectResult,
    /// Exact qualified snapshot evidence.
    pub contracts: Evidence,
}
