// Fixture crate for the mutation catalogue generator's golden test
// (`sail_verus::mutate`). It is never compiled; it only has to lex.
use vstd::prelude::*;

mod spec_fns;

verus! {

pub fn clamp(a: u64, hi: u64) -> (r: u64)
    requires
        hi > 0,
    ensures
        r <= hi,
        match a <= hi {
            true => r == a,
            false => r == hi,
        },
{
    if a > hi {
        hi
    } else {
        a
    }
}

fn count_up(n: u64) -> (r: u64)
    requires
        n < 1000,
    ensures
        r == n,
{
    let mut i: u64 = 0;
    while i < n
        invariant
            i <= n,
        decreases n - i,
    {
        proof {
            assert(i + 1 <= n);
        }
        i = i + 1;
    }
    i
}

fn sum_first(v: &Vec<u64>, k: usize) -> (r: u64)
    requires
        k <= v.len(),
        forall|j: int| 0 <= j < v.len() ==> v[j] < 100,
    ensures
        r <= 100 * k,
{
    let mut s: u64 = 0;
    let mut j: usize = 0;
    while j < k
        invariant
            j <= k,
            k <= v.len(),
            s <= 100 * j,
            forall|t: int| 0 <= t < v.len() ==> v[t] < 100,
        decreases k - j,
    {
        s = s + v[j];
        j = j + 1;
    }
    s
}

fn is_small(x: u32) -> (b: bool)
    ensures
        b == (x < 10),
{
    x < 10 && !(x >= 10)
}

} // verus!
