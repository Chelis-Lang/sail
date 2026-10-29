use vstd::prelude::*;

verus! {

pub fn identity(x: u64) -> (r: u64)
    ensures r == x,
{ x }

}
