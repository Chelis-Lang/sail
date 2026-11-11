use vstd::prelude::*;
verus! {
proof fn f() {
    assert(ext_equal(1int, true));
}
}
fn main() {}
