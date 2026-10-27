//! Checked entry count joined to the single actual owned slot map.
//!
//! This is candidate storage metadata, not solver-origin or proof authority.
#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::cache_candidate::{Entry, Slot};
use crate::cache_slots::Slots;
#[cfg(sail_verus)]
use crate::cache_slots::{reticked_lookup, row_count};

sail_code! {
    /// Owns one real map and the checked native counter used for constant-time
    /// full-key stats. No raw map or counter constructor escapes.
    pub struct CountedSlots<Value, Stamp> {
        slots: Slots<Value, Stamp>,
        entries: usize,
    }
}

sail_verified! {
    /// Reserve exactly one count only for a new complete key; overflow refuses
    /// before the caller mutates the actual owned map.
    pub fn next_count(entries: usize, absent: bool) -> (out: Option<usize>)
        requires (true)
        ensures (match out {
            Some(count) => count as int == entries as int + if absent { 1int } else { 0int },
            None => absent && entries == usize::MAX,
        }, out.is_none() == (absent && entries == usize::MAX))
    {
        if absent { entries.checked_add(1) } else { Some(entries) }
    }
}

sail_code! {
    impl<Value, Stamp> CountedSlots<Value, Stamp> {
        sail_impl_spec! {
            /// The one owned map, not an independent authoritative ledger.
            pub closed spec fn slots_view(&self) -> Slots<Value, Stamp> { self.slots }
            /// The actual native stats counter.
            pub closed spec fn entries_view(&self) -> usize { self.entries }
            /// Every counted entry is derived from actual map/chain storage.
            pub open spec fn valid(&self) -> bool {
                self.slots_view().valid()
                    && self.entries_view() as nat == row_count(self.slots_view().buckets_view())
            }
            /// Total full-key lookup through the original actual map.
            pub open spec fn lookup_view(&self, key: Seq<u8>) -> Option<Slot<Value, Stamp>> {
                self.slots_view().lookup_view(key)
            }
            /// The stored exact entry count bounds actual nonempty buckets.
            pub proof fn bucket_bound(&self)
                requires self.valid(),
                ensures self.entries_view() as nat >= self.slots_view().buckets_view().len(),
            { crate::cache_slots::row_count_buckets(self.slots_view().buckets_view()); }

            /// Equal actual owned fields imply equal counted state. This is a
            /// checked datatype lemma, not a primitive/native pointer axiom.
            pub proof fn state_extensional(before: Self, after: Self)
                requires before.slots_view() == after.slots_view(),
                    before.entries_view() == after.entries_view(),
                ensures before == after,
            { }
        }

        sail_method! {
            /// Observe the complete actual map/counter header, including padding.
            pub fn owner_bytes() -> (out: usize)
                requires (true) ensures (out as nat == vstd::layout::size_of::<Self>())
            { std::mem::size_of::<Self>() }
        }

        sail_method! {
            /// Start only an actually empty owner/counter pair.
            pub fn empty() -> (out: Self)
                requires (true) ensures (out.valid(), out.entries_view() == 0)
            { Self { slots: Slots::empty(), entries: 0 } }
        }

        sail_method! {
            /// Read the exact complete-key count in constant time.
            pub fn len(&self) -> (out: usize)
                requires (self.valid())
                ensures (out == self.entries_view(), out as nat == row_count(self.slots_view().buckets_view()))
            { self.entries }
        }

        sail_method! {
            /// Report emptiness from the same exact native count.
            pub fn is_empty(&self) -> (out: bool)
                requires (self.valid())
                ensures (out == (self.entries_view() == 0),
                    out == self.slots_view().buckets_view().is_empty())
            {
                sail_proof! { self.bucket_bound(); }
                self.entries == 0
            }
        }

        sail_method! {
            /// Borrow only the original selected slot; no mutable escape exists.
            pub fn get<'a>(&'a self, key: &[u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
                requires (self.valid())
                ensures (match out {
                    Some(slot) => self.lookup_view(key@) == Some(*slot),
                    None => self.lookup_view(key@).is_none(),
                })
            { self.slots.get(key) }
        }

        sail_method! {
            /// Install the original incoming entry, or return that same entry
            /// without any map/counter mutation when new-key counting overflows.
            /// # Errors
            /// Returns the same original entry when a new complete key would overflow the count.
            pub fn insert(&mut self, entry: Entry<Value, Stamp>)
                -> (out: Result<Option<Entry<Value, Stamp>>, Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    out.is_err() == (old(self).lookup_view(entry.0@).is_none()
                        && old(self).entries_view() == usize::MAX),
                    match out {
                        Err(original) => original == entry
                            && final(self).slots_view() == old(self).slots_view()
                            && final(self).entries_view() == old(self).entries_view(),
                        Ok(previous) => final(self).entries_view() as int
                                == old(self).entries_view() as int + if previous.is_none() { 1int } else { 0int }
                            && (match previous {
                                Some(original) => original.0@ == entry.0@
                                    && old(self).lookup_view(entry.0@) == Some(original.1),
                                None => old(self).lookup_view(entry.0@).is_none(),
                            })
                            && forall|key: Seq<u8>| key.len() == 32 ==> final(self).lookup_view(key)
                                == if key == entry.0@ { Some(entry.1) } else { old(self).lookup_view(key) },
                    })
            {
                let absent = self.slots.get(&entry.0).is_none();
                let Some(count) = next_count(self.entries, absent) else { return Err(entry); };
                let previous = self.slots.insert(entry);
                self.entries = count;
                Ok(previous)
            }
        }

        sail_method! {
            /// Remove only the original complete key and decrement only for an
            /// actual returned entry; all colliding neighbors remain counted.
            pub fn take(&mut self, key: &[u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    old(self).entries_view() as int == final(self).entries_view() as int
                        + if out.is_some() { 1int } else { 0int },
                    match out {
                        Some(original) => original.0@ == key@
                            && old(self).lookup_view(key@) == Some(original.1),
                        None => old(self).lookup_view(key@).is_none(),
                    },
                    forall|other: Seq<u8>| other.len() == 32 ==> final(self).lookup_view(other)
                        == if other == key@ { None } else { old(self).lookup_view(other) })
            {
                let original = self.slots.take(key);
                if original.is_some() { self.entries = self.entries.saturating_sub(1); }
                original
            }
        }

        sail_method! {
            /// Retick the actual selected slot without changing count or identity.
            pub fn touch(&mut self, key: &[u8; 32], tick: u64) -> (out: Option<u64>)
                requires (old(self).valid())
                ensures (final(self).valid(), final(self).entries_view() == old(self).entries_view(),
                    out == match old(self).lookup_view(key@) {
                        Some(slot) => Some(slot.tick_view()), None => None },
                    forall|other: Seq<u8>| other.len() == 32 ==> final(self).lookup_view(other)
                        == if other == key@ { reticked_lookup(old(self).lookup_view(key@), tick) }
                            else { old(self).lookup_view(other) })
            { self.slots.touch(key, tick) }
        }

        sail_method! {
            /// Retained map capacity is observed directly, including after removal.
            /// Its standard lower-bound contract supplies no RSS guarantee.
            pub fn capacity(&self) -> (out: usize)
                requires (true) ensures (out >= self.slots_view().buckets_view().len())
            { self.slots.capacity() }
        }

        sail_method! {
            /// Report primitive routes separately from full-key entry count.
            pub fn bucket_count(&self) -> (out: usize)
                requires (true) ensures (out == self.slots_view().buckets_view().len())
            { self.slots.bucket_count() }
        }

        sail_method! {
            /// Clear the real entries and their count through iterative Drop.
            pub fn clear(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).valid(), final(self).entries_view() == 0,
                    final(self).slots_view().buckets_view().is_empty())
            { self.slots.clear(); self.entries = 0; }
        }
    }
}
