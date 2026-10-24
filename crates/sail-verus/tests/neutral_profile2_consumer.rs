//! Exact consumer-emitted source compiles with its original non-Copy ownership.
#[allow(
    clippy::manual_clamp,
    clippy::nonminimal_bool,
    reason = "The exact consumer fixture retains direct branches and unsimplified generated requirement formulas."
)]
#[path = "fixtures/neutral-contracts/order_service_profile2_consumer.rs"]
mod runtime;
use runtime::State;
#[test]
fn actual_consumer_source_keeps_runtime_checks_and_six_requirement_bindings()
-> Result<(), Box<dyn std::error::Error>> {
    let original = include_str!("fixtures/neutral-contracts/order_service_profile2_consumer.rs");
    let lowered = sail_verus::contracts::lower_profile(original, "oyster_order_service", 2)?;
    assert_eq!(lowered.manifest.bindings.len(), 6);
    for (state, active) in [(State::Pending, true), (State::CanceledRefunded, false)] {
        let order = runtime::Order { state };
        for inventory in [false, true] {
            let out = runtime::submit(&order, inventory);
            assert_eq!(out.fulfilled, active && inventory);
            assert_eq!(out.backordered, active && !inventory);
            assert!(!out.refunded);
        }
        assert_eq!(runtime::cancel(&order).refunded, active);
    }
    for amount in [0, 9, 10, 11, 99, 100, 101, u32::MAX] {
        assert_eq!(runtime::quote(amount), amount.clamp(10, 100));
    }
    for initial in [0, 9, 10, 11, u32::MAX] {
        let mut count = initial;
        assert!(runtime::advance(&mut count));
        assert_eq!(count, if initial < 10 { initial + 1 } else { initial });
    }
    sail_verus::contracts::validate_manifest(original, &lowered.manifest)?;
    Ok(())
}
