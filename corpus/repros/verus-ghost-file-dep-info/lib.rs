use vstd::prelude::*;

// Compiled only in the verification phase.
#[cfg(verus_keep_ghost)]
mod ghost;

verus! {

// Expanded only in the verification phase.
proof fn included_value_is_two() {
    assert(include!("value.rs") == 2);
}

} // verus!
