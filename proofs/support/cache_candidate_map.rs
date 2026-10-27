//! Unrun whole-file support probe for the actual native byte-key/map surface.
//! Generic Value/Stamp are the production Slot's actual type parameters, not a
//! second ledger. This establishes neither native Mutex/history nor solver origin.
//! In particular no new key-model assumption or specification is introduced.
#![forbid(unsafe_code)]
#![allow(unused_imports)]
use std::collections::HashMap;
use vstd::prelude::*;
#[macro_use]
#[path = "../../crates/sail-acceptance/src/syntax.rs"]
mod syntax;
#[path = "../../crates/sail-acceptance/src/cache_candidate.rs"]
pub mod cache_candidate;
#[path = "../../crates/sail-acceptance/src/inputs.rs"]
pub mod inputs;

use cache_candidate::{Entry, Slot};

verus! {
/// A requested library prerequisite, not an assumption granted to production.
pub proof fn original_byte_array_hash_key_model()
    ensures vstd::std_specs::hash::obeys_key_model::<[u8; 32]>(),
{
}

/// The exact production remove_entry call over the actual shared Slot shape.
pub fn actual_remove_entry<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
    ensures match out {
        Some((original_key, original_slot)) => original_key == key
            && old(slots)@.contains_key(key) && old(slots)@[key] == original_slot
            && final(slots)@ == old(slots)@.remove(key),
        None => !old(slots)@.contains_key(key) && final(slots)@ == old(slots)@,
    },
{
    slots.remove_entry(&key)
}

/// The production get_mut/touch path must establish membership, not merely typecheck.
pub fn actual_get_mut<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32], tick: u64) -> (out: bool)
    ensures out == old(slots)@.contains_key(key),
{
    if let Some(slot) = slots.get_mut(&key) {
        slot.touch(tick);
        true
    } else {
        false
    }
}

/// Avoiding remove_entry does not silently discharge the same byte-key premise.
pub fn actual_remove<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32]) -> (out: Option<Slot<Value, Stamp>>)
    ensures final(slots)@ == old(slots)@.remove(key),
        match out {
            Some(original) => old(slots)@.contains_key(key) && original == old(slots)@[key],
            None => !old(slots)@.contains_key(key),
        },
{
    slots.remove(&key)
}

/// Nonempty primitive-key positive control for existing library specifications.
pub fn primitive_key_control() -> (out: bool)
    ensures out,
{
    let mut slots = HashMap::<u64, u64>::new();
    slots.insert(7, 11);
    matches!(slots.remove(&7), Some(11))
}
}

fn main() {}
