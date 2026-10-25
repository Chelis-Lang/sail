//! Real public report records exercise the checked authority and caller surfaces.

use sail_acceptance::authority::CompletionPolicy;
use sail_api::acceptance::{
    complete_report, complete_report_ordered, complete_target, complete_whole_report,
    focused_command, requested_coverage, requested_target,
};
use sail_api::output::{
    ErrorInfo, EstablishmentName, Report, SourceSummaryName, Summary, VerdictName,
};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn passing_report() -> Result<Report, Box<dyn std::error::Error>> {
    let mut report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    report.exit_code = Some(0);
    report.error = None;
    for krate in &mut report.crates {
        krate
            .functions
            .retain(|f| f.verdict == VerdictName::Verified);
        krate.verus.errors = Some(0);
        krate.verus.success = Some(true);
        krate.verus.encountered_vir_error = Some(false);
    }
    report.summary = Summary::of(&report.crates);
    assert_ne!(report.crates[0].functions.len(), 0);
    Ok(report)
}

fn target(name: &str, report: Report) -> TargetResult {
    TargetResult {
        package_id: "cargo-package#1.0.0".into(),
        target: name.into(),
        kind: vec!["bin".into()],
        establishing_run: "establishing-run".into(),
        reused: false,
        scope_reuse: None,
        requested: true,
        complete: true,
        report,
        dependency_store: None,
        dependency_build: None,
    }
}

fn project(targets: Vec<TargetResult>) -> ProjectResult {
    ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "current-run".into(),
        workspace: "/workspace".into(),
        context_sha256: "context".into(),
        input_revision: "revision".into(),
        currentness: Currentness::Current,
        exit_code: Some(0),
        targets,
        execution: Execution::default(),
        notes: Vec::new(),
        error: None,
        engine: None,
        no_reuse: None,
    }
}

#[test]
fn actual_success_and_solver_free_crates_mint_authority() -> TestResult {
    let report = passing_report()?;
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
    assert!(complete_report(&report, CompletionPolicy::Repair).is_some());
    assert_eq!(report.invariant_violations(), Vec::<String>::new());

    let mut zero = report;
    for krate in &mut zero.crates {
        krate.functions.clear();
        krate.verus.verified = Some(0);
    }
    zero.summary = Summary::of(&zero.crates);
    assert!(complete_whole_report(&zero).is_some());
    zero.crates[0].verus.success = None;
    assert!(complete_report(&zero, CompletionPolicy::Receipt).is_some());
    assert!(complete_whole_report(&zero).is_none());
    assert!(complete_report(&zero, CompletionPolicy::Repair).is_none());
    Ok(())
}

#[test]
fn nonempty_record_projection_has_a_constructive_positive_control() -> TestResult {
    let mut report = passing_report()?;
    report.crates.truncate(1);
    report.crates[0].functions.truncate(1);
    report.crates[0].verus.verified = Some(1);
    let function = &mut report.crates[0].functions[0];
    function.obligations = None;
    function.provenance.source = SourceSummaryName::Unrecorded;
    function.provenance.checks_solved = 0;
    function.provenance.checks_served = 0;
    function.provenance.checks_unrecorded = 0;
    report.summary = Summary::of(&report.crates);
    assert_eq!(report.summary.functions, 1);
    assert_eq!(report.invariant_violations(), Vec::<String>::new());
    assert!(complete_report_ordered(&report, CompletionPolicy::Receipt, &[], &[]).is_some());
    Ok(())
}

#[test]
fn actual_function_records_override_forged_success_counts() -> TestResult {
    let original = passing_report()?;
    for case in 0..5 {
        let mut report = original.clone();
        let f = &mut report.crates[0].functions[0];
        match case {
            0 => f.verdict = VerdictName::Failed,
            1 => f.establishment = EstablishmentName::Provisional,
            2 => f.evidence_tier = None,
            3 => f.provenance.checks_unrecorded = 1,
            _ => report.summary.verified += 1,
        }
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());
    }
    Ok(())
}

#[test]
fn missing_exit_errors_and_malformed_headers_never_mint_authority() -> TestResult {
    let original = passing_report()?;
    for case in 0..7 {
        let mut report = original.clone();
        match case {
            0 => report.exit_code = None,
            1 => report.exit_code = Some(1),
            2 => report.schema_version += 1,
            3 => {
                report.error = Some(ErrorInfo {
                    code: "SAIL3107".into(),
                    message: "failure".into(),
                });
            }
            4 => report.crates.clear(),
            5 => report.crates[0].verus.errors = None,
            _ => report.crates[0].verus.encountered_vir_error = Some(true),
        }
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());
    }
    Ok(())
}

#[test]
fn every_actual_obligation_is_checked_and_sort_candidates_are_untrusted() -> TestResult {
    let mut report = passing_report()?;
    let first = report.crates[0].functions[0].obligations.clone();
    let Some(mut ids) = first else {
        return Err("fixture has no obligations".into());
    };
    assert_ne!(ids.len(), 0);
    ids.push(ids[0].clone());
    report.crates[0].functions[0].obligations = Some(ids);
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());

    let mut report = passing_report()?;
    if let Some(ids) = &mut report.crates[0].functions[0].obligations {
        ids[0].id = "unrelated::owner#Function-Def/0".into();
    }
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());

    let report = passing_report()?;
    assert!(complete_report_ordered(&report, CompletionPolicy::Receipt, &[], &[]).is_none());
    assert!(
        complete_report_ordered(&report, CompletionPolicy::Receipt, &[usize::MAX], &[0]).is_none()
    );
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
    Ok(())
}

#[test]
fn actual_forwarded_scope_blocks_whole_target_promotion() -> TestResult {
    for spelling in [
        "--verify-function=lemma",
        "--verify-module",
        "--verify-root",
        "--verify-only-module=m",
    ] {
        let mut report = passing_report()?;
        report.command = vec!["--".into(), spelling.into()];
        assert!(focused_command(&report.command));
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
        assert!(complete_whole_report(&report).is_none());
        assert!(!sail_api::repair::accepted(&project(vec![target(
            "app", report
        )])));
    }
    assert!(!focused_command(&[
        "--verify-function=before-separator".into()
    ]));
    assert!(!focused_command(&[
        "--".into(),
        "--verify-functional=near-prefix".into()
    ]));
    Ok(())
}

#[test]
fn exact_request_coverage_preserves_failed_siblings_and_colliding_names() -> TestResult {
    let good = target("foo-bar", passing_report()?);
    let mut bad = target("foo_bar", passing_report()?);
    bad.complete = false;
    bad.report.exit_code = Some(1);
    let targets = vec![good, bad];
    let expected: Vec<_> = targets
        .iter()
        .map(|t| requested_target(&t.package_id, &t.target, "bin"))
        .collect();
    assert!(requested_coverage(&expected, &targets));
    assert!(!sail_api::repair::accepted(&project(targets.clone())));
    assert!(!requested_coverage(&expected, &[targets[0].clone()]));
    assert!(!requested_coverage(
        &expected,
        &[targets[0].clone(), targets[0].clone(), targets[1].clone()]
    ));
    let mut unselected = targets;
    unselected[1].requested = false;
    assert!(!requested_coverage(&expected, &unselected));
    Ok(())
}

#[test]
fn selected_authority_refuses_duplicate_partial_and_stale_envelopes() -> TestResult {
    let good = target("app", passing_report()?);
    assert!(complete_target(&good).is_some());
    let full = project(vec![good.clone()]);
    assert!(sail_api::repair::accepted(&full));
    for case in 0..4 {
        let mut changed = full.clone();
        match case {
            0 => changed.currentness = Currentness::Historical,
            1 => changed.currentness = Currentness::Unknown,
            2 => changed.targets.push(good.clone()),
            _ => changed.targets[0].scope_reuse = Some(vec!["--".into(), "--verify-root".into()]),
        }
        assert!(!sail_api::repair::accepted(&changed));
    }
    Ok(())
}
