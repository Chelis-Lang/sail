//! Explicit profile three: qualified truncating division and remainder.
use sail_verus::contracts::{
    PROFILE_THREE_LOWERING_VERSION, lower, lower_profile, validate_manifest,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

const DIVISION: &str = include_str!("fixtures/neutral-contracts/truncating_division_profile3.rs");
const FLOOR: &str = include_str!("fixtures/neutral-contracts/floor_intuition_profile3.rs");

mod runtime_division {
    include!("fixtures/neutral-contracts/truncating_division_profile3.rs");
}
mod runtime_floor {
    include!("fixtures/neutral-contracts/floor_intuition_profile3.rs");
}

#[test]
fn profile_three_lowers_truncating_division_with_explicit_definedness() -> TestResult {
    let lowered = lower_profile(DIVISION, "neutral", 3)?;
    assert_eq!(
        (lowered.manifest.version, lowered.manifest.contract_version),
        (PROFILE_THREE_LOWERING_VERSION, 3)
    );
    assert!(lowered.manifest.arithmetic.contains("truncate toward zero"));
    let source = lowered.source.replace(' ', "");
    assert!(source.contains("crate::__sail_truncating_div((valueasint),(2asint))"));
    assert!(source.contains("crate::__sail_truncating_rem((valueasint),(4asint))"));
    // Rust's checked remainder refuses the minimum by negative one.
    assert!(source.contains("((4asint))==-1"));
    assert!(source.contains("(((divisorasint))!=0)"));
    // Unsigned operands are nonnegative, so Euclidean int division already truncates.
    assert!(source.contains("((valueasint))/((divisorasint))"));
    assert_eq!(lowered, lower_profile(DIVISION, "neutral", 3)?);
    validate_manifest(DIVISION, &lowered.manifest)?;
    assert_eq!(lowered.manifest.bindings.len(), 3);
    Ok(())
}

#[test]
fn earlier_profiles_keep_refusing_division_and_keep_their_exact_bytes() -> TestResult {
    for profile in [1, 2] {
        assert!(lower_profile(DIVISION, "neutral", profile).is_err());
        // Executable division was already admitted; only its guarantee is new.
        assert!(lower_profile(FLOOR, "neutral", profile).is_ok());
    }
    let order = include_str!("fixtures/neutral-contracts/order_service_profile2.rs");
    let two = lower_profile(order, "neutral", 2)?;
    assert_eq!(two.manifest.version, 3);
    assert!(!two.source.contains("__sail_truncating"));
    assert!(!two.manifest.arithmetic.contains("truncate"));
    let one = lower(
        include_str!("fixtures/neutral-contracts/add_one.rs"),
        "neutral",
    )?;
    assert!(!one.source.contains("__sail_truncating"));
    // A retained manifest cannot be replayed under another version's semantics.
    let mut relabeled = lower_profile(DIVISION, "neutral", 3)?.manifest;
    relabeled.version = 3;
    assert!(validate_manifest(DIVISION, &relabeled).is_err());
    Ok(())
}

#[test]
fn profile_three_refuses_division_outside_fixed_width_integers() {
    for guarantee in [
        "ret == (flag / flag)",
        "ret == ((value / 2u16) == 0)",
        "ret == (value % wide == 0)",
    ] {
        let source = format!(
            "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| {guarantee})] pub fn f(value: u8, wide: u16, flag: bool) -> bool {{ flag }}"
        );
        assert!(lower_profile(&source, "neutral", 3).is_err(), "{guarantee}");
    }
}

#[test]
fn runtime_division_matches_truncating_rust_semantics_exhaustively() {
    for value in i8::MIN..=i8::MAX {
        assert_eq!(runtime_division::half(value), value / 2);
        assert_eq!(runtime_division::remainder(value), value % 4);
    }
    for value in [0u16, 1, 7, 255, 4096, u16::MAX] {
        for divisor in [1u16, 2, 3, 255, u16::MAX] {
            assert_eq!(runtime_division::quotient(value, divisor), value / divisor);
        }
    }
    assert!(std::panic::catch_unwind(|| runtime_division::quotient(1, 0)).is_err());
    // A floor-division guarantee fails at runtime on a negative odd value.
    assert!(std::panic::catch_unwind(|| runtime_floor::half(-7)).is_err());
    assert_eq!(runtime_floor::half(6), 3);
}
