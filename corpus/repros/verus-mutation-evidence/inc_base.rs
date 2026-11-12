use vstd::prelude::*;

verus! {

pub fn increment(x: u8) -> (r: u8)
    requires x < 100,
    ensures r == x + 1,
{ x + 1 }

} // verus!
