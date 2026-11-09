// Profile three: a signed remainder by a symbolic divisor of either sign.

#[sail_contracts::satisfies("REM-SYMBOLIC")]
#[sail_contracts::requires(divisor != 0 && !(value == -128 && divisor == -1))]
#[sail_contracts::ensures(|ret| ret == value % divisor)]
pub fn remainder(value: i8, divisor: i8) -> i8 {
    value % divisor
}

#[sail_contracts::satisfies("REM-SYMBOLIC")]
#[sail_contracts::requires(divisor != 0 && !(value == -2147483648 && divisor == -1))]
#[sail_contracts::ensures(|ret| ret == value % divisor && (ret == 0 || (ret < 0) == (value < 0)))]
pub fn wide_remainder(value: i32, divisor: i32) -> i32 {
    value % divisor
}
