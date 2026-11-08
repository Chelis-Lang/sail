//! Requirement-aware views derived from authenticated neutral source-job evidence.
//!
//! This interface describes the submitted immutable source snapshot. A client must
//! separately establish that its live source still matches, and establish any
//! contract-to-requirement entailment. Reading a view never runs a verifier.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::contracts::{CheckCounts, ContractProof, LoweringManifest, ProofState};
use crate::jobs::{Lifecycle, ResultDocument};
use crate::output::{
    AnswerName, AttributionStatus, EstablishmentName, FunctionReport, OutcomeName, SolverInfo,
    VerdictName,
};
use crate::project::{Currentness, ProjectResult};

/// Requirement assurance view contract version.
pub const VERSION: u32 = 2;

/// Evidence strength, independent of establishment and source currentness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceTier {
    /// Observations of executed test cases.
    Sampled,
    /// Exhaustive checking within declared finite bounds.
    Bounded,
    /// An attributed SMT-backed implementation proof.
    SmtVerified,
    /// An independently checked proof certificate.
    CertificateChecked,
    /// A proof checked by a trusted proof-assistant kernel.
    KernelChecked,
}

/// Established relationship; matching requirement identities cannot add one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relationship {
    /// Exact submitted implementation satisfies its stated neutral contract.
    ContractToCode,
}

/// Evidence applies to a sealed source snapshot, not implicitly to live files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// The source whose exact bytes were submitted and authenticated by Sail.
    SourceSnapshot,
}

/// Mutation observations remain distinct from the underlying proof verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MutationStatus {
    /// No mutation campaign is established by this source-job result.
    NotEstablished,
    /// A reauthenticated bounded campaign, separate from the underlying proof tier.
    Bounded {
        /// Exact durable campaign identity.
        campaign_id: String,
        /// Versioned admitted mutation policy.
        policy: String,
        /// Whole campaign passes only with every candidate conclusively killed.
        passed: bool,
        /// Actual target-function conclusive verifier rejections.
        killed: usize,
        /// Actual target-function whole proofs.
        survived: usize,
        /// Missing, unknown or unexecuted target observations.
        inconclusive: usize,
    },
}

/// One exact guarantee, its implementation proof and retained provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Content-derived identity, independent of source line numbering.
    pub obligation_id: String,
    /// The relationship this evidence actually checks.
    pub relationship: Relationship,
    /// Exact original qualified function name.
    pub function: String,
    /// Exact source snapshot digest.
    pub source_sha256: String,
    /// Exact guarantee syntax digest from the authenticated lowering manifest.
    pub guarantee_sha256: String,
    /// Exact generated shadow digest.
    pub lowered_sha256: String,
    /// Original backend report digest, not an invented verifier receipt.
    pub backend_sha256: String,
    /// The conservative whole-job proof outcome, never a source refutation.
    pub outcome: ProofState,
    /// Present only for complete confirmed evidence with actual tool identity.
    pub establishment: Option<EstablishmentName>,
    /// Present only when that strength has actually been established.
    pub tier: Option<EvidenceTier>,
    /// Scope of the proof; live source matching is a client responsibility.
    pub scope: Scope,
    /// All actual distinct solver identities, sorted canonically.
    pub solvers: Vec<SolverInfo>,
    /// Explicit assumptions, trust and missing relationships.
    pub qualifiers: Vec<String>,
    /// Mutation establishment is never inferred from a passing proof.
    pub mutation: MutationStatus,
    /// Source-named inputs from the solver model of the function's first failed
    /// or unknown check; absent when proved, without a model, or from older services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<crate::counterexample::Decoded>,
}

/// Evidence grouped by stable requirement identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    /// Original owner-selected identity carried by a neutral contract.
    pub requirement_id: String,
    /// Sorted exact function/guarantee relationships for this identity.
    pub evidence: Vec<Evidence>,
}

/// Versioned read-only view over one authenticated neutral source job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// Equals [`VERSION`].
    pub version: u32,
    /// Exact retained job identity.
    pub job_id: String,
    /// Currentness of caller files is unknown to a source-snapshot service.
    pub currentness: Currentness,
    /// Requirement groups sorted by stable identity.
    pub requirements: Vec<Requirement>,
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Project an already authenticated source-job result, checking its raw proof
/// projection again. The daemon authenticates source and stored output seals
/// before calling this function; caller-created documents are not authentication.
///
/// # Errors
/// Refuses unfinished jobs, missing evidence, mismatched attribution and invalid
/// content identities. Unknown and incomplete proofs retain no established tier.
pub fn from_contract_result(document: &ResultDocument) -> Result<View, String> {
    from_contract_result_named(document, &crate::counterexample::FieldNames::new())
}

/// [`from_contract_result`], naming decoded struct fields from the retained
/// source's declaration order instead of by position.
///
/// # Errors
/// As [`from_contract_result`].
#[allow(
    clippy::too_many_lines,
    reason = "One projection checks all attribution before constructing any assurance row."
)]
pub fn from_contract_result_named(
    document: &ResultDocument,
    fields: &crate::counterexample::FieldNames,
) -> Result<View, String> {
    let invalid = || "SAIL3143: invalid or incomplete neutral assurance evidence".to_owned();
    let contracts = document.contracts.as_deref().ok_or_else(invalid)?;
    let backend: ProjectResult =
        serde_json::from_value(document.result.clone().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
    let manifest = &contracts.manifest;
    if document.job.version != crate::jobs::VERSION
        || document.job.state != Lifecycle::Completed
        || document.job.id.is_empty()
        || document.job.error.is_some()
        || document.repair.is_some()
        || contracts.version != crate::contracts::VERSION
        || !matches!(
            (manifest.version, manifest.contract_version),
            (1 | 2, 1) | (3, 2) | (4, 3)
        )
        || manifest.bindings.is_empty()
        || document.job.input_revision.as_deref() != Some(manifest.original_sha256.as_str())
        || !valid_digest(&manifest.original_sha256)
        || !valid_digest(&manifest.lowered_sha256)
        || !valid_digest(&contracts.translation_trust.source_policy_sha256)
        || !valid_digest(&contracts.translation_trust.metadata_sha256)
        || !valid_digest(&contracts.translation_trust.cargo_lock_sha256)
        || backend.kind != "sail-project-result"
        || backend.schema_version != crate::project::SCHEMA_VERSION
        || document.job.exit_code.is_none()
        || document.job.exit_code != backend.exit_code
        || contracts.proof != observed_contract_proof(manifest, &backend)
    {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    for binding in &manifest.bindings {
        if binding.requirement_id.is_empty()
            || binding.function.is_empty()
            || binding.lowered_function.is_empty()
            || !valid_digest(&binding.guarantee_sha256)
            || !seen.insert((&binding.requirement_id, &binding.function))
        {
            return Err(invalid());
        }
    }
    let backend_sha256 = digest(&serde_json::to_vec(&backend).map_err(|_| invalid())?);
    let (complete_identity, solvers) = solver_identities(&backend).map_err(|()| invalid())?;
    let confirmed = contracts.proof.state == ProofState::Proved && complete_identity;
    let mut grouped: BTreeMap<String, Vec<Evidence>> = BTreeMap::new();
    for binding in &manifest.bindings {
        let mut qualifiers = vec![
            "applies to the exact submitted source snapshot under its stated preconditions".into(),
            format!(
                "neutral lowering version {}: {}",
                manifest.version, manifest.arithmetic
            ),
            "neutral source translation and the pinned Verus/SMT toolchain are trusted".into(),
        ];
        qualifiers.extend(SHARED_QUALIFIERS.map(String::from));
        if !complete_identity {
            qualifiers.push(NO_IDENTITY_QUALIFIER.into());
        }
        qualifiers.sort();
        let identity = serde_json::to_vec(&(
            &manifest.original_sha256,
            &binding.function,
            &binding.guarantee_sha256,
            &backend.context_sha256,
        ))
        .map_err(|_| invalid())?;
        grouped
            .entry(binding.requirement_id.clone())
            .or_default()
            .push(Evidence {
                obligation_id: format!("neutral:{}", digest(&identity)),
                relationship: Relationship::ContractToCode,
                function: binding.function.clone(),
                source_sha256: manifest.original_sha256.clone(),
                guarantee_sha256: binding.guarantee_sha256.clone(),
                lowered_sha256: manifest.lowered_sha256.clone(),
                backend_sha256: backend_sha256.clone(),
                outcome: contracts.proof.state,
                establishment: confirmed.then_some(EstablishmentName::Confirmed),
                tier: confirmed.then_some(EvidenceTier::SmtVerified),
                scope: Scope::SourceSnapshot,
                solvers: solvers.clone(),
                qualifiers,
                mutation: MutationStatus::NotEstablished,
                counterexample: (contracts.proof.state != ProofState::Proved)
                    .then(|| unique_function(&backend, &binding.lowered_function))
                    .flatten()
                    .and_then(|function| crate::counterexample::decode(function, fields)),
            });
    }
    let requirements = grouped
        .into_iter()
        .map(|(requirement_id, mut evidence)| {
            evidence.sort_by(|a, b| a.obligation_id.cmp(&b.obligation_id));
            Requirement {
                requirement_id,
                evidence,
            }
        })
        .collect();
    Ok(View {
        version: VERSION,
        job_id: document.job.id.clone(),
        currentness: Currentness::Unknown,
        requirements,
    })
}

/// The single requested-target report for `function`, if exactly one exists.
fn unique_function<'a>(backend: &'a ProjectResult, function: &str) -> Option<&'a FunctionReport> {
    let mut matches = backend
        .targets
        .iter()
        .filter(|t| t.requested)
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .filter(|f| f.function == function);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// Recompute the conservative neutral proof projection from original backend records.
///
/// This is shared by source-job validation and assurance views.
#[must_use]
pub fn observed_contract_proof(
    manifest: &LoweringManifest,
    backend: &ProjectResult,
) -> ContractProof {
    let (checks, reasons) = check_counts(backend);
    let targets = requested_targets(backend);
    let mut proved = BTreeSet::new();
    for binding in &manifest.bindings {
        if exactly_one_proved(&targets, &binding.lowered_function) {
            proved.insert(binding.lowered_function.clone());
        }
    }
    let complete = run_complete(backend, &checks)
        && !manifest.bindings.is_empty()
        && manifest
            .bindings
            .iter()
            .all(|b| proved.contains(&b.lowered_function));
    ContractProof {
        state: if complete {
            ProofState::Proved
        } else {
            ProofState::NotProved
        },
        checks,
        unknown_reasons: reasons.into_iter().collect(),
        proved_functions: proved.into_iter().collect(),
    }
}

/// Every recorded check answer, including advisory obligations and dependency targets.
pub(crate) fn check_counts(backend: &ProjectResult) -> (CheckCounts, BTreeSet<String>) {
    let mut checks = CheckCounts::default();
    let mut reasons = BTreeSet::new();
    for check in backend
        .targets
        .iter()
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .filter_map(|f| f.obligations.as_ref())
        .flatten()
        .flat_map(|o| &o.checks)
    {
        match check.answer {
            Some(AnswerName::Unsat) => checks.unsat += 1,
            Some(AnswerName::Sat) => checks.sat += 1,
            Some(AnswerName::Unknown) => {
                checks.unknown += 1;
                if let Some(reason) = &check.reason_unknown {
                    reasons.insert(reason.clone());
                }
            }
            None => checks.missing_answer += 1,
        }
        checks.missing_source += usize::from(check.source.is_none());
    }
    (checks, reasons)
}

pub(crate) fn requested_targets(backend: &ProjectResult) -> Vec<&crate::project::TargetResult> {
    backend.targets.iter().filter(|t| t.requested).collect()
}

/// Whether exactly one requested-target function has this exact name and it is proved.
pub(crate) fn exactly_one_proved(targets: &[&crate::project::TargetResult], name: &str) -> bool {
    let matches = targets
        .iter()
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .filter(|f| f.function == name)
        .collect::<Vec<_>>();
    matches.len() == 1 && function_proved(matches[0])
}

/// Whole-run completeness: one passing function cannot promote an incomplete run.
pub(crate) fn run_complete(backend: &ProjectResult, checks: &CheckCounts) -> bool {
    let targets = requested_targets(backend);
    backend.currentness == Currentness::Current
        && backend.error.is_none()
        && backend.no_reuse.is_none()
        && backend.exit_code == Some(0)
        && backend.targets.iter().all(|t| {
            t.complete
                && t.report.attribution.status == AttributionStatus::Complete
                && t.report.attribution.unmatched_sessions == 0
                && t.report.invariant_violations().is_empty()
        })
        && !targets.is_empty()
        && targets.iter().all(|t| {
            t.complete
                && t.report.error.is_none()
                && t.report.exit_code == Some(0)
                && !t.report.crates.is_empty()
                && t.report.crates.iter().all(|c| {
                    c.verus.success == Some(true)
                        && c.verus.encountered_vir_error == Some(false)
                        && c.verus.errors == Some(0)
                })
        })
        && checks.unsat > 0
        && checks.sat == 0
        && checks.unknown == 0
        && checks.missing_answer == 0
        && checks.missing_source == 0
}

/// Complete actual solver identity across every target, with the sorted distinct identities.
pub(crate) fn solver_identities(backend: &ProjectResult) -> Result<(bool, Vec<SolverInfo>), ()> {
    let mut solvers = BTreeMap::new();
    let mut complete_identity =
        !backend.targets.is_empty() && valid_digest(&backend.context_sha256);
    for target in &backend.targets {
        if let Some(solver) = &target.report.solver {
            complete_identity &= !solver.name.is_empty()
                && solver.version.as_ref().is_some_and(|v| !v.is_empty())
                && solver.sha256.as_deref().is_some_and(valid_digest);
            solvers.insert(
                serde_json::to_string(solver).map_err(|_| ())?,
                solver.clone(),
            );
        } else {
            complete_identity = false;
        }
    }
    Ok((complete_identity, solvers.into_values().collect()))
}

/// Qualifiers shared by every requirement assurance row, sorted by the caller.
pub(crate) const SHARED_QUALIFIERS: [&str; 2] = [
    "contract-to-requirement entailment and required-input feasibility are not established",
    "mutation assurance and termination are not established by this result",
];

/// Added when solver identity is incomplete.
pub(crate) const NO_IDENTITY_QUALIFIER: &str =
    "complete original solver identity is unavailable; no tier is established";

pub(crate) fn function_proved(function: &FunctionReport) -> bool {
    if function.verdict != VerdictName::Verified
        || function.establishment != EstablishmentName::Confirmed
        || function.unstable
        || function.verus_success != Some(true)
        || function.provenance.checks_unrecorded != 0
    {
        return false;
    }
    let Some(obligations) = &function.obligations else {
        return false;
    };
    let primary = obligations
        .iter()
        .filter(|o| !o.advisory)
        .collect::<Vec<_>>();
    !primary.is_empty()
        && primary.iter().all(|o| {
            o.outcome == OutcomeName::Proved
                && !o.unstable
                && !o.checks.is_empty()
                && o.checks
                    .iter()
                    .all(|c| c.answer == Some(AnswerName::Unsat) && c.source.is_some())
        })
}
