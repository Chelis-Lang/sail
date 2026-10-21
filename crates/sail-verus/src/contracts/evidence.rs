//! Conservative projection of actual backend evidence. Legacy verdicts remain intact.

use std::collections::BTreeSet;

use sail_api::output::{
    AnswerName, AttributionStatus, EstablishmentName, FunctionReport, OutcomeName, VerdictName,
};
use sail_api::project::{Currentness, ProjectResult};
use serde::{Deserialize, Serialize};

use super::LoweringManifest;

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
    /// Sorted exact bound functions with complete nonadvisory proof evidence.
    pub proved_functions: Vec<String>,
}

pub(super) fn summarize(manifest: &LoweringManifest, backend: &ProjectResult) -> ContractProof {
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
    let targets = backend
        .targets
        .iter()
        .filter(|t| t.requested)
        .collect::<Vec<_>>();
    let mut proved = BTreeSet::new();
    for binding in &manifest.bindings {
        let matches = targets
            .iter()
            .flat_map(|t| &t.report.crates)
            .flat_map(|c| &c.functions)
            .filter(|f| f.function == binding.lowered_function)
            .collect::<Vec<_>>();
        if matches.len() == 1 && function_proved(matches[0]) {
            proved.insert(binding.lowered_function.clone());
        }
    }
    let complete = backend.currentness == Currentness::Current
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
        && !manifest.bindings.is_empty()
        && manifest
            .bindings
            .iter()
            .all(|b| proved.contains(&b.lowered_function))
        && checks.unsat > 0
        && checks.sat == 0
        && checks.unknown == 0
        && checks.missing_answer == 0
        && checks.missing_source == 0;
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

fn function_proved(function: &FunctionReport) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_unknown_is_not_promoted_by_a_legacy_failed_verdict()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = super::super::lower(
            include_str!("../../tests/fixtures/neutral-contracts/add_one.rs"),
            "neutral",
        )?
        .manifest;
        // Exact requested-target projections of the recorded native helper run.
        let good: ProjectResult = serde_json::from_str(include_str!(
            "../../tests/fixtures/neutral-contracts/positive-requested-target.json"
        ))?;
        let bad: ProjectResult = serde_json::from_str(include_str!(
            "../../tests/fixtures/neutral-contracts/body-edit-requested-target.json"
        ))?;
        assert_eq!(summarize(&manifest, &good).state, ProofState::Proved);
        let failed = summarize(&manifest, &bad);
        assert_eq!(failed.state, ProofState::NotProved);
        assert_eq!(failed.checks.unknown, 1);
        assert!(failed.unknown_reasons[0].contains("incomplete quantifiers"));
        let mut missing = good.clone();
        missing.targets.clear();
        assert_eq!(summarize(&manifest, &missing).state, ProofState::NotProved);
        let mut wrong = good.clone();
        wrong.targets[0].report.crates[0].functions[0].function = "neutral::other".into();
        assert_eq!(summarize(&manifest, &wrong).state, ProofState::NotProved);
        let mut partial = good.clone();
        partial.targets[0].report.attribution.status = AttributionStatus::Partial;
        assert_eq!(summarize(&manifest, &partial).state, ProofState::NotProved);
        let mut advisory = good;
        for obligation in advisory.targets[0].report.crates[0].functions[0]
            .obligations
            .as_mut()
            .into_iter()
            .flatten()
        {
            obligation.advisory = true;
        }
        assert_eq!(summarize(&manifest, &advisory).state, ProofState::NotProved);
        Ok(())
    }
}
