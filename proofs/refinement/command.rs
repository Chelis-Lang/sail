//! Whole shared command/policy root and actual constructive native operations.
//! Session state, solver meaning and external stream origins remain separate.
use vstd::prelude::*;

verus! {
    /// The actual original Push token and default count survive classification.
    pub fn command_constructive_control() -> (passed: bool)
        ensures passed,
    { crate::query_command::classifier_constructive_control() }
}
