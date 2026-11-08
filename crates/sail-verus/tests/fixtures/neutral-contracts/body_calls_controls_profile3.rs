// Profile three: a narrowing cast truncates, so this guarantee fails.

#[sail_contracts::satisfies("BODY-NARROW")]
#[sail_contracts::ensures(|ret| ret == value)]
pub fn narrow(value: u16) -> u16 {
    let byte = value as u8;
    byte as u16
}
