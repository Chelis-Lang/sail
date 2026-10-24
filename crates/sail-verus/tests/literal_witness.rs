//! Exact-source scalar witness admission; no backend executions are invented.
use sail_api::contracts::Submission;
use sail_api::mutation::digest;
use sail_verus::contracts::literal_witness::prepare;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn input(source: &str) -> Submission {
    Submission {
        profile: 1,
        source: source.into(),
        source_sha256: digest(source.as_bytes()),
        crate_name: "neutral".into(),
    }
}
const BOOL: &str = "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|ret| ret == value)] pub fn candidate(value:bool)->bool { if value {true} else {false} }";
#[test]
fn exact_primitive_queries_are_deterministic_and_validate_source_links() -> TestResult {
    let source = input(BOOL);
    let first = prepare(&source, 8)?;
    assert_eq!(first, prepare(&source, 8)?);
    first.validate()?;
    assert_eq!(first.prepared.len(), 2);
    assert_eq!(first.unsupported, Vec::<String>::new());
    assert_eq!(first.omitted, 0);
    for prepared in &first.prepared {
        assert_eq!(prepared.query.requirement_ids, ["FLAG"]);
        assert!(prepared.query.script.contains("(declare-const x0 Bool)"));
        assert!(!prepared.query.script.contains("forall"));
        assert!(!prepared.query.script.contains("get-model"));
    }
    let mut changed = first.clone();
    changed.prepared[0].candidate.source.source.push(' ');
    assert!(changed.validate().is_err());
    let mut changed = first.clone();
    changed.prepared[0].query.script.push(' ');
    assert!(changed.validate().is_err());
    let mut changed = first;
    changed.prepared[0].candidate.body_end = usize::MAX;
    assert!(changed.validate().is_err());
    Ok(())
}
#[test]
fn checked_arithmetic_short_circuit_and_old_definedness_are_retained() -> TestResult {
    let source = input(
        "#[sail_contracts::satisfies(\"SCALAR\")] #[sail_contracts::requires(amount < 255)] #[sail_contracts::ensures(|ret| (!(amount < 255)) || ret == old(amount + 1))] pub fn candidate(amount:u8)->u8 { amount + 1 }",
    );
    let result = prepare(&source, 8)?;
    result.validate()?;
    assert_eq!(result.prepared.len(), 2);
    for p in &result.prepared {
        assert!(p.query.script.contains("(<= 0 x0) (<= x0 255)"));
        assert!(p.query.script.contains("(assert (< x0 255))"));
        assert!(p.query.script.contains("(<= (+ x0 1) 255)"));
        assert!(p.query.script.contains("(not (or"));
    }
    Ok(())
}
#[test]
fn unsupported_scope_and_budget_never_disappear() -> TestResult {
    let mut source = input(
        "#[sail_contracts::satisfies(\"REF\")] #[sail_contracts::ensures(|ret| ret == *value)] fn candidate(value:&bool)->bool { *value }",
    );
    source.profile = 2;
    let result = prepare(&source, 8)?;
    assert_eq!(result.prepared, Vec::new());
    assert_eq!(result.unsupported.len(), 2);
    assert!(
        result
            .unsupported
            .iter()
            .all(|s| s.contains("owned Boolean"))
    );
    let source = input(include_str!("fixtures/neutral-contracts/add_one.rs"));
    let result = prepare(&source, 8)?;
    assert_eq!(result.prepared, Vec::new());
    assert_eq!(result.unsupported.len(), 2);
    assert!(
        result
            .unsupported
            .iter()
            .all(|s| s.contains("predicate calls"))
    );
    let result = prepare(&input(BOOL), 1)?;
    assert_eq!(result.prepared.len(), 1);
    assert_eq!(result.omitted, 1);
    assert!(prepare(&input(BOOL), 0).is_err());
    assert!(prepare(&input(BOOL), 9).is_err());
    Ok(())
}
#[test]
fn fixed_width_extrema_and_nondefault_profiles_are_exact() -> TestResult {
    let mut source = input(
        "#[sail_contracts::satisfies(\"MAX\")] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:u128)->u128 { x }",
    );
    source.profile = 2;
    let result = prepare(&source, 8)?;
    result.validate()?;
    assert_eq!(result.prepared.len(), 2);
    assert!(
        result.prepared[0]
            .query
            .script
            .contains("340282366920938463463374607431768211455")
    );
    let result = prepare(
        &input(
            "#[sail_contracts::satisfies(\"MIN\")] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:i128)->i128 { x }",
        ),
        8,
    )?;
    assert!(
        result.prepared[0]
            .query
            .script
            .contains("(- 170141183460469231731687303715884105728)")
    );
    Ok(())
}

// Injected responses exercise independent model decoding/evaluation only. They
// are not records of backend execution, solver authentication or native kills.
fn injected_model(
    plan: &sail_api::literal_witness::Plan,
    index: usize,
    pairs: &str,
) -> TestResultValue<sail_api::runtime::QueryResult> {
    use sail_api::runtime::{
        Answer, Check, Outcome, Product, ProductKind, QueryResult, SolverIdentity,
    };
    let prepared = &plan.prepared[index];
    let query = sail_api::literal_witness::model_query(prepared);
    let commands = sail_smtlib::sexpr::parse_all(query.script.as_bytes())?;
    Ok(QueryResult {
        obligation_id: query.obligation_id,
        requirement_ids: query.requirement_ids,
        backend: "z3".into(),
        outcome: Outcome::Completed,
        checks: vec![Check {
            command_index: commands.len() - 2,
            answer: Answer::Sat,
            products: vec![Product {
                command_index: commands.len() - 1,
                kind: ProductKind::Values,
                raw: pairs.as_bytes().into(),
            }],
        }],
        stdout: format!("sat\n{pairs}\n").into_bytes(),
        stderr: vec![],
        solver: Some(SolverIdentity {
            backend: "z3".into(),
            version: "injected-test-only".into(),
            sha256: "a".repeat(64),
            platform: "test-only".into(),
        }),
        script_sha256: digest(query.script.as_bytes()),
        limits: query.limits,
        cache_hit: false,
        error: None,
    })
}
type TestResultValue<T> = Result<T, Box<dyn std::error::Error>>;
#[test]
fn decoded_models_must_violate_the_actual_unchanged_contract() -> TestResult {
    let plan = prepare(&input(BOOL), 8)?;
    let good = injected_model(&plan, 0, "((x0 true) (true true))")?;
    let validated = sail_verus::contracts::literal_witness::validate(
        &plan,
        &plan.prepared[0].candidate.id,
        &good,
    )?;
    assert_eq!(
        validated.values,
        [("value".into(), sail_api::output::ItfValue::Bool(true))]
    );
    let not_a_violation = injected_model(&plan, 0, "((x0 false) (true true))")?;
    assert!(
        sail_verus::contracts::literal_witness::validate(
            &plan,
            &plan.prepared[0].candidate.id,
            &not_a_violation
        )
        .is_err()
    );
    for pair in [
        "((x1 true) (true true))",
        "((x0 1) (true true))",
        "((x0 true))",
        "((x0 true) (true false))",
    ] {
        let bad = injected_model(&plan, 0, pair)?;
        assert!(
            sail_verus::contracts::literal_witness::validate(
                &plan,
                &plan.prepared[0].candidate.id,
                &bad
            )
            .is_err()
        );
    }
    let mut changed = good.clone();
    changed.checks[0].answer = sail_api::runtime::Answer::Unknown;
    assert!(
        sail_verus::contracts::literal_witness::validate(
            &plan,
            &plan.prepared[0].candidate.id,
            &changed
        )
        .is_err()
    );
    let mut changed = good.clone();
    changed.stdout = b"sat\n((x0 false) (true true))\n".into();
    assert!(
        sail_verus::contracts::literal_witness::validate(
            &plan,
            &plan.prepared[0].candidate.id,
            &changed
        )
        .is_err()
    );
    let mut changed = plan.clone();
    changed.prepared[0].query.script.push_str("; changed\n");
    changed.prepared[0].query_sha256 = digest(changed.prepared[0].query.script.as_bytes());
    changed.prepared[0].query.obligation_id = format!(
        "{}:{}:{}",
        changed.policy, changed.prepared[0].candidate.id, changed.prepared[0].query_sha256
    );
    assert!(
        sail_verus::contracts::literal_witness::validate(
            &changed,
            &changed.prepared[0].candidate.id,
            &good
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn concrete_validation_refuses_false_preconditions_out_of_range_and_undefined_arithmetic()
-> TestResult {
    for (source, pairs, expected) in [
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::requires(x < 10)] #[sail_contracts::ensures(|ret| ret == x + 1)] fn candidate(x:u8)->u8 {x + 1}",
            "((x0 100) (true true))",
            false,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:u8)->u8 {x}",
            "((x0 256) (true true))",
            false,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| ret == x + 1)] fn candidate(x:u8)->u8 {x + 1}",
            "((x0 255) (true true))",
            false,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| ret == old(x + 1))] fn candidate(x:u8)->u8 {x + 1}",
            "((x0 255) (true true))",
            false,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:i128)->i128 {x}",
            "((x0 (- 170141183460469231731687303715884105728)) (true true))",
            true,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:u128)->u128 {x}",
            "((x0 340282366920938463463374607431768211455) (true true))",
            true,
        ),
        (
            "#[sail_contracts::satisfies(\"INT\")] #[sail_contracts::ensures(|ret| (x == 255) || ret == x + 1)] fn candidate(x:u8)->u8 {x}",
            "((x0 254) (true true))",
            true,
        ),
    ] {
        let plan = prepare(&input(source), 8)?;
        assert_ne!(plan.prepared, Vec::new());
        let model = injected_model(&plan, 0, pairs)?;
        assert_eq!(
            sail_verus::contracts::literal_witness::validate(
                &plan,
                &plan.prepared[0].candidate.id,
                &model
            )
            .is_ok(),
            expected,
            "{source}"
        );
    }
    Ok(())
}
#[test]
fn zero_input_witness_and_shadowed_result_are_handled_explicitly() -> TestResult {
    let plan = prepare(
        &input(
            "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|ret| ret)] fn candidate()->bool {true || false}",
        ),
        8,
    )?;
    let result = injected_model(&plan, 0, "((true true))")?;
    assert!(
        sail_verus::contracts::literal_witness::validate(
            &plan,
            &plan.prepared[0].candidate.id,
            &result
        )
        .is_ok()
    );
    let shadowed = prepare(
        &input(
            "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|value| value)] fn candidate(value:bool)->bool {value}",
        ),
        8,
    );
    assert!(shadowed.is_err());
    Ok(())
}
