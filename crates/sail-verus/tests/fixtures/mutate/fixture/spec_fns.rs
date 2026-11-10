use vstd::prelude::*;

verus! {

pub open spec(checked) fn bounded(s: Seq<u64>, n: nat) -> bool {
    &&& s.len() >= 1
    &&& forall|i: int| 0 <= i < s.len() ==> s[i] <= n
}

proof fn bounded_empty_prefix(s: Seq<u64>, n: nat)
    requires
        bounded(s, n),
    ensures
        s.len() > 0,
        s[0] <= n + 1,
{
    assert(s[0] <= n);
}

} // verus!
