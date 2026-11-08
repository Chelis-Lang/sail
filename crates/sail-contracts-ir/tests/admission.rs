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

#[test]
fn explicit_profile_preserves_legacy_reference_and_nominal_rejections() -> TestResult {
    let source = r#"#[derive(Debug,PartialEq,Eq)] enum State { Pending, Done }
    #[satisfies("R2")] #[satisfies("R1")] #[ensures(|ret| ret == (*state == State::Pending))]
    fn active(state: &State)->bool { *state == State::Pending }"#;
    assert!(parse_source(source).is_err());
    let selected = sail_contracts_ir::parse_source_with_profile(source, 2)?;
    assert_eq!(selected.profile, 2);
    assert_eq!(selected.enums.len(), 1);
    assert_eq!(
        selected.functions[0]
            .function
            .requirements
            .iter()
            .map(sail_contracts_ir::RequirementId::as_str)
            .collect::<Vec<_>>(),
        vec!["R1", "R2"]
    );
    let legacy_nominal = r#"#[satisfies("R")] #[ensures(|ret| ret == State::Pending)] fn f()->State {State::Pending}"#;
    assert!(parse_source(legacy_nominal).is_err());
    assert!(sail_contracts_ir::parse_source_with_profile(source, 0).is_err());
    Ok(())
}

#[test]
fn profile_three_alone_admits_calls_casts_borrows_and_literal_match() {
    for body in [
        "helper(x)",
        "self::helper(x) as u8",
        "match x { 0 => 1, 1..=3 | 9 => 2, _ => x }",
        "{ let y = &x; *y }",
    ] {
        let source = format!(
            "#[satisfies(\"R\")] #[ensures(|ret| ret == ret)] fn f(x: u8) -> u8 {{ {body} }}"
        );
        assert!(
            sail_contracts_ir::parse_source_with_profile(&source, 2).is_err(),
            "{body}"
        );
        assert!(
            sail_contracts_ir::parse_source_with_profile(&source, 3).is_ok(),
            "{body}"
        );
    }
    for body in [
        "(|v: u8| v)(x)",
        "x.min(1)",
        "x as usize as u8",
        "match x { y => y }",
        "{ let mut y = x; let r = &mut y; *r }",
        "{ while x > 0 {} x }",
    ] {
        let source = format!(
            "#[satisfies(\"R\")] #[ensures(|ret| ret == ret)] fn f(x: u8) -> u8 {{ {body} }}"
        );
        assert!(
            sail_contracts_ir::parse_source_with_profile(&source, 3).is_err(),
            "{body}"
        );
    }
}

#[test]
fn profile_three_alone_admits_while_loops_with_a_head() {
    let head = "sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x - i);";
    let source = format!(
        "#[satisfies(\"R\")] #[ensures(|ret| ret == ret)] fn f(x: u8) -> u8 {{ let mut i: u8 = 0; while i < x {{ {head} i += 1; }} i }}"
    );
    assert!(sail_contracts_ir::parse_source_with_profile(&source, 2).is_err());
    assert!(sail_contracts_ir::parse_source_with_profile(&source, 3).is_ok());
    let misplaced = source.replace("i += 1; }", "i += 1; sail_contracts::decreases!(x); }");
    assert!(sail_contracts_ir::parse_source_with_profile(&misplaced, 3).is_err());
}
