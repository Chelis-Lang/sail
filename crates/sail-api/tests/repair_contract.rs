//! Focus success never authorizes full-target regression, historical or partial evidence.
use sail_api::output::{EstablishmentName, Report, VerdictName};
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
            package_id: "demo".into(),
            target: "demo".into(),
            kind: vec!["lib".into()],
            establishing_run: "run".into(),
            reused: false,
            requested: true,
            complete: true,
            report,
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
        engine: None,
    })
}
fn success(mut project: ProjectResult) -> ProjectResult {
    let report = &mut project.targets[0].report;
    report.exit_code = Some(0);
    report.error = None;
    report.summary.failed = 0;
    report.summary.unknown = 0;
    report.summary.provisional = 0;
    for c in &mut report.crates {
        c.verus.success = Some(true);
        c.verus.errors = Some(0);
    }
    for f in report.crates.iter_mut().flat_map(|c| &mut c.functions) {
        f.establishment = EstablishmentName::Confirmed;
        f.verdict = VerdictName::Verified;
    }
    project
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
