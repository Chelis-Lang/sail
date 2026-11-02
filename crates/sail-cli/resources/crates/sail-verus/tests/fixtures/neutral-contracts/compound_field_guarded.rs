#[derive(Clone, Copy)]
pub struct Row {
    pub flag: bool,
}
#[sail_contracts::satisfies("FIELD-DEFINED")]
#[sail_contracts::requires(x < 255)]
#[sail_contracts::ensures(|ret| ret == (if x + 1 > 0 { a } else { b }).flag)]
pub fn candidate(x: u8, a: Row, b: Row) -> bool {
    a.flag
}
