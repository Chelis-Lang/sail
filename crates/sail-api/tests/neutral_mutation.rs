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

#[allow(
    clippy::items_after_statements,
    reason = "The fixture-specific record encoding is local to this recorded receipt control."
)]
fn recorded_receipt() -> TestResult<(mutation::View, WorkerResult)> {
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
    Ok((view, baseline))
}

#[test]
fn complete_recorded_receipt_revalidates_and_refuses_member_or_gate_tampering() -> TestResult {
    let (view, baseline) = recorded_receipt()?;
    mutation::validate_view(&view)?;
    let mut assurance = original_assurance(&baseline, "original-job")?;
    let tier = assurance.requirements[0].evidence[0].tier;
    mutation::attach_assurance(&view, &mut assurance)?;
    assert_eq!(assurance.requirements[0].evidence[0].tier, tier);
    assert!(matches!(
        assurance.requirements[0].evidence[0].mutation,
        sail_api::assurance::MutationStatus::Bounded {
            passed: false,
            inconclusive: 1,
            ..
        }
    ));
    let mut mismatched = original_assurance(&baseline, "original-job")?;
    mismatched.requirements[0].evidence[0].backend_sha256 = "0".repeat(64);
    assert!(mutation::attach_assurance(&view, &mut mismatched).is_err());

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

#[test]
fn retained_timing_roundtrips_without_changing_baseline_or_mutant_digest() -> TestResult {
    // Recorded classification controls with a timing value observed in an actual
    // campaign. This constructs receipts, not new verifier execution evidence.
    // The two adjacent values decode differently without float_roundtrip.
    let precise = f64::from_bits(0x40dc_0a0c_3276_74d2);
    let lossy = f64::from_bits(0x40dc_0a0c_3276_74d1);
    for baseline_timing in [true, false] {
        let (mut view, mut baseline) = recorded_receipt()?;
        let mut mutant = recorded(false)?;
        let observation = &mut view.observations[0];
        mutant.contracts.manifest.original_sha256 =
            observation.candidate.source.source_sha256.clone();
        mutant.contracts.proof = sail_api::assurance::observed_contract_proof(
            &mutant.contracts.manifest,
            &mutant.backend,
        );
        if baseline_timing {
            baseline.backend.execution.wall_ms = precise;
        } else {
            mutant.backend.execution.wall_ms = precise;
        }
        view.baseline_original.worker_stdout = serde_json::to_string(&baseline)?;
        view.baseline_backend_sha256 = mutation::digest(&serde_json::to_vec(&baseline.backend)?);
        observation.original = Some(mutation::OriginalEvidence {
            source: observation.candidate.source.clone(),
            worker_stdout: serde_json::to_string(&mutant)?,
            worker_stderr: String::new(),
        });
        observation.evidence_name = Some(format!("m-{}-0", view.id));
        observation.backend_sha256 = Some(mutation::digest(&serde_json::to_vec(&mutant.backend)?));
        (observation.outcome, observation.reason) =
            mutation::classify(&baseline, &mutant, &observation.candidate);
        assert_eq!(observation.outcome, Outcome::Inconclusive);
        view.state = mutation::State::Completed;
        let original = if baseline_timing {
            &view.baseline_original
        } else {
            view.observations[0].original.as_ref().ok_or("original")?
        };
        assert!(original.worker_stdout.contains("28712.190580000002"));
        assert_eq!(
            mutation::decode_original(original)?
                .backend
                .execution
                .wall_ms
                .to_bits(),
            precise.to_bits()
        );
        mutation::validate_view(&view)?;
        for _ in 0..4 {
            view = serde_json::from_slice(&serde_json::to_vec(&view)?)?;
            mutation::validate_view(&view)?;
            assert!(!view.passed);
            assert_eq!(view.observations[0].outcome, Outcome::Inconclusive);
        }
        // Old lossy hashes remain inconsistent with the exact retained stream.
        // No fallback, receipt rewriting or timing normalization is permitted.
        if baseline_timing {
            baseline.backend.execution.wall_ms = lossy;
            view.baseline_backend_sha256 =
                mutation::digest(&serde_json::to_vec(&baseline.backend)?);
        } else {
            mutant.backend.execution.wall_ms = lossy;
            view.observations[0].backend_sha256 =
                Some(mutation::digest(&serde_json::to_vec(&mutant.backend)?));
        }
        assert!(mutation::validate_view(&view).is_err());
    }
    Ok(())
}

// These are deliberately edited classification inputs, not an actual new SAT run.
// Keep the real failed Cargo target shape: wrapper101/report1/completefalse.
fn simulated_conclusive_failed_target() -> TestResult<WorkerResult> {
    let mut result = recorded(false)?;
    for check in result
        .backend
        .targets
        .iter_mut()
        .flat_map(|t| &mut t.report.crates)
        .flat_map(|c| &mut c.functions)
        .flat_map(|f| f.obligations.iter_mut().flatten())
        .flat_map(|o| &mut o.checks)
    {
        if check.answer == Some(sail_api::output::AnswerName::Unknown) {
            check.answer = Some(sail_api::output::AnswerName::Sat);
            check.reason_unknown = None;
        }
    }
    result.contracts.proof =
        sail_api::assurance::observed_contract_proof(&result.contracts.manifest, &result.backend);
    Ok(result)
}
#[test]
fn simulated_conclusive_failure_uses_failed_target_completion_and_wrapper_status() -> TestResult {
    let baseline = recorded(true)?;
    let mut mutant = simulated_conclusive_failed_target()?;
    assert_eq!(mutant.backend.exit_code, Some(101));
    assert_eq!(mutant.backend.targets[0].report.exit_code, Some(1));
    assert!(!mutant.backend.targets[0].complete);
    assert_eq!(
        mutation::classify(&baseline, &mutant, &candidate(&mutant)).0,
        Outcome::Killed
    );
    mutant.backend.exit_code = Some(1);
    assert_eq!(
        mutation::classify(&baseline, &mutant, &candidate(&mutant)).0,
        Outcome::Killed
    );
    Ok(())
}
#[test]
fn simulated_failure_still_requires_complete_frontend_attribution_and_known_answers() -> TestResult
{
    type Control = (&'static str, fn(&mut WorkerResult));
    let baseline = recorded(true)?;
    let controls: &[Control] = &[
        ("incomplete verification totals", |r| {
            r.backend.targets[0].report.crates[0].verus.verified = None;
        }),
        ("missing verification outcome", |r| {
            r.backend.targets[0].report.crates[0].verus.success = None;
        }),
        ("compiler error", |r| {
            r.backend.targets[0].report.crates[0]
                .verus
                .encountered_vir_error = Some(true);
        }),
        ("compiler process status", |r| {
            r.backend.targets[0].report.exit_code = Some(101);
        }),
        ("missing process completion", |r| {
            r.backend.targets[0].report.exit_code = None;
        }),
        ("contradictory successful target flag", |r| {
            r.backend.targets[0].complete = true;
        }),
        ("unknown wrapper status", |r| r.backend.exit_code = Some(2)),
        ("partial scope receipt", |r| {
            r.backend.targets[0].scope_reuse = Some(vec!["focus".into()]);
        }),
        ("incomplete attribution", |r| {
            r.backend.targets[0].report.attribution.status =
                sail_api::output::AttributionStatus::Partial;
        }),
        ("missing original solver", |r| {
            r.backend.targets[0].report.solver = None;
        }),
        ("unrecorded checks", |r| {
            r.backend.targets[0].report.crates[0].functions[0]
                .provenance
                .checks_unrecorded = 1;
        }),
        ("unstable rejection", |r| {
            r.backend.targets[0].report.crates[0].functions[0].unstable = true;
        }),
        ("missing obligations", |r| {
            r.backend.targets[0].report.crates[0].functions[0].obligations = None;
        }),
        ("only advisory failure", |r| {
            for o in r.backend.targets[0].report.crates[0].functions[0]
                .obligations
                .iter_mut()
                .flatten()
            {
                o.advisory = true;
            }
        }),
    ];
    for (name, change) in controls {
        let mut mutant = simulated_conclusive_failed_target()?;
        change(&mut mutant);
        mutant.contracts.proof = sail_api::assurance::observed_contract_proof(
            &mutant.contracts.manifest,
            &mutant.backend,
        );
        assert_eq!(
            mutation::classify(&baseline, &mutant, &candidate(&mutant)).0,
            Outcome::Inconclusive,
            "{name}"
        );
    }
    Ok(())
}
fn original_assurance(
    result: &WorkerResult,
    job_id: &str,
) -> TestResult<sail_api::assurance::View> {
    use sail_api::jobs::{Currentness, Lifecycle, ResultDocument, Status};
    Ok(sail_api::assurance::from_contract_result(
        &ResultDocument {
            job: Status {
                version: 1,
                id: job_id.into(),
                project_dir: "recorded-source".into(),
                args: vec![],
                state: Lifecycle::Completed,
                submitted_at_ms: 0,
                started_at_ms: Some(0),
                finished_at_ms: Some(0),
                exit_code: result.backend.exit_code,
                input_revision: Some(result.contracts.manifest.original_sha256.clone()),
                currentness: Currentness::Unknown,
                error: None,
            },
            result: Some(serde_json::to_value(&result.backend)?),
            repair: None,
            contracts: Some(Box::new(result.contracts.clone())),
        },
    )?)
}
