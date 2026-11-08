// Profile three: a call must establish its callee's precondition, so this fails.

#[sail_contracts::satisfies("BODY-POSITIVE")]
#[sail_contracts::requires(divisor > 0)]
#[sail_contracts::ensures(|ret| ret <= value)]
pub fn share(value: u32, divisor: u32) -> u32 {
    value / divisor
}

#[sail_contracts::satisfies("BODY-POSITIVE")]
#[sail_contracts::ensures(|ret| ret <= value)]
pub fn unchecked(value: u32, divisor: u32) -> u32 {
    share(value, divisor)
}
