use vstd::prelude::*;
verus! {
fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

fn sum_to(n: u32) -> (s: u64)
    requires n < 1000,
    ensures s == (n as u64) * ((n as u64) + 1) / 2,
{
    let mut s: u64 = 0;
    let mut i: u32 = 0;
    while i < n
        invariant i <= n, n < 1000, s == (i as u64) * ((i as u64) + 1) / 2,
        decreases n - i,
    {
        i += 1;
        s += i as u64;
    }
    s
}

fn bad(x: u32) -> (r: u32)
    ensures r > x,
{ x }

fn main() {}
}
