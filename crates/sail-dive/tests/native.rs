//! Real pinned-library outcomes and independent witness controls, without Java.
use sail_dive::{Model, Operation, Outcome, Request, VERSION};
use serde_json::json;
use std::collections::BTreeMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn request(source: &str, config: &str) -> Request {
    Request {
        version: VERSION,
        model: Model {
            files: BTreeMap::from([
                ("M.tla".into(), source.into()),
                ("M.cfg".into(), config.into()),
            ]),
            root: "M.tla".into(),
            config: "M.cfg".into(),
        },
        operation: Operation::Check {
            max_states: 100,
            max_depth: 100,
        },
    }
}
const COUNTER: &str = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 2 THEN x+1 ELSE 0\nInv == x < 2\n====\n";
const BOOLEAN: &str =
    "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";

#[test]
fn real_safety_temporal_and_stopped_outcomes_are_distinct() {
    let safe = sail_dive::execute(&request(COUNTER, "INIT Init\nNEXT Next\n"));
    assert_eq!(safe.outcome, Outcome::Holds, "{safe:?}");
    assert_eq!(safe.native_report["options"]["store"], "exact");
    assert_eq!(safe.completion, sail_dive::Completion::Safety);
    assert!(
        safe.requested_checks
            .as_ref()
            .is_some_and(|c| c.properties.is_empty())
    );
    // Pinned native JSON classifies the explicitly selected final route as
    // temporal even when its private typed result is ordinary safety success.
    assert_eq!(safe.native_report["check_kind"], "temporal");
    assert_eq!(safe.native_report["execution"]["capture_completed"], false);
    let fair = BOOLEAN.replace("====", "Spec == Init /\\ [][Next]_x /\\ WF_x(Next)\n====");
    let temporal = sail_dive::execute(&request(&fair, "SPECIFICATION Spec\nPROPERTY P\n"));
    assert_eq!(temporal.outcome, Outcome::Holds, "{temporal:?}");
    assert_eq!(temporal.completion, sail_dive::Completion::Temporal);
    assert_eq!(
        temporal
            .requested_checks
            .as_ref()
            .map(|c| c.properties.as_slice()),
        Some(["P".to_owned()].as_slice())
    );
    assert_eq!(
        temporal.native_report["execution"]["capture_completed"],
        true
    );
    let mut stopped = request(COUNTER, "INIT Init\nNEXT Next\n");
    stopped.operation = Operation::Check {
        max_states: 1,
        max_depth: 100,
    };
    let stopped = sail_dive::execute(&stopped);
    assert_eq!(stopped.outcome, Outcome::Stopped, "{stopped:?}");
}

#[test]
fn concrete_violation_replays_and_forged_initial_state_is_invalid() -> TestResult {
    let mut req = request(COUNTER, "INIT Init\nNEXT Next\nINVARIANT Inv\n");
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Refuted, "{result:?}");
    assert_eq!(result.validations[0]["result"], "valid");
    let trace = result.itf_traces.first().ok_or("missing ITF")?.clone();
    assert_eq!(sail_dive::itf_to_native(&trace)?, result.native_traces[0]);
    req.operation = Operation::Validate {
        trace: trace.clone(),
    };
    assert_eq!(sail_dive::execute(&req).outcome, Outcome::Valid);
    let mut forged = trace;
    forged.states[0].insert("x".into(), json!({"#bigint":"1"}));
    req.operation = Operation::Validate { trace: forged };
    assert_eq!(sail_dive::execute(&req).outcome, Outcome::Invalid);
    Ok(())
}

#[test]
fn real_temporal_counterexample_has_complete_independent_validation() {
    let source = BOOLEAN.replace("x' = ~x", "x' = x");
    let result = sail_dive::execute(&request(&source, "INIT Init\nNEXT Next\nPROPERTY P\n"));
    assert_eq!(result.outcome, Outcome::Refuted, "{result:?}");
    assert_eq!(result.validations[0]["result"], "valid");
    assert_eq!(result.itf_traces[0].properties, ["P"]);
}

#[test]
fn simulation_is_sampled_and_every_trace_is_independently_validated() {
    let mut req = request(COUNTER, "INIT Init\nNEXT Next\n");
    req.operation = Operation::Simulate {
        traces: 2,
        depth: 4,
    };
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Sampled, "{result:?}");
    assert_eq!(result.native_report["exhaustive"], false);
    assert_eq!(result.native_report["model_proved"], false);
    assert_eq!(result.itf_traces.len(), 2);
    assert!(result.validations.iter().all(|v| v["result"] == "valid"));
}

#[test]
fn closure_and_host_admission_happen_before_evaluation() {
    let host = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nASSUME PrintT(\"host effect must not run\")\nInit == x = 0\nNext == x' = x\n====\n";
    let result = sail_dive::execute(&request(host, "INIT Init\nNEXT Next\n"));
    assert_eq!(result.outcome, Outcome::Unsupported, "{result:?}");
    assert!(result.native_report.is_null());
    // D.I.V.E. installs native overrides by module name, including user files.
    let mut shadow = request(host, "INIT Init\nNEXT Next\n");
    shadow.model.files.insert(
        "TLC.tla".into(),
        "---- MODULE TLC ----\nPrintT(value) == TRUE\n====\n".into(),
    );
    let shadow = sail_dive::execute(&shadow);
    assert_eq!(shadow.outcome, Outcome::Unsupported, "{shadow:?}");
    assert!(shadow.native_report.is_null());
    let mut bad = request(COUNTER, "INIT Init\nNEXT Next\n");
    bad.model
        .files
        .insert("../outside.tla".into(), COUNTER.into());
    assert_eq!(sail_dive::execute(&bad).outcome, Outcome::Rejected);
    let missing = COUNTER.replace("EXTENDS Naturals", "EXTENDS MissingModule");
    assert_eq!(
        sail_dive::execute(&request(&missing, "INIT Init\nNEXT Next\n")).outcome,
        Outcome::Rejected
    );
}

#[test]
fn itf_bigints_roundtrip_without_integer_narrowing_and_refuse_unknown_tags() -> TestResult {
    let native = json!({"format":"dive-trace","version":1,"variables":["x"],
        "states":[{"action":null,"values":{"x":{"int":"123456789012345678901234567890"}}}],
        "ending":{"kind":"behavior"}});
    let trace = sail_dive::native_to_itf(&native)?;
    assert_eq!(
        trace.states[0]["x"],
        json!({"#bigint":"123456789012345678901234567890"})
    );
    assert_eq!(sail_dive::itf_to_native(&trace)?, native);
    let mut bad = trace.clone();
    bad.states[0].insert("x".into(), json!({"#unserializable":"m1"}));
    assert!(sail_dive::itf_to_native(&bad).is_err());
    bad = trace.clone();
    bad.vars.push("x".into());
    assert!(sail_dive::itf_to_native(&bad).is_err());
    bad = trace;
    bad.states[0].remove("x");
    assert!(sail_dive::itf_to_native(&bad).is_err());
    Ok(())
}

#[test]
fn direct_pinned_library_parity_preserves_deciding_native_outcome() -> TestResult {
    let directory = std::env::temp_dir().join(format!("sail-dive-parity-{}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let cleanup = || std::fs::remove_dir_all(&directory);
    let result = (|| -> TestResult {
        std::fs::write(directory.join("M.tla"), COUNTER)?;
        std::fs::write(
            directory.join("M.cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )?;
        let native = dive_api::check_file(&dive_api::CheckRequest {
            file: directory.join("M.tla"),
            config: Some(directory.join("M.cfg")),
            store: dive_api::Store::Exact,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            temporal: dive_api::TemporalMode::FinalShort,
            max_states: Some(100),
            max_depth: Some(100),
            time_limit: dive_api::BudgetSetting::Unlimited,
            max_memory: dive_api::BudgetSetting::Unlimited,
            ..Default::default()
        })?;
        let adapted =
            sail_dive::execute(&request(COUNTER, "INIT Init\nNEXT Next\nINVARIANT Inv\n"));
        let native = native.to_json();
        for field in [
            "verdict",
            "property",
            "statistics",
            "counterexample",
            "collision",
        ] {
            assert_eq!(adapted.native_report[field], native[field], "{field}");
        }
        Ok(())
    })();
    cleanup()?;
    result
}

#[test]
fn wire_refuses_duplicates_trailing_documents_and_unknown_payload_fields() -> TestResult {
    let request = request(COUNTER, "INIT Init\nNEXT Next\n");
    let encoded = serde_json::to_vec(&request)?;
    assert!(sail_dive::decode_request(&encoded).is_ok());
    let mut trailing = encoded.clone();
    trailing.extend_from_slice(b"{}");
    assert!(sail_dive::decode_request(&trailing).is_err());
    let duplicate =
        String::from_utf8(encoded)?.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert!(sail_dive::decode_request(duplicate.as_bytes()).is_err());
    Ok(())
}
