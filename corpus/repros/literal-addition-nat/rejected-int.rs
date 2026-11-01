use vstd::prelude::*;
verus! {
proof fn refine() {
    let result: int = 1 + 1;
    assert(result == 2);
}
}
