//! One owned counted slot map and its actual ordered index.
//!
//! This staged owner closes full-key/tick correspondence through private
//! construction and executable conflict checks. It supplies no solver origin,
//! global charge, lifetime-clock or Mutex ownership claim.
#[cfg(sail_verus)]
use vstd::prelude::*;

use std::collections::BTreeMap;

use crate::cache_candidate::{Entry, Slot};
use crate::cache_counted::CountedSlots;
use crate::cache_order;
use crate::inputs::byte_eq;

sail_spec! {
    /// Both directions refer to the same actual counted map and ordered index.
    /// Array keys are compared in full; no digest/collision premise is used.
    pub open spec fn paired<Value, Stamp>(slots: CountedSlots<Value, Stamp>,
        order: Map<u64, [u8; 32]>) -> bool {
        slots.valid()
            && order.len() == slots.entries_view() as nat
            && (forall|key: Seq<u8>| key.len() == 32 ==> match slots.lookup_view(key) {
                Some(slot) => order.contains_key(slot.tick_view())
                    && order[slot.tick_view()]@ == key,
                None => true,
            })
            && (forall|tick: u64| order.contains_key(tick) ==>
                match slots.lookup_view(order[tick]@) {
                    Some(slot) => slot.tick_view() == tick,
                    None => false,
                })
    }

    /// The actual old index is consumed in ascending order while the actual
    /// new index records processed slots. Both partition the original keys;
    /// this is a predicate over real state, not an additional runtime ledger.
    pub open spec fn reindexing<Value, Stamp>(before: CountedSlots<Value, Stamp>,
        original: Map<u64, [u8; 32]>, current: CountedSlots<Value, Stamp>,
        indexed: Map<u64, [u8; 32]>, remaining: Map<u64, [u8; 32]>, tick: u64) -> bool {
        paired(before, original)
            && current.valid()
            && current.entries_view() == before.entries_view()
            && indexed.len() == tick as nat
            && indexed.len() + remaining.len() == original.len()
            && (forall|old_tick: u64| #[trigger] remaining.contains_key(old_tick) ==>
                original.contains_key(old_tick) && remaining[old_tick] == original[old_tick])
            && (forall|key: Seq<u8>| key.len() == 32 ==> match before.lookup_view(key) {
                None => current.lookup_view(key).is_none(),
                Some(slot) => if remaining.contains_key(slot.tick_view()) {
                    current.lookup_view(key) == Some(slot)
                } else {
                    current.lookup_view(key).is_some()
                        && current.lookup_view(key) == Some(slot.reticked_view(
                            current.lookup_view(key).unwrap().tick_view()))
                        && indexed.contains_key(current.lookup_view(key).unwrap().tick_view())
                        && indexed[current.lookup_view(key).unwrap().tick_view()]@ == key
                },
            })
            && (forall|new_tick: u64| indexed.contains_key(new_tick) ==>
                1 <= new_tick <= tick
                    && before.lookup_view(indexed[new_tick]@).is_some()
                    && !remaining.contains_key(before.lookup_view(indexed[new_tick]@).unwrap().tick_view())
                    && current.lookup_view(indexed[new_tick]@).is_some()
                    && current.lookup_view(indexed[new_tick]@).unwrap().tick_view() == new_tick)
            && (forall|a: u64, b: u64| original.contains_key(a) && original.contains_key(b)
                && !remaining.contains_key(a) && remaining.contains_key(b) ==> a < b)
            && (forall|a: u64, b: u64| original.contains_key(a) && original.contains_key(b)
                && !remaining.contains_key(a) && !remaining.contains_key(b) && a < b ==>
                    current.lookup_view(original[a]@).unwrap().tick_view()
                        < current.lookup_view(original[b]@).unwrap().tick_view())
    }

    /// Reassemble the checked real-state partition from separately established
    /// clauses. This lemma supplies no state, identity or assumed observation.
    pub proof fn reindexing_join<Value, Stamp>(before: CountedSlots<Value, Stamp>,
        original: Map<u64, [u8; 32]>, current: CountedSlots<Value, Stamp>,
        indexed: Map<u64, [u8; 32]>, remaining: Map<u64, [u8; 32]>, tick: u64)
        requires
            paired(before, original),
            current.valid(),
            current.entries_view() == before.entries_view(),
            indexed.len() == tick as nat,
            indexed.len() + remaining.len() == original.len(),
            (forall|old_tick: u64| #[trigger] remaining.contains_key(old_tick) ==>
                original.contains_key(old_tick) && remaining[old_tick] == original[old_tick]),
            (forall|key: Seq<u8>| key.len() == 32 ==> match before.lookup_view(key) {
                None => current.lookup_view(key).is_none(),
                Some(slot) => if remaining.contains_key(slot.tick_view()) {
                    current.lookup_view(key) == Some(slot)
                } else {
                    current.lookup_view(key).is_some()
                        && current.lookup_view(key) == Some(slot.reticked_view(
                            current.lookup_view(key).unwrap().tick_view()))
                        && indexed.contains_key(current.lookup_view(key).unwrap().tick_view())
                        && indexed[current.lookup_view(key).unwrap().tick_view()]@ == key
                },
            }),
            (forall|new_tick: u64| indexed.contains_key(new_tick) ==>
                1 <= new_tick <= tick
                    && before.lookup_view(indexed[new_tick]@).is_some()
                    && !remaining.contains_key(before.lookup_view(indexed[new_tick]@).unwrap().tick_view())
                    && current.lookup_view(indexed[new_tick]@).is_some()
                    && current.lookup_view(indexed[new_tick]@).unwrap().tick_view() == new_tick),
            (forall|a: u64, b: u64| original.contains_key(a) && original.contains_key(b)
                && !remaining.contains_key(a) && remaining.contains_key(b) ==> a < b),
            (forall|a: u64, b: u64| original.contains_key(a) && original.contains_key(b)
                && !remaining.contains_key(a) && !remaining.contains_key(b) && a < b ==>
                    current.lookup_view(original[a]@).unwrap().tick_view()
                        < current.lookup_view(original[b]@).unwrap().tick_view()),
        ensures reindexing(before, original, current, indexed, remaining, tick),
    { reveal(reindexing); }

    /// Removing one actual old-index member preserves every retained original
    /// key/value correspondence. No slot, clock or origin premise is supplied.
    pub proof fn remaining_index_frame(original: Map<u64, [u8; 32]>,
        previous: Map<u64, [u8; 32]>, tick: u64)
        requires forall|index: u64| #[trigger] previous.contains_key(index) ==>
            original.contains_key(index) && previous[index] == original[index],
        ensures forall|index: u64| #[trigger] previous.remove(tick).contains_key(index) ==>
            original.contains_key(index) && previous.remove(tick)[index] == original[index],
    {
        assert forall|index: u64| #[trigger] previous.remove(tick).contains_key(index) implies
            original.contains_key(index) && previous.remove(tick)[index] == original[index] by {
            assert(previous.contains_key(index));
            assert(original.contains_key(index));
            assert(previous[index] == original[index]);
            assert(previous.remove(tick)[index] == previous[index]);
        }
    }

}

sail_code! {
    /// The actual map and actual ordered index have one owner. Neither field
    /// has a raw constructor, mutable observer, clone or deserialization path.
    pub struct IndexedSlots<Value, Stamp> {
        slots: CountedSlots<Value, Stamp>,
        order: BTreeMap<u64, [u8; 32]>,
    }

    impl<Value, Stamp> IndexedSlots<Value, Stamp> {
        sail_impl_spec! {
            /// The one real counted slot map.
            pub closed spec fn slots_view(&self) -> CountedSlots<Value, Stamp> { self.slots }
            /// The one real ordered index.
            pub closed spec fn order_view(&self) -> Map<u64, [u8; 32]> { self.order@ }
            /// Every selected full key has precisely its original indexed tick.
            pub open spec fn valid(&self) -> bool { paired(self.slots_view(), self.order_view()) }
            /// Total full-key selection from the original storage.
            pub open spec fn lookup_view(&self, key: Seq<u8>) -> Option<Slot<Value, Stamp>> {
                self.slots_view().lookup_view(key)
            }
        }

        sail_method! {
            /// Construct only a genuinely empty map/index owner.
            pub fn empty() -> (out: Self)
                requires (true)
                ensures (out.valid(), out.slots_view().entries_view() == 0,
                    out.order_view().is_empty())
            {
                Self { slots: CountedSlots::empty(), order: BTreeMap::new() }
            }
        }

        sail_method! {
            /// Observe the complete actual wrapper carrier including padding.
            pub fn owner_bytes() -> (out: usize)
                requires (true) ensures (out as nat == vstd::layout::size_of::<Self>())
            { std::mem::size_of::<Self>() }
        }

        sail_method! {
            /// Read the original slot without exposing mutable storage/index state.
            pub fn get<'a>(&'a self, key: &[u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
                requires (self.valid())
                ensures (match out {
                    Some(slot) => self.lookup_view(key@) == Some(*slot),
                    None => self.lookup_view(key@).is_none(),
                })
            { self.slots.get(key) }
        }

        sail_method! {
            /// Observe the actual index for diagnostics/assertions. This shared
            /// borrow cannot replace, mutate or detach its owning slot map.
            pub fn order(&self) -> (out: &BTreeMap<u64, [u8; 32]>)
                requires (true) ensures (out@ == self.order_view())
            { &self.order }
        }

        sail_method! {
            /// Observe actual complete-key/index emptiness in constant time.
            pub fn is_empty(&self) -> (out: bool)
                requires (self.valid())
                ensures (out == (self.slots_view().entries_view() == 0),
                    out == self.order_view().is_empty())
            { self.slots.is_empty() }
        }

        sail_method! {
            /// Constant-time full-key count, derived from the actual stored rows.
            pub fn len(&self) -> (out: usize)
                requires (self.valid())
                ensures (out == self.slots_view().entries_view(), out as nat == self.order_view().len())
            { self.slots.len() }
        }

        sail_method! {
            /// Observe retained primitive table capacity; no allocator/RSS bound.
            pub fn capacity(&self) -> (out: usize)
                requires (true)
                ensures (out >= self.slots_view().slots_view().buckets_view().len())
            { self.slots.capacity() }
        }

        sail_method! {
            /// Count routes separately from complete keys and ordered ticks.
            pub fn bucket_count(&self) -> (out: usize)
                requires (true)
                ensures (out == self.slots_view().slots_view().buckets_view().len())
            { self.slots.bucket_count() }
        }

        sail_method! {
            /// Admit only a new full key and unoccupied tick. Any conflict or
            /// count overflow returns the same original entry without mutation.
            /// # Errors
            /// Returns the original entry when its key/tick conflicts or counting overflows.
            pub fn insert_new(&mut self, entry: Entry<Value, Stamp>)
                -> (out: Result<(), Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    out.is_ok() == (old(self).lookup_view(entry.0@).is_none()
                        && !old(self).order_view().contains_key(entry.1.tick_view())
                        && old(self).slots_view().entries_view() < usize::MAX),
                    match out {
                        Err(original) => original == entry
                            && final(self).slots_view() == old(self).slots_view()
                            && final(self).order_view() == old(self).order_view(),
                        Ok(()) => final(self).order_view()
                                == old(self).order_view().insert(entry.1.tick_view(), entry.0)
                            && final(self).slots_view().entries_view() as int
                                == old(self).slots_view().entries_view() as int + 1
                            && forall|other: Seq<u8>| other.len() == 32 ==>
                                final(self).lookup_view(other)
                                    == if other == entry.0@ { Some(entry.1) }
                                        else { old(self).lookup_view(other) },
                    })
            {
                sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
                sail_snapshot!(before_slots = self.slots);
                sail_snapshot!(before_order = self.order);
                sail_snapshot!(incoming = entry);
                let key = entry.0;
                let tick = entry.1.tick();
                if self.slots.get(&key).is_some() || self.order.contains_key(&tick) {
                    return Err(entry);
                }
                match self.slots.insert(entry) {
                    Err(original) => {
                        sail_proof! { CountedSlots::state_extensional(before_slots, self.slots); }
                        Err(original)
                    }
                    Ok(_previous) => {
                        sail_proof! {
                            assert(_previous.is_none());
                            assert(key == incoming.0);
                            assert(key@.len() == 32);
                            assert(self.slots.lookup_view(key@) == Some(incoming.1));
                            assert forall|other: Seq<u8>| other.len() == 32
                                implies self.slots.lookup_view(other)
                                    == if other == key@ { Some(incoming.1) }
                                        else { before_slots.lookup_view(other) } by { }
                        }
                        self.order.insert(tick, key);
                        sail_proof! {
                            vstd::set::lemma_set_insert_len(before_order@.dom(), tick);
                            assert(self.order@.dom() == before_order@.dom().insert(tick));
                            assert(self.order@.len() == before_order@.len() + 1);
                            assert forall|other: Seq<u8>| other.len() == 32
                                implies match self.slots.lookup_view(other) {
                                    Some(slot) => self.order@.contains_key(slot.tick_view())
                                        && self.order@[slot.tick_view()]@ == other,
                                    None => true,
                                } by {
                                if other == key@ {
                                    assert(self.slots.lookup_view(other) == Some(incoming.1));
                                } else if let Some(slot) = before_slots.lookup_view(other) {
                                    assert(before_order@.contains_key(slot.tick_view()));
                                    assert(slot.tick_view() != tick);
                                }
                            }
                            assert forall|index: u64| self.order@.contains_key(index)
                                implies match self.slots.lookup_view(self.order@[index]@) {
                                    Some(slot) => slot.tick_view() == index,
                                    None => false,
                                } by {
                                if index == tick {
                                    assert(self.order@[index] == key);
                                    assert(self.slots.lookup_view(key@) == Some(incoming.1));
                                } else {
                                    assert(before_order@.contains_key(index));
                                    let original_key = before_order@[index];
                                    assert(before_slots.lookup_view(original_key@).is_some());
                                    assert(original_key@ != key@);
                                }
                            }
                        }
                        Ok(())
                    }
                }
            }
        }

        sail_method! {
            /// Retire the exact original full-key slot and its own indexed tick.
            /// Colliding neighbors and unrelated routes retain their slots/order.
            pub fn take(&mut self, key: &[u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    match out {
                        None => old(self).lookup_view(key@).is_none()
                            && final(self).order_view() == old(self).order_view(),
                        Some(original) => original.0@ == key@
                            && old(self).lookup_view(key@) == Some(original.1)
                            && final(self).order_view()
                                == old(self).order_view().remove(original.1.tick_view()),
                    },
                    forall|other: Seq<u8>| other.len() == 32 ==> final(self).lookup_view(other)
                        == if other == key@ { None } else { old(self).lookup_view(other) })
            {
                sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
                let original = self.slots.take(key);
                if let Some((_, slot)) = &original { self.order.remove(&slot.tick()); }
                original
            }
        }

        sail_method! {
            /// Retick only the selected original slot. A tick occupied by a
            /// different full key refuses unchanged rather than displacing it.
            #[allow(clippy::collapsible_if, reason = "Explicit conflict refusal retains the checked body and full-runtime failure controls.")]
            pub fn touch(&mut self, key: &[u8; 32], tick: u64) -> (out: Option<u64>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    final(self).slots_view().entries_view() == old(self).slots_view().entries_view(),
                    out.is_some() == (old(self).lookup_view(key@).is_some()
                        && (!old(self).order_view().contains_key(tick)
                            || old(self).order_view()[tick]@ == key@)),
                    match out {
                        None => final(self).order_view() == old(self).order_view()
                            && forall|other: Seq<u8>| other.len() == 32 ==>
                                final(self).lookup_view(other) == old(self).lookup_view(other),
                        Some(previous) => old(self).lookup_view(key@).unwrap().tick_view() == previous
                            && final(self).order_view()
                                == old(self).order_view().remove(previous).insert(tick, *key)
                            && forall|other: Seq<u8>| other.len() == 32 ==>
                                final(self).lookup_view(other)
                                    == if other == key@ {
                                        Some(old(self).lookup_view(key@).unwrap().reticked_view(tick))
                                    } else { old(self).lookup_view(other) },
                    })
            {
                sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
                sail_snapshot!(before_slots = self.slots);
                sail_snapshot!(before_order = self.order);
                if let Some(current) = self.order.get(&tick) {
                    if !byte_eq(current, key) { return None; }
                }
                let previous = self.slots.touch(key, tick);
                if let Some(previous) = previous {
                    self.order.remove(&previous);
                    self.order.insert(tick, *key);
                    sail_proof! {
                        let original = before_slots.lookup_view(key@).unwrap();
                        original.reticked_fields(tick);
                        assert(original.tick_view() == previous);
                        assert(before_order@.contains_key(previous));
                        if tick != previous { assert(!before_order@.contains_key(tick)); }
                        vstd::set::lemma_set_remove_len(before_order@.dom(), previous);
                        vstd::set::lemma_set_insert_len(before_order@.dom().remove(previous), tick);
                        assert(self.order@.dom() == before_order@.dom().remove(previous).insert(tick));
                        assert(self.order@.len() == before_order@.len());
                        assert forall|other: Seq<u8>| other.len() == 32
                            implies match self.slots.lookup_view(other) {
                                Some(slot) => self.order@.contains_key(slot.tick_view())
                                    && self.order@[slot.tick_view()]@ == other,
                                None => true,
                            } by {
                            if other == key@ {
                                assert(self.slots.lookup_view(other) == Some(original.reticked_view(tick)));
                            } else if let Some(slot) = before_slots.lookup_view(other) {
                                assert(before_order@.contains_key(slot.tick_view()));
                                assert(before_order@[slot.tick_view()]@ == other);
                                assert(slot.tick_view() != previous);
                                assert(slot.tick_view() != tick);
                            }
                        }
                        assert forall|index: u64| self.order@.contains_key(index)
                            implies match self.slots.lookup_view(self.order@[index]@) {
                                Some(slot) => slot.tick_view() == index,
                                None => false,
                            } by {
                            if index == tick {
                                assert(self.order@[index]@ == key@);
                            } else {
                                assert(before_order@.contains_key(index));
                                let original_key = before_order@[index];
                                assert(original_key@ != key@);
                            }
                        }
                    }
                }
                previous
            }
        }

        sail_method! {
            /// Remove the actual least original tick and its same full-key slot.
            #[allow(clippy::question_mark, reason = "Explicit empty refusal preserves the checked body and full-runtime failure controls.")]
            pub fn take_oldest(&mut self) -> (out: Option<Entry<Value, Stamp>>)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    match out {
                        None => old(self).order_view().is_empty(),
                        Some(original) => old(self).lookup_view(original.0@) == Some(original.1)
                            && final(self).lookup_view(original.0@).is_none()
                            && final(self).order_view()
                                == old(self).order_view().remove(original.1.tick_view())
                            && forall|tick: u64| old(self).order_view().contains_key(tick)
                                ==> original.1.tick_view() <= tick,
                    },
                    forall|key: Seq<u8>| key.len() == 32 ==> final(self).lookup_view(key)
                        == if out.is_some() && key == out.unwrap().0@ { None }
                            else { old(self).lookup_view(key) })
            {
                let Some((_tick, key)) = cache_order::pop_oldest(&mut self.order) else { return None; };
                self.slots.take(&key)
            }
        }

        sail_method! {
            /// Dense recency renumbering consumes the actual old least ticks.
            /// Original slot payload/state/generation and relative LRU order
            /// survive; only recency changes. This never constructs a Clock.
            #[allow(clippy::too_many_lines, reason = "The original checked loop and its erased per-entry framing proofs stay together in the actual shared implementation.")]
            pub fn renumber(&mut self) -> (out: u64)
                requires (old(self).valid())
                ensures (final(self).valid(),
                    out as nat == final(self).slots_view().entries_view() as nat,
                    final(self).slots_view().entries_view() == old(self).slots_view().entries_view(),
                    forall|key: Seq<u8>| key.len() == 32 ==> match old(self).lookup_view(key) {
                        None => final(self).lookup_view(key).is_none(),
                        Some(slot) => final(self).lookup_view(key).is_some()
                            && final(self).lookup_view(key) == Some(slot.reticked_view(
                                final(self).lookup_view(key).unwrap().tick_view())),
                    },
                    forall|a: u64, b: u64| old(self).order_view().contains_key(a)
                        && old(self).order_view().contains_key(b) && a < b ==>
                            final(self).lookup_view(old(self).order_view()[a]@).unwrap().tick_view()
                                < final(self).lookup_view(old(self).order_view()[b]@).unwrap().tick_view(),
                    forall|tick: u64| final(self).order_view().contains_key(tick) ==> 1 <= tick <= out)
            {
                sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
                sail_snapshot!(before = self.slots);
                sail_snapshot!(original = self.order);
                let mut remaining = cache_order::take_order(&mut self.order);
                let mut tick = 0u64;
                sail_while! { (!remaining.is_empty())
                    invariant (reindexing(before, original@, self.slots, self.order@, remaining@, tick))
                    decreases (remaining@.len())
                    {
                        sail_snapshot!(remaining_before = remaining);
                        sail_snapshot!(slots_before = self.slots);
                        sail_snapshot!(order_before = self.order);
                        sail_snapshot!(tick_before = tick);
                        sail_proof! {
                            assert(reindexing(before, original@, slots_before,
                                order_before@, remaining_before@, tick_before));
                        }
                        let Some((_original_tick, key)) = cache_order::pop_oldest(&mut remaining) else {
                            sail_proof! { assert(false); }
                            return tick;
                        };
                        sail_proof! {
                            assert(remaining_before@.contains_key(_original_tick));
                            assert(remaining@ == remaining_before@.remove(_original_tick));
                            remaining_index_frame(original@, remaining_before@, _original_tick);
                            assert(original@.contains_key(_original_tick));
                            assert(original@[_original_tick] == key);
                            assert(before.lookup_view(key@).is_some());
                            assert(before.lookup_view(key@).unwrap().tick_view() == _original_tick);
                            assert(slots_before.lookup_view(key@) == before.lookup_view(key@));
                            assert((tick_before as nat) < original@.len());
                            assert(original@.len() == before.entries_view() as nat);
                            assert(usize::MAX as nat <= u64::MAX as nat);
                            assert(tick_before < u64::MAX);
                        }
                        let next = tick.saturating_add(1);
                        let previous = self.slots.touch(&key, next);
                        sail_proof! {
                            assert(next as int == tick_before as int + 1);
                            assert(previous == Some(_original_tick));
                            before.lookup_view(key@).unwrap().reticked_fields(next);
                        }
                        if previous.is_some() {
                            tick = next;
                            self.order.insert(tick, key);
                            sail_proof! {
                                assert(!order_before@.contains_key(next));
                                vstd::set::lemma_set_remove_len(remaining_before@.dom(), _original_tick);
                                vstd::set::lemma_set_insert_len(order_before@.dom(), next);
                                assert(remaining@.dom() == remaining_before@.dom().remove(_original_tick));
                                assert(self.order@.dom() == order_before@.dom().insert(next));
                                assert(self.order@.len() == next as nat);
                                assert(self.order@.len() + remaining@.len() == original@.len());
                                assert(self.slots.valid());
                                assert(self.slots.entries_view() == before.entries_view());
                                assert forall|old_tick: u64| remaining@.contains_key(old_tick) implies
                                    original@.contains_key(old_tick)
                                        && remaining@[old_tick] == original@[old_tick] by {
                                    assert(remaining_before@.contains_key(old_tick));
                                    assert(remaining@[old_tick] == remaining_before@[old_tick]);
                                }
                                assert forall|other: Seq<u8>| other.len() == 32 implies
                                    match before.lookup_view(other) {
                                        None => self.slots.lookup_view(other).is_none(),
                                        Some(slot) => if remaining@.contains_key(slot.tick_view()) {
                                            self.slots.lookup_view(other) == Some(slot)
                                        } else {
                                            self.slots.lookup_view(other).is_some()
                                                && self.slots.lookup_view(other) == Some(slot.reticked_view(
                                                    self.slots.lookup_view(other).unwrap().tick_view()))
                                                && self.order@.contains_key(self.slots.lookup_view(other).unwrap().tick_view())
                                                && self.order@[self.slots.lookup_view(other).unwrap().tick_view()]@ == other
                                        },
                                    } by {
                                    if other == key@ {
                                        assert(self.slots.lookup_view(other) == Some(
                                            before.lookup_view(key@).unwrap().reticked_view(next)));
                                    } else if let Some(slot) = before.lookup_view(other) {
                                        assert(slot.tick_view() != _original_tick);
                                        assert(self.slots.lookup_view(other) == slots_before.lookup_view(other));
                                        if !remaining_before@.contains_key(slot.tick_view()) {
                                            let old_tick = slots_before.lookup_view(other).unwrap().tick_view();
                                            assert(order_before@.contains_key(old_tick));
                                            assert(old_tick < next);
                                        }
                                    }
                                }
                                assert forall|new_tick: u64| self.order@.contains_key(new_tick) implies
                                    1 <= new_tick <= next
                                        && before.lookup_view(self.order@[new_tick]@).is_some()
                                        && !remaining@.contains_key(before.lookup_view(self.order@[new_tick]@).unwrap().tick_view())
                                        && self.slots.lookup_view(self.order@[new_tick]@).is_some()
                                        && self.slots.lookup_view(self.order@[new_tick]@).unwrap().tick_view() == new_tick by {
                                    if new_tick == next {
                                        assert(self.order@[new_tick] == key);
                                    } else {
                                        assert(order_before@.contains_key(new_tick));
                                        let original_key = order_before@[new_tick];
                                        assert(original_key@ != key@);
                                        assert(self.slots.lookup_view(original_key@) == slots_before.lookup_view(original_key@));
                                    }
                                }
                                assert forall|a: u64, b: u64| original@.contains_key(a) && original@.contains_key(b)
                                    && !remaining@.contains_key(a) && remaining@.contains_key(b) implies a < b by {
                                    if a == _original_tick {
                                        assert(remaining_before@.contains_key(b));
                                        assert(_original_tick <= b);
                                        assert(a != b);
                                    }
                                }
                                assert forall|a: u64, b: u64| original@.contains_key(a) && original@.contains_key(b)
                                    && !remaining@.contains_key(a) && !remaining@.contains_key(b) && a < b implies
                                        self.slots.lookup_view(original@[a]@).unwrap().tick_view()
                                            < self.slots.lookup_view(original@[b]@).unwrap().tick_view() by {
                                    if a == _original_tick {
                                        assert(!remaining_before@.contains_key(b));
                                        assert(b < _original_tick);
                                    } else {
                                        assert(original@[a]@ != key@);
                                        assert(self.slots.lookup_view(original@[a]@) == slots_before.lookup_view(original@[a]@));
                                        if b == _original_tick {
                                            assert(order_before@.contains_key(slots_before.lookup_view(original@[a]@).unwrap().tick_view()));
                                            assert(slots_before.lookup_view(original@[a]@).unwrap().tick_view() < next);
                                        } else {
                                            assert(original@[b]@ != key@);
                                            assert(self.slots.lookup_view(original@[b]@) == slots_before.lookup_view(original@[b]@));
                                        }
                                    }
                                }
                                assert(paired(before, original@));
                                assert(self.order@.len() == tick as nat);
                                assert(self.order@.len() + remaining@.len() == original@.len());
                                assert(forall|old_tick: u64| remaining@.contains_key(old_tick) ==>
                                    original@.contains_key(old_tick) && remaining@[old_tick] == original@[old_tick]);
                                reindexing_join(before, original@, self.slots, self.order@, remaining@, tick);
                                assert(reindexing(before, original@, self.slots, self.order@, remaining@, tick));
                            }
                        }
                    }
                }
                sail_proof! {
                    assert(remaining@.is_empty());
                    assert(self.order@.len() == self.slots.entries_view() as nat);
                    assert forall|key: Seq<u8>| key.len() == 32 implies match self.slots.lookup_view(key) {
                        Some(slot) => self.order@.contains_key(slot.tick_view())
                            && self.order@[slot.tick_view()]@ == key,
                        None => true,
                    } by { assert(!before.lookup_view(key).is_none() || self.slots.lookup_view(key).is_none()); }
                }
                tick
            }
        }

        sail_method! {
            /// Clear both real structures and release retained primitive capacity.
            /// The separate original lifetime clock is never reconstructed here.
            pub fn clear(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).valid(), final(self).slots_view().entries_view() == 0,
                    final(self).order_view().is_empty())
            {
                self.slots = CountedSlots::empty();
                self.order.clear();
            }
        }
    }
}
