//! An order service whose real behavior is exercised by Pearl's generated tests.
#![forbid(unsafe_code)]

/// Lifecycle state.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    /// Newly submitted.
    Pending,
    /// Already canceled and refunded.
    CanceledRefunded,
}

/// A real order, not a generated sample constant.
#[derive(Debug)]
pub struct Order {
    /// Current state.
    pub state: State,
}

/// Observable implementation outputs.
#[derive(Debug)]
pub struct Outcome {
    /// Whether fulfillment occurred.
    pub fulfilled: bool,
    /// Whether backordering occurred.
    pub backordered: bool,
    /// Whether a refund occurred.
    pub refunded: bool,
}

#[sail_contracts::satisfies("ORD-001")]
#[sail_contracts::satisfies("ORD-002")]
#[sail_contracts::satisfies("ORD-003")]
#[sail_contracts::ensures(|ret| ((!(((true && inventory) && old((order.state != crate::State::CanceledRefunded))))) || (ret.fulfilled)) && ((!(((true && (!inventory)) && old((order.state != crate::State::CanceledRefunded))))) || (ret.backordered)) && ((!(old((order.state == crate::State::CanceledRefunded)))) || ((!ret.fulfilled))))]
/// Process a submission without reviving a refunded order.
#[must_use]
pub fn submit(order: &Order, inventory: bool) -> Outcome {
    let active = order.state != State::CanceledRefunded;
    let fulfilled = active && inventory;
    Outcome {
        fulfilled,
        backordered: active && !inventory,
        refunded: false,
    }
}

#[sail_contracts::satisfies("ORD-004")]
#[sail_contracts::ensures(|ret| (!((true && old((order.state != crate::State::CanceledRefunded))))) || (ret.refunded))]
/// Cancel an order and refund it once.
#[must_use]
pub fn cancel(order: &Order) -> Outcome {
    Outcome {
        fulfilled: false,
        backordered: false,
        refunded: order.state != State::CanceledRefunded,
    }
}

#[sail_contracts::satisfies("BND-001")]
#[sail_contracts::ensures(|ret| (!(true)) || (((!(ret < 10)) && (ret <= 100))))]
/// Quote a price with both inclusive bounds.
#[must_use]
pub fn quote(amount: u32) -> u32 {
    if amount < 10 {
        10
    } else if amount > 100 {
        100
    } else {
        amount
    }
}

#[sail_contracts::satisfies("ORD-ADVANCE-001")]
#[sail_contracts::ensures(|ret| (!(true)) || ((*count == (if (old(*count) < 10) { (old(*count) + 1) } else { old(*count) }))))]
/// Advance a bounded state counter through an actual mutable reference.
pub fn advance(count: &mut u32) -> bool {
    if *count < 10 {
        *count += 1;
    }
    true
}
