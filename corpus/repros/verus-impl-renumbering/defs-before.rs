use vstd::prelude::*;
verus! {
pub struct Unrelated {}
pub struct Kept {}
impl Kept {
    pub open spec fn value(&self, x: int) -> int { x + 1 }
    pub fn run(&self, x: u8) -> (r: u8)
        ensures r == x
    {
        let same = |v: u8| -> (out: u8) ensures out == v { v };
        same(x)
    }
}
}
