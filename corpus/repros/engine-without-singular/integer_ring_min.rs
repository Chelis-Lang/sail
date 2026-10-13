use vstd::prelude::*;

verus! {

proof fn mod_after_mul(x: int, y: int, z: int, m: int) by (integer_ring)
    requires
        (x - y) % m == 0,
    ensures
        (x * z - y * z) % m == 0,
{
}

fn main() {
}

} // verus!
