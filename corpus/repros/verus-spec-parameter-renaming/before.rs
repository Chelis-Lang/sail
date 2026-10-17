use vstd::prelude::*;
verus! {
mod defs {
    use vstd::prelude::*;
    pub open spec fn distance(left: int, unused: int, right: int) -> int {
        left - right
    }
}
mod client {
    use vstd::prelude::*;
    proof fn check(x: int, y: int) {
        assert(crate::defs::distance(x, 91, y) == x - y);
    }
}
}
fn main() {}
