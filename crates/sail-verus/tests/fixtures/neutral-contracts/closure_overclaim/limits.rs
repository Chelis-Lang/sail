//! An overclaiming helper.

/// Claims a sum is below its first operand.
#[sail_contracts::satisfies("CL-OVER")]
#[sail_contracts::requires(a <= 100 && b <= 100)]
#[sail_contracts::ensures(|ret| ret <= a)]
pub fn total(a: u8, b: u8) -> u8 {
    a + b
}
