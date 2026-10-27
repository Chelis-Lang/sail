//! Exact operations on the actual candidate-cache ordered index.
//!
//! These preserve B-tree contents/order; they do not establish the global
//! slot/accounting relation, Mutex ownership or solver origin.
use std::collections::BTreeMap;
#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::std_specs::cmp::OrdSpec;
#[cfg(sail_verus)]
use vstd::std_specs::iter::IteratorSpec;

sail_verified! {
    /// Transfer the actual old index, leaving an actually empty index behind.
    pub fn take_order(order: &mut BTreeMap<u64, [u8; 32]>)
        -> (out: BTreeMap<u64, [u8; 32]>)
        requires (true)
        ensures (out@ == old(order)@, final(order)@.is_empty())
    {
        let mut original = BTreeMap::new();
        std::mem::swap(order, &mut original);
        original
    }
}

sail_verified! {
    /// Remove and return the exact least original tick/full key. An empty index
    /// returns None unchanged; no separately supplied selection is trusted.
#[allow(clippy::single_match_else, reason = "The explicit branches retain the checked iterator witnesses and full-body runtime controls without changing native operations.")]
pub fn pop_oldest(order: &mut BTreeMap<u64, [u8; 32]>)
    -> (out: Option<(u64, [u8; 32])>)
    requires (true)
    ensures (
        match out {
            None => old(order)@.is_empty() && final(order)@ == old(order)@,
            Some((tick, key)) => old(order)@.contains_key(tick)
                && key == old(order)@[tick]
                && final(order)@ == old(order)@.remove(tick)
                && forall|other: u64| old(order)@.contains_key(other) ==> tick <= other,
        })
{
    sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
    let selected = {
        let mut values = order.iter();
        sail_snapshot!(pairs = values.remaining());
        sail_snapshot!(ticks = pairs.map_values(|pair: (&u64, &[u8; 32])| *pair.0));
        sail_proof! {
            assert(pairs.len() == order@.dom().len());
            assert(vstd::std_specs::btree::increasing_seq(
                pairs.map_values(|pair: (&u64, &[u8; 32])| *pair.0)));
            vstd::std_specs::btree::axiom_increasing_seq_meaning(
                pairs.map_values(|pair: (&u64, &[u8; 32])| *pair.0));
        }
        match values.next() {
            Some((tick, key)) => {
                sail_proof! {
                    assert(pairs.len() > 0);
                    assert((*tick, *key) == (*pairs[0].0, *pairs[0].1));
                    assert forall|other: u64| order@.contains_key(other)
                        implies *tick <= other by {
                        assert(pairs.contains((&other, &order@[other])));
                        let i = choose|i: int| 0 <= i < pairs.len()
                            && pairs[i] == (&other, &order@[other]);
                        if i > 0 {
                            assert(ticks[0] == *pairs[0].0);
                            assert(ticks[i] == *pairs[i].0);
                            assert(ticks[0].cmp_spec(&ticks[i]) is Less);
                            assert(*pairs[0].0 < *pairs[i].0);
                        }
                    }
                }
                Some((*tick, *key))
            }
            None => {
                sail_proof! { assert(pairs.len() == 0); }
                None
            }
        }
    };
    if let Some((tick, key)) = selected {
        order.remove(&tick);
        Some((tick, key))
    } else {
        None
    }
}
}

sail_verified! {
    /// A nonempty original index transfers intact and retires in increasing
    /// tick order, including equal-prefix full keys with different tails.
    pub fn original_order_control() -> (out: bool)
        requires (true) ensures (out)
    {
        sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
        let first = [7u8; 32];
        let mut neighbor = [7u8; 32];
        neighbor[31] = 8;
        let mut order = BTreeMap::new();
        order.insert(30u64, neighbor);
        order.insert(10u64, first);
        let mut original = take_order(&mut order);
        if !order.is_empty() { return false; }
        sail_snapshot!(initial = original);
        let Some((first_tick, first_key)) = pop_oldest(&mut original) else { return false; };
        sail_proof! {
            assert(initial@.contains_key(10u64));
            assert(first_tick <= 10u64);
            assert(first_tick == 10u64);
            assert(first_key == first);
        }
        if first_tick != 10 || first_key[31] != 7 { return false; }
        let Some((next_tick, next_key)) = pop_oldest(&mut original) else { return false; };
        if next_tick != 30 || next_key[31] != 8 { return false; }
        pop_oldest(&mut original).is_none()
    }
}
