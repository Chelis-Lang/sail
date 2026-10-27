//! Actual primitive-key map owning complete-key collision chains.
//!
//! This staged storage preserves candidate entries. It does not establish
//! solver origin, native Mutex ownership or physical memory accounting.
#[cfg(sail_verus)]
use vstd::prelude::*;

use std::collections::HashMap;
use std::collections::hash_map::Entry as MapEntry;

use crate::cache_bucket::route;
use crate::cache_candidate::{Entry, Slot};
use crate::cache_chain::Chain;

sail_code! {
    /// Owns the actual bucket map; no public raw-map constructor or mutable escape.
    pub struct Slots<Value, Stamp> {
        buckets: HashMap<u128, Chain<Value, Stamp>>,
    }
}

sail_spec! {
    /// Number of actual complete-key entries, derived from the single owned map.
    pub open spec fn row_count<Value, Stamp>(buckets: Map<u128, Chain<Value, Stamp>>) -> nat
        decreases buckets.len(),
    {
        if buckets.len() == 0 { 0 }
        else {
            let r = buckets.dom().choose();
            buckets[r].entries_view().len() + row_count(buckets.remove(r))
        }
    }

    /// Separate any actual bucket from the complete owned entry count.
    pub proof fn row_count_split<Value, Stamp>(buckets: Map<u128, Chain<Value, Stamp>>, r: u128)
        requires buckets.contains_key(r),
        ensures row_count(buckets) == buckets[r].entries_view().len() + row_count(buckets.remove(r)),
        decreases buckets.len(),
    {
        let chosen = buckets.dom().choose();
        assert(buckets.contains_key(chosen));
        if chosen != r {
            row_count_split(buckets.remove(chosen), r);
            row_count_split(buckets.remove(r), chosen);
            assert(buckets.remove(chosen).remove(r) =~= buckets.remove(r).remove(chosen));
        }
    }

    /// Frame the entry count through the same real primitive-map target bucket.
    pub proof fn row_count_change<Value, Stamp>(before: Map<u128, Chain<Value, Stamp>>,
        after: Map<u128, Chain<Value, Stamp>>, r: u128)
        requires before.remove(r) == after.remove(r),
        ensures row_count(after) as int == row_count(before) as int
            + (if after.contains_key(r) { after[r].entries_view().len() as int } else { 0 })
            - (if before.contains_key(r) { before[r].entries_view().len() as int } else { 0 }),
    {
        if before.contains_key(r) { row_count_split(before, r); }
        else { assert(before.remove(r) == before); }
        if after.contains_key(r) { row_count_split(after, r); }
        else { assert(after.remove(r) == after); }
    }

    /// Every occupied valid bucket contributes at least one actual full-key row.
    pub proof fn row_count_buckets<Value, Stamp>(buckets: Map<u128, Chain<Value, Stamp>>)
        requires forall|r: u128| buckets.contains_key(r) ==> buckets[r].entries_view().len() > 0,
        ensures row_count(buckets) >= buckets.len(),
        decreases buckets.len(),
    {
        if buckets.len() > 0 {
            let r = buckets.dom().choose();
            assert(buckets.contains_key(r));
            row_count_buckets(buckets.remove(r));
            vstd::set::lemma_set_remove_len(buckets.dom(), r);
        }
    }

    /// Original-key interpretation of the actual primitive map.
    pub open spec fn map_lookup<Value, Stamp>(buckets: Map<u128, Chain<Value, Stamp>>, key: Seq<u8>)
        -> Option<Slot<Value, Stamp>> {
        if key.len() == 32 {
            let r = crate::cache_bucket::packed_prefix(key, 16);
            if buckets.contains_key(r) { chain_lookup(buckets[r].entries_view(), key) }
            else { None }
        } else { None }
    }
    /// Complete-key membership/value; the choice is used only after a real witness.
    pub open spec fn chain_lookup<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>, key: Seq<u8>)
        -> Option<Slot<Value, Stamp>> {
        if exists|i: int| 0 <= i < rows.len() && rows[i].0@ == key {
            let i = choose|i: int| 0 <= i < rows.len() && rows[i].0@ == key;
            Some(rows[i].1)
        } else { None }
    }

    /// Complete-key uniqueness, independent of route collision assumptions.
    pub open spec fn unique_keys<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>) -> bool {
        forall|i: int, j: int| 0 <= i < j < rows.len() ==> rows[i].0@ != rows[j].0@
    }

    /// A real stored index determines the unique lookup result.
    pub proof fn lookup_at<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>, key: Seq<u8>, index: int)
        requires unique_keys(rows), 0 <= index < rows.len(), rows[index].0@ == key,
        ensures chain_lookup(rows, key) == Some(rows[index].1),
    {
        assert(exists|i: int| 0 <= i < rows.len() && rows[i].0@ == key) by { }
        let chosen = choose|i: int| 0 <= i < rows.len() && rows[i].0@ == key;
        if chosen < index { assert(rows[chosen].0@ != rows[index].0@); }
        if index < chosen { assert(rows[index].0@ != rows[chosen].0@); }
        assert(chosen == index);
    }

    /// Removing one original position preserves every other complete key/value.
    pub proof fn lookup_remove<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>, index: int, key: Seq<u8>)
        requires unique_keys(rows), 0 <= index < rows.len(),
        ensures unique_keys(rows.remove(index)),
            chain_lookup(rows.remove(index), key) ==
                if key == rows[index].0@ { None } else { chain_lookup(rows, key) },
    {
        let remaining = rows.remove(index);
        assert forall|i: int| 0 <= i < remaining.len() implies
            remaining[i] == rows[if i < index { i } else { i + 1 }] by { }
        assert forall|i: int, j: int| 0 <= i < j < remaining.len() implies
            remaining[i].0@ != remaining[j].0@ by {
            let a = if i < index { i } else { i + 1 };
            let b = if j < index { j } else { j + 1 };
            assert(0 <= a < b < rows.len());
        }
        if key == rows[index].0@ {
            assert forall|i: int| 0 <= i < remaining.len() implies remaining[i].0@ != key by {
                let original = if i < index { i } else { i + 1 };
                if original < index { assert(rows[original].0@ != rows[index].0@); }
                else { assert(rows[index].0@ != rows[original].0@); }
            }
        } else if exists|i: int| 0 <= i < rows.len() && rows[i].0@ == key {
            let original = choose|i: int| 0 <= i < rows.len() && rows[i].0@ == key;
            assert(original != index);
            let after = if original < index { original } else { original - 1 };
            assert(0 <= after < remaining.len());
            assert(remaining[after] == rows[original]);
            lookup_at(rows, key, original);
            lookup_at(remaining, key, after);
        } else {
            assert forall|i: int| 0 <= i < remaining.len() implies remaining[i].0@ != key by {
                let original = if i < index { i } else { i + 1 };
                assert(rows[original].0@ != key);
            }
        }
    }

    /// Prepending an absent complete key retains all colliding neighbors.
    pub proof fn lookup_prepend<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>,
        entry: Entry<Value, Stamp>, key: Seq<u8>)
        requires unique_keys(rows),
            forall|i: int| 0 <= i < rows.len() ==> rows[i].0@ != entry.0@,
        ensures unique_keys(seq![entry] + rows),
            chain_lookup(seq![entry] + rows, key) ==
                if key == entry.0@ { Some(entry.1) } else { chain_lookup(rows, key) },
    {
        let added = seq![entry] + rows;
        assert forall|i: int, j: int| 0 <= i < j < added.len() implies added[i].0@ != added[j].0@ by {
            if i == 0 { assert(rows[j - 1].0@ != entry.0@); }
            else { assert(rows[i - 1].0@ != rows[j - 1].0@); }
        }
        if key == entry.0@ { lookup_at(added, key, 0); }
        else if exists|i: int| 0 <= i < rows.len() && rows[i].0@ == key {
            let original = choose|i: int| 0 <= i < rows.len() && rows[i].0@ == key;
            lookup_at(rows, key, original);
            lookup_at(added, key, original + 1);
        } else {
            assert forall|i: int| 0 <= i < added.len() implies added[i].0@ != key by {
                if i > 0 { assert(rows[i - 1].0@ != key); }
            }
        }
    }

    /// Lift the actual Chain insert's exact owned-entry result to all keys.
    pub proof fn inserted_rows<Value, Stamp>(before: Seq<Entry<Value, Stamp>>,
        after: Seq<Entry<Value, Stamp>>, entry: Entry<Value, Stamp>, previous: Option<Entry<Value, Stamp>>)
        requires unique_keys(before), match previous {
            Some(original) => original.0@ == entry.0@
                && exists|i: int| 0 <= i < before.len() && original == before[i]
                    && after == seq![entry] + before.remove(i),
            None => after == seq![entry] + before
                && forall|i: int| 0 <= i < before.len() ==> before[i].0@ != entry.0@,
        },
        ensures unique_keys(after), forall|key: Seq<u8>| chain_lookup(after, key)
            == if key == entry.0@ { Some(entry.1) } else { chain_lookup(before, key) },
    {
        match previous {
            Some(original) => {
                let index = choose|i: int| 0 <= i < before.len() && original == before[i]
                    && after == seq![entry] + before.remove(i);
                lookup_remove(before, index, entry.0@);
                assert forall|i: int| 0 <= i < before.remove(index).len()
                    implies before.remove(index)[i].0@ != entry.0@ by {
                    let old = if i < index { i } else { i + 1 };
                    if old < index { assert(before[old].0@ != before[index].0@); }
                    else { assert(before[index].0@ != before[old].0@); }
                }
                lookup_prepend(before.remove(index), entry, entry.0@);
                assert forall|key: Seq<u8>| chain_lookup(after, key)
                    == if key == entry.0@ { Some(entry.1) } else { chain_lookup(before, key) } by {
                    lookup_remove(before, index, key);
                    lookup_prepend(before.remove(index), entry, key);
                }
            }
            None => {
                lookup_prepend(before, entry, entry.0@);
                assert forall|key: Seq<u8>| chain_lookup(after, key)
                    == if key == entry.0@ { Some(entry.1) } else { chain_lookup(before, key) } by {
                    lookup_prepend(before, entry, key);
                }
            }
        }
    }

    /// Lift the actual Chain take's exact original position to all keys.
    pub proof fn removed_rows<Value, Stamp>(before: Seq<Entry<Value, Stamp>>,
        after: Seq<Entry<Value, Stamp>>, key: Seq<u8>, previous: Option<Entry<Value, Stamp>>)
        requires unique_keys(before), match previous {
            Some(original) => original.0@ == key
                && exists|i: int| 0 <= i < before.len() && original == before[i] && after == before.remove(i),
            None => after == before && forall|i: int| 0 <= i < before.len() ==> before[i].0@ != key,
        },
        ensures unique_keys(after), forall|other: Seq<u8>| chain_lookup(after, other)
            == if other == key { None } else { chain_lookup(before, other) },
    {
        match previous {
            Some(original) => {
                let index = choose|i: int| 0 <= i < before.len() && original == before[i] && after == before.remove(i);
                lookup_remove(before, index, key);
                assert forall|other: Seq<u8>| chain_lookup(after, other)
                    == if other == key { None } else { chain_lookup(before, other) } by {
                    lookup_remove(before, index, other);
                }
            }
            None => { }
        }
    }

    /// Updating one original position with the same complete key preserves all
    /// other complete keys; route-prefix collisions do not enter the argument.
    pub proof fn lookup_update<Value, Stamp>(rows: Seq<Entry<Value, Stamp>>, index: int,
        entry: Entry<Value, Stamp>, key: Seq<u8>)
        requires unique_keys(rows), 0 <= index < rows.len(), entry.0@ == rows[index].0@,
        ensures unique_keys(rows.update(index, entry)),
            chain_lookup(rows.update(index, entry), key)
                == if key == entry.0@ { Some(entry.1) } else { chain_lookup(rows, key) },
    {
        let updated = rows.update(index, entry);
        assert forall|i: int, j: int| 0 <= i < j < updated.len() implies
            updated[i].0@ != updated[j].0@ by {
            assert(updated[i].0@ == rows[i].0@);
            assert(updated[j].0@ == rows[j].0@);
        }
        if key == entry.0@ { lookup_at(updated, key, index); }
        else if exists|i: int| 0 <= i < rows.len() && rows[i].0@ == key {
            let original = choose|i: int| 0 <= i < rows.len() && rows[i].0@ == key;
            assert(original != index);
            lookup_at(rows, key, original);
            lookup_at(updated, key, original);
        } else {
            assert forall|i: int| 0 <= i < updated.len() implies updated[i].0@ != key by {
                assert(updated[i].0@ == rows[i].0@);
            }
        }
    }

    /// Total recency-only projection; absence stays absent.
    pub open spec fn reticked_lookup<Value, Stamp>(before: Option<Slot<Value, Stamp>>, tick: u64)
        -> Option<Slot<Value, Stamp>> {
        match before { Some(slot) => Some(slot.reticked_view(tick)), None => None }
    }

    /// Lift the actual allocation-free Chain touch to every original key.
    pub proof fn touched_rows<Value, Stamp>(before: Seq<Entry<Value, Stamp>>,
        after: Seq<Entry<Value, Stamp>>, key: Seq<u8>, tick: u64, previous: Option<u64>)
        requires unique_keys(before), match previous {
            Some(old_tick) => exists|i: int| 0 <= i < before.len() && before[i].0@ == key
                && before[i].1.tick_view() == old_tick
                && after == before.update(i, (before[i].0, before[i].1.reticked_view(tick))),
            None => after == before && forall|i: int| 0 <= i < before.len() ==> before[i].0@ != key,
        },
        ensures unique_keys(after), after.len() == before.len(),
            previous == match chain_lookup(before, key) { Some(slot) => Some(slot.tick_view()), None => None },
            forall|other: Seq<u8>| chain_lookup(after, other)
                == if other == key { reticked_lookup(chain_lookup(before, key), tick) }
                    else { chain_lookup(before, other) },
    {
        match previous {
            Some(old_tick) => {
                let index = choose|i: int| 0 <= i < before.len() && before[i].0@ == key
                    && before[i].1.tick_view() == old_tick
                    && after == before.update(i, (before[i].0, before[i].1.reticked_view(tick)));
                lookup_at(before, key, index);
                lookup_update(before, index, (before[index].0, before[index].1.reticked_view(tick)), key);
                assert forall|other: Seq<u8>| chain_lookup(after, other)
                    == if other == key { reticked_lookup(chain_lookup(before, key), tick) }
                        else { chain_lookup(before, other) } by {
                    lookup_update(before, index, (before[index].0, before[index].1.reticked_view(tick)), other);
                }
            }
            None => { }
        }
    }
}

sail_code! {
    impl<Value, Stamp> Slots<Value, Stamp> {
        sail_impl_spec! {
            /// The real primitive map, rather than a separate runtime ledger.
            pub closed spec fn buckets_view(&self) -> Map<u128, Chain<Value, Stamp>> { self.buckets@ }
            /// Placement, nonempty buckets and complete-key uniqueness.
            pub open spec fn valid(&self) -> bool {
                forall|r: u128| self.buckets_view().contains_key(r) ==> {
                    let rows = self.buckets_view()[r].entries_view();
                    rows.len() > 0
                    && (forall|i: int| 0 <= i < rows.len() ==>
                        crate::cache_bucket::packed_prefix(rows[i].0@, 16) == r)
                    && (forall|i: int, j: int| 0 <= i < j < rows.len() ==> rows[i].0@ != rows[j].0@)
                }
            }
            /// Total original-key read, with no collision-freedom premise.
            pub open spec fn lookup_view(&self, key: Seq<u8>) -> Option<Slot<Value, Stamp>> {
                map_lookup(self.buckets_view(), key)
            }
        }

        sail_method! {
            /// Observe this actual primitive-map header's native carrier.
            pub fn owner_bytes() -> (out: usize)
                requires (true) ensures (out as nat == vstd::layout::size_of::<Self>())
            { std::mem::size_of::<Self>() }
        }

        sail_method! {
            /// Observe the actual padded primitive route/chain carrier. Dynamic
            /// table allocation/control bytes retain their separate native scope.
            pub fn bucket_carrier_bytes() -> (out: usize)
                requires (true)
                ensures (out as nat == vstd::layout::size_of::<(u128, Chain<Value, Stamp>)>())
            { std::mem::size_of::<(u128, Chain<Value, Stamp>)>() }
        }

        sail_method! {
            /// Start a real empty map under the primitive default hasher.
            pub fn empty() -> (out: Self)
                requires (true)
                ensures (out.valid(), out.buckets_view().is_empty())
            { Self { buckets: HashMap::new() } }
        }

        sail_method! {
            /// Read only the original complete-key slot; prefixes may collide.
            #[allow(clippy::question_mark, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn get<'a>(&'a self, key: &[u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
                requires (self.valid())
                ensures (match out {
                    Some(slot) => self.lookup_view(key@) == Some(*slot),
                    None => self.lookup_view(key@).is_none(),
                })
            {
                let r = route(key);
                let Some(bucket) = self.buckets.get(&r) else { return None; };
                bucket.get(key)
            }
        }

        sail_method! {
            /// Install the exact incoming entry and return only its previous
            /// complete-key value. Every other key's original slot is preserved.
            pub fn insert(&mut self, entry: Entry<Value, Stamp>) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    row_count(final(self).buckets_view()) == row_count(old(self).buckets_view())
                        + if out.is_none() { 1nat } else { 0nat },
                    match out {
                        Some(original) => original.0@ == entry.0@
                            && old(self).lookup_view(entry.0@) == Some(original.1),
                        None => old(self).lookup_view(entry.0@).is_none(),
                    },
                    forall|key: Seq<u8>| key.len() == 32 ==> final(self).lookup_view(key)
                        == if key == entry.0@ { Some(entry.1) } else { old(self).lookup_view(key) })
            {
                let r = route(&entry.0);
                sail_snapshot!(before_buckets = self.buckets_view());
                let original = match self.buckets.entry(r) {
                    MapEntry::Occupied(bucket) => {
                        let chain = bucket.into_mut();
                        sail_snapshot!(before = chain.entries_view());
                        let original = chain.insert(entry);
                        sail_proof! { inserted_rows(before, chain.entries_view(), entry, original); }
                        original
                    }
                    MapEntry::Vacant(bucket) => {
                        let _chain = bucket.insert(Chain::prepend(entry, Chain::empty()));
                        sail_proof! {
                            inserted_rows(Seq::empty(), _chain.entries_view(), entry, None);
                        }
                        None
                    }
                };
                sail_proof! {
                    assert(self.buckets@.remove(r) == before_buckets.remove(r));
                    row_count_change(before_buckets, self.buckets@, r);
                    assert forall|key: Seq<u8>| key.len() == 32 implies self.lookup_view(key)
                        == if key == entry.0@ { Some(entry.1) } else { map_lookup(before_buckets, key) } by {
                        let query_route = crate::cache_bucket::packed_prefix(key, 16);
                        if query_route != r {
                            assert(key != entry.0@);
                            assert(self.buckets@.contains_key(query_route)
                                == before_buckets.contains_key(query_route));
                            if before_buckets.contains_key(query_route) {
                                assert(self.buckets@[query_route] == before_buckets[query_route]);
                            }
                        } else {
                            assert(self.buckets@.contains_key(r));
                            assert(chain_lookup(self.buckets@[r].entries_view(), key)
                                == if key == entry.0@ { Some(entry.1) }
                                    else if before_buckets.contains_key(r) {
                                        chain_lookup(before_buckets[r].entries_view(), key)
                                    } else { None });
                        }
                    }
                }
                original
            }
        }

        sail_method! {
            /// Retire only the original complete key and drop a bucket only when
            /// its actual chain becomes empty. Other keys/routes remain exact.
            pub fn take(&mut self, key: &[u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    row_count(old(self).buckets_view()) == row_count(final(self).buckets_view())
                        + if out.is_some() { 1nat } else { 0nat },
                    match out {
                        Some(original) => original.0@ == key@
                            && old(self).lookup_view(key@) == Some(original.1),
                        None => old(self).lookup_view(key@).is_none(),
                    },
                    forall|other: Seq<u8>| other.len() == 32 ==> final(self).lookup_view(other)
                        == if other == key@ { None } else { old(self).lookup_view(other) })
            {
                let r = route(key);
                sail_snapshot!(before_buckets = self.buckets_view());
                let original = match self.buckets.entry(r) {
                    MapEntry::Occupied(mut bucket) => {
                        let chain = bucket.get_mut();
                        sail_snapshot!(before = chain.entries_view());
                        let original = chain.take(key);
                        sail_proof! {
                            removed_rows(before, chain.entries_view(), key@, original);
                            chain.empty_entries();
                        }
                        let empty = chain.is_empty();
                        if empty { bucket.remove(); }
                        original
                    }
                    MapEntry::Vacant(_) => None,
                };
                sail_proof! {
                    assert(self.buckets@.remove(r) == before_buckets.remove(r));
                    row_count_change(before_buckets, self.buckets@, r);
                    assert forall|other: Seq<u8>| other.len() == 32 implies self.lookup_view(other)
                        == if other == key@ { None } else { map_lookup(before_buckets, other) } by {
                        let query_route = crate::cache_bucket::packed_prefix(other, 16);
                        if query_route != r { assert(other != key@); }
                    }
                }
                original
            }
        }

        sail_method! {
            /// Change only the selected original slot's recency, returning its
            /// old tick for the actual LRU index. No insertion identity is issued.
            pub fn touch(&mut self, key: &[u8; 32], tick: u64) -> (out: Option<u64>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    row_count(final(self).buckets_view()) == row_count(old(self).buckets_view()),
                    out == match old(self).lookup_view(key@) {
                        Some(slot) => Some(slot.tick_view()), None => None },
                    forall|other: Seq<u8>| other.len() == 32 ==> final(self).lookup_view(other)
                        == if other == key@ { reticked_lookup(old(self).lookup_view(key@), tick) }
                            else { old(self).lookup_view(other) })
            {
                let r = route(key);
                sail_snapshot!(before_buckets = self.buckets_view());
                let previous = match self.buckets.entry(r) {
                    MapEntry::Occupied(bucket) => {
                        let chain = bucket.into_mut();
                        sail_snapshot!(before = chain.entries_view());
                        let previous = chain.touch(key, tick);
                        sail_proof! { touched_rows(before, chain.entries_view(), key@, tick, previous); }
                        previous
                    }
                    MapEntry::Vacant(_) => None,
                };
                sail_proof! {
                    assert(self.buckets@.remove(r) == before_buckets.remove(r));
                    row_count_change(before_buckets, self.buckets@, r);
                    assert forall|other: Seq<u8>| other.len() == 32 implies self.lookup_view(other)
                        == if other == key@ { reticked_lookup(map_lookup(before_buckets, key@), tick) }
                            else { map_lookup(before_buckets, other) } by {
                        let query_route = crate::cache_bucket::packed_prefix(other, 16);
                        if query_route != r { assert(other != key@); }
                    }
                }
                previous
            }
        }

        sail_method! {
            /// Observe actual retained primitive-map capacity. The standard
            /// contract gives a lower bound, not an allocator or RSS theorem.
            pub fn capacity(&self) -> (out: usize)
                requires (true) ensures (out >= self.buckets_view().len())
            { self.buckets.capacity() }
        }

        sail_method! {
            /// Observe the actual number of primitive buckets, not full keys.
            pub fn bucket_count(&self) -> (out: usize)
                requires (true) ensures (out == self.buckets_view().len())
            { self.buckets.len() }
        }

        sail_method! {
            /// Clear the actual map through the same iterative chain Drop bodies.
            pub fn clear(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).valid(), final(self).buckets_view().is_empty())
            { self.buckets.clear(); }
        }
    }
}
