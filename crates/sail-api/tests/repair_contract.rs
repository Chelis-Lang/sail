//! Focus success never authorizes full-target regression, historical or partial evidence.
use sail_api::output::{EstablishmentName, EvidenceTierName, Report, Summary, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};
use sail_api::repair::BatchResult;

fn project() -> Result<ProjectResult, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    Ok(ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "run".into(),
        workspace: "workspace".into(),
        context_sha256: "context".into(),
        input_revision: "before".into(),
        currentness: Currentness::Current,
        exit_code: Some(0),
        targets: vec![TargetResult {
            dependency_store: None,
            dependency_build: None,
            package_id: "demo".into(),
            target: "demo".into(),
            kind: vec!["lib".into()],
            establishing_run: "run".into(),
            reused: false,
            scope_reuse: None,
            requested: true,
            complete: true,
            report,
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
        engine: None,
        no_reuse: None,
    })
}
fn success(mut project: ProjectResult) -> ProjectResult {
    let report = &mut project.targets[0].report;
    report.exit_code = Some(0);
    report.error = None;
    for c in &mut report.crates {
        c.verus.success = Some(true);
        c.verus.errors = Some(0);
    }
    for f in report.crates.iter_mut().flat_map(|c| &mut c.functions) {
        f.establishment = EstablishmentName::Confirmed;
        f.verdict = VerdictName::Verified;
        f.evidence_tier = Some(EvidenceTierName::SmtVerified);
    }
    report.summary = Summary::of(&report.crates);
    project
}

#[test]
fn inconsistent_success_summaries_cannot_authorize_repairs()
-> Result<(), Box<dyn std::error::Error>> {
    let control = success(project()?);
    assert_eq!(
        control.targets[0].report.invariant_violations(),
        Vec::<String>::new()
    );
    assert!(sail_api::repair::accepted(&control));
    for verdict in [
        VerdictName::Failed,
        VerdictName::Unknown,
        VerdictName::Provisional,
    ] {
        let mut full = control.clone();
        let function = &mut full.targets[0].report.crates[0].functions[0];
        function.verdict = verdict;
        function.establishment = if verdict == VerdictName::Provisional {
            EstablishmentName::Provisional
        } else {
            EstablishmentName::Confirmed
        };
        function.evidence_tier = EvidenceTierName::for_smt(verdict, function.establishment);
        assert_eq!(full.targets[0].report.summary.failed, 0);
        assert_eq!(full.targets[0].report.summary.unknown, 0);
        assert_eq!(full.targets[0].report.summary.provisional, 0);
        assert!(!sail_api::repair::accepted(&full), "{verdict:?}");
    }
    // A matching success count cannot promote provisional evidence either.
    let mut full = control.clone();
    full.targets[0].report.crates[0].functions[0].establishment = EstablishmentName::Provisional;
    assert!(!sail_api::repair::accepted(&full));
    let mut full = control;
    full.targets[0].report.summary.verified += 1;
    assert!(!sail_api::repair::accepted(&full));
    Ok(())
}

#[test]
fn repair_acceptance_requires_explicit_error_free_whole_target_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let control = success(project()?);
    for errors in [None, Some(1)] {
        let mut full = control.clone();
        full.targets[0].report.crates[0].verus.errors = errors;
        assert!(!sail_api::repair::accepted(&full), "{errors:?}");
    }
    let mut full = control.clone();
    full.targets[0].scope_reuse = Some(vec!["--".into(), "--verify-root".into()]);
    assert!(!sail_api::repair::accepted(&full));
    let mut full = control.clone();
    full.targets[0].report.schema_version += 1;
    assert!(!sail_api::repair::accepted(&full));
    // A valid complete target need not declare any functions.
    let mut empty = control;
    for report in &mut empty.targets[0].report.crates {
        report.functions.clear();
        report.verus.verified = Some(0);
    }
    empty.targets[0].report.summary = Summary::of(&empty.targets[0].report.crates);
    assert_eq!(
        empty.targets[0].report.invariant_violations(),
        Vec::<String>::new()
    );
    assert!(sail_api::repair::accepted(&empty));
    Ok(())
}
#[test]
fn focused_success_does_not_hide_full_regression_late_compiler_failure_or_edits()
-> Result<(), Box<dyn std::error::Error>> {
    let mut full = success(project()?);
    let mut batch = BatchResult {
        focus: Some(full.clone()),
        full: Some(full.clone()),
        manifest_sha256: "manifest".into(),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        engine_fault: None,
    };
    assert!(batch.accepted());
    batch.selection_error = Some(sail_api::output::ErrorInfo {
        code: "SAIL3122".into(),
        message: "ambiguous exact function".into(),
    });
    let document = sail_api::repair::ResultDocument::from(batch.clone());
    assert!(!document.accepted);
    assert_eq!(document.batch.full.as_ref(), Some(&full));
    assert!(sail_api::repair::accepted(&full));
    batch.selection_error = None;
    assert!(batch.accepted());
    // A job result carries the full phase once, as its `result`.
    batch.full = None;
    assert!(!batch.accepted());
    assert!(batch.accepted_with(&full));
    let value = serde_json::to_value(sail_api::repair::ResultDocument::from(batch.clone()))?;
    assert!(value["batch"].get("full").is_none(), "{value}");
    full.targets[0].report.summary.failed = 1;
    full.targets[0].report.exit_code = Some(1);
    full.exit_code = Some(1);
    assert!(!batch.accepted_with(&full));
    full = success(project()?);
    full.targets[0].complete = false;
    full.targets[0].scope_reuse = Some(vec!["--".into(), "--verify-root".into()]);
    assert!(!batch.accepted_with(&full));
    full = success(project()?);
    full.currentness = Currentness::Historical;
    assert!(!batch.accepted_with(&full));
    full.currentness = Currentness::Unknown;
    assert!(!batch.accepted_with(&full));
    Ok(())
}
#[test]
fn engine_fault_and_stock_fallback_never_accept_a_repair() -> Result<(), Box<dyn std::error::Error>>
{
    let engine = |established_by| sail_api::output::EngineProvenance {
        name: "interactive".into(),
        manifest_sha256: "ab".repeat(32),
        established_by,
    };
    let mut full = success(project()?);
    full.engine = Some(sail_api::project::EngineIdentity {
        name: "interactive".into(),
        manifest_sha256: "ab".repeat(32),
    });
    full.targets[0].report.engine = Some(engine(sail_api::output::EngineEstablishment::Engine));
    let mut batch = BatchResult {
        focus: None,
        full: Some(full.clone()),
        manifest_sha256: "ab".repeat(32),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        engine_fault: None,
    };
    assert!(batch.accepted(), "engine-established control");
    // A target that fell back to stock after an engine fault cannot accept,
    // even if its stock report is complete and successful.
    full.targets[0].report.engine =
        Some(engine(sail_api::output::EngineEstablishment::StockFallback));
    batch.full = Some(full.clone());
    assert!(!batch.accepted());
    assert!(!sail_api::repair::accepted(&full));
    // A batch-level fault retains the stock full report but never accepts.
    batch.full = Some(success(project()?));
    assert!(batch.accepted(), "stock-only full control");
    batch.engine_fault = Some(sail_api::output::ErrorInfo {
        code: "SAIL3123".into(),
        message: "interactive engine fault".into(),
    });
    let document = sail_api::repair::ResultDocument::from(batch.clone());
    assert!(!document.accepted);
    let value = serde_json::to_value(&document)?;
    assert_eq!(value["batch"]["engine_fault"]["code"], "SAIL3123");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/repair-result-v1.json"))?;
    assert!(schema["properties"]["batch"]["properties"]["engine_fault"].is_object());
    Ok(())
}
