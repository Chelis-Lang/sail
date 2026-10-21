#[sail_contracts::requirement("NEGATED-OVERFLOW")]
pub fn overflowing(value: u8) -> bool { value + 1 == 0 }

#[sail_contracts::ensures(|result| !overflowing(result))]
#[sail_contracts::satisfies("NEGATED-OVERFLOW")]
pub fn candidate() -> u8 { 255 }
