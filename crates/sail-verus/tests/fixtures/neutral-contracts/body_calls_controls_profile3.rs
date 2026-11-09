// Profile three: a narrowing cast is admitted, but Verus gives an out-of-range
// cast no value, so this guarantee is unprovable and fails. Verus notes that the
// value may be out of range of the target type.

#[sail_contracts::satisfies("BODY-NARROW")]
#[sail_contracts::ensures(|ret| ret == value)]
pub fn narrow(value: u16) -> u16 {
    let byte = value as u8;
    byte as u16
}
