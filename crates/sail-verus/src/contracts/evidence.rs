//! Conservative projection of original backend evidence, shared with assurance views.

#[cfg(test)]
use super::ProofState;
use super::{ContractProof, LoweringManifest};
use sail_api::project::ProjectResult;

pub(super) fn summarize(manifest: &LoweringManifest, backend: &ProjectResult) -> ContractProof {
    sail_api::assurance::observed_contract_proof(manifest, backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_api::output::AttributionStatus;

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
