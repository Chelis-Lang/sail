//! Focus success never authorizes full-target regression, historical or partial evidence.
use sail_api::output::{Report, TierName, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};
use sail_api::repair::BatchResult;

fn project() -> Result<ProjectResult, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    Ok(ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: 1,
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
        f.tier = TierName::Confirmed;
        f.verdict = VerdictName::Verified;
    }
    project
}
#[test]
fn focused_success_does_not_hide_full_regression_late_compiler_failure_or_edits()
-> Result<(), Box<dyn std::error::Error>> {
    let full = success(project()?);
    let mut batch = BatchResult {
        focus: Some(full.clone()),
        full,
        manifest_sha256: "manifest".into(),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        preparation: None,
        engine_fault: None,
    };
    assert!(batch.accepted());
    batch.selection_error = Some(sail_api::output::ErrorInfo {
        code: "SAIL3122".into(),
        message: "ambiguous exact function".into(),
    });
    let document = sail_api::repair::ResultDocument::from(batch.clone());
    assert!(!document.accepted);
    assert_eq!(document.batch.full, batch.full);
    assert_eq!(document.batch.full.exit_code, Some(0));
    assert!(sail_api::repair::accepted(&document.batch.full));
    batch.selection_error = None;
    assert!(batch.accepted());
    batch.full.targets[0].report.summary.failed = 1;
    batch.full.targets[0].report.exit_code = Some(1);
    batch.full.exit_code = Some(1);
    assert!(!batch.accepted());
    batch.full = success(project()?);
    batch.full.targets[0].complete = false;
    assert!(!batch.accepted());
    batch.full = success(project()?);
    batch.full.currentness = Currentness::Historical;
    assert!(!batch.accepted());
    batch.full.currentness = Currentness::Unknown;
    assert!(!batch.accepted());
    Ok(())
}
#[test]
fn engine_fault_and_stock_fallback_never_accept_a_repair() -> Result<(), Box<dyn std::error::Error>>
{
    let engine = |established_by| sail_api::output::EngineProvenance {
        name: "interactive".into(),
        manifest_sha256: "ab".repeat(32),
        established_by,
        persistent_preparation: false,
    };
    let mut full = success(project()?);
    full.engine = Some(sail_api::project::EngineIdentity {
        name: "interactive".into(),
        manifest_sha256: "ab".repeat(32),
        persistent_preparation: false,
    });
    full.targets[0].report.engine = Some(engine(sail_api::output::EngineEstablishment::Engine));
    let mut batch = BatchResult {
        focus: None,
        full,
        manifest_sha256: "ab".repeat(32),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        preparation: None,
        engine_fault: None,
    };
    assert!(batch.accepted(), "engine-established control");
    // A target that fell back to stock after an engine fault cannot accept,
    // even if its stock report is complete and successful.
    batch.full.targets[0].report.engine =
        Some(engine(sail_api::output::EngineEstablishment::StockFallback));
    assert!(!batch.accepted());
    assert!(!sail_api::repair::accepted(&batch.full));
    // A batch-level fault retains the stock full report but never accepts.
    batch.full = success(project()?);
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
fn preparation_stats_preserve_driver_counters_without_inventing_support()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = r#"{"version":1,"enabled":true,"batch_hits":2,"persistent_hits":1,"misses":3,"peak_retained_bytes":1024,"memory_budget_bytes":4096,"disk_budget_bytes":8192}"#;
    let stats: sail_api::repair::PreparationStats = serde_json::from_str(raw)?;
    assert_eq!(stats.batch_hits, 2);
    assert_eq!(stats.persistent_hits, 1);
    assert_eq!(stats.peak_retained_bytes, 1024);
    assert!(stats.target.is_none() && stats.phase.is_none());
    assert_eq!(
        serde_json::to_value(&stats)?,
        serde_json::from_str::<serde_json::Value>(raw)?
    );
    for malformed in [
        raw.replace("\"version\":1", "\"version\":2"),
        raw.replace("\"batch_hits\":2", "\"batch_hits\":-1"),
        raw.replace("\"batch_hits\":2", "\"batch_hits\":18446744073709551616"),
        raw.replace("\"batch_hits\":2", "\"batch_hits\":2,\"batch_hits\":3"),
        raw.replace("\"enabled\":true", "\"enabled\":true,\"verified\":true"),
    ] {
        assert!(
            serde_json::from_str::<sail_api::repair::PreparationStats>(&malformed).is_err(),
            "{malformed}"
        );
    }
    let old = BatchResult {
        focus: None,
        full: success(project()?),
        manifest_sha256: "manifest".into(),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        preparation: None,
        engine_fault: None,
    };
    let json = serde_json::to_value(&old)?;
    assert!(json.get("preparation").is_none());
    let decoded: BatchResult = serde_json::from_value(json)?;
    assert!(decoded.preparation.is_none());
    let with_stats = BatchResult {
        preparation: Some(vec![stats]),
        ..old.clone()
    };
    assert_eq!(
        with_stats.accepted(),
        old.accepted(),
        "telemetry cannot authorize proof acceptance"
    );
    Ok(())
}
