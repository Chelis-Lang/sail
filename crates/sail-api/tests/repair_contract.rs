//! Focus success never authorizes full-target regression, historical or partial evidence.
use sail_api::output::{OutcomeName, Report, TierName, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};
use sail_api::repair::{BatchResult, Change};

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
fn exact_target_obligation_comparisons_preserve_scope_and_removal_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let before = success(project()?);
    let mut after = before.clone();
    after.input_revision = "after".into();
    let f = &mut after.targets[0].report.crates[0].functions[0];
    f.verdict = VerdictName::Failed;
    let Some(obligations) = f.obligations.as_mut() else {
        return Err("fixture requires attributed obligations".into());
    };
    let obligation = obligations
        .first_mut()
        .ok_or("fixture requires an obligation")?;
    let id = obligation.id.clone();
    obligation.outcome = OutcomeName::Failed;
    obligation.advisory = false;
    let mut before = before;
    before.targets[0].report.crates[0].functions[0]
        .obligations
        .as_mut()
        .ok_or("fixture requires obligations")?[0]
        .outcome = OutcomeName::Proved;
    let comparison = sail_api::repair::compare(&before, &after);
    assert!(comparison.compatible);
    assert_eq!(comparison.differences[0].change, Change::Regressed);
    assert!(
        comparison.differences[0]
            .obligations
            .iter()
            .any(|diff| diff.id == id && diff.change == Change::Regressed)
    );
    after.targets[0].report.crates[0].functions.remove(0);
    after.targets[0].report.crates[0].verus.success = None;
    assert_eq!(
        sail_api::repair::compare(&before, &after).differences[0].change,
        Change::NotObserved
    );
    after.targets[0].report.crates[0].verus.success = Some(true);
    assert_eq!(
        sail_api::repair::compare(&before, &after).differences[0].change,
        Change::Removed
    );
    after.targets[0].kind = vec!["bin".into()];
    assert!(!sail_api::repair::compare(&before, &after).compatible);
    Ok(())
}

#[test]
fn repeated_friendly_names_do_not_invent_order_dependent_regressions()
-> Result<(), Box<dyn std::error::Error>> {
    let mut before = success(project()?);
    let mut duplicate = before.targets[0].report.crates[0].functions[0].clone();
    duplicate.verdict = VerdictName::Failed;
    let name = duplicate.function.clone();
    let original_count = before.targets[0].report.crates[0].functions.len();
    before.targets[0].report.crates[0].functions.push(duplicate);
    let mut after = before.clone();
    after.targets[0].report.crates[0].functions.reverse();
    let compared = sail_api::repair::compare(&before, &after);
    assert!(compared.compatible);
    assert_eq!(compared.differences.len(), original_count);
    let difference = compared
        .differences
        .iter()
        .find(|d| d.function == name)
        .ok_or("ambiguous function difference missing")?;
    assert!(
        compared
            .differences
            .iter()
            .filter(|d| d.function != name)
            .all(|d| d.change == Change::Unchanged)
    );
    assert_eq!(difference.change, sail_api::repair::Change::Incomparable);
    assert_eq!(difference.before, None);
    assert_eq!(difference.after, None);
    assert_eq!(difference.obligations, []);
    assert!(
        compared
            .notes
            .iter()
            .any(|n| n.contains("Repeated friendly function names"))
    );
    Ok(())
}

#[test]
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
