//! Syntax and source closure admission checks.
use sail_contracts_ir::parse_source;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn resolves_actual_predicates_and_preserves_source_order() -> TestResult {
    let source = r#"
        mod rules {
            #[requirement("REQ-ONE")]
            pub fn one(x: u64, ret: u64) -> bool { ret == x + 1 }
        }
        #[requires(x < 10)]
        #[requires(x > 0)]
        #[ensures(|ret| rules::one(x, ret))]
        #[satisfies("REQ-ONE")]
        fn increment(x: u64) -> u64 { x + 1 }
    "#;
    let model = parse_source(source)?;
    assert_eq!(model.functions.len(), 2);
    assert!(model.functions[0].function.predicate);
    assert_eq!(model.functions[1].function.requirement.as_str(), "REQ-ONE");
    assert_eq!(model.functions[1].function.requires.len(), 2);
    Ok(())
}

#[test]
fn rejects_unresolved_unmarked_and_other_requirement_callees() {
    for predicate in [
        "",
        "fn valid(x: u64) -> bool { true }",
        "#[requirement(\"OTHER\")] fn valid(x: u64) -> bool { x > 0 }",
    ] {
        let source = format!(
            "{predicate} #[satisfies(\"R\")] #[ensures(|ret| valid(ret))] fn f(x:u64)->u64 {{x}}"
        );
        assert!(parse_source(&source).is_err(), "{predicate}");
    }
}

#[test]
fn rejects_effectful_predicates_and_unsupported_executables() {
    for body in [
        "side_effect(x)",
        "{ x = 1; true }",
        "{ loop {} }",
        "unsafe { true }",
        "x.method()",
        "some_macro!()",
    ] {
        let source = format!("#[requirement(\"R\")] fn predicate(x:u64)->bool {{ {body} }}");
        assert!(parse_source(&source).is_err(), "{body}");
    }
    for body in ["loop {}", "f(x)", "{ unsafe { x } }", "arbitrary!()"] {
        let source =
            format!("#[satisfies(\"R\")] #[ensures(|ret| ret == x)] fn f(x:u64)->u64 {{ {body} }}");
        assert!(parse_source(&source).is_err(), "{body}");
    }
}

#[test]
fn rejects_invalid_ids_duplicate_guarantees_and_old_in_preconditions() {
    for attributes in [
        "#[satisfies(\"\")] #[ensures(|ret| ret == x)]",
        "#[satisfies(\"R\")] #[ensures(|ret| ret == x)] #[ensures(|ret| ret > 0)]",
        "#[satisfies(\"R\")] #[requires(old(x) > 0)] #[ensures(|ret| ret == x)]",
        "#[satisfies(\"R\")] #[ensures(|ret| old(side_effect(x)) == ret)]",
        "#[satisfies(\"R\")] #[ensures(|ret| old(ret) == x)]",
    ] {
        assert!(parse_source(&format!("{attributes} fn f(x:u64)->u64 {{x}} ")).is_err());
    }
}

#[test]
fn source_admission_rechecks_bodies_instead_of_generated_names() {
    let source = r#"
        fn __sail_pure_unmarked(x:u64)->bool { true }
        #[satisfies("R")] #[ensures(|ret| unmarked(ret))]
        fn identity(x:u64)->u64 { x }
    "#;
    assert!(parse_source(source).is_err());
}
