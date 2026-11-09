//! Explicit profile three: while loops with invariants and termination measures.
use sail_verus::contracts::{lower_profile, validate_manifest};
type TestResult = Result<(), Box<dyn std::error::Error>>;

const LOOPS: &str = include_str!("fixtures/neutral-contracts/loops_profile3.rs");

mod runtime_loops {
    include!("fixtures/neutral-contracts/loops_profile3.rs");
}
mod runtime_weak {
    include!("fixtures/neutral-contracts/loop_weak_invariant_profile3.rs");
}

#[test]
fn profile_three_lowers_loop_heads_to_verus_clauses() -> TestResult {
    let lowered = lower_profile(LOOPS, "neutral", 3)?;
    validate_manifest(LOOPS, &lowered.manifest)?;
    let source = lowered.source.replace(' ', "");
    assert!(source.contains("whilei<ninvariant"));
    assert!(source.contains("decreases((nasint))-((iasint)),{i+=1;total+=2;}"));
    // A loop without invariants keeps only its measure.
    assert!(source.contains("whileleft>0decreases(leftasint),{left-=1;}"));
    assert!(!source.contains("sail_contracts::invariant"));
    assert!(!source.contains("sail_contracts::decreases"));
    assert!(!source.contains("assume"));
    for profile in [1, 2] {
        assert!(lower_profile(LOOPS, "neutral", profile).is_err());
    }
    Ok(())
}

#[test]
fn profile_three_refuses_loops_outside_the_admitted_head() {
    for (body, expected) in [
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x - i); i += 1; } i",
            None,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; } i",
            None,
        ),
        (
            "let mut i: u8 = 0; while i < x { i += 1; } i",
            Some("a while loop body must start with an optional"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); i += 1; } i",
            Some("a while loop body must start with an optional"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); sail_contracts::invariant!(i <= x); i += 1; } i",
            Some("items and macros in neutral function bodies"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i, i); i += 1; } i",
            Some("a while loop body must start with an optional"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(); sail_contracts::decreases!(x - i); i += 1; } i",
            Some("a while loop body must start with an optional"),
        ),
        (
            "let mut i: u8 = 0; 'outer: while i < x { sail_contracts::decreases!(x - i); i += 1; } i",
            Some("loop labels and attributes are unsupported"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; break; } i",
            Some("unsupported neutral executable body"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; continue; } i",
            Some("unsupported neutral executable body"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(old(x) == x); sail_contracts::decreases!(x - i); i += 1; } i",
            Some("old is unsupported in loop invariants"),
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x > i); i += 1; } i",
            Some("a loop measure must be a fixed-width integer expression"),
        ),
        // The head sees locals only through explicit scalar annotations.
        (
            "let mut i = 0u8; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x - i); i += 1; } i",
            Some("unresolved specification binding"),
        ),
        (
            "let mut i: u8 = 0; let i: u8 = 1; while i < x { sail_contracts::decreases!(x - i); } i",
            Some("i is declared twice or shadows an input"),
        ),
        (
            "let x: u8 = 1; while x < 9 { sail_contracts::decreases!(9 - x); } x",
            Some("x is declared twice or shadows an input"),
        ),
        (
            "let mut i: u8 = 0; loop { i += 1; if i > x { return i; } }",
            Some("unsupported neutral executable body"),
        ),
        (
            "let mut i: u8 = 0; for _ in 0..x { i += 1; } i",
            Some("unsupported neutral executable body"),
        ),
        (
            "let mut i: u8 = 0; sail_contracts::decreases!(x); i",
            Some("items and macros in neutral function bodies"),
        ),
    ] {
        let source = format!(
            "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == ret)] pub fn f(x: u8) -> u8 {{ {body} }}"
        );
        let reason = lower_profile(&source, "neutral", 3)
            .err()
            .map(|error| error.to_string());
        refused_for(body, reason.as_deref(), expected);
    }
}

#[test]
fn runtime_loops_run_as_ordinary_rust() {
    assert_eq!(runtime_loops::double(7), 14);
    assert_eq!(runtime_loops::drain(200), 0);
    assert_eq!(runtime_loops::cells(1000), 4000);
    assert_eq!(runtime_loops::cells(0), 0);
    // Invariants are type checked but not evaluated at runtime.
    assert_eq!(runtime_weak::double(3), 6);
}

/// Admitted rows expect no error; refused rows name the refusal they expect.
fn refused_for(body: &str, reason: Option<&str>, expected: Option<&str>) {
    match expected {
        None => assert_eq!(reason, None, "{body}"),
        Some(want) => assert!(
            reason.is_some_and(|reason| reason.contains(want)),
            "{body}: expected {want:?}, got {reason:?}"
        ),
    }
}
