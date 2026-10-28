//! Exact raw-prefix and acknowledged-input operations, conditional on explicit primitives.
use vstd::prelude::*;
use crate::key_prefix::HashHost;

verus! {
    /// These original nonempty executable witnesses remain part of the whole root.
    pub fn prefix_constructive_controls() -> (passed: bool)
        ensures passed,
    {
        let framing = crate::key_prefix::nonempty_framing_control();
        let overflow = crate::key_prefix::overflow_framing_control();
        let feed = crate::key_prefix::owned_feed_control();
        let owner = crate::key_prefix::joint_owner_control();
        let head = crate::query_raw::commented_head_control();
        framing && overflow && feed && owner && head
    }

    /// The complete original success, partial-failure and frozen-token witnesses
    /// check actual generic controller calls, not operating-system behavior.
    pub fn delivery_constructive_controls<Host: HashHost>(host: Host) -> (passed: bool)
        requires host.bytes().len() == 0,
        ensures passed,
    {
        let complete = crate::query_delivery::complete_delivery_control(host.duplicate());
        let failed = crate::query_delivery::failed_delivery_control(host.duplicate());
        let sealed = crate::query_delivery::sealed_delivery_control(host);
        complete && failed && sealed
    }
}
