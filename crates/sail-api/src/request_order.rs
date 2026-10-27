//! Internal original-target ordering for checked request-set construction.
//!
//! The original Strings remain immutable and unnormalized. The comparator
//! follows their UTF-8 bytes, then the next tuple field, exactly as the native
//! String-tuple order. This module supplies no metadata or launch authority.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use std::cmp::Ordering;

use sail_acceptance::{
    sail_code, sail_inherited_method, sail_method, sail_proof, sail_spec, sail_verified, sail_while,
};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::std_specs::cmp::{
    OrdSpec, OrdSpecImpl, PartialEqSpec, PartialEqSpecImpl, PartialOrdSpec, PartialOrdSpecImpl,
};

sail_spec! {
    pub open spec fn equal_prefix(left: Seq<u8>, right: Seq<u8>, end: int) -> bool {
        0 <= end <= left.len() && end <= right.len()
            && forall |i: int| 0 <= i < end ==> left[i] == right[i]
    }

    pub open spec fn byte_less(left: Seq<u8>, right: Seq<u8>) -> bool {
        exists |i: int| 0 <= i <= left.len() && i < right.len()
            && equal_prefix(left, right, i)
            && (i == left.len() || left[i] < right[i])
    }

    pub open spec fn byte_order(left: Seq<u8>, right: Seq<u8>) -> Ordering {
        if byte_less(left, right) { Ordering::Less }
        else if left == right { Ordering::Equal }
        else { Ordering::Greater }
    }

    pub proof fn byte_less_strict(left: Seq<u8>, right: Seq<u8>)
        ensures byte_less(left, right) ==> left != right,
            !(byte_less(left, right) && byte_less(right, left)),
    {
        if byte_less(left, right) {
            let i = choose |i: int| 0 <= i <= left.len() && i < right.len()
                && equal_prefix(left, right, i) && (i == left.len() || left[i] < right[i]);
            if left == right { assert(i < left.len()); assert(left[i] == right[i]); }
            if byte_less(right, left) {
                let j = choose |j: int| 0 <= j <= right.len() && j < left.len()
                    && equal_prefix(right, left, j) && (j == right.len() || right[j] < left[j]);
                if i < j { assert(i < left.len()); assert(left[i] == right[i]); }
                else if j < i { assert(j < right.len()); assert(right[j] == left[j]); }
                else { assert(i < left.len() && i < right.len()); }
            }
        }
    }

    pub proof fn byte_less_transitive(first: Seq<u8>, second: Seq<u8>, third: Seq<u8>)
        requires byte_less(first, second), byte_less(second, third),
        ensures byte_less(first, third),
    {
        let i = choose |i: int| 0 <= i <= first.len() && i < second.len()
            && equal_prefix(first, second, i) && (i == first.len() || first[i] < second[i]);
        let j = choose |j: int| 0 <= j <= second.len() && j < third.len()
            && equal_prefix(second, third, j) && (j == second.len() || second[j] < third[j]);
        let k = if i < j { i } else { j };
        assert(equal_prefix(first, third, k)) by {
            assert forall |n: int| 0 <= n < k implies first[n] == third[n] by {
                assert(first[n] == second[n]);
                assert(second[n] == third[n]);
            }
        }
        if i < j {
            assert(k < third.len());
            if k < first.len() { assert(second[k] == third[k]); }
        } else if j < i {
            assert(k < first.len() && k < second.len());
            assert(first[k] == second[k]);
        } else if k < first.len() {
            assert(k < second.len());
        }
        assert(k <= first.len() && k < third.len());
        assert(k == first.len() || first[k] < third[k]);
        assert(byte_less(first, third));
    }

    pub proof fn byte_order_total_from(left: Seq<u8>, right: Seq<u8>, at: int)
        requires equal_prefix(left, right, at),
        ensures left == right || byte_less(left, right) || byte_less(right, left),
        decreases left.len() + right.len() - 2 * at,
    {
        if at == left.len() {
            if at == right.len() { assert(left =~= right); }
            else { assert(byte_less(left, right)); }
        } else if at == right.len() {
            assert(equal_prefix(right, left, at));
            assert(byte_less(right, left));
        } else if left[at] < right[at] {
            assert(byte_less(left, right));
        } else if right[at] < left[at] {
            assert(equal_prefix(right, left, at));
            assert(byte_less(right, left));
        } else {
            assert(left[at] == right[at]);
            assert(equal_prefix(left, right, at + 1));
            byte_order_total_from(left, right, at + 1);
        }
    }

    pub proof fn byte_order_laws(first: Seq<u8>, second: Seq<u8>, third: Seq<u8>)
        ensures
            (byte_order(first, second) == Ordering::Equal) == (first == second),
            (byte_order(first, second) == Ordering::Less)
                == (byte_order(second, first) == Ordering::Greater),
            (byte_order(first, second) == Ordering::Greater)
                == (byte_order(second, first) == Ordering::Less),
            byte_order(first, second) == Ordering::Less && byte_order(second, third) == Ordering::Less
                ==> byte_order(first, third) == Ordering::Less,
            byte_order(first, second) == Ordering::Greater && byte_order(second, third) == Ordering::Greater
                ==> byte_order(first, third) == Ordering::Greater,
    {
        byte_less_strict(first, second);
        byte_order_total_from(first, second, 0);
        if byte_order(first, second) == Ordering::Less && byte_order(second, third) == Ordering::Less {
            byte_less_transitive(first, second, third);
        }
        if byte_order(first, second) == Ordering::Greater && byte_order(second, third) == Ordering::Greater {
            byte_less_strict(second, third);
            byte_order_total_from(second, third, 0);
            byte_less_transitive(third, second, first);
            byte_less_strict(third, first);
        }
    }
}

sail_verified! {
    /// Compares the actual original byte slices without allocation or rewriting.
    pub fn compare_original_bytes(left: &[u8], right: &[u8]) -> (order: Ordering)
        requires(true)
        ensures(order == byte_order(left@, right@))
    {
        let mut i = 0usize;
        sail_while! { (i < left.len() && i < right.len())
            invariant(equal_prefix(left@, right@, i as int))
            decreases(left.len() - i)
            {
                if left[i] < right[i] {
                    sail_proof! { assert(byte_less(left@, right@)); }
                    return Ordering::Less;
                }
                if right[i] < left[i] {
                    sail_proof! {
                        assert(equal_prefix(right@, left@, i as int));
                        assert(byte_less(right@, left@));
                        byte_less_strict(right@, left@);
                    }
                    return Ordering::Greater;
                }
                i += 1;
            }
        }
        if left.len() < right.len() {
            sail_proof! { assert(i == left.len()); assert(byte_less(left@, right@)); }
            Ordering::Less
        } else if right.len() < left.len() {
            sail_proof! {
                assert(i == right.len());
                assert(equal_prefix(right@, left@, i as int));
                assert(byte_less(right@, left@));
                byte_less_strict(right@, left@);
            }
            Ordering::Greater
        } else {
            sail_proof! {
                assert(i == left.len()); assert(left@ =~= right@);
                byte_less_strict(left@, right@);
            }
            Ordering::Equal
        }
    }
}

sail_verified! {
    /// Equality of the actual original bytes, using the same checked comparator.
    pub fn original_bytes_equal(left: &[u8], right: &[u8]) -> (equal: bool)
        requires(true)
        ensures(equal == (left@ == right@))
    {
        let order = compare_original_bytes(left, right);
        sail_proof! { byte_order_laws(left@, right@, left@); }
        matches!(order, Ordering::Equal)
    }
}

sail_code! {
    /// Immutable original target tuple with its actual bytewise order. This is
    /// an internal dictionary key, not requested-scope or verification authority.
    #[derive(Debug)]
    pub struct OriginalTargetKey {
        original: (String, String, String),
    }

    impl OriginalTargetKey {
        sail_method! {
            /// Retains every original String without normalization or copying.
            pub fn new(original: (String, String, String)) -> (key: Self)
                requires(true) ensures(key_original(&key) == original,
                    key == original_key_value(original))
            { Self { original } }
        }
        sail_method! {
            /// Borrows the original immutable tuple used by request construction.
            pub fn as_tuple(&self) -> (original: &(String, String, String))
                requires(true) ensures(*original == key_original(self),
                    original_key_value(*original) == *self)
            { &self.original }
        }
    }
}

sail_spec! {
    pub closed spec fn key_original(key: &OriginalTargetKey) -> (String, String, String) {
        key.original
    }
    pub closed spec fn original_key_value(original: (String, String, String)) -> OriginalTargetKey {
        OriginalTargetKey { original }
    }
    pub open spec fn key_package(key: &OriginalTargetKey) -> Seq<u8> {
        vstd::utf8::encode_utf8(key_original(key).0@)
    }
    pub open spec fn key_target(key: &OriginalTargetKey) -> Seq<u8> {
        vstd::utf8::encode_utf8(key_original(key).1@)
    }
    pub open spec fn key_kind(key: &OriginalTargetKey) -> Seq<u8> {
        vstd::utf8::encode_utf8(key_original(key).2@)
    }
    pub open spec fn key_equal(left: &OriginalTargetKey, right: &OriginalTargetKey) -> bool {
        key_package(left) == key_package(right) && key_target(left) == key_target(right)
            && key_kind(left) == key_kind(right)
    }
    pub open spec fn key_order(left: &OriginalTargetKey, right: &OriginalTargetKey) -> Ordering {
        let package = byte_order(key_package(left), key_package(right));
        if package != Ordering::Equal { package }
        else {
            let target = byte_order(key_target(left), key_target(right));
            if target != Ordering::Equal { target }
            else { byte_order(key_kind(left), key_kind(right)) }
        }
    }

    impl PartialEqSpecImpl for OriginalTargetKey {
        open spec fn obeys_eq_spec() -> bool { true }
        open spec fn eq_spec(&self, other: &Self) -> bool { key_equal(self, other) }
    }
    impl PartialOrdSpecImpl for OriginalTargetKey {
        open spec fn obeys_partial_cmp_spec() -> bool { true }
        open spec fn partial_cmp_spec(&self, other: &Self) -> Option<Ordering> { Some(key_order(self, other)) }
    }
    impl OrdSpecImpl for OriginalTargetKey {
        open spec fn obeys_cmp_spec() -> bool { true }
        open spec fn cmp_spec(&self, other: &Self) -> Ordering { key_order(self, other) }
    }
}

sail_code! {
    impl PartialEq for OriginalTargetKey {
        sail_inherited_method! {
        fn eq(&self, other: &Self) -> (equal: bool)
            ensures(equal == key_equal(self, other))
        {
            original_bytes_equal(self.original.0.as_str().as_bytes(), other.original.0.as_str().as_bytes())
                && original_bytes_equal(self.original.1.as_str().as_bytes(), other.original.1.as_str().as_bytes())
                && original_bytes_equal(self.original.2.as_str().as_bytes(), other.original.2.as_str().as_bytes())
        }
        }
    }
    impl Eq for OriginalTargetKey {}
    impl PartialOrd for OriginalTargetKey {
        sail_inherited_method! {
        fn partial_cmp(&self, other: &Self) -> (order: Option<Ordering>)
            ensures(order == Some(key_order(self, other)))
        { Some(self.cmp(other)) }
        }
    }
    impl Ord for OriginalTargetKey {
        sail_inherited_method! {
        fn cmp(&self, other: &Self) -> (order: Ordering)
            ensures(order == key_order(self, other))
        {
            let package = compare_original_bytes(self.original.0.as_str().as_bytes(), other.original.0.as_str().as_bytes());
            if !matches!(package, Ordering::Equal) { return package; }
            let target = compare_original_bytes(self.original.1.as_str().as_bytes(), other.original.1.as_str().as_bytes());
            if !matches!(target, Ordering::Equal) { return target; }
            compare_original_bytes(self.original.2.as_str().as_bytes(), other.original.2.as_str().as_bytes())
        }
        }
    }
    impl Clone for OriginalTargetKey {
        sail_inherited_method! {
        fn clone(&self) -> (key: Self)
            ensures(key_original(&key) == key_original(self), key == *self)
        {
            Self { original: (self.original.0.clone(), self.original.1.clone(), self.original.2.clone()) }
        }
        }
    }
}

sail_spec! {
    pub proof fn key_order_laws(first: &OriginalTargetKey, second: &OriginalTargetKey, third: &OriginalTargetKey)
        ensures
            (key_order(first, second) == Ordering::Equal) == key_equal(first, second),
            (key_order(first, second) == Ordering::Less) == (key_order(second, first) == Ordering::Greater),
            (key_order(first, second) == Ordering::Greater) == (key_order(second, first) == Ordering::Less),
            key_order(first, second) == Ordering::Less && key_order(second, third) == Ordering::Less
                ==> key_order(first, third) == Ordering::Less,
            key_order(first, second) == Ordering::Greater && key_order(second, third) == Ordering::Greater
                ==> key_order(first, third) == Ordering::Greater,
    {
        byte_order_laws(key_package(first), key_package(second), key_package(third));
        byte_order_laws(key_package(second), key_package(third), key_package(first));
        byte_order_laws(key_package(first), key_package(third), key_package(second));
        byte_order_laws(key_target(first), key_target(second), key_target(third));
        byte_order_laws(key_target(second), key_target(third), key_target(first));
        byte_order_laws(key_target(first), key_target(third), key_target(second));
        byte_order_laws(key_kind(first), key_kind(second), key_kind(third));
        byte_order_laws(key_kind(second), key_kind(third), key_kind(first));
        byte_order_laws(key_kind(first), key_kind(third), key_kind(second));
    }

    pub proof fn original_target_key_order_is_lawful()
        ensures vstd::laws_cmp::obeys_cmp::<OriginalTargetKey>(),
    {
        reveal(vstd::laws_eq::obeys_eq_spec_properties);
        reveal(vstd::laws_cmp::obeys_cmp_partial_ord);
        reveal(vstd::laws_cmp::obeys_cmp_ord);
        reveal(vstd::laws_cmp::obeys_partial_cmp_spec_properties);
        assert forall |left: OriginalTargetKey, right: OriginalTargetKey|
            (key_order(&left, &right) == Ordering::Equal) == key_equal(&left, &right) by {
            key_order_laws(&left, &right, &left);
        }
        assert forall |left: OriginalTargetKey, right: OriginalTargetKey|
            (key_order(&left, &right) == Ordering::Less) == (key_order(&right, &left) == Ordering::Greater) by {
            key_order_laws(&left, &right, &left);
        }
        assert forall |first: OriginalTargetKey, second: OriginalTargetKey, third: OriginalTargetKey|
            key_order(&first, &second) == Ordering::Less && key_order(&second, &third) == Ordering::Less
                implies key_order(&first, &third) == Ordering::Less by {
            key_order_laws(&first, &second, &third);
        }
        assert forall |first: OriginalTargetKey, second: OriginalTargetKey, third: OriginalTargetKey|
            key_order(&first, &second) == Ordering::Greater && key_order(&second, &third) == Ordering::Greater
                implies key_order(&first, &third) == Ordering::Greater by {
            key_order_laws(&first, &second, &third);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OriginalTargetKey;
    use std::collections::BTreeSet;

    #[test]
    fn byte_key_order_matches_the_original_native_tuple_order() {
        let tuples: Vec<(String, String, String)> = vec![
            ("".into(), "".into(), "".into()),
            ("pkg".into(), "target".into(), "lib".into()),
            ("pkg".into(), "target".into(), "bin".into()),
            ("pkg".into(), "t".into(), "lib".into()),
            ("pkg".into(), "target\0".into(), "lib".into()),
            ("pkg".into(), "target".into(), "lib\n".into()),
            ("λ".into(), "target".into(), "lib".into()),
            ("pkg".into(), "λ".into(), "lib".into()),
            ("pkg".into(), "target".into(), "lib".into()),
        ];
        for left in &tuples {
            for right in &tuples {
                let actual_left = OriginalTargetKey::new(left.clone());
                let actual_right = OriginalTargetKey::new(right.clone());
                assert_eq!(actual_left.cmp(&actual_right), left.cmp(right));
                assert_eq!(
                    actual_left.partial_cmp(&actual_right),
                    left.partial_cmp(right)
                );
                assert_eq!(actual_left == actual_right, left == right);
                assert_eq!(actual_left.clone().as_tuple(), left);
            }
        }
        let original: BTreeSet<_> = tuples.iter().cloned().collect();
        let actual: BTreeSet<_> = tuples.into_iter().map(OriginalTargetKey::new).collect();
        assert_eq!(actual.len(), original.len());
        let projection: Vec<_> = actual.iter().map(OriginalTargetKey::as_tuple).collect();
        assert_eq!(projection, original.iter().collect::<Vec<_>>());
    }
}
