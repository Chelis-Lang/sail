use vstd::prelude::*;

verus! {

pub fn increment(x: u8) -> (r: u8)
    requires x < 100,
    ensures r == x + 1,
{ 0 }

} // verus!
