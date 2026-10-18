use vstd::prelude::*;
verus! {
proof fn check(s: crate::defs::Kept, x: int) {
    assert(s.value(x) == x + 1);
}
pub fn call(s: &crate::defs::Kept, x: u8) -> (r: u8)
    ensures r == x
{
    s.run(x)
}
}
