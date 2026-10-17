use vstd::prelude::*;
verus! {
mod defs {
    use vstd::prelude::*;
    pub open spec fn distance(renamed_left: int, unused: int, right: int) -> int {
        renamed_left - right
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
