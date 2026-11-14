use super::*;
use crate::attribution::{AirQuery, Answer, Effort, SmtCheck, SmtOp};
use serde_json::json;

fn document(functions: &[&str]) -> Value {
    let rows: Vec<_> = functions
        .iter()
        .map(|name| json!({"function":name,"success":true}))
        .collect();
    json!({"times-ms":{"smt":{"smt-run-module-times":[{"module":"demo", "function-breakdown":rows}]}}})
}

fn check() -> SmtCheck {
    SmtCheck {
        reply_index: 73,
        answer: Some(Answer::Unsat),
        effort: Effort {
            limit: Some(100),
            used: Some(1),
            reason: None,
        },
        model: None,
    }
}

#[test]
fn repeated_module_labels_are_bounded_before_legacy_row_expansion() {
    let rows: Vec<_> = (0..20_000)
        .map(|index| json!({"function":format!("f{index}")}))
        .collect();
    let doc = json!({"times-ms":{"smt":{"smt-run-module-times":[{
        "module":"m".repeat(MAX_LABEL_BYTES), "function-breakdown":rows
    }]}}});
    let error = collect(&[doc], &[log(&[("f0", 1)], 'a')]).err();
    assert_eq!(
        error,
        Some(Error("expanded timing row text budget exceeded"))
    );
}

fn log(functions: &[(&str, usize)], hash: char) -> LogFile {
    LogFile {
        stem: "module".into(),
        air: functions
            .iter()
            .map(|(function, _)| AirQuery {
                function: (*function).into(),
                kind: "Function-Def".into(),
                span: None,
                identity: None,
            })
            .collect(),
        spinoff_reason: None,
        ops: functions
            .iter()
            .map(|(_, count)| SmtOp {
                checks: vec![check(); *count],
            })
            .collect(),
        input_sha256: hash.to_string().repeat(64),
        solver_version: Some("4.16.0".into()),
    }
}

#[test]
fn complete_session_positions_preserve_prior_checks_and_obligation_ordinals() -> Result<(), Error> {
    let input = log(
        &[("demo::first", 2), ("demo::second", 1), ("demo::first", 1)],
        'a',
    );
    let bindings = collect(&[document(&["demo::first", "demo::second"])], &[input])?;
    assert_eq!(bindings.len(), 4);
    assert_eq!(
        bindings.iter().map(|b| b.session_check).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        bindings
            .iter()
            .map(|b| (b.ordinal, b.check_in_obligation))
            .collect::<Vec<_>>(),
        vec![(0, 0), (0, 1), (0, 0), (1, 0)]
    );
    assert_eq!(bindings[2].function, "demo::second");
    assert!(bindings.iter().all(|b| b.input_sha256 == "a".repeat(64)));
    Ok(())
}

#[test]
fn log_and_timing_row_order_do_not_rename_or_reorder_descriptions() -> Result<(), Error> {
    let first = log(&[("demo::first", 1)], 'a');
    let second = log(&[("demo::second", 1)], 'b');
    let expected = collect(
        &[document(&["demo::first", "demo::second"])],
        &[first.clone(), second.clone()],
    )?;
    assert_eq!(
        collect(
            &[document(&["demo::second", "demo::first"])],
            &[second, first]
        )?,
        expected
    );
    Ok(())
}

#[test]
fn ambiguous_missing_and_duplicate_process_or_obligation_owners_refuse() {
    let first = log(&[("demo::first", 1)], 'a');
    assert!(
        collect(
            &[document(&["demo::first", "demo::first"])],
            std::slice::from_ref(&first)
        )
        .is_err()
    );
    assert!(collect(&[document(&["other::first"])], std::slice::from_ref(&first)).is_err());
    assert!(
        collect(
            &[document(&["demo::first"])],
            &[first.clone(), log(&[("demo::first", 1)], 'b')]
        )
        .is_err()
    );
    assert!(
        collect(
            &[document(&["demo::first", "demo::second"])],
            &[first, log(&[("demo::second", 1)], 'a')]
        )
        .is_err()
    );
}

#[test]
fn named_compiler_paths_are_required_and_corroborated() -> Result<(), Box<dyn std::error::Error>> {
    let paths = ["demo::impl&%0::tag", "demo::impl&%1::tag"];
    let rows: Vec<_> = paths
        .iter()
        .map(|path| json!({"function":"alloc::Vec::tag", "function-path":path}))
        .collect();
    let doc = json!({"verus":{"commit":"a3109bed5e02313050d5ba6a23f1bc3e1d549c9b"},"times-ms":{"smt":{"smt-run-module-times":[{"module":"demo", "function-breakdown":rows}]}}});
    let mut input = log(&[("alloc::Vec::tag", 1), ("alloc::Vec::tag", 1)], 'a');
    for (air, path) in input.air.iter_mut().zip(paths) {
        air.identity = Some(identity::parse(&format!("sail-query-identity-v1 {}", json!({"schema":1,"kind":"Function-Def","function":"alloc::Vec::tag","function_path":path,"span":"file.rs:1:1"}))).ok_or("invalid test identity")?);
    }
    let bindings = collect(std::slice::from_ref(&doc), std::slice::from_ref(&input))?;
    assert_eq!(bindings[0].function, paths[0]);
    assert_eq!(bindings[1].function, paths[1]);
    input.air[0].identity = None;
    assert!(collect(std::slice::from_ref(&doc), std::slice::from_ref(&input)).is_err());
    assert!(collect(&[doc, document(&["unrelated::function"])], &[input]).is_err());
    Ok(())
}

#[test]
fn spinoff_positions_are_independent_complete_processes() -> Result<(), Error> {
    let mut input = log(&[("demo::first", 2)], 'a');
    input.stem = "module._7_expand".into();
    let result = collect(&[document(&["demo::first"])], &[input])?;
    assert_eq!(result.len(), 2);
    assert!(
        result
            .iter()
            .all(|b| b.spinoff == Some(7) && b.ordinal == 0)
    );
    assert_eq!(result[1].session_check, 1);
    Ok(())
}

#[test]
fn missing_malformed_and_over_budget_descriptions_return_no_collection() {
    let doc = document(&["a"]);
    let input = log(&[("a", 1)], 'a');
    assert!(collect(&[], std::slice::from_ref(&input)).is_err());
    assert!(collect(std::slice::from_ref(&doc), &[]).is_err());
    assert!(
        collect(
            &vec![doc.clone(); MAX_DOCUMENTS + 1],
            std::slice::from_ref(&input)
        )
        .is_err()
    );
    assert!(
        collect(
            std::slice::from_ref(&doc),
            &vec![input.clone(); MAX_LOGS + 1]
        )
        .is_err()
    );
    let mut malformed = input.clone();
    malformed.ops.clear();
    assert!(collect(std::slice::from_ref(&doc), &[malformed]).is_err());
    let mut malformed = input.clone();
    malformed.input_sha256 = "A".repeat(64);
    assert!(collect(std::slice::from_ref(&doc), &[malformed]).is_err());
    assert!(
        collect(
            &[json!({"times-ms":vec![Value::Null; MAX_NODES+1]})],
            &[input]
        )
        .is_err()
    );
    assert!(collect(&[doc], &[log(&[("a", MAX_CHECKS + 1)], 'a')]).is_err());
    let long_name = "a".repeat(MAX_LABEL_BYTES);
    assert!(
        collect(
            &[document(&[&long_name])],
            &[log(&[(&long_name, 600)], 'a')]
        )
        .is_err()
    );
    let too_long = "a".repeat(MAX_LABEL_BYTES + 1);
    assert!(collect(&[document(&[&too_long])], &[log(&[(&too_long, 1)], 'a')]).is_err());
}
