//! Simulation of the source-shared production queue and replay operations.
//!
//! The maps interpret owned Rust payloads as exact model bindings/bytes. The
//! shim's parser, key derivation and process observations must establish that
//! interpretation; this module does not replace those obligations with axioms.
use crate::query_core::{BoundPair, CorrelatedQueue, PublicationBatch, PublicationGate, ReplayCursor};
use crate::query_association as model;
use crate::{cache_candidate, cache_indexed};
use vstd::prelude::*;

verus! {

/// Exact pending suffix relation; neither stream may drop an unmatched head.
pub open spec fn queue_refines<E, R>(
    queue: &CorrelatedQueue<E, R>,
    state: model::Pairer,
    binding: spec_fn(E) -> model::Binding,
    bytes: spec_fn(R) -> Seq<u8>,
) -> bool {
    model::pairer_wf(state)
    && queue.halted() == state.stopped
    && queue.expectations().len() + state.paired == state.commands.len()
    && queue.responses().len() + state.paired == state.replies.len()
    && (forall|i: int| 0 <= i < queue.expectations().len() ==>
        binding(queue.expectations()[i]) == state.commands[i + state.paired])
    && (forall|i: int| 0 <= i < queue.responses().len() ==>
        bytes(queue.responses()[i]) == state.replies[i + state.paired])
}

pub fn initial_queue_refines<E, R>(
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
) -> (queue: CorrelatedQueue<E, R>)
    ensures queue_refines(&queue, model::pairer_init(), binding, bytes),
{
    let queue = CorrelatedQueue::empty();
    proof { model::pairer_initially_valid(); }
    queue
}

pub fn expectation_arrival_refines<E, R>(
    queue: &mut CorrelatedQueue<E, R>,
    expectation: E,
    Ghost(state): Ghost<model::Pairer>,
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
)
    requires
        queue_refines(old(queue), state, binding, bytes),
        model::binding_wf(binding(expectation)),
    ensures queue_refines(final(queue), model::enqueue_command(state, binding(expectation)), binding, bytes),
{
    queue.expect(expectation);
    proof {
        model::command_arrival_preserves(state, binding(expectation));
        assert forall|i: int| 0 <= i < queue.expectations().len()
            implies binding(queue.expectations()[i])
                == model::enqueue_command(state, binding(expectation)).commands[i + state.paired] by {
            if i < old(queue).expectations().len() {} else {
                assert(i == old(queue).expectations().len());
            }
        }
    }
}

pub fn reply_arrival_refines<E, R>(
    queue: &mut CorrelatedQueue<E, R>,
    reply: R,
    Ghost(state): Ghost<model::Pairer>,
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
)
    requires queue_refines(old(queue), state, binding, bytes),
    ensures queue_refines(final(queue), model::enqueue_reply(state, bytes(reply)), binding, bytes),
{
    queue.reply(reply);
    proof {
        model::reply_arrival_preserves(state, bytes(reply));
        assert forall|i: int| 0 <= i < queue.responses().len()
            implies bytes(queue.responses()[i])
                == model::enqueue_reply(state, bytes(reply)).replies[i + state.paired] by {
            if i < old(queue).responses().len() {} else {
                assert(i == old(queue).responses().len());
            }
        }
    }
}

/// Calls the actual production removal body and matches the model transition.
pub fn pairing_refines<E, R>(
    queue: &mut CorrelatedQueue<E, R>,
    Ghost(state): Ghost<model::Pairer>,
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
) -> (pair: Option<BoundPair<E, R>>)
    requires queue_refines(old(queue), state, binding, bytes),
    ensures
        pair.is_some() == model::can_pair(state),
        pair.is_some() ==> (
            binding(pair.unwrap().expected()) == state.commands[state.paired as int]
            && bytes(pair.unwrap().response()) == state.replies[state.paired as int]
            && queue_refines(final(queue), model::pair_one(state), binding, bytes)
        ),
        pair.is_none() ==> queue_refines(final(queue), state, binding, bytes),
{
    let pair = queue.take_pair();
    proof {
        if pair.is_some() {
            model::pairing_preserves_order(state);
            let next = model::pair_one(state);
            assert forall|i: int| 0 <= i < queue.expectations().len()
                implies binding(queue.expectations()[i]) == next.commands[i + next.paired] by {
                assert(queue.expectations()[i] == old(queue).expectations()[i + 1]);
            }
            assert forall|i: int| 0 <= i < queue.responses().len()
                implies bytes(queue.responses()[i]) == next.replies[i + next.paired] by {
                assert(queue.responses()[i] == old(queue).responses()[i + 1]);
            }
        }
    }
    pair
}

pub fn stopping_refines<E, R>(
    queue: &mut CorrelatedQueue<E, R>,
    Ghost(state): Ghost<model::Pairer>,
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
)
    requires queue_refines(old(queue), state, binding, bytes),
    ensures queue_refines(final(queue), model::stop_pairer(state), binding, bytes),
{
    queue.stop();
    proof { model::stop_is_permanent(state); }
}

/// The countdown is the model's exact unserved prefix, without wrapping counters.
pub open spec fn replay_refines(cursor: &ReplayCursor, skip: nat, seen: nat) -> bool {
    cursor.left() == if seen < skip { (skip - seen) as nat } else { 0nat }
}

pub fn replay_initially_refines(skip: u64) -> (cursor: ReplayCursor)
    ensures replay_refines(&cursor, skip as nat, 0),
{ ReplayCursor::new(skip) }

pub fn replay_frame_refines(
    cursor: &mut ReplayCursor,
    Ghost(skip): Ghost<nat>,
    Ghost(seen): Ghost<nat>,
) -> (forward: bool)
    requires replay_refines(old(cursor), skip, seen),
    ensures
        forward == (seen >= skip),
        replay_refines(final(cursor), skip, seen + 1),
{ cursor.advance() }

/// Publication preserves all owned payloads and requires the model's exact
/// queue-completion predicate. Physical EOF/exit/diagnostic observations remain
/// inputs supplied by the production adapter.
pub fn publication_refines<T>(
    batch: PublicationBatch<T>,
    gate: PublicationGate,
    Ghost(state): Ghost<model::Pairer>,
    Ghost(entry): Ghost<spec_fn(T) -> model::Entry>,
) -> (published: Option<Vec<T>>)
    requires
        model::pairer_wf(state),
        gate.pairing_active == !state.stopped,
        gate.queues_empty == (state.paired == state.commands.len() && state.paired == state.replies.len()),
        batch.contents().len() == state.entries.len(),
        forall|i: int| 0 <= i < batch.contents().len() ==> entry(batch.contents()[i]) == state.entries[i],
    ensures
        published.is_some() == gate.complete(),
        published.is_some() ==> (
            published.unwrap()@.len() == model::published(state,
                gate.input_closed && gate.normal_exit && gate.quiet && gate.forwarding_ok).len()
            && forall|i: int| 0 <= i < published.unwrap()@.len() ==>
                entry(published.unwrap()@[i]).binding == state.commands[i]
                && entry(published.unwrap()@[i]).reply == state.replies[i]
        ),
{
    let allowed = gate.allows();
    let published = batch.release(allowed);
    proof {
        model::publication_has_ordered_origin(state,
            gate.input_closed && gate.normal_exit && gate.quiet && gate.forwarding_ok);
    }
    published
}


/// Nonempty witness using two different keys/replies and asynchronous arrival.
/// It calls every production mutation involved in pairing and publication.
pub fn distinct_pairs_survive_asynchronous_arrival() -> (result: Vec<(u8, u8)>)
    ensures result@ == seq![(1u8, 10u8), (2u8, 20u8)],
{
    let ghost binding = |key: u8| model::Binding {
        solver: model::example_solver(), mode: crate::context::Mode::Default,
        prefix: seq![key], endpoint: 1,
    };
    let ghost bytes = |reply: u8| seq![reply];
    let ghost s0 = model::pairer_init();
    let mut queue = initial_queue_refines(Ghost(binding), Ghost(bytes));
    // Replies and expectations may arrive independently. An absent expectation
    // cannot consume this response and shift the next pair.
    reply_arrival_refines(&mut queue, 10u8, Ghost(s0), Ghost(binding), Ghost(bytes));
    let ghost s1 = model::enqueue_reply(s0, bytes(10));
    let waiting = pairing_refines(&mut queue, Ghost(s1), Ghost(binding), Ghost(bytes));
    proof { assert(waiting.is_none()); }
    expectation_arrival_refines(&mut queue, 1u8, Ghost(s1), Ghost(binding), Ghost(bytes));
    let ghost s2 = model::enqueue_command(s1, binding(1));
    expectation_arrival_refines(&mut queue, 2u8, Ghost(s2), Ghost(binding), Ghost(bytes));
    let ghost s3 = model::enqueue_command(s2, binding(2));
    reply_arrival_refines(&mut queue, 20u8, Ghost(s3), Ghost(binding), Ghost(bytes));
    let ghost s4 = model::enqueue_reply(s3, bytes(20));
    let first = pairing_refines(&mut queue, Ghost(s4), Ghost(binding), Ghost(bytes));
    let first = match first { Some(pair) => pair.into_parts(), None => return Vec::new() };
    let ghost s5 = model::pair_one(s4);
    let second = pairing_refines(&mut queue, Ghost(s5), Ghost(binding), Ghost(bytes));
    let second = match second { Some(pair) => pair.into_parts(), None => return Vec::new() };
    let ghost s6 = model::pair_one(s5);
    proof {
        assert(binding(first.0) == binding(1));
        assert(binding(first.0).prefix[0] == first.0);
        assert(bytes(first.1)[0] == first.1);
        assert(binding(second.0) == binding(2));
        assert(binding(second.0).prefix[0] == second.0);
        assert(bytes(second.1)[0] == second.1);
        assert(first == (1u8, 10u8)); assert(second == (2u8, 20u8));
    }
    let mut batch = PublicationBatch::empty();
    let accepted_first = batch.push(first, 1, 2);
    let accepted_second = batch.push(second, 1, 2);
    let active = !queue.is_stopped();
    let empty = queue.is_empty();
    let gate = PublicationGate {
        input_closed: true, pairing_active: active, queues_empty: empty,
        normal_exit: true, quiet: true, forwarding_ok: true,
    };
    let ghost entry = |pair: (u8, u8)| model::Entry {
        binding: binding(pair.0), reply: bytes(pair.1),
    };
    proof {
        assert(accepted_first && accepted_second);
        assert forall|i: int| 0 <= i < batch.contents().len()
            implies entry(batch.contents()[i]) == s6.entries[i] by {
            if i == 0 {} else { assert(i == 1); }
        }
    }
    match publication_refines(batch, gate, Ghost(s6), Ghost(entry)) {
        Some(entries) => entries,
        None => Vec::new(),
    }
}

/// A nonempty real batch cannot escape through an incomplete/diagnostic gate.
pub fn noisy_batch_is_refused() {
    let mut batch = PublicationBatch::empty();
    let accepted = batch.push(42u8, 1, 10);
    let gate = PublicationGate {
        input_closed: true, pairing_active: true, queues_empty: true,
        normal_exit: true, quiet: false, forwarding_ok: true,
    };
    let allowed = gate.allows();
    let entries = batch.release(allowed);
    proof { assert(accepted); assert(entries.is_none()); }
}

/// Integrity controls exercise actual owned payload recovery. A header's
/// provenance string remains insufficient to establish a clean solver origin.
pub fn recovery_preserves_bytes_and_refuses_corruption() {
    let key = vec![11u8];
    let digest = vec![22u8];
    let payload = vec![33u8, 44u8];
    let record = crate::query_core::RecoveryRecord::new(7u8, payload, 2, key, digest);
    let recovered = record.recover(2, &[11u8], &[22u8]);
    proof {
        assert(match recovered { Ok((metadata, reply)) => metadata == 7 && reply@ == seq![33u8, 44u8], _ => false });
    }
    let corrupt = crate::query_core::RecoveryRecord::new(8u8, vec![55u8], 2, vec![11u8], vec![22u8]);
    let rejected = corrupt.recover(2, &[11u8], &[99u8]);
    proof { assert(rejected == Err(crate::query_core::RecoveryRejection::HashMismatch)); }
}


/// Releasing held checks may permute the canonical command order. Validate a
/// full bijection and preserve the immutable pair at every mapped position.
/// Silence certificates have no representation in this pair-only batch type.
pub fn permutation_publication_refines<E, R>(
    batch: PublicationBatch<BoundPair<E, R>>,
    gate: PublicationGate,
    forward: &[usize], inverse: &[usize],
    Ghost(state): Ghost<model::Pairer>,
    Ghost(binding): Ghost<spec_fn(E) -> model::Binding>,
    Ghost(bytes): Ghost<spec_fn(R) -> Seq<u8>>,
) -> (published: Option<Vec<BoundPair<E, R>>>)
    requires
        model::pairer_wf(state),
        gate.pairing_active == !state.stopped,
        gate.queues_empty == (state.paired == state.commands.len() && state.paired == state.replies.len()),
        batch.contents().len() == state.entries.len(),
        forward.len() == batch.contents().len(),
        forall|i: int| 0 <= i < batch.contents().len() ==>
            forward[i] < state.entries.len()
            && binding(batch.contents()[i].expected()) == state.entries[forward[i] as int].binding
            && bytes(batch.contents()[i].response()) == state.entries[forward[i] as int].reply,
    ensures
        published.is_some() == (gate.complete()
            && crate::inputs_core::valid_permutation(forward@, inverse@, state.entries.len())),
        published.is_some() ==> (
            published.unwrap()@ == batch.contents()
            && published.unwrap()@.len() == state.entries.len()
            && forall|i: int| 0 <= i < published.unwrap()@.len() ==>
                forward[i] < state.entries.len()
                && binding(published.unwrap()@[i].expected()) == state.commands[forward[i] as int]
                && bytes(published.unwrap()@[i].response()) == state.replies[forward[i] as int]
        ),
        published.is_some() ==> forall|j: int| #![trigger state.entries[j]] 0 <= j < state.entries.len() ==>
            exists|i: int| 0 <= i < published.unwrap()@.len() && forward[i] == j
                && binding(published.unwrap()@[i].expected()) == state.entries[j].binding
                && bytes(published.unwrap()@[i].response()) == state.entries[j].reply,
{
    let valid = crate::inputs_core::permutation(forward, inverse, batch.entries_count());
    let allowed = gate.allows();
    let published = batch.release(allowed && valid);
    proof {
        if published.is_some() {
            assert forall|j: int| #![trigger state.entries[j]] 0 <= j < state.entries.len() implies
                exists|i: int| 0 <= i < published.unwrap()@.len() && forward[i] == j
                    && binding(published.unwrap()@[i].expected()) == state.entries[j].binding
                    && bytes(published.unwrap()@[i].response()) == state.entries[j].reply by {
                assert(crate::inputs_core::permutation_at(forward@, inverse@, state.entries.len(), j));
                let i = inverse[j] as int;
                assert(0 <= i < published.unwrap()@.len());
                assert(forward[i] == j);
            }
        }
    }
    published
}

/// Nonempty held-check order [1,3,2,4] preserves four distinct paired replies.
pub fn held_release_permutation_is_nonempty() -> (published: Vec<BoundPair<u8, u8>>)
    ensures published@.len() == 4,
        forall|i: int| 0 <= i < 4 ==>
            published@[i].expected() == seq![1u8, 3u8, 2u8, 4u8][i]
            && published@[i].response() == seq![10u8, 30u8, 20u8, 40u8][i],
{
    let ghost binding = |key: u8| model::Binding {
        solver: model::example_solver(), mode: crate::context::Mode::Default,
        prefix: seq![key], endpoint: 1,
    };
    let ghost bytes = |reply: u8| seq![reply];
    let mut queue = CorrelatedQueue::empty();
    queue.expect(1u8); queue.expect(2u8); queue.expect(3u8); queue.expect(4u8);
    queue.reply(10u8); queue.reply(20u8); queue.reply(30u8); queue.reply(40u8);
    let one = match queue.take_pair() { Some(pair) => pair, None => return Vec::new() };
    let two = match queue.take_pair() { Some(pair) => pair, None => return Vec::new() };
    let three = match queue.take_pair() { Some(pair) => pair, None => return Vec::new() };
    let four = match queue.take_pair() { Some(pair) => pair, None => return Vec::new() };
    let mut batch = PublicationBatch::empty();
    let a = batch.push(one, 1, 4);
    let b = batch.push(three, 1, 4);
    let c = batch.push(two, 1, 4);
    let d = batch.push(four, 1, 4);
    let order = [0usize, 2usize, 1usize, 3usize];
    let inverse = [0usize, 2usize, 1usize, 3usize];
    let gate = PublicationGate { input_closed: true, pairing_active: true, queues_empty: true,
        normal_exit: true, quiet: true, forwarding_ok: true };
    let ghost state = model::Pairer {
        commands: seq![binding(1), binding(2), binding(3), binding(4)],
        replies: seq![bytes(10), bytes(20), bytes(30), bytes(40)],
        paired: 4,
        entries: seq![
            model::Entry { binding: binding(1), reply: bytes(10) },
            model::Entry { binding: binding(2), reply: bytes(20) },
            model::Entry { binding: binding(3), reply: bytes(30) },
            model::Entry { binding: binding(4), reply: bytes(40) }],
        stopped: false,
    };
    proof {
        assert(a && b && c && d);
        assert forall|i: int| 0 <= i < 4 implies model::binding_wf(state.commands[i]) by {
            if i == 0 {} else if i == 1 {} else if i == 2 {} else { assert(i == 3); }
        }
        assert forall|i: int| 0 <= i < 4 implies
            state.entries[i].binding == state.commands[i] && state.entries[i].reply == state.replies[i] by {
            if i == 0 {} else if i == 1 {} else if i == 2 {} else { assert(i == 3); }
        }
        assert forall|i: int| 0 <= i < 4 implies order@[i] < 4
            && binding(batch.contents()[i].expected()) == state.entries[order@[i] as int].binding
            && bytes(batch.contents()[i].response()) == state.entries[order@[i] as int].reply by {
            if i == 0 {} else if i == 1 {} else if i == 2 {} else { assert(i == 3); }
        }
    }
    match permutation_publication_refines(batch, gate, &order, &inverse,
        Ghost(state), Ghost(binding), Ghost(bytes)) {
        Some(published) => published,
        None => Vec::new(),
    }
}

}


// Existing constructive cache witnesses retained from the frozen ownership core.
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
