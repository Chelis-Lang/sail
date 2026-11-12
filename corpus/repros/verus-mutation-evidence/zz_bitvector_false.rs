use vstd::prelude::*;

verus! {

pub fn low_bit(x: u8)
{
    assert(x & 1 == 1) by (bit_vector);
}

} // verus!
