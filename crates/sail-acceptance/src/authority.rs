//! Exact identity helpers for source-shared positive-result authority.
//!
//! Report validation uses the actual API report structures, so no copied
//! `report_valid` flag stands between their fields and the checked predicate.

/// Exact byte comparison shared with input validation.
pub use super::inputs::byte_eq as exact_bytes;
use super::inputs::{byte_lt, permutation};
#[cfg(sail_verus)]
use super::inputs::{lex_less, lex_less_transitive, permutation_at, valid_permutation};

#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Distinct policies preserve the receipt and repair contracts.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CompletionPolicy {
        /// Accept a successful exact-scope receipt, including absent whole-crate `success`.
        Receipt,
        /// Require an explicit successful whole-crate terminal summary.
        Whole,
        /// Require full whole-crate success and reject independent engine fallback.
        Repair,
    }

    /// One independently resolved requested Cargo target, retaining exact bytes.
    #[derive(Debug, PartialEq, Eq)]
    pub struct RequestedTarget {
        /// Exact Cargo package ID.
        pub package: Vec<u8>,
        /// Exact Cargo target name; never its normalized Rust crate name.
        pub name: Vec<u8>,
        /// Exact selected Cargo target kind.
        pub kind: Vec<u8>,
    }
}

sail_verified! {
    /// Copies an exact byte slice with a checked, allocation-only body.
    pub fn copy_bytes(bytes: &[u8]) -> (out: Vec<u8>)
        requires(true)
        ensures(out@ == bytes@)
    {
        let mut out = Vec::new();
        let mut i = 0usize;
        sail_while! { (i < bytes.len())
            invariant(i <= bytes.len(), out@ == bytes@.take(i as int))
            decreases(bytes.len() - i)
            {
                out.push(bytes[i]);
                i += 1;
            }
        }
        out
    }
}

sail_spec! {
    pub open spec fn flag_matches(bytes: Seq<u8>, flag: Seq<u8>) -> bool {
        bytes.len() >= flag.len() && bytes.take(flag.len() as int) == flag
            && (bytes.len() == flag.len() || bytes[flag.len() as int] == 61u8)
    }

    pub open spec fn distinct_id_values(ids: Seq<Vec<u8>>) -> bool {
        forall |i: int, j: int| 0 <= i < j < ids.len() ==> ids[i]@ != ids[j]@
    }

    pub proof fn strict_order_differs(a: Seq<u8>, b: Seq<u8>)
        requires lex_less(a, b),
        ensures a != b,
    {
        reveal(lex_less);
        let i = choose |i: int| super::inputs::lex_at(a, b, i);
        if a == b { assert(a.len() == b.len()); }
    }

    pub open spec fn owned_id(name: &[u8], id: &[u8]) -> bool {
        id@.len() > name@.len() && id@[name@.len() as int] == 35u8
            && forall |i: int| 0 <= i < name@.len() ==> id@[i] == name@[i]
    }
}

sail_verified! {
    /// Matches an exact flag or its `flag=value` spelling, never a prefix alias.
    pub fn argument_flag(bytes: &[u8], flag: &[u8]) -> (out: bool)
        requires(true)
        ensures(out == flag_matches(bytes@, flag@))
    {
        if bytes.len() < flag.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < flag.len())
            invariant(i <= flag.len(), flag.len() <= bytes.len(),
                forall |j: int| 0 <= j < i ==> bytes@[j] == flag@[j])
            decreases(flag.len() - i)
            {
                if bytes[i] != flag[i] { return false; }
                i += 1;
            }
        }
        sail_proof! { assert(bytes@.take(flag@.len() as int) =~= flag@); }
        bytes.len() == flag.len() || bytes[flag.len()] == b'='
    }
}

sail_verified! {
    /// Checks an arbitrary sorting candidate in linear time, including coverage.
    pub fn unique_obligations(ids: &[Vec<u8>], forward: &[usize], inverse: &[usize]) -> (out: bool)
        requires(true)
        ensures(out ==> distinct_id_values(ids@),
            ids@.len() == 0 && forward@.len() == 0 && inverse@.len() == 0 ==> out)
    {
        if !permutation(forward, inverse, ids.len()) { return false; }
        sail_proof! {
            assert forall |k: int| 0 <= k < forward@.len() implies forward@[k] < ids@.len() by {
                assert(permutation_at(forward@, inverse@, ids@.len(), k));
            }
        }
        let mut i = 0usize;
        sail_while! { (i < forward.len())
            invariant(i <= forward.len(), valid_permutation(forward@, inverse@, ids@.len()),
                forall |k: int| 0 <= k < forward@.len() ==> forward@[k] < ids@.len(),
                forall |j: int, k: int| 0 <= j < k < i ==>
                    lex_less(ids@[forward@[j] as int]@, ids@[forward@[k] as int]@))
            decreases(forward.len() - i)
            {
                if i > 0 {
                    if !byte_lt(&ids[forward[i - 1]], &ids[forward[i]]) { return false; }
                    sail_proof! {
                        assert forall |j: int| 0 <= j < i implies
                            lex_less(ids@[forward@[j] as int]@, ids@[forward@[i as int] as int]@) by {
                            if j < i - 1 {
                                lex_less_transitive(ids@[forward@[j] as int]@,
                                    ids@[forward@[i as int - 1] as int]@,
                                    ids@[forward@[i as int] as int]@);
                            }
                        }
                    }
                }
                i += 1;
            }
        }
        sail_proof! {
            assert forall |a: int, b: int| 0 <= a < b < ids@.len() implies ids@[a]@ != ids@[b]@ by {
                assert(permutation_at(forward@, inverse@, ids@.len(), a));
                assert(permutation_at(forward@, inverse@, ids@.len(), b));
                let p = inverse@[a] as int;
                let q = inverse@[b] as int;
                if p < q { strict_order_differs(ids@[a]@, ids@[b]@); }
                else if q < p { strict_order_differs(ids@[b]@, ids@[a]@); }
            }
        }
        true
    }
}

sail_verified! {
    /// Checks the exact `<function>#...` ownership boundary.
    pub fn owned_obligation(name: &[u8], id: &[u8]) -> (out: bool)
        requires(true)
        ensures(out == owned_id(name, id))
    {
        if id.len() <= name.len() { return false; }
        if id[name.len()] != b'#' { return false; }
        let mut i = 0usize;
        sail_while! { (i < name.len())
            invariant(i <= name.len(), name.len() < id.len(),
                forall |j: int| 0 <= j < i ==> name@[j] == id@[j])
            decreases(name.len() - i)
            {
                if name[i] != id[i] { return false; }
                i += 1;
            }
        }
        true
    }
}
