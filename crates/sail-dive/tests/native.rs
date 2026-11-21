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
fn recheck_guard_preserves_config_order_locations_model_closure_and_limits() {
    let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\nINVARIANT Aux\n";
    let original = request(COUNTER, cfg);
    let equivalent = |other: &Request| sail_dive::equivalent_check_config(&original, other);
    assert!(equivalent(&request(
        COUNTER,
        &format!("{cfg}\\* comment\n")
    )));
    for edited in [
        cfg.replace(
            "INVARIANT Inv\nINVARIANT Aux",
            "INVARIANT Aux\nINVARIANT Inv",
        ),
        format!("\n{cfg}"),
        format!("{cfg}CHECK_DEADLOCK FALSE\n"),
        format!("{cfg}CHECK_DEADLOCK invalid"),
    ] {
        assert!(!equivalent(&request(COUNTER, &edited)), "{edited}");
    }
    let mut new = original.clone();
    new.model.files.insert("Other.tla".into(), COUNTER.into());
    assert!(!equivalent(&new));
    let mut new = original.clone();
    new.model.root = "Other.tla".into();
    assert!(!equivalent(&new));
    let mut new = original.clone();
    new.model.config = "Other.cfg".into();
    assert!(!equivalent(&new));
    let mut new = original;
    new.operation = Operation::Check {
        max_states: 100,
        max_depth: 99,
    };
    assert!(!sail_dive::equivalent_check_config(
        &request(COUNTER, cfg),
        &new
    ));
}

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
    assert_eq!(safe.native_report["check_kind"], "safety");
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
fn temporal_state_and_depth_bounds_below_the_model_stop_without_a_claim() -> TestResult {
    // Three reachable states, so one state or a zero depth bound cannot
    // complete graph capture. A stop must never surface as holds or refuted.
    let source = COUNTER.replace(
        "Inv == x < 2",
        "P == []<>(x = 0)\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)",
    );
    for (max_states, max_depth) in [(1, 100), (100, 0)] {
        let mut req = request(&source, "SPECIFICATION Spec\nPROPERTY P\n");
        req.operation = Operation::Check {
            max_states,
            max_depth,
        };
        let result = sail_dive::execute(&req);
        assert_eq!(result.outcome, Outcome::Stopped, "{result:?}");
        assert_eq!(result.native_report["verdict"], "stopped", "{result:?}");
        assert!(result.native_traces.is_empty() && result.validations.is_empty());
        assert_eq!(result.completion, sail_dive::Completion::None);
        let stopped = &result.native_report["stopped"];
        assert_eq!(stopped["phase"], "graph-capture", "{stopped}");
        assert_eq!(
            stopped["resource"],
            if max_states == 1 { "states" } else { "depth" },
            "{stopped}"
        );
        sail_dive::validate_response(&req, &result)?;
    }
    // Control: the same model under the ordinary bounds completes and holds.
    let control = sail_dive::execute(&request(&source, "SPECIFICATION Spec\nPROPERTY P\n"));
    assert_eq!(control.outcome, Outcome::Holds, "{control:?}");
    assert_eq!(control.completion, sail_dive::Completion::Temporal);
    Ok(())
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
fn simulation_deadlock_under_a_constraint_is_a_validated_refutation() {
    // The constraint prunes every successor of x = 2, which D.I.V.E.'s
    // simulation reports as a deadlock (#766). Sail must validate that word
    // as a sampled deadlock, not as an exhaustive-checking deadlock.
    let source = COUNTER.replace("Inv == x < 2", "Small == x <= 2");
    let mut req = request(&source, "INIT Init\nNEXT Next\nCONSTRAINT Small\n");
    req.operation = Operation::Simulate {
        traces: 2,
        depth: 10,
    };
    let source = source.replace("x' = IF x < 2 THEN x+1 ELSE 0", "x' = x + 1");
    req.model.files.insert("M.tla".into(), source);
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Refuted, "{result:?}");
    assert_eq!(result.native_report["result"], "violation", "{result:?}");
    assert!(result.message.is_none(), "{result:?}");
    assert_eq!(result.validations.len(), result.native_traces.len());
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
            temporal: dive_api::TemporalMode::Ordinary,
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

#[test]
fn generated_step_properties_and_within_monitor_select_typed_safety_roles() -> TestResult {
    let req = oyster_step_within();
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Holds, "{result:?}");
    assert_eq!(result.completion, sail_dive::Completion::Safety);
    assert_eq!(result.native_report["check_kind"], "safety");
    sail_dive::validate_response(&req, &result)?;
    Ok(())
}
fn oyster_step_within() -> Request {
    let files = [
        (
            "Signal.tla",
            include_str!("../fixtures/oyster-step-within/Signal.tla"),
        ),
        (
            "SignalRequirements.tla",
            include_str!("../fixtures/oyster-step-within/SignalRequirements.tla"),
        ),
        (
            "Signal.cfg",
            include_str!("../fixtures/oyster-step-within/Signal.cfg"),
        ),
    ];
    Request {
        version: VERSION,
        model: Model {
            files: files
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            root: "Signal.tla".into(),
            config: "Signal.cfg".into(),
        },
        operation: Operation::Check {
            max_states: 100,
            max_depth: 100,
        },
    }
}

#[test]
fn oversized_simulation_is_refused_from_its_probe_and_a_normal_one_runs() {
    // 100,000 traces at depth 100 would far exceed the 16 MiB response bound;
    // the refusal comes from the first 16 traces, not after the full run (#767).
    let mut req = request(COUNTER, "INIT Init\nNEXT Next\n");
    req.operation = Operation::Simulate {
        traces: 100_000,
        depth: 100,
    };
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Rejected, "{result:?}");
    let message = result.message.unwrap_or_default();
    assert!(message.starts_with("SAIL4216: "), "{message}");
    assert!(message.contains("the first 16 traces"), "{message}");
    assert_eq!(result.native_traces.len(), 0);

    req.operation = Operation::Simulate {
        traces: 40,
        depth: 4,
    };
    let result = sail_dive::execute(&req);
    assert_eq!(result.outcome, Outcome::Sampled, "{result:?}");
    assert_eq!(result.itf_traces.len(), 40);
    assert!(result.validations.iter().all(|v| v["result"] == "valid"));
}

#[test]
fn model_atoms_keep_their_identity_through_current_source_validation() -> TestResult {
    let mut job = request(
        "---- MODULE M ----\nCONSTANT Atom\nVARIABLE x,y\nInit == /\\ x = \"N\" /\\ y = Atom\nNext == /\\ x' = Atom /\\ y' = Atom\nInv == x /= Atom\n====\n",
        "CONSTANT Atom = N\nINIT Init\nNEXT Next\nINVARIANT Inv\n",
    );
    let result = sail_dive::execute(&job);
    assert_eq!(result.outcome, Outcome::Refuted, "{result:?}");
    let mut trace = result.itf_traces.first().ok_or("missing trace")?.clone();
    let wire = serde_json::to_value(&trace)?;
    assert_eq!(wire["value_encoding"], "sail-model-values-v1");
    assert_eq!(trace.states[0]["x"], "N");
    assert_eq!(trace.states[0]["y"], json!({"#sail-model":"N"}));
    assert_eq!(trace.states[1]["x"], json!({"#sail-model":"N"}));
    job.operation = Operation::Validate {
        trace: trace.clone(),
    };
    assert_eq!(sail_dive::execute(&job).outcome, Outcome::Valid);
    trace.states[1].insert("x".into(), json!("N"));
    // y still contains a tagged atom. The mutated trace is structurally valid.
    job.operation = Operation::Validate { trace };
    assert_eq!(sail_dive::execute(&job).outcome, Outcome::Invalid);
    Ok(())
}

#[test]
fn nested_atoms_round_trip_in_keys_values_records_and_sets() -> TestResult {
    let native = json!({"format":"dive-trace","version":1,"variables":["x"],"states":[
        {"action":null,"values":{"x":{"fcn":[[{"model":"N"},{"set":["N",{"model":"N"}]}],
        ["N",{"record":{"model":"N","nested":{"seq":[{"model":"N"}]}}}]]}}}],"ending":{"kind":"behavior"}});
    let trace = sail_dive::native_to_itf(&native)?;
    assert_eq!(sail_dive::itf_to_native(&trace)?, native);
    let itf = serde_json::to_value(trace)?;
    let entries = &itf["states"][0]["x"]["#map"];
    assert_eq!(entries[0][0], json!({"#sail-model":"N"}));
    assert_eq!(entries[1][0], "N");
    assert_eq!(entries[1][1]["model"], "N");
    Ok(())
}

#[test]
fn deterministic_tlc_import_runs_actual_check_simulation_and_replay() {
    let source = r"---- MODULE M ----
EXTENDS TLC, FiniteSetsExt
CONSTANT Ids
VARIABLE x
Sym == Permutations(Ids)
Table == (1 :> 2) @@ (2 :> 1)
Init == x = 1
Next == x' = Table[x]
Inv == x <= Max(DOMAIN Table)
Bad == x = 1
====
";
    let cfg = "CONSTANT Ids={p,q}\nINIT Init\nNEXT Next\nSYMMETRY Sym\nINVARIANT Inv";
    let original = request(source, cfg);
    let rows = sail_dive::execute(&original);
    assert_eq!(rows.outcome, Outcome::Holds, "{rows:?}");
    assert_eq!(rows.native_report["statistics"]["distinct"], 2);
    assert_eq!(rows.native_report["statistics"]["generated"], 3);
    let mut bad = request(source, &format!("{cfg}\nINVARIANT Bad"));
    let checked = sail_dive::execute(&bad);
    assert_eq!(checked.outcome, Outcome::Refuted, "{checked:?}");
    assert_eq!(checked.validations[0]["result"], "valid");
    let mut word = checked.itf_traces[0].clone();
    bad.operation = Operation::Validate {
        trace: word.clone(),
    };
    assert_eq!(sail_dive::execute(&bad).outcome, Outcome::Valid);
    // A non-admitted, deterministic TLC name is still outside the label contract.
    word.actions[1] = json!({"name":"Any","location":null});
    bad.operation = Operation::Validate { trace: word };
    let refused = sail_dive::execute(&bad);
    assert_eq!(refused.outcome, Outcome::Unsupported, "{refused:?}");
    assert!(
        refused
            .message
            .as_deref()
            .is_some_and(|message| message.contains("trace label"))
    );

    // Core simulation does not support configured symmetry.
    let mut sample = request(source, &cfg.replace("SYMMETRY Sym\n", ""));
    sample.operation = Operation::Simulate {
        traces: 1,
        depth: 3,
    };
    let sampled = sail_dive::execute(&sample);
    assert_eq!(sampled.outcome, Outcome::Sampled, "{sampled:?}");
    assert_eq!(sampled.validations[0]["result"], "valid");
}

#[test]
fn unqualified_deterministic_operations_and_cfg_redirects_refuse_before_binding() {
    let source = COUNTER.replace(
        "EXTENDS Naturals",
        "EXTENDS Naturals, TLC\nCONSTANT Limit\nThree == 3",
    );
    let cfg = "CONSTANT Limit=3\nINIT Init\nNEXT Next";
    for (module, config) in [
        (
            source.replace("====", "Unused == ToString(1)\n===="),
            cfg.into(),
        ),
        (
            source.replace(
                "====",
                "Use(F(_), a) == F(a)\nUnused == Use(ToString, 1)\n====",
            ),
            cfg.into(),
        ),
        (source.clone(), cfg.replace("Limit=3", "Limit <- Three")),
        (source.clone(), format!("{cfg}\nVIEW Any")),
    ] {
        let rejected = sail_dive::execute(&request(&module, &config));
        assert_eq!(rejected.outcome, Outcome::Unsupported, "{rejected:?}");
        assert!(rejected.native_report.is_null());
        assert!(
            rejected
                .message
                .as_deref()
                .is_some_and(|m| m.starts_with("SAIL4202"))
        );
    }
}
