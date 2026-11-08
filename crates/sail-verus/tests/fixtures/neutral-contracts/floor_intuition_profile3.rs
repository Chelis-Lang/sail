// Profile three: a floor-division guarantee fails against truncating Rust division.

#[sail_contracts::satisfies("DIV-FLOOR")]
#[sail_contracts::ensures(|ret| 2 * ret <= value)]
pub fn half(value: i8) -> i8 {
    value / 2
}
