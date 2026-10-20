use vstd::prelude::*;

verus! {

pub trait Size {
    spec fn size(&self) -> nat;
}

pub struct Pair {
    pub a: u8,
    pub b: u8,
}

impl Size for Pair {
    open spec fn size(&self) -> nat {
        2
    }
}

pub fn apply(x: u8) -> (r: u8)
    ensures r == x,
{
    let f = |y: u8| -> (z: u8) ensures z == y { y };
    f(x)
}

proof fn pair_size(p: Pair)
    ensures p.size() == 2,
{
}

} // verus!
