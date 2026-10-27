use vstd::prelude::*;

verus! {
    proof fn external_literal_is_seven() {
        assert(sail_read_escape::external_literal!() == 7u64);
    }
}

fn main() {}
