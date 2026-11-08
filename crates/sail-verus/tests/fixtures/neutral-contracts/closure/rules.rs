//! Requirement predicates shared by the closure.

/// A span bound.
#[sail_contracts::requirement("CL-SPAN")]
pub fn fits(value: u16) -> bool {
    value <= 512
}
