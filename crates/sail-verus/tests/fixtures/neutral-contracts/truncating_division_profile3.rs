// Profile three: Rust's truncating division and remainder in specifications.

#[sail_contracts::requirement("DIV-EVEN")]
pub fn even(value: u16) -> bool {
    value % 2 == 0
}

#[sail_contracts::satisfies("DIV-HALF")]
#[sail_contracts::ensures(|ret| ret == value / 2 && (value >= 0 || value % 2 == 0 || 2 * ret == value + 1))]
pub fn half(value: i8) -> i8 {
    value / 2
}

#[sail_contracts::satisfies("DIV-REM")]
#[sail_contracts::ensures(|ret| ret == value % 4 && (ret == 0 || (ret < 0) == (value < 0)))]
pub fn remainder(value: i8) -> i8 {
    value % 4
}

#[sail_contracts::satisfies("DIV-EVEN")]
#[sail_contracts::requires(divisor != 0)]
#[sail_contracts::ensures(|ret| ret == value / divisor && even(value - value % 2))]
pub fn quotient(value: u16, divisor: u16) -> u16 {
    value / divisor
}
