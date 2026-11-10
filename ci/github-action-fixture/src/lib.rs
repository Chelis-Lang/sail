use vstd::prelude::*;

verus! {

// The GitHub Action's hosted test (.github/workflows/ci.yml, `action-*`
// jobs) verifies this crate, then breaks `action_fixture_lemma` to check
// that the action keeps `sail verify`'s failing exit code.
pub fn bounded_increment(x: u64) -> (result: u64)
    requires x < u64::MAX,
    ensures result == x + 1,
{
    x + 1
}

pub proof fn action_fixture_lemma(x: nat)
    ensures x + 1 > x,
{
}

}
