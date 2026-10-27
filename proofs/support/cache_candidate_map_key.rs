//! Unrun full-file follow-up separating the key-model premise from unsupported
//! direct HashMap methods. This introduces no specification or assumption and
//! does not select a production map representation or solver-origin architecture.
#![forbid(unsafe_code)]
#![allow(unused_imports)]
use std::collections::HashMap;
use std::collections::hash_map::Entry as MapEntry;
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
/// A requested library fact, not a precondition supplied to the caller.
pub proof fn original_byte_array_hash_key_model()
    ensures vstd::std_specs::hash::obeys_key_model::<[u8; 32]>(),
{
}

/// Actual insert's useful map effect must hold without granting a key premise.
pub fn byte_key_insert<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32], value: Slot<Value, Stamp>) -> (out: Option<Slot<Value, Stamp>>)
    ensures final(slots)@ == old(slots)@.insert(key, value),
        match out {
            Some(original) => old(slots)@.contains_key(key) && original == old(slots)@[key],
            None => !old(slots)@.contains_key(key),
        },
{
    slots.insert(key, value)
}

/// Actual borrowed get must establish membership and original payload equality.
pub fn byte_key_get<'a, Value, Stamp>(slots: &'a HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
    ensures match out {
        Some(original) => slots@.contains_key(key) && *original == slots@[key],
        None => !slots@.contains_key(key),
    },
{
    slots.get(&key)
}

/// Already specified remove still has to establish the real byte-key result.
pub fn byte_key_remove<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32]) -> (out: Option<Slot<Value, Stamp>>)
    ensures final(slots)@ == old(slots)@.remove(key),
        match out {
            Some(original) => old(slots)@.contains_key(key) && original == old(slots)@[key],
            None => !old(slots)@.contains_key(key),
        },
{
    slots.remove(&key)
}

/// The library's occupied-entry API preserves the actual entry; it must not
/// silently assume that selecting it establishes a byte-key map correspondence.
pub fn byte_key_entry_remove<Value, Stamp>(slots: &mut HashMap<[u8; 32], Slot<Value, Stamp>>,
    key: [u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
    ensures final(slots)@ == old(slots)@.remove(key),
        match out {
            Some((original_key, original)) => original_key == key
                && old(slots)@.contains_key(key) && original == old(slots)@[key],
            None => !old(slots)@.contains_key(key),
        },
{
    match slots.entry(key) {
        MapEntry::Occupied(entry) => Some(entry.remove_entry()),
        MapEntry::Vacant(_) => None,
    }
}

/// Nonempty primitive-key positive control, with the same library/flags.
pub fn primitive_key_control() -> (out: bool)
    ensures out,
{
    let mut slots = HashMap::<u64, u64>::new();
    slots.insert(7, 11);
    let Some(original) = slots.get(&7) else { return false; };
    if *original != 11 { return false; }
    matches!(slots.remove(&7), Some(11))
}
}

fn main() {}
