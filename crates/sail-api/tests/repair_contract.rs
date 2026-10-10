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
    };
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
