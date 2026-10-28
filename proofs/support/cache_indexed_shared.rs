//! Actual indexed owner prerequisite. No native/clock/accounting/origin claim.
#![forbid(unsafe_code)]
#![allow(unused_imports)]
#[cfg(sail_verus)]
use vstd::prelude::*;
#[macro_use]
#[path = "../../crates/sail-acceptance/src/syntax.rs"]
mod syntax;
#[path = "../../crates/sail-acceptance/src/cache_bucket.rs"]
pub mod cache_bucket;
#[path = "../../crates/sail-acceptance/src/cache_candidate.rs"]
pub mod cache_candidate;
#[path = "../../crates/sail-acceptance/src/cache_chain.rs"]
pub mod cache_chain;
#[path = "../../crates/sail-acceptance/src/cache_counted.rs"]
pub mod cache_counted;
#[path = "../../crates/sail-acceptance/src/cache_indexed.rs"]
pub mod cache_indexed;
#[path = "../../crates/sail-acceptance/src/cache_order.rs"]
pub mod cache_order;
#[path = "../../crates/sail-acceptance/src/cache_slots.rs"]
pub mod cache_slots;
#[path = "../../crates/sail-acceptance/src/inputs.rs"]
pub mod inputs;

sail_verified! {
    /// Real same-prefix neighbors survive conflict refusal, touch, replacement
    /// refusal and least-entry/full-key retirement in the one actual owner.
    pub fn original_indexed_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let first = [7u8; 32];
        let mut second = [7u8; 32];
        second[31] = 8;
        let other = [8u8; 32];
        let mut clock = cache_candidate::Clock::start();
        let Some((a, _)) = clock.entry(first, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((b, _)) = clock.entry(second, 22u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((c, _)) = clock.entry(other, 33u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((replacement, _)) = clock.entry(first, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let mut indexed = cache_indexed::IndexedSlots::empty();
        if !indexed.is_empty() { return false; }
        if indexed.insert_new(a).is_err() || indexed.insert_new(b).is_err()
            || indexed.insert_new(c).is_err() { return false; }
        let Err(original) = indexed.insert_new(replacement) else { return false; };
        if original.1.tick() != 4 || *original.1.value() != 11 || indexed.len() != 3 {
            return false;
        }
        let mut other_clock = cache_candidate::Clock::start();
        let Some((same_tick, _)) = other_clock.entry([9u8; 32], 44u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Err(original) = indexed.insert_new(same_tick) else { return false; };
        if original.1.tick() != 1 || *original.1.value() != 44 || indexed.len() != 3 {
            return false;
        }
        if indexed.touch(&second, 1).is_some() || indexed.len() != 3 { return false; }
        let Some(before_neighbor) = indexed.get(&second) else { return false; };
        sail_proof! { before_neighbor.reticked_fields(20); }
        if indexed.touch(&second, 20) != Some(2) { return false; }
        sail_snapshot!(before_oldest = indexed);
        let Some(oldest) = indexed.take_oldest() else { return false; };
        sail_proof! {
            assert(before_oldest.order_view().contains_key(1u64));
            assert(oldest.1.tick_view() <= 1u64);
            assert(before_oldest.order_view().contains_key(oldest.1.tick_view()));
            assert(oldest.1.tick_view() == 1u64);
            assert(oldest.0 == first);
            assert(first@ != second@);
        }
        if oldest.0[31] != 7 || *oldest.1.value() != 11 || oldest.1.tick() != 1 {
            return false;
        }
        let Some(neighbor) = indexed.get(&second) else { return false; };
        if *neighbor.value() != 22 || neighbor.tick() != 20 { return false; }
        let Some(original) = indexed.take(&second) else { return false; };
        if *original.1.value() != 22 || original.1.tick() != 20 || indexed.len() != 1 {
            return false;
        }
        let Some(unrelated) = indexed.get(&other) else { return false; };
        if *unrelated.value() != 33 || unrelated.tick() != 3 { return false; }
        sail_proof! { unrelated.reticked_fields(1); }
        if indexed.renumber() != 1 { return false; }
        let Some(unrelated) = indexed.get(&other) else { return false; };
        if *unrelated.value() != 33 || unrelated.tick() != 1 { return false; }
        indexed.clear();
        indexed.is_empty() && indexed.len() == 0 && indexed.get(&first).is_none()
            && indexed.get(&second).is_none() && indexed.get(&other).is_none()
    }
}

// Preserve both complete existing constructive support functions verbatim.
sail_verified! {
    /// Different complete keys share a route; replacement/removal retain the
    /// original colliding neighbor and a third original key.
    pub fn original_map_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let first_key = [7u8; 32];
        let mut second_key = [7u8; 32];
        second_key[31] = 8;
        let other_key = [8u8; 32];
        let mut clock = cache_candidate::Clock::start();
        let Some((first, _)) = clock.entry(first_key, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((second, _)) = clock.entry(second_key, 22u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((other, _)) = clock.entry(other_key, 33u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((replacement, _)) = clock.entry(first_key, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let mut slots = cache_slots::Slots::empty();
        if slots.insert(first).is_some() || slots.insert(second).is_some()
            || slots.insert(other).is_some() { return false; }
        let Some(original) = slots.insert(replacement) else { return false; };
        if *original.1.value() != 11 || original.1.tick() != 1 { return false; }
        let Some(current) = slots.take(&first_key) else { return false; };
        if *current.1.value() != 11 || current.1.tick() != 4 { return false; }
        if slots.get(&first_key).is_some() { return false; }
        let Some(neighbor) = slots.get(&second_key) else { return false; };
        if *neighbor.value() != 22 || neighbor.tick() != 2 { return false; }
        sail_snapshot!(generation = neighbor.generation_view());
        sail_snapshot!(before_neighbor = *neighbor);
        sail_proof! {
            neighbor.reticked_fields(20);
            assert(first_key@[31] == 7);
            assert(second_key@[31] == 8);
            assert(first_key@ != second_key@);
        }
        if slots.touch(&second_key, 20) != Some(2) { return false; }
        if slots.touch(&first_key, 21).is_some() { return false; }
        let Some(reticked) = slots.get(&second_key) else { return false; };
        if *reticked.value() != 22 || reticked.tick() != 20 { return false; }
        sail_proof! {
            assert(*reticked == before_neighbor.reticked_view(20));
            assert(reticked.generation_view() == generation);
        }
        let Some(unrelated) = slots.get(&other_key) else { return false; };
        if *unrelated.value() != 33 || unrelated.tick() != 3 { return false; }
        slots.clear();
        slots.get(&second_key).is_none() && slots.get(&other_key).is_none()
    }
}

sail_verified! {
    /// Count the same original owned entries through collision, replacement,
    /// full-key removal, recency-only change and clear; check overflow separately
    /// through the exact numeric operation the real insertion calls.
    pub fn original_counted_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let first_key = [7u8; 32];
        let mut second_key = [7u8; 32];
        second_key[31] = 8;
        let other_key = [8u8; 32];
        let mut clock = cache_candidate::Clock::start();
        let Some((first, _)) = clock.entry(first_key, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((second, _)) = clock.entry(second_key, 22u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((other, _)) = clock.entry(other_key, 33u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((replacement, _)) = clock.entry(first_key, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let mut slots = cache_counted::CountedSlots::empty();
        if slots.len() != 0 || !slots.is_empty() { return false; }
        if !matches!(slots.insert(first), Ok(None)) || slots.len() != 1 || slots.is_empty() { return false; }
        if !matches!(slots.insert(second), Ok(None)) || slots.len() != 2 { return false; }
        if !matches!(slots.insert(other), Ok(None)) || slots.len() != 3 { return false; }
        let Ok(Some(original)) = slots.insert(replacement) else { return false; };
        if *original.1.value() != 11 || original.1.tick() != 1 || slots.len() != 3 { return false; }
        let Some(current) = slots.take(&first_key) else { return false; };
        if *current.1.value() != 11 || current.1.tick() != 4 || slots.len() != 2 { return false; }
        if slots.take(&first_key).is_some() || slots.len() != 2 { return false; }
        let Some(_before_neighbor) = slots.get(&second_key) else { return false; };
        sail_proof! { _before_neighbor.reticked_fields(20); }
        if slots.touch(&second_key, 20) != Some(2) || slots.len() != 2 { return false; }
        let Some(neighbor) = slots.get(&second_key) else { return false; };
        if *neighbor.value() != 22 || neighbor.tick() != 20 { return false; }
        let Some(unrelated) = slots.get(&other_key) else { return false; };
        if *unrelated.value() != 33 || unrelated.tick() != 3 { return false; }
        sail_proof! {
            reveal_with_fuel(cache_bucket::packed_prefix, 17);
            let first_route = cache_bucket::packed_prefix(second_key@, 16);
            let other_route = cache_bucket::packed_prefix(other_key@, 16);
            assert forall|i: int| 0 <= i < 16 implies second_key@[i] == 7 by { }
            assert forall|i: int| 0 <= i < 16 implies other_key@[i] == 8 by { }
            assert((0u128 | (7u128 << 0) | (7u128 << 8) | (7u128 << 16) | (7u128 << 24) | (7u128 << 32) | (7u128 << 40) | (7u128 << 48) | (7u128 << 56) | (7u128 << 64) | (7u128 << 72) | (7u128 << 80) | (7u128 << 88) | (7u128 << 96) | (7u128 << 104) | (7u128 << 112) | (7u128 << 120)) == 0x07070707070707070707070707070707u128) by (bit_vector);
            assert((0u128 | (8u128 << 0) | (8u128 << 8) | (8u128 << 16) | (8u128 << 24) | (8u128 << 32) | (8u128 << 40) | (8u128 << 48) | (8u128 << 56) | (8u128 << 64) | (8u128 << 72) | (8u128 << 80) | (8u128 << 88) | (8u128 << 96) | (8u128 << 104) | (8u128 << 112) | (8u128 << 120)) == 0x08080808080808080808080808080808u128) by (bit_vector);
            assert(first_route == 0x07070707070707070707070707070707u128);
            assert(other_route == 0x08080808080808080808080808080808u128);
            assert(first_route != other_route);
            let buckets = slots.slots_view().buckets_view();
            assert(buckets.contains_key(first_route));
            assert(buckets.contains_key(other_route));
            vstd::set::lemma_set_remove_len(buckets.dom(), first_route);
            vstd::set::lemma_set_contains_len(buckets.dom().remove(first_route), other_route);
            slots.bucket_bound();
            assert(buckets.len() == 2);
        }
        if slots.bucket_count() != 2 || slots.capacity() < 2 { return false; }
        slots.clear();
        if slots.len() != 0 || slots.get(&second_key).is_some() || slots.get(&other_key).is_some() {
            return false;
        }
        cache_counted::next_count(usize::MAX, true).is_none()
            && cache_counted::next_count(usize::MAX, false) == Some(usize::MAX)
            && cache_counted::next_count(0, true) == Some(1)
    }
}

sail_verified! {
    /// Renumber three original entries in their actual touched LRU order, then
    /// use the original establishing ticket on the same post-renumber slot.
    pub fn original_renumber_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let first = [7u8; 32];
        let mut second = [7u8; 32];
        second[31] = 8;
        let third = [8u8; 32];
        let mut clock = cache_candidate::Clock::start();
        let Some((a, ticket_a)) = clock.entry(first, 11u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((b, _)) = clock.entry(second, 22u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let Some((c, _)) = clock.entry(third, 33u64,
            cache_candidate::SlotState::<u64>::Pending) else { return false; };
        let mut indexed = cache_indexed::IndexedSlots::empty();
        if indexed.insert_new(a).is_err() || indexed.insert_new(b).is_err()
            || indexed.insert_new(c).is_err() { return false; }
        let Some(before_a) = indexed.get(&first) else { return false; };
        sail_proof! { before_a.reticked_fields(100); }
        if indexed.touch(&first, 100) != Some(1) { return false; }
        let Some(before_a) = indexed.get(&first) else { return false; };
        let Some(before_b) = indexed.get(&second) else { return false; };
        let Some(before_c) = indexed.get(&third) else { return false; };
        sail_proof! {
            before_a.reticked_fields(3);
            before_b.reticked_fields(1);
            before_c.reticked_fields(2);
        }
        if indexed.renumber() != 3 { return false; }
        let Some(a) = indexed.get(&first) else { return false; };
        let Some(b) = indexed.get(&second) else { return false; };
        let Some(c) = indexed.get(&third) else { return false; };
        if a.tick() != 3 || b.tick() != 1 || c.tick() != 2
            || *a.value() != 11 || *b.value() != 22 || *c.value() != 33 {
            return false;
        }
        let Some(original) = indexed.take(&first) else { return false; };
        let update = cache_candidate::complete(
            original, &ticket_a, cache_candidate::Completion::Written(9));
        sail_proof! { update.variant_fields(); }
        let cache_candidate::EntryUpdate::Written((key, original)) = update else { return false; };
        if key[31] != 7 || original.tick() != 3 || *original.value() != 11
            || !matches!(original.state(), cache_candidate::SlotState::OnDisk(9)) {
            return false;
        }
        indexed.len() == 2 && indexed.get(&first).is_none()
            && indexed.get(&second).is_some() && indexed.get(&third).is_some()
    }
}

fn main() {
    assert!(original_indexed_control());
    assert!(original_map_control());
    assert!(original_counted_control());
    assert!(original_renumber_control());
}
