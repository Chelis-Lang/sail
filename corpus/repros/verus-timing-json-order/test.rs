use vstd::prelude::*;

verus! {

mod m3 { pub proof fn g2() { assert(true); } pub proof fn g1() { assert(true); } }
mod m1 { pub proof fn g2() { assert(true); } pub proof fn g1() { assert(true); } }
mod m2 { pub proof fn g2() { assert(true); } pub proof fn g1() { assert(true); } }

proof fn f8() { assert(true); }
proof fn f3() { assert(true); }
proof fn f6() { assert(true); }
proof fn f1() { assert(true); }
proof fn f7() { assert(true); }
proof fn f2() { assert(true); }
proof fn f5() { assert(true); }
proof fn f4() { assert(true); }

} // verus!
