//! Recorded report classification controls; no new backend execution is claimed.
use sail_api::{
    contracts::{Evidence, LoweringManifest, TranslationTrust, WorkerResult},
    mutation::{self, Candidate, Outcome},
};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn recorded(positive: bool) -> TestResult<WorkerResult> {
    let manifest: LoweringManifest = serde_json::from_str(include_str!(
        "../../sail-verus/tests/fixtures/neutral-contracts/add_one.v1-manifest.json"
    ))?;
    let backend = serde_json::from_str(if positive {
        include_str!(
            "../../sail-verus/tests/fixtures/neutral-contracts/positive-requested-target.json"
        )
    } else {
        include_str!(
            "../../sail-verus/tests/fixtures/neutral-contracts/body-edit-requested-target.json"
        )
    })?;
    let proof = sail_api::assurance::observed_contract_proof(&manifest, &backend);
    Ok(WorkerResult {
        version: 1,
        backend,
        contracts: Evidence {
            version: 1,
            manifest,
            proof,
            translation_trust: TranslationTrust {
                source_policy_sha256: "a".repeat(64),
                metadata_sha256: "b".repeat(64),
                cargo_lock_sha256: "c".repeat(64),
            },
        },
    })
}
fn candidate(result: &WorkerResult) -> Candidate {
    let binding = &result.contracts.manifest.bindings[0];
    Candidate {
        id: "candidate".into(),
        function: binding.function.clone(),
        requirement_id: binding.requirement_id.clone(),
        guarantee_sha256: binding.guarantee_sha256.clone(),
        replacement: "0".into(),
        body_start: 0,
        body_end: 0,
        source: sail_api::contracts::Submission {
            source: "recorded source identity only; execution not claimed".into(),
            source_sha256: result.contracts.manifest.original_sha256.clone(),
            crate_name: "neutral".into(),
        },
    }
}
#[test]
fn actual_recorded_unknown_is_inconclusive_even_with_failed_legacy_verdict() -> TestResult {
    let baseline = recorded(true)?;
    let mutant = recorded(false)?;
    assert!(mutant.contracts.proof.checks.unknown > 0);
    assert_eq!(
        mutation::classify(&baseline, &mutant, &candidate(&mutant)).0,
        Outcome::Inconclusive
    );
    Ok(())
}
#[test]
fn changed_guarantee_tool_or_missing_evidence_can_never_be_killed() -> TestResult {
    let baseline = recorded(true)?;
    let mutant = recorded(false)?;
    let mut changed = candidate(&mutant);
    changed.guarantee_sha256 = "0".repeat(64);
    assert_eq!(
        mutation::classify(&baseline, &mutant, &changed).0,
        Outcome::Inconclusive
    );
    let mut changed = mutant.clone();
    changed.contracts.proof.checks.unknown = 0;
    assert_eq!(
        mutation::classify(&baseline, &changed, &candidate(&changed)).0,
        Outcome::Inconclusive
    );
    let mut changed = baseline.clone();
    changed.backend.targets[0].report.solver = None;
    assert_eq!(
        mutation::classify(&baseline, &changed, &candidate(&changed)).0,
        Outcome::Inconclusive
    );
    Ok(())
}

#[test]
#[allow(
    clippy::items_after_statements,
    reason = "The fixture-specific record encoding is local to this recorded receipt control."
)]
fn complete_recorded_receipt_revalidates_and_refuses_member_or_gate_tampering() -> TestResult {
    use mutation::{OriginalEvidence, Plan, State, Submission, View};
    let original = include_str!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.rs");
    let baseline = recorded(true)?;
    let source = sail_api::contracts::Submission {
        source: original.into(),
        source_sha256: mutation::digest(original.as_bytes()),
        crate_name: "neutral".into(),
    };
    let start = original.find("{\n").ok_or("body start")?;
    let end = original.rfind('}').ok_or("body end")? + 1;
    let mut changed = original.to_owned();
    changed.replace_range(start..end, "{ 0 }");
    let mut candidate = candidate(&baseline);
    candidate.replacement = "0".into();
    candidate.body_start = start;
    candidate.body_end = end;
    candidate.source = sail_api::contracts::Submission {
        source_sha256: mutation::digest(changed.as_bytes()),
        source: changed,
        crate_name: "neutral".into(),
    };
    candidate.id = mutation::digest(
        format!(
            "{}:{}:{}:0",
            mutation::POLICY,
            source.source_sha256,
            candidate.function
        )
        .as_bytes(),
    );
    let submission = Submission {
        baseline_job_id: "original-job".into(),
        max_mutants: 8,
        wall_time_ms: 1000,
    };
    let plan = Plan {
        version: 1,
        policy: mutation::POLICY.into(),
        baseline_source_sha256: source.source_sha256.clone(),
        candidates: vec![candidate.clone()],
        unsupported: vec![],
        omitted: 0,
    };
    // Match the service record's declared serialization order, rather than a map's key order.
    #[derive(serde::Serialize)]
    struct Record<'a> {
        version: u32,
        submission: &'a Submission,
        plan: &'a Plan,
        jobs: Vec<&'a str>,
        started_at_ms: u64,
    }
    let id = mutation::digest(&serde_json::to_vec(&Record {
        version: 1,
        submission: &submission,
        plan: &plan,
        jobs: vec!["mutant-job"],
        started_at_ms: 1,
    })?);
    let view = View {
        version: 1,
        id: id.clone(),
        started_at_ms: 1,
        submission,
        baseline_source: source.clone(),
        baseline_original: OriginalEvidence {
            source,
            worker_stdout: serde_json::to_string(&baseline)?,
            worker_stderr: String::new(),
        },
        baseline_evidence_name: format!("m-{id}-b"),
        baseline_backend_sha256: mutation::digest(&serde_json::to_vec(&baseline.backend)?),
        plan,
        state: State::Interrupted,
        observations: vec![mutation::Observation {
            candidate,
            job_id: "mutant-job".into(),
            evidence_name: None,
            backend_sha256: None,
            original: None,
            outcome: Outcome::Inconclusive,
            reason: "actual source job has no retained conclusive evidence".into(),
        }],
        passed: false,
        qualifiers: mutation::qualifiers(),
    };
    mutation::validate_view(&view)?;
    let mut changed = view.clone();
    changed.passed = true;
    assert!(mutation::validate_view(&changed).is_err());
    let mut changed = view.clone();
    changed.observations[0].outcome = Outcome::Killed;
    assert!(mutation::validate_view(&changed).is_err());
    let mut changed = view.clone();
    changed.baseline_original.worker_stdout.push('x');
    assert!(mutation::validate_view(&changed).is_err());
    let mut changed = view;
    changed.observations[0].candidate.source.source.push(' ');
    assert!(mutation::validate_view(&changed).is_err());
    Ok(())
}
