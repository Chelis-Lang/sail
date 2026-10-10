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

#[test]
fn query_progress_is_bounded_exact_and_has_no_verdict_claim()
-> Result<(), Box<dyn std::error::Error>> {
    let mut progress = event(function()?);
    let name = progress
        .function
        .clone()
        .ok_or("fixture function missing")?;
    progress.obligation = Some(format!("{name}#Function-Def/0"));
    progress.observation = Observation::ObligationProgress(sail_api::proof::ObligationProgress {
        description: "source query completed".into(),
        completed: true,
    });
    assert!(progress.validate().is_ok());
    let encoded = serde_json::to_value(&progress)?;
    assert!(encoded["observation"]["value"].get("verdict").is_none());
    progress.obligation = Some("different::function#Function-Def/0".into());
    assert!(progress.validate().is_err());
    progress.obligation = None;
    assert!(progress.validate().is_err());
    progress.obligation = Some(format!("{name}#Function-Def/0"));
    if let Observation::ObligationProgress(value) = &mut progress.observation {
        value.description = "x".repeat(4097);
    }
    assert!(progress.validate().is_err());
    let mut forged = encoded;
    forged["observation"]["value"]["verified"] = serde_json::json!(true);
    assert!(serde_json::from_value::<InputEvent>(forged).is_err());
    Ok(())
}
