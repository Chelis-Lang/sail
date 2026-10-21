use vstd::prelude::*;

verus! {

pub fn bounded_increment(x: u64) -> (result: u64)
    requires x < u64::MAX,
    ensures result == x + 1,
{
    x + 1
}

pub proof fn low_bits_roundtrip(slot: u32, generation: u32)
    ensures
        (((slot as u64) | ((generation as u64) << 32u64)) & 0xFFFF_FFFFu64) as u32 == slot,
{
    assert((((slot as u64) | ((generation as u64) << 32u64)) & 0xFFFF_FFFFu64) as u32 == slot) by(bit_vector);
}

}
