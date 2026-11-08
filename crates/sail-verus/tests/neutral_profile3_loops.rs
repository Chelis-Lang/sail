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
    for (body, admitted) in [
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x - i); i += 1; } i",
            true,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; } i",
            true,
        ),
        ("let mut i: u8 = 0; while i < x { i += 1; } i", false),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); sail_contracts::invariant!(i <= x); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i, i); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(); sail_contracts::decreases!(x - i); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; 'outer: while i < x { sail_contracts::decreases!(x - i); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; break; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::decreases!(x - i); i += 1; continue; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(old(x) == x); sail_contracts::decreases!(x - i); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x > i); i += 1; } i",
            false,
        ),
        // The head sees locals only through explicit scalar annotations.
        (
            "let mut i = 0u8; while i < x { sail_contracts::invariant!(i <= x); sail_contracts::decreases!(x - i); i += 1; } i",
            false,
        ),
        (
            "let mut i: u8 = 0; let i: u8 = 1; while i < x { sail_contracts::decreases!(x - i); } i",
            false,
        ),
        (
            "let x: u8 = 1; while x < 9 { sail_contracts::decreases!(9 - x); } x",
            false,
        ),
        (
            "let mut i: u8 = 0; loop { i += 1; if i > x { return i; } }",
            false,
        ),
        ("let mut i: u8 = 0; for _ in 0..x { i += 1; } i", false),
        ("let mut i: u8 = 0; sail_contracts::decreases!(x); i", false),
    ] {
        let source = format!(
            "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == ret)] pub fn f(x: u8) -> u8 {{ {body} }}"
        );
        let lowered = lower_profile(&source, "neutral", 3);
        assert_eq!(lowered.is_ok(), admitted, "{body}: {lowered:?}");
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
