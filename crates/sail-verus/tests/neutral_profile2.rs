//! Explicit profile-two admission, per-requirement mapping and retained legacy identity.
use sail_verus::contracts::{lower, lower_profile, validate_manifest};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const SOURCE: &str = r#"//! Bounded flat order source.
#![forbid(unsafe_code)]
#[derive(Debug,PartialEq,Eq)] pub enum State { Pending, CanceledRefunded }
#[derive(Debug)] pub struct Order { pub state: State }
#[derive(Debug)] pub struct Outcome { pub fulfilled:bool, pub refunded:bool }
#[sail_contracts::satisfies("ORD-002")]
#[sail_contracts::satisfies("ORD-001")]
#[sail_contracts::ensures(|ret| (ret.fulfilled == (order.state == State::Pending && inventory)) && (ret.refunded == old(order.state == State::CanceledRefunded)))]
#[must_use] pub fn submit(order: &Order, inventory:bool)->Outcome {
    Outcome { fulfilled: order.state == State::Pending && inventory, refunded: order.state == State::CanceledRefunded }
}
#[sail_contracts::satisfies("STEP")]
#[sail_contracts::ensures(|ret| ret && (*count == (if old(*count) < 10 { old(*count) + 1 } else { old(*count) }))) ]
pub fn advance(count:&mut u32)->bool { if *count < 10 { *count = *count + 1; } true }
"#;
#[test]
fn profile_two_retains_nominal_types_and_conjunctive_requirement_rows() -> TestResult {
    assert!(lower(SOURCE, "order").is_err());
    let result = lower_profile(SOURCE, "order", 2)?;
    assert_eq!(
        (result.manifest.version, result.manifest.contract_version),
        (3, 2)
    );
    assert_eq!(result.manifest.bindings.len(), 3);
    let rows = result
        .manifest
        .bindings
        .iter()
        .filter(|b| b.function.ends_with("submit"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].guarantee_sha256, rows[1].guarantee_sha256);
    assert_eq!(rows[0].lowered_function, rows[1].lowered_function);
    assert!(result.source.contains("final (count)"));
    validate_manifest(SOURCE, &result.manifest)?;
    assert_eq!(result, lower_profile(SOURCE, "order", 2)?);
    Ok(())
}
#[test]
fn profile_two_refuses_payloads_discriminants_traits_derived_copy_and_unknown_nominals() {
    for source in [
        SOURCE.replace("Pending, CanceledRefunded", "Pending(u8), CanceledRefunded"),
        SOURCE.replace("Pending, CanceledRefunded", "Pending = 1, CanceledRefunded"),
        SOURCE.replace(
            "derive(Debug)] pub struct Order",
            "derive(Debug,Clone,Copy)] pub struct Order",
        ),
        SOURCE.replace("State::Pending", "State::Unresolved"),
        SOURCE.replace("&mut u32", "&mut Order"),
        SOURCE.replace("&mut u32", "&mut State"),
        SOURCE.replace(
            "old(order.state == State::CanceledRefunded)",
            "old(order.state) == State::CanceledRefunded",
        ),
        SOURCE.replace("#![forbid(unsafe_code)]", "#![cfg(any())]"),
    ] {
        assert!(lower_profile(&source, "order", 2).is_err(), "{source}");
    }
    assert!(lower_profile(SOURCE, "order", 3).is_err());
}

#[allow(
    clippy::manual_clamp,
    reason = "The admitted fixture uses direct branches and no executable calls."
)]
#[path = "fixtures/neutral-contracts/order_service_profile2.rs"]
mod runtime_order;
use runtime_order::State;

#[test]
fn actual_order_fixture_compiles_without_copy_and_retains_six_real_bindings() -> TestResult {
    let original = include_str!("fixtures/neutral-contracts/order_service_profile2.rs");
    let lowered = lower_profile(original, "oyster_order_service", 2)?;
    assert_eq!(lowered.manifest.bindings.len(), 6);
    assert_eq!(
        lowered
            .manifest
            .bindings
            .iter()
            .map(|b| b.requirement_id.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "ORD-001",
            "ORD-002",
            "ORD-003",
            "ORD-004",
            "BND-001",
            "ORD-ADVANCE-001"
        ])
    );
    for (state, active) in [
        (runtime_order::State::Pending, true),
        (runtime_order::State::CanceledRefunded, false),
    ] {
        let order = runtime_order::Order { state };
        for inventory in [false, true] {
            let out = runtime_order::submit(&order, inventory);
            assert_eq!(out.fulfilled, active && inventory);
            assert_eq!(out.backordered, active && !inventory);
            assert!(!out.refunded);
        }
        assert_eq!(runtime_order::cancel(&order).refunded, active);
    }
    for amount in [0, 9, 10, 11, 99, 100, 101, u32::MAX] {
        assert_eq!(runtime_order::quote(amount), amount.clamp(10, 100));
    }
    for initial in [0, 9, 10, 11, u32::MAX] {
        let mut count = initial;
        assert!(runtime_order::advance(&mut count));
        assert_eq!(count, if initial < 10 { initial + 1 } else { initial });
    }
    validate_manifest(original, &lowered.manifest)?;
    Ok(())
}
