use vstd::prelude::*;

verus! {

pub fn identity(b: bool) -> (r: bool)
    ensures r == b,
{ b }

} // verus!
