use vstd::prelude::*;

verus! {

mod m1 {
    pub fn a() {}
    pub fn inserted() {}
}

mod m2 {
    pub fn b() -> (out: u8)
        ensures out == 3,
    {
        let f = |x: u8| -> (r: u8) ensures r == x { x };
        f(3)
    }
}

fn main() {}

} // verus!
