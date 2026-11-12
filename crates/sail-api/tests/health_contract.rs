//! Health views preserve original identity, coverage and evidence classes.

use sail_api::health::{FunctionHealth, View};
use sail_api::output::{EstablishmentName, Report, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};

fn project() -> Result<ProjectResult, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../resources/crates/sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    Ok(ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: 2,
        run_id: "original".into(),
        workspace: "workspace".into(),
        context_sha256: "context".into(),
        input_revision: "old".into(),
        currentness: Currentness::Historical,
        exit_code: Some(1),
        targets: vec![TargetResult {
            package_id: "demo".into(),
            target: "same_name".into(),
            kind: vec!["lib".into()],
            establishing_run: "original".into(),
            reused: true,
            scope_reuse: None,
            requested: true,
            complete: false,
            report,
            dependency_store: None,
            dependency_build: None,
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
        engine: None,
        no_reuse: None,
    })
}

#[test]
fn bounded_views_preserve_target_and_provisional_identity_without_trusting_derived_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let mut project = project()?;
    let mut binary = project.targets[0].clone();
    binary.kind = vec!["bin".into()];
    project.targets.push(binary);
    let function = &mut project.targets[0].report.crates[0].functions[0];
    function.verdict = VerdictName::Provisional;
    function.establishment = EstablishmentName::Provisional;
    function.obligations = None;
    function.health = Some(FunctionHealth::from_obligations(Some(&[])));
    let original = project.clone();
    let view = View::of(&project, 1000)?;
    assert_eq!(view.currentness, Currentness::Historical);
    let entry = view
        .entries
        .iter()
        .find(|entry| entry.function == "input::Counter::get" && entry.target.kind == ["lib"])
        .ok_or("missing exact library identity")?;
    assert_eq!(entry.verdict, VerdictName::Provisional);
    assert_eq!(entry.establishment, EstablishmentName::Provisional);
    assert!(!entry.health.attributed);
    assert!(!entry.health.complete);
    assert_eq!(entry.establishing_run, "original");
    assert!(entry.reused);
    assert!(entry.fragility.is_none());
    assert!(
        view.entries
            .iter()
            .any(|entry| entry.function == "input::Counter::get"
                && entry.target.kind == ["bin"]
                && entry.health.attributed)
    );
    let bounded = View::of(&project, 1)?;
    assert_eq!(bounded.entries.len(), 1);
    assert_eq!(bounded.total_functions, view.total_functions);
    assert_eq!(bounded.omitted, view.total_functions - 1);
    assert_eq!(project, original);
    assert!(View::of(&project, 0).is_err());
    assert!(View::of(&project, 1001).is_err());
    project.targets.push(project.targets[0].clone());
    assert!(View::of(&project, 20).is_err());
    Ok(())
}

#[test]
fn duplicate_target_identities_refuse_even_without_overlapping_functions()
-> Result<(), Box<dyn std::error::Error>> {
    let mut project = project()?;
    let mut duplicate = project.targets[0].clone();
    for krate in &mut duplicate.report.crates {
        krate.functions.clear();
    }
    project.targets.push(duplicate);
    assert!(View::of(&project, 20).is_err());
    Ok(())
}
