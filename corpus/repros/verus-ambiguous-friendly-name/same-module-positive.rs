use vstd::prelude::*;
verus! {
trait Tag { fn tag(&self) -> (r: u8); }
impl Tag for Vec<u8> {
    fn tag(&self) -> (r: u8) ensures r == 1 { 1 }
}
impl Tag for Vec<u16> {
    fn tag(&self) -> (r: u8) ensures r == 2 { 2 }
}
proof fn unrelated() { assert(true); }
}
