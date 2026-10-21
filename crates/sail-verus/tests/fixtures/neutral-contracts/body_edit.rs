#[sail_contracts::requirement("ADD-ONE")]
pub fn next_value(before: u8, after: u8) -> bool { after == before + 1 }

#[sail_contracts::requires(input < 100)]
#[sail_contracts::ensures(|result| next_value(input, result))]
#[sail_contracts::satisfies("ADD-ONE")]
pub fn candidate(input: u8) -> u8 {
    if input == 0 { return 1; }
    input
}
