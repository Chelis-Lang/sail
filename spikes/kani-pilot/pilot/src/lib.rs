//! Kani pilot harnesses over the pinned `arrayvec` 0.7.8 (issue 135 spike).
//!
//! The pinned crate's source is untouched: harnesses live in this separate
//! crate. Every harness states an explicit unwind bound. `CAP` is the
//! declared data bound: every claim below is "for all vectors of capacity 4
//! holding `u8`", nothing wider.

#![cfg_attr(not(kani), allow(unused))]

#[cfg(kani)]
mod harnesses {
    use arrayvec::{ArrayString, ArrayVec};

    pub const CAP: usize = 4;
    pub type V = ArrayVec<u8, CAP>;

    /// Any vector of capacity `CAP`: any length, any contents.
    pub fn any_vec() -> V {
        let mut v = V::new();
        let n: usize = kani::any();
        kani::assume(n <= CAP);
        for _ in 0..n {
            v.push(kani::any());
        }
        v
    }

    // ---- Expected to pass -------------------------------------------------

    /// P1. On a non-full vector, `push` then `pop` returns the element and
    /// restores the length.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p1_push_pop_roundtrip() {
        let mut v = any_vec();
        kani::assume(!v.is_full());
        let len = v.len();
        let x: u8 = kani::any();
        v.push(x);
        assert_eq!(v.len(), len + 1);
        assert_eq!(v[len], x);
        assert_eq!(v.pop(), Some(x));
        assert_eq!(v.len(), len);
    }

    /// P2. `try_push` on a full vector returns the element and changes
    /// nothing. Also the kill target for the seeded off-by-one.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p2_try_push_full_is_err() {
        let a: [u8; CAP] = kani::any();
        let mut v = V::from(a);
        let x: u8 = kani::any();
        match v.try_push(x) {
            Err(e) => assert_eq!(e.element(), x),
            Ok(()) => panic!("try_push accepted an element beyond capacity"),
        }
        assert_eq!(v.len(), CAP);
        assert!(v.as_slice() == &a[..]);
    }

    /// P3. `insert(i, x)` then `remove(i)` is the identity, for every valid
    /// index.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p3_insert_remove_inverse() {
        let mut v = any_vec();
        kani::assume(!v.is_full());
        let before = v.clone();
        let i: usize = kani::any();
        kani::assume(i <= v.len());
        let x: u8 = kani::any();
        v.insert(i, x);
        assert_eq!(v.len(), before.len() + 1);
        assert_eq!(v[i], x);
        assert_eq!(v.remove(i), x);
        assert!(v.as_slice() == before.as_slice());
    }

    /// P4. `truncate(n)` for every `usize` keeps exactly the first
    /// `min(n, len)` elements.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p4_truncate_keeps_prefix() {
        let mut v = any_vec();
        let before = v.clone();
        let n: usize = kani::any();
        v.truncate(n);
        let expect = if n < before.len() { n } else { before.len() };
        assert_eq!(v.len(), expect);
        assert!(v.as_slice() == &before.as_slice()[..expect]);
    }

    /// P5. `retain` keeps only elements satisfying the predicate and never
    /// grows the vector.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p5_retain_filters() {
        let mut v = any_vec();
        let old_len = v.len();
        v.retain(|x| *x & 1 == 0);
        assert!(v.len() <= old_len);
        for x in v.iter() {
            assert!(*x & 1 == 0);
        }
    }

    /// P6 (cost probe, beyond the five). Any `char` fits a 4-byte
    /// `ArrayString` and occupies its UTF-8 length.
    #[kani::proof]
    #[kani::unwind(6)]
    fn p6_array_string_push_char() {
        let c: char = kani::any();
        let mut s = ArrayString::<4>::new();
        assert!(s.try_push(c).is_ok());
        assert_eq!(s.len(), c.len_utf8());
    }

    // ---- Expected to fail (no `should_panic`: the raw verdict is wanted) ---

    /// F1. Genuine documented panic: `push` on a full vector.
    #[kani::proof]
    #[kani::unwind(6)]
    fn f1_push_never_panics() {
        let mut v = any_vec();
        v.push(kani::any());
    }

    /// F2. Genuine panic path: `drain` with an arbitrary range reaches the
    /// slice bounds check.
    #[kani::proof]
    #[kani::unwind(6)]
    fn f2_drain_any_range_never_panics() {
        let mut v = any_vec();
        let a: usize = kani::any();
        let b: usize = kani::any();
        let d = v.drain(a..b);
        drop(d);
    }

    /// F3 is `p2_try_push_full_is_err` run against the seeded copy of
    /// arrayvec (`patches/seeded-try-push-off-by-one.diff`).

    /// E1 (bound probe). The passing P5 property under an unwind bound that
    /// is too small: the expected result is an unwinding-assertion failure,
    /// which is "bound insufficient", not a refutation of the property.
    #[kani::proof]
    #[kani::unwind(2)]
    fn e1_retain_unwind_too_small() {
        let mut v = any_vec();
        let old_len = v.len();
        v.retain(|x| *x & 1 == 0);
        assert!(v.len() <= old_len);
    }
}

/// Function-contract experiments (`-Z function-contracts`).
#[cfg(all(kani, feature = "contracts"))]
mod contracts {
    use super::harnesses::{any_vec, CAP, V};

    /// C1. A local wrapper over the foreign `ArrayVec::push`, with a
    /// precondition, a frame and a postcondition over a `&mut` argument.
    #[kani::requires(!v.is_full())]
    #[kani::modifies(v)]
    #[kani::ensures(|_| v.len() == old(v.len()) + 1 && v[old(v.len())] == x)]
    pub fn push_checked(v: &mut V, x: u8) {
        v.push(x);
    }

    #[kani::proof_for_contract(push_checked)]
    #[kani::unwind(6)]
    fn c1_check_push_checked() {
        let mut v = any_vec();
        let x: u8 = kani::any();
        push_checked(&mut v, x);
    }

    /// C2. A pure wrapper over `remaining_capacity`, then used as a verified
    /// stub in a caller's proof.
    // Written overflow-free on purpose: as a verified stub the result is
    // any `usize` and this expression is evaluated to assume it, so the
    // natural `*r + v.len() == CAP` fails with "attempt to add with
    // overflow" in the caller's proof (observed, Kani 0.68.0).
    #[kani::ensures(|r| *r <= CAP && *r + v.len() == CAP)]
    pub fn remaining_checked(v: &V) -> usize {
        v.remaining_capacity()
    }

    #[kani::proof_for_contract(remaining_checked)]
    #[kani::unwind(6)]
    fn c2_check_remaining_checked() {
        let v = any_vec();
        let _ = remaining_checked(&v);
    }

    /// Would `n` more elements fit? Proved against C2's contract only.
    fn fits(v: &V, n: usize) -> bool {
        remaining_checked(v) >= n
    }

    #[kani::proof]
    #[kani::stub_verified(remaining_checked)]
    #[kani::unwind(6)]
    fn c2_caller_uses_contract_as_stub() {
        let v = any_vec();
        let n: usize = kani::any();
        kani::assume(n <= CAP);
        if fits(&v, n) {
            assert!(v.len() + n <= CAP);
        }
    }

    /// C2x. The same contract as first written, kept as evidence: it proves,
    /// and then fails at its use site with "attempt to add with overflow".
    #[kani::ensures(|r| *r + v.len() == CAP)]
    pub fn remaining_unguarded(v: &V) -> usize {
        v.remaining_capacity()
    }

    #[kani::proof_for_contract(remaining_unguarded)]
    #[kani::unwind(6)]
    fn c2x_check_remaining_unguarded() {
        let v = any_vec();
        let _ = remaining_unguarded(&v);
    }

    #[kani::proof]
    #[kani::stub_verified(remaining_unguarded)]
    #[kani::unwind(6)]
    fn c2x_caller_fails_inside_postcondition() {
        let v = any_vec();
        let _ = remaining_unguarded(&v);
    }

    /// C1 as a verified stub: needs to havoc `*v`, a foreign type with no
    /// `kani::Arbitrary` implementation.
    #[cfg(feature = "contract-stub-foreign")]
    #[kani::proof]
    #[kani::stub_verified(push_checked)]
    #[kani::unwind(6)]
    fn c1_caller_uses_contract_as_stub() {
        let mut v = any_vec();
        kani::assume(!v.is_full());
        let len = v.len();
        push_checked(&mut v, 7);
        assert_eq!(v.len(), len + 1);
    }

    /// C3. A contract written on the dependency's own unsafe method (in the
    /// patched copy, `patches/contract-in-crate.diff`), checked from here.
    #[cfg(feature = "contract-in-crate")]
    #[kani::proof_for_contract(arrayvec::ArrayVec::<u8, 4>::push_unchecked)]
    #[kani::unwind(6)]
    fn c3_check_in_crate_push_unchecked() {
        let mut v = any_vec();
        let x: u8 = kani::any();
        unsafe { v.push_unchecked(x) };
    }
}
