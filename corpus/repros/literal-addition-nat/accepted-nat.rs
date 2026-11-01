use vstd::prelude::*;
verus! {
proof fn refine() {
    let result: nat = 1 + 1;
    assert(result == 2);
}
}
