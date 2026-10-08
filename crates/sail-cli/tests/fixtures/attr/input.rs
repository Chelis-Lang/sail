use vstd::prelude::*;
verus! {

mod arith {
    use vstd::prelude::*;

    pub proof fn mul_pos(a: int, b: int)
        requires a > 0, b > 0,
        ensures a * b > 0,
    {
        assert(a * b > 0) by (nonlinear_arith)
            requires a > 0, b > 0;
    }

    pub proof fn bv_and(x: u32)
        ensures x & 0 == 0,
    {
        assert(x & 0 == 0) by (bit_vector);
    }

    pub proof fn bv_bad(x: u32)
    {
        assert(x & 1 == 0) by (bit_vector);
    }
}

pub struct Counter { pub n: u64 }

impl Counter {
    pub fn incr(&mut self)
        requires old(self).n < 100,
        ensures final(self).n == old(self).n + 1,
    {
        self.n = self.n + 1;
    }

    pub fn get(&self) -> (r: u64)
        ensures r == self.n,
    { self.n }
}

fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

fn bad(x: u32) -> (r: u32)
    ensures r > x,
{ x }

fn main() {}
}
