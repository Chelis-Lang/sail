use vstd::prelude::*;
verus! {
mod defs {
    pub open spec fn distance(left: int, unused: int, right: int) -> int {
        left - right
    }
}
mod client {
    proof fn check(x: int, y: int) {
        assert(crate::defs::distance(x, 91, y) == x - y);
    }
}
}
fn main() {}
