//! Submitter attribution over recorded ordinary job results: tiers are earned only
//! by complete, current, attributed evidence and never become neutral assurance.
use sail_api::assurance::{EvidenceTier, MutationStatus};
use sail_api::attribution::{self, Relationship, RequirementAttribution, Scope};
use sail_api::contracts::ProofState;
use sail_api::jobs::{Currentness, Lifecycle, Mode, Request, ResultDocument, Status, Submission};
use sail_api::output::EstablishmentName;
use sail_api::project::ProjectResult;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// The job service applies live currentness to the backend document it projects.
fn recorded(positive: bool, currentness: Currentness) -> TestResult<ResultDocument> {
    let mut backend: ProjectResult = serde_json::from_str(if positive {
        include_str!(
            "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/positive-requested-target.json"
        )
    } else {
        include_str!(
            "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/body-edit-requested-target.json"
        )
    })?;
    backend.currentness = match currentness {
        Currentness::Current => sail_api::project::Currentness::Current,
        Currentness::Historical => sail_api::project::Currentness::Historical,
        Currentness::Unknown => sail_api::project::Currentness::Unknown,
    };
    Ok(ResultDocument {
        job: Status {
            version: 1,
            id: "ordinary-job".into(),
            project_dir: "/recorded/project".into(),
            args: Vec::new(),
            state: Lifecycle::Completed,
            submitted_at_ms: 0,
            started_at_ms: Some(0),
            finished_at_ms: Some(0),
            exit_code: backend.exit_code,
            input_revision: Some(backend.input_revision.clone()),
            currentness,
            error: None,
        },
        result: Some(serde_json::to_value(backend)?),
        repair: None,
        contracts: None,
    })
}

fn claim(requirement_id: &str, function: &str) -> RequirementAttribution {
    RequirementAttribution {
        requirement_id: requirement_id.into(),
        function: function.into(),
    }
}

#[test]
fn attributed_rows_earn_tiers_only_from_complete_current_evidence() -> TestResult {
    const PROVED: &str = "neutral::candidate";
    // (positive result, live currentness, function, proved and established, reported)
    let cases = [
        (true, Currentness::Current, PROVED, true, true),
        (true, Currentness::Unknown, PROVED, false, true),
        (true, Currentness::Historical, PROVED, false, true),
        (true, Currentness::Current, "neutral::absent", false, false),
        (false, Currentness::Current, PROVED, false, true),
    ];
    for (positive, currentness, function, established, reported) in cases {
        let label = format!("{positive} {currentness:?} {function}");
        let document = recorded(positive, currentness)?;
        let view = attribution::from_project_result(
            &document,
            &[claim("REQ-2", function), claim("REQ-1", function)],
        )?;
        assert_eq!(view.version, attribution::VERSION, "{label}");
        assert_eq!(view.currentness, currentness, "{label}");
        let ids: Vec<_> = view
            .requirements
            .iter()
            .map(|r| r.requirement_id.as_str())
            .collect();
        assert_eq!(ids, ["REQ-1", "REQ-2"], "{label}");
        let row = &view.requirements[0].evidence[0];
        assert_eq!(row.relationship, Relationship::VerusSpecificationToCode);
        assert_eq!(row.scope, Scope::InputRevision);
        assert!(row.obligation_id.starts_with("verus:"), "{label}");
        assert_ne!(
            row.obligation_id,
            view.requirements[1].evidence[0].obligation_id
        );
        assert_eq!(row.function_reported, reported, "{label}");
        assert_eq!(row.mutation, MutationStatus::NotEstablished);
        let outcome = if established {
            ProofState::Proved
        } else {
            ProofState::NotProved
        };
        assert_eq!(row.outcome, outcome, "{label}");
        assert_eq!(
            row.tier,
            established.then_some(EvidenceTier::SmtVerified),
            "{label}"
        );
        assert_eq!(
            row.establishment,
            established.then_some(EstablishmentName::Confirmed),
            "{label}"
        );
        let mut sorted = row.qualifiers.clone();
        sorted.sort();
        assert_eq!(sorted, row.qualifiers, "{label}");
        for required in [
            "requirement attribution was supplied by the submitter",
            "no neutral contract was admitted, lowered or checked",
            "contract-to-requirement entailment and required-input feasibility are not established",
            "mutation assurance and termination are not established by this result",
        ] {
            assert!(
                row.qualifiers.iter().any(|q| q.contains(required)),
                "{label}: {required}"
            );
        }
        assert_eq!(
            row.qualifiers
                .iter()
                .any(|q| q.contains("not confirmed current")),
            currentness != Currentness::Current,
            "{label}"
        );
    }
    Ok(())
}

#[test]
fn neutral_unfinished_and_invalid_attributions_are_refused() -> TestResult {
    let good = recorded(true, Currentness::Current)?;
    let function = "neutral::candidate";
    let mut running = good.clone();
    running.job.state = Lifecycle::Running;
    let mut missing = good.clone();
    missing.result = None;
    for (label, document, claims) in [
        ("running", &running, vec![claim("R", function)]),
        ("missing", &missing, vec![claim("R", function)]),
        ("empty", &good, vec![]),
        (
            "duplicate",
            &good,
            vec![claim("R", function), claim("R", function)],
        ),
        ("blank id", &good, vec![claim("", function)]),
        ("control", &good, vec![claim("R\n", function)]),
    ] {
        assert!(
            attribution::from_project_result(document, &claims).is_err(),
            "{label}"
        );
    }
    // A neutral source job is reported only through neutral assurance.
    let neutral: ResultDocument = serde_json::from_value({
        let mut value = serde_json::to_value(&good)?;
        value["contracts"] = serde_json::to_value(
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/add_one.v1-manifest.json"
            ))
            .map(|manifest| {
                serde_json::json!({
                    "version": 1,
                    "manifest": manifest,
                    "proof": {"state": "proved", "checks": {"unsat": 1, "sat": 0, "unknown": 0, "missing_answer": 0, "missing_source": 0}, "unknown_reasons": [], "proved_functions": []},
                    "translation_trust": {"source_policy_sha256": "a".repeat(64), "metadata_sha256": "b".repeat(64), "cargo_lock_sha256": "c".repeat(64)}
                })
            })?,
        )?;
        value
    })?;
    assert!(
        attribution::from_project_result(&neutral, &[claim("R", function)])
            .is_err_and(|error| error.contains("neutral source jobs"))
    );
    Ok(())
}

#[test]
fn attributed_submission_is_a_separate_operation_and_plain_submissions_are_unchanged() -> TestResult
{
    let submission = Submission {
        mode: Mode::Stock,
        project_dir: "/project".into(),
        args: vec!["--lib".into()],
        force: false,
        environment: std::collections::BTreeMap::default(),
        input_revision: None,
    };
    let plain = serde_json::to_value(Request::Submit {
        submission: submission.clone(),
    })?;
    assert_eq!(
        plain,
        serde_json::json!({"operation": "submit", "submission": {"mode": {"kind": "stock"}, "project_dir": "/project", "args": ["--lib"], "force": false, "environment": {}, "input_revision": null}})
    );
    let attributed = serde_json::to_value(Request::SubmitAttributed {
        submission,
        requirements: vec![claim("REQ-1", "demo::checksum")],
    })?;
    assert_eq!(attributed["operation"], "submit_attributed");
    assert_eq!(
        attributed["requirements"],
        serde_json::json!([{"requirement_id": "REQ-1", "function": "demo::checksum"}])
    );
    assert!(attribution::mode_supported(&Mode::Stock));
    assert!(attribution::mode_supported(&Mode::Standalone));
    assert!(!attribution::mode_supported(&Mode::Interactive));
    assert_eq!(
        attribution::canonical(vec![claim("B", "f"), claim("A", "g")])?,
        vec![claim("A", "g"), claim("B", "f")]
    );
    Ok(())
}
