#[sail_contracts::ensures(|result| result == 255 || result + 1 > result)]
#[sail_contracts::satisfies("SHORT-CIRCUIT")]
pub fn candidate(input: u8) -> u8 { input }
