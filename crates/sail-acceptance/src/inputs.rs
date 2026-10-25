//! Exact declared-input comparison and original observation guards.
//!
//! The same executable bodies run in Sail and in the direct stock-Verus gate.
//! Filesystem reads, digest production and compiler inventory discovery remain
//! explicit adapters; this module never accepts a caller-supplied Current flag.

#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    /// Strict lexicographic byte order, without hashing or normalization.
    pub open spec fn lex_at(a: Seq<u8>, b: Seq<u8>, i: int) -> bool {
        0 <= i <= a.len() && i <= b.len()
            && (forall|j: int| 0 <= j < i ==> a[j] == b[j])
            && (i == a.len() && i < b.len()
                || i < a.len() && i < b.len() && a[i] < b[i])
    }

    /// Lexicographic order has an explicit first-difference witness.
    pub open spec fn lex_less(a: Seq<u8>, b: Seq<u8>) -> bool {
        exists|i: int| lex_at(a, b, i)
    }

    /// One position agrees with its forward/inverse certificate.
    pub open spec fn permutation_at(forward: Seq<usize>, inverse: Seq<usize>, length: nat, i: int) -> bool {
        forward[i] < length && inverse[i] < length
            && inverse[forward[i] as int] == i
            && forward[inverse[i] as int] == i
    }

    /// A forward/inverse certificate explicitly accounts for every position.
    pub open spec fn valid_permutation(forward: Seq<usize>, inverse: Seq<usize>, length: nat) -> bool {
        forward.len() == length && inverse.len() == length
            && forall|i: int| 0 <= i < length ==> #[trigger] permutation_at(forward, inverse, length, i)
    }
}

sail_verified! {
    /// Compare every byte, proving exact sequence equality rather than a digest law.
    pub fn byte_eq(a: &[u8], b: &[u8]) -> (equal: bool)
        requires (true)
        ensures (equal == (a@ == b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0;
        sail_while! { (i < a.len())
            invariant (i <= a.len(), a.len() == b.len(),
                forall|j: int| 0 <= j < i ==> a[j] == b[j])
            decreases (a.len() - i)
            {
                if a[i] != b[i] { return false; }
                i += 1;
            }
        }
        sail_proof! { assert(a@ =~= b@); }
        true
    }
}

sail_verified! {
    /// Strict byte order used to check a caller's proposed ordering certificate.
    pub fn byte_lt(a: &[u8], b: &[u8]) -> (less: bool)
        requires (true)
        ensures (less == lex_less(a@, b@))
    {
        sail_proof! { reveal(lex_less); }
        let mut i = 0;
        sail_while! { (i < a.len() && i < b.len())
            invariant (i <= a.len(), i <= b.len(),
                forall|j: int| 0 <= j < i ==> a[j] == b[j])
            decreases (a.len() - i)
            {
                if a[i] < b[i] {
                    sail_proof! { assert(lex_at(a@, b@, i as int)); assert(lex_less(a@, b@)); }
                    return true;
                }
                if a[i] > b[i] {
                    sail_proof! { if lex_less(a@, b@) {
                        let k = choose|k: int| lex_at(a@, b@, k);
                        if k < i { assert(a[k] == b[k]); }
                        else if k == i {} else { assert(a[i as int] == b[i as int]); }
                    } }
                    return false;
                }
                i += 1;
            }
        }
        let less = a.len() < b.len();
        sail_proof! {
            if less {
                assert(i == a.len());
                assert(lex_at(a@, b@, i as int));
                assert(lex_less(a@, b@));
            } else if lex_less(a@, b@) {
                let k = choose|k: int| lex_at(a@, b@, k);
                assert(i == b.len());
                assert(k < i);
                assert(a[k] == b[k]);
            }
        }
        less
    }
}

sail_verified! {
    /// Validate complete bijection data in linear time, without trusting a sorter.
    pub fn permutation(forward: &[usize], inverse: &[usize], length: usize) -> (valid: bool)
        requires (true)
        ensures (valid == valid_permutation(forward@, inverse@, length as nat))
    {
        if forward.len() != length || inverse.len() != length { return false; }
        let mut i = 0;
        sail_while! { (i < length)
            invariant (i <= length, forward.len() == length, inverse.len() == length,
                forall|j: int| 0 <= j < i ==> #[trigger] permutation_at(forward@, inverse@, length as nat, j))
            decreases (length - i)
            {
                let f = forward[i];
                let r = inverse[i];
                if f >= length || r >= length { sail_proof! {
                    assert(!permutation_at(forward@, inverse@, length as nat, i as int));
                    if valid_permutation(forward@, inverse@, length as nat) { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                } return false; }
                if inverse[f] != i || forward[r] != i { sail_proof! {
                    assert(!permutation_at(forward@, inverse@, length as nat, i as int));
                    if valid_permutation(forward@, inverse@, length as nat) { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                } return false; }
                sail_proof! { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                i += 1;
            }
        }
        true
    }
}

sail_spec! {
    /// Strict byte ordering is transitive and therefore cannot hide duplicates.
    pub proof fn lex_less_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
        requires lex_less(a, b), lex_less(b, c),
        ensures lex_less(a, c),
    {
        reveal(lex_less);
        let i = choose|i: int| lex_at(a, b, i);
        let k = choose|k: int| lex_at(b, c, k);
        let n = if i < k { i } else { k };
        assert forall|j: int| 0 <= j < n implies a[j] == c[j] by {}
        if i < k {
            assert(b[i] == c[i]);
        } else if k < i {
            assert(a[k] == b[k]);
        }
        assert(0 <= n <= a.len() && n <= c.len());
        assert(n == a.len() && n < c.len()
            || n < a.len() && n < c.len() && a[n] < c[n]);
        assert(lex_at(a, c, n));
        assert(lex_less(a, c));
    }
}
