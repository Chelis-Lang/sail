#[sail_contracts::ensures(|result| result >= -128i8)]
#[sail_contracts::satisfies("SIGNED-MIN")]
pub fn candidate(input: i8) -> i8 { input }
