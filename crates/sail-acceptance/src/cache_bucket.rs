//! Owned complete-key bucket operations for an unselected map representation.
//!
//! The actual stored entries are moved unchanged. A primitive route selects a
//! bucket, while every identity decision compares all original 32 key bytes.
//! This staged leaf is not yet the native tier's selected storage or authority.
#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::cache_candidate::{Entry, Slot};
use crate::inputs::byte_eq;

sail_code! {
    /// Owns the actual complete-key entries of one primitive route.
    #[derive(Debug)]
    pub struct Bucket<Value, Stamp> {
        route: u128,
        entries: Vec<Entry<Value, Stamp>>,
    }
}

sail_spec! {
    /// Exact little-endian packing of the first n bytes. No injectivity claim.
    pub open spec fn packed_prefix(key: Seq<u8>, n: nat) -> u128
        recommends n <= 16, key.len() == 32
        decreases n,
    {
        if n == 0 { 0u128 } else {
            packed_prefix(key, (n - 1) as nat)
                | ((key[n as int - 1] as u128) << ((n - 1) * 8))
        }
    }

    /// Every complete key belongs to this route; no two complete keys repeat.
    pub open spec fn bucket_valid<Value, Stamp>(route: u128,
        entries: Seq<Entry<Value, Stamp>>) -> bool {
        (forall|i: int| 0 <= i < entries.len() ==>
            packed_prefix(entries[i].0@, 16) == route)
        && (forall|i: int, j: int| 0 <= i < j < entries.len() ==>
            entries[i].0@ != entries[j].0@)
    }

    /// Equal route bytes produce equal routes even when complete keys differ.
    pub proof fn equal_prefix_route(first: Seq<u8>, second: Seq<u8>, n: nat)
        requires first.len() == 32, second.len() == 32, n <= 16,
            forall|i: int| 0 <= i < n ==> first[i] == second[i],
        ensures packed_prefix(first, n) == packed_prefix(second, n),
        decreases n,
    {
        if n > 0 { equal_prefix_route(first, second, (n - 1) as nat); }
    }
}

sail_verified! {
    /// Route only the original first16 bytes; full keys remain stored/compared.
    pub fn route(key: &[u8; 32]) -> (out: u128)
        requires (true)
        ensures (out == packed_prefix(key@, 16))
    {
        let mut out = 0u128;
        let mut i = 0usize;
        sail_while! { (i < 16) invariant (i <= 16,
            out == packed_prefix(key@, i as nat)) decreases (16 - i) {
            out |= u128::from(key[i]) << (i * 8);
            i += 1;
        } }
        out
    }
}

sail_verified! {
    /// Find the first exact complete key, retaining a complete absence witness.
    pub fn find<Value, Stamp>(entries: &[Entry<Value, Stamp>], key: &[u8; 32])
        -> (out: Option<usize>)
        requires (true)
        ensures (match out {
            Some(i) => i < entries@.len() && entries@[i as int].0@ == key@
                && forall|j: int| 0 <= j < i ==> entries@[j].0@ != key@,
            None => forall|j: int| 0 <= j < entries@.len() ==> entries@[j].0@ != key@,
        })
    {
        let mut i = 0usize;
        sail_while! { (i < entries.len()) invariant (i <= entries.len(),
            forall|j: int| 0 <= j < i ==> entries@[j].0@ != key@)
            decreases (entries.len() - i) {
            if byte_eq(&entries[i].0, key) { return Some(i); }
            i += 1;
        } }
        None
    }
}

sail_code! {
    impl<Value, Stamp> Bucket<Value, Stamp> {
        sail_impl_spec! {
            /// Actual route used by the outer primitive map.
            pub closed spec fn route_view(&self) -> u128 { self.route }
            /// Actual original entries, without a copied runtime ledger.
            pub closed spec fn entries_view(&self) -> Seq<Entry<Value, Stamp>> { self.entries@ }
            /// Placement and complete-key uniqueness of actual storage.
            pub open spec fn valid(&self) -> bool {
                bucket_valid(self.route_view(), self.entries_view())
            }
            /// Membership means a complete32-byte match in actual storage.
            pub open spec fn contains_view(&self, key: Seq<u8>) -> bool {
                exists|i: int| 0 <= i < self.entries_view().len()
                    && self.entries_view()[i].0@ == key
            }
            /// Expose a concrete complete-key membership witness to callers.
            pub proof fn member_at(&self, index: int)
                requires 0 <= index < self.entries_view().len(),
                ensures self.contains_view(self.entries_view()[index].0@),
            {
                assert(exists|i: int| 0 <= i < self.entries_view().len()
                    && self.entries_view()[i].0@ == self.entries_view()[index].0@) by {
                    assert(0 <= index < self.entries_view().len());
                }
            }
        }

        sail_method! {
            /// Start an actually empty bucket at its original primitive route.
            pub fn empty(route: u128) -> (out: Self)
                requires (true)
                ensures (out.valid(), out.route_view() == route,
                    out.entries_view() == Seq::<Entry<Value, Stamp>>::empty())
            { Self { route, entries: Vec::new() } }
        }

        sail_method! {
            /// Exact full-key read; a route collision alone never returns a slot.
            #[allow(clippy::question_mark, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn get<'a>(&'a self, key: &[u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
                requires (self.valid())
                ensures (out.is_some() == self.contains_view(key@), match out {
                    Some(slot) => exists|i: int| 0 <= i < self.entries_view().len()
                        && self.entries_view()[i].0@ == key@
                        && *slot == self.entries_view()[i].1,
                    None => forall|i: int| 0 <= i < self.entries_view().len() ==>
                        self.entries_view()[i].0@ != key@,
                })
            {
                let Some(i) = find(&self.entries, key) else { return None; };
                Some(&self.entries[i].1)
            }
        }

        sail_method! {
            /// Replace only the exact full key, or append a new complete key.
            /// A caller cannot install an entry in a route it does not belong to.
            /// # Errors
            /// Returns the same original entry if its complete key has a different route.
            pub fn insert(&mut self, entry: Entry<Value, Stamp>)
                -> (out: Result<Option<Entry<Value, Stamp>>, Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    final(self).route_view() == old(self).route_view(),
                    out.is_ok() == (packed_prefix(entry.0@, 16) == old(self).route_view()),
                    out.is_ok() ==> final(self).contains_view(entry.0@),
                    match out {
                        Err(original) => original == entry
                            && final(self).entries_view() == old(self).entries_view()
                            && packed_prefix(entry.0@, 16) != old(self).route_view(),
                        Ok(Some(original)) => exists|i: int| 0 <= i < old(self).entries_view().len()
                            && original == old(self).entries_view()[i]
                            && original.0@ == entry.0@
                            && final(self).entries_view() == old(self).entries_view().update(i, entry),
                        Ok(None) => final(self).entries_view() == old(self).entries_view().push(entry)
                            && forall|i: int| 0 <= i < old(self).entries_view().len() ==>
                                old(self).entries_view()[i].0@ != entry.0@,
                    })
            {
                if route(&entry.0) != self.route { return Err(entry); }
                if let Some(i) = find(&self.entries, &entry.0) {
                    let original = self.entries.remove(i);
                    self.entries.insert(i, entry);
                    sail_proof! {
                        self.member_at(i as int);
                    }
                    Ok(Some(original))
                } else {
                    self.entries.push(entry);
                    sail_proof! {
                        self.member_at(self.entries@.len() - 1);
                    }
                    Ok(None)
                }
            }
        }

        sail_method! {
            /// Return the original complete-key entry and remove only its position.
            #[allow(clippy::question_mark, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn take(&mut self, key: &[u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    final(self).route_view() == old(self).route_view(),
                    out.is_some() == old(self).contains_view(key@),
                    match out {
                        Some(original) => exists|i: int| 0 <= i < old(self).entries_view().len()
                            && original == old(self).entries_view()[i]
                            && original.0@ == key@
                            && final(self).entries_view() == old(self).entries_view().remove(i),
                        None => final(self).entries_view() == old(self).entries_view()
                            && forall|i: int| 0 <= i < old(self).entries_view().len() ==>
                                old(self).entries_view()[i].0@ != key@,
                    })
            {
                let Some(i) = find(&self.entries, key) else { return None; };
                Some(self.entries.remove(i))
            }
        }
    }
}
