// Profile three: a known limit. The guarantee is exactly the body, and it is
// true, but the pinned Verus cannot prove a signed quotient by a symbolic
// divisor that may be negative. Verus bounds its executable quotient to the
// result type, and showing that bound holds needs nonlinear reasoning.
// Sail adds no lemma, so this fails verification.

#[sail_contracts::satisfies("DIV-SYMBOLIC")]
#[sail_contracts::requires(divisor != 0 && !(value == -128 && divisor == -1))]
#[sail_contracts::ensures(|ret| ret == value / divisor)]
pub fn quotient(value: i8, divisor: i8) -> i8 {
    value / divisor
}
