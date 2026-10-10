//! Live observations cannot bypass complete target finalization.

use sail_api::output::{FunctionReport, Report, TierName, VerdictName};
use sail_api::proof::{InputEvent, Observation, Phase, Target};

fn function() -> Result<FunctionReport, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    report
        .crates
        .first()
        .and_then(|c| c.functions.first())
        .cloned()
        .ok_or_else(|| "checked-in report must have functions".into())
}

fn event(report: FunctionReport) -> InputEvent {
    InputEvent {
        version: 1,
        event_id: "unique-1".into(),
        run_id: "run-1".into(),
        context_sha256: "context".into(),
        input_revision: "revision".into(),
        target: Target {
            package_id: "demo@1.0.0".into(),
            name: "demo".into(),
            kind: vec!["lib".into()],
        },
        phase: Phase::Full,
        function: Some(report.function.clone()),
        obligation: None,
        observation: Observation::FunctionObserved(report),
    }
}

#[test]
fn a_confirmed_report_cannot_be_published_as_a_live_function_claim()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = function()?;
    report.verdict = VerdictName::Verified;
    report.tier = TierName::Confirmed;
    assert!(event(report.clone()).validate().is_err());
    report.verdict = VerdictName::Provisional;
    report.tier = TierName::Provisional;
    assert!(event(report.clone()).validate().is_ok());
    let mut wrong_identity = event(report);
    wrong_identity.function = Some("different::function".into());
    assert!(wrong_identity.validate().is_err());
    Ok(())
}

#[test]
fn same_name_targets_remain_different_and_identity_is_mandatory()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = function()?;
    report.verdict = VerdictName::Provisional;
    report.tier = TierName::Provisional;
    let library = event(report);
    let mut binary = library.clone();
    binary.target.kind = vec!["bin".into()];
    assert_ne!(library.target, binary.target);
    binary.target.package_id.clear();
    assert!(binary.validate().is_err());
    Ok(())
}
