use vstd::prelude::*;

verus! {
pub proof fn false_assertion() {
    assert(false);
}
}

fn main() {}
