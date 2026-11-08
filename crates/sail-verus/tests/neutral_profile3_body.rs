//! Explicit profile three: calls, casts and match in executable bodies.
use sail_verus::contracts::{lower_profile, validate_manifest};
type TestResult = Result<(), Box<dyn std::error::Error>>;

const CALLS: &str = include_str!("fixtures/neutral-contracts/body_calls_profile3.rs");

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    reason = "The fixtures exercise Rust's own cast semantics."
)]
mod runtime_calls {
    include!("fixtures/neutral-contracts/body_calls_profile3.rs");
}
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    reason = "The fixtures exercise Rust's own cast semantics."
)]
mod runtime_narrow {
    include!("fixtures/neutral-contracts/body_calls_controls_profile3.rs");
}
mod runtime_precondition {
    include!("fixtures/neutral-contracts/body_call_precondition_profile3.rs");
}

#[test]
fn profile_three_lowers_bodies_verbatim_and_earlier_profiles_refuse_them() -> TestResult {
    let lowered = lower_profile(CALLS, "neutral", 3)?;
    validate_manifest(CALLS, &lowered.manifest)?;
    let source = lowered.source.replace(' ', "");
    // Bodies are emitted as written; lowering adds no assertion or lemma.
    assert!(source.contains("letclamped=clamp(value,255);clampedasu16"));
    assert!(source.contains("level(Self::raw(self))"));
    assert!(source.contains("self.headroom()==0"));
    assert!(source.contains("1..=9=>1"));
    assert!(!source.contains("assume"));
    assert_eq!(lowered.manifest.bindings.len(), 8);
    for profile in [1, 2] {
        assert!(lower_profile(CALLS, "neutral", profile).is_err());
    }
    Ok(())
}

#[test]
fn profile_three_refuses_calls_casts_and_patterns_outside_the_admitted_forms() {
    let callee = "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == x)] pub fn id(x: u8) -> u8 { x }";
    let predicate = "#[sail_contracts::requirement(\"R\")] pub fn small(x: u8) -> bool { x < 9 }";
    for (body, admitted) in [
        ("id(x)", true),
        ("crate::id(x) as u8", true),
        ("(x == 0) as u8", true),
        (
            "match x { 0 if x > 0 => 0, 1 | 2 => 1, 3..=9 => 2, _ => x }",
            true,
        ),
        ("missing(x)", false),
        ("u8::max(x, 1)", false),
        ("if small(x) { x } else { 0 }", false),
        ("f(x)", false),
        ("g(x)", false),
        ("(|v: u8| v)(x)", false),
        ("id::<>(x)", false),
        ("x.min(1)", false),
        ("x as usize as u8", false),
        ("match x { y => y }", false),
        ("match Some(x) { Some(v) => v, None => 0 }", false),
        ("{ let mut y = x; let r = &mut y; *r }", false),
        ("{ for _ in 0..1 {} x }", false),
        ("{ println!(\"{}\", x); x }", false),
    ] {
        // g always calls f, so a call from f back to g closes a cycle.
        let source = format!(
            "{callee} {predicate} \
             #[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == ret)] pub fn f(x: u8) -> u8 {{ {body} }} \
             #[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == ret)] pub fn g(x: u8) -> u8 {{ f(x) }}"
        );
        let lowered = lower_profile(&source, "neutral", 3);
        assert_eq!(lowered.is_ok(), admitted, "{body}: {lowered:?}");
    }
    assert!(
        lower_profile(
            "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret == ret)] pub fn f(x: u8) -> bool { x as bool }",
            "neutral",
            3
        )
        .is_err()
    );
}

#[test]
fn runtime_bodies_follow_rust_semantics() {
    for value in [0u32, 1, 254, 255, 256, u32::MAX] {
        assert_eq!(
            runtime_calls::level(value),
            u16::try_from(value.min(255)).unwrap_or(255)
        );
    }
    assert_eq!(runtime_calls::rank(&runtime_calls::Mode::High), 2);
    assert_eq!(runtime_calls::rank(&runtime_calls::Mode::Low), 1);
    assert_eq!(runtime_calls::rank(&runtime_calls::Mode::Off), 0);
    assert_eq!(
        [0u8, 5, 10, 11, 12, 255].map(runtime_calls::bucket),
        [0, 1, 2, 2, 3, 3]
    );
    let meter = runtime_calls::Meter {
        reading: 300,
        limit: 300,
    };
    assert!(meter.full());
    assert_eq!(meter.display(), 255);
    // The narrowing cast truncates, so its runtime guarantee check fails.
    assert_eq!(runtime_narrow::narrow(7), 7);
    assert!(std::panic::catch_unwind(|| runtime_narrow::narrow(256)).is_err());
    // The callee's runtime precondition check rejects a zero divisor.
    assert!(std::panic::catch_unwind(|| runtime_precondition::unchecked(1, 0)).is_err());
}
