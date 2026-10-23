//! Recorded backend projections test tier and relationship admission, not new proof execution.
use sail_api::assurance::{self, EvidenceTier, MutationStatus, Relationship, Scope};
use sail_api::contracts::{Evidence, LoweringManifest, ProofState, TranslationTrust};
use sail_api::jobs::{Currentness, Lifecycle, ResultDocument, Status};
use sail_api::output::EstablishmentName;
use sail_api::project::ProjectResult;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn recorded(positive: bool) -> TestResult<ResultDocument> {
    let manifest: LoweringManifest = serde_json::from_str(include_str!(
        "../../sail-verus/tests/fixtures/neutral-contracts/add_one.v1-manifest.json"
    ))?;
    let backend: ProjectResult = serde_json::from_str(if positive {
        include_str!(
            "../../sail-verus/tests/fixtures/neutral-contracts/positive-requested-target.json"
        )
    } else {
        include_str!(
            "../../sail-verus/tests/fixtures/neutral-contracts/body-edit-requested-target.json"
        )
    })?;
    let proof = assurance::observed_contract_proof(&manifest, &backend);
    let job = Status {
        version: 1,
        id: "retained-source-job".into(),
        project_dir: "/recorded/source-snapshot".into(),
        args: Vec::new(),
        state: Lifecycle::Completed,
        submitted_at_ms: 0,
        started_at_ms: Some(0),
        finished_at_ms: Some(0),
        exit_code: backend.exit_code,
        input_revision: Some(manifest.original_sha256.clone()),
        currentness: Currentness::Unknown,
        error: None,
    };
    Ok(ResultDocument {
        job,
        result: Some(serde_json::to_value(backend)?),
        repair: None,
        contracts: Some(Box::new(Evidence {
            version: 1,
            manifest,
            proof,
            translation_trust: TranslationTrust {
                source_policy_sha256: "a".repeat(64),
                metadata_sha256: "b".repeat(64),
                cargo_lock_sha256: "c".repeat(64),
            },
        })),
    })
}

#[test]
fn complete_recorded_proof_establishes_only_snapshot_contract_to_code() -> TestResult {
    let document = recorded(true)?;
    let view = assurance::from_contract_result(&document)?;
    assert_eq!(view.version, assurance::VERSION);
    assert_eq!(view.currentness, sail_api::project::Currentness::Unknown);
    assert_eq!(view.job_id, document.job.id);
    assert_eq!(view.requirements.len(), 1);
    let evidence = &view.requirements[0].evidence[0];
    assert_eq!(evidence.outcome, ProofState::Proved);
    assert_eq!(evidence.establishment, Some(EstablishmentName::Confirmed));
    assert_eq!(evidence.tier, Some(EvidenceTier::SmtVerified));
    assert_eq!(evidence.relationship, Relationship::ContractToCode);
    assert_eq!(evidence.scope, Scope::SourceSnapshot);
    assert_eq!(evidence.mutation, MutationStatus::NotEstablished);
    assert_ne!(evidence.solvers, [] as [sail_api::output::SolverInfo; 0]);
    assert!(evidence.qualifiers.iter().any(|q| q.contains("entailment")));
    assert!(
        evidence
            .qualifiers
            .iter()
            .any(|q| q.contains("preconditions"))
    );
    assert_eq!(
        serde_json::to_vec(&view)?,
        serde_json::to_vec(&assurance::from_contract_result(&document)?)?
    );
    Ok(())
}

#[test]
fn recorded_unknown_is_not_a_refutation_or_an_established_tier() -> TestResult {
    let mut document = recorded(false)?;
    let view = assurance::from_contract_result(&document)?;
    let evidence = &view.requirements[0].evidence[0];
    assert_eq!(evidence.outcome, ProofState::NotProved);
    assert_eq!(evidence.establishment, None);
    assert_eq!(evidence.tier, None);
    document.contracts.as_mut().ok_or("contracts")?.proof.state = ProofState::Proved;
    assert!(assurance::from_contract_result(&document).is_err());
    Ok(())
}

#[test]
fn missing_tool_identity_and_claimed_live_currentness_cannot_raise_assurance() -> TestResult {
    let mut document = recorded(true)?;
    document.job.currentness = Currentness::Current;
    document.result.as_mut().ok_or("backend")?["targets"][0]["report"]["solver"] =
        serde_json::Value::Null;
    let view = assurance::from_contract_result(&document)?;
    assert_eq!(view.currentness, sail_api::project::Currentness::Unknown);
    let evidence = &view.requirements[0].evidence[0];
    assert_eq!(evidence.tier, None);
    assert_eq!(evidence.establishment, None);
    assert!(
        evidence
            .qualifiers
            .iter()
            .any(|q| q.contains("solver identity"))
    );
    Ok(())
}

#[test]
fn missing_altered_and_duplicate_bindings_do_not_become_evidence() -> TestResult {
    let document = recorded(true)?;
    let mut cases = Vec::new();
    let mut missing = document.clone();
    missing.result = None;
    cases.push(missing);
    let mut stale = document.clone();
    stale.job.input_revision = Some("d".repeat(64));
    cases.push(stale);
    let mut pending = document.clone();
    pending.job.state = Lifecycle::Running;
    cases.push(pending);
    let mut future = document.clone();
    future.job.version += 1;
    cases.push(future);
    let mut contradictory = document.clone();
    contradictory.job.exit_code = Some(1);
    cases.push(contradictory);
    let mut altered = document.clone();
    altered
        .contracts
        .as_mut()
        .ok_or("contracts")?
        .manifest
        .bindings[0]
        .guarantee_sha256 = "not-a-digest".into();
    cases.push(altered);
    let mut duplicate = document.clone();
    let contracts = duplicate.contracts.as_mut().ok_or("contracts")?;
    contracts
        .manifest
        .bindings
        .push(contracts.manifest.bindings[0].clone());
    cases.push(duplicate);
    let mut counts = document;
    counts
        .contracts
        .as_mut()
        .ok_or("contracts")?
        .proof
        .checks
        .unsat += 1;
    cases.push(counts);
    for changed in cases {
        assert!(assurance::from_contract_result(&changed).is_err());
    }
    Ok(())
}

#[test]
fn legacy_capabilities_round_trip_without_new_fields() -> TestResult {
    let old = r#"{"response":"capabilities","value":{"proof_version":1,"repair":true}}"#;
    let decoded: sail_api::jobs::Response = serde_json::from_str(old)?;
    assert_eq!(serde_json::to_string(&decoded)?, old);
    Ok(())
}
