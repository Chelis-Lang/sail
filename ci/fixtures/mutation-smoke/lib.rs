use vstd::prelude::*;

verus! {

pub open spec fn triangle(n: nat) -> nat
    decreases n,
{
    if n == 0 { 0 } else { n + triangle((n - 1) as nat) }
}

pub fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{
    if a > b { a } else { b }
}

pub fn clamp(x: u32, lo: u32, hi: u32) -> (r: u32)
    requires lo <= hi,
    ensures lo <= r <= hi, lo <= x && x <= hi ==> r == x,
{
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

pub fn sum_to(n: u32) -> (s: u64)
    requires n < 1000,
    ensures s == triangle(n as nat),
{
    let mut s: u64 = 0;
    let mut i: u32 = 0;
    while i < n
        invariant
            i <= n,
            n < 1000,
            s == triangle(i as nat),
            s <= 1000 * (i as u64),
        decreases n - i,
    {
        i = i + 1;
        s = s + i as u64;
    }
    s
}

} // verus!
