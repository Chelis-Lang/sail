#[sail_contracts::ensures(|result| result == input || old(input + 1) > 0)]
#[sail_contracts::satisfies("ENTRY-CAPTURE")]
pub fn candidate(input: u8) -> u8 { input }
