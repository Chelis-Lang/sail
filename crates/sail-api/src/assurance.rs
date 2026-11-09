//! Evidence-strength vocabulary and the conservative whole-run proof rules
//! shared by requirement attribution ([`crate::attribution`], ADR 0104).
//!
//! Nothing here runs a verifier. Unknown and incomplete proofs never establish
//! a tier.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::output::{
    AnswerName, AttributionStatus, EstablishmentName, FunctionReport, OutcomeName, SolverInfo,
    VerdictName,
};
use crate::project::{Currentness, ProjectResult};

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

/// Whether the attributed function was proved under its stated preconditions.
///
/// `NotProved` covers failed, unknown, incomplete and unattributed evidence; it
/// is never a refutation of the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofState {
    /// Every required record is complete and attributed, with no failure or unknown.
    Proved,
    /// Anything else.
    NotProved,
}

/// Counts of every recorded solver check answer in one backend result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckCounts {
    /// Checks answered `unsat`.
    pub unsat: usize,
    /// Checks answered `sat`.
    pub sat: usize,
    /// Checks answered `unknown`.
    pub unknown: usize,
    /// Checks with no recorded answer.
    pub missing_answer: usize,
    /// Checks with no recorded source.
    pub missing_source: usize,
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
