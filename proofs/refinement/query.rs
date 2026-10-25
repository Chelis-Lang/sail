//! Simulation of the source-shared production queue and replay operations.
//!
//! The maps interpret owned Rust payloads as exact model bindings/bytes. The
//! shim's parser, key derivation and process observations must establish that
//! interpretation; this module does not replace those obligations with axioms.
use crate::query_core::{BoundPair, CorrelatedQueue, PublicationBatch, PublicationGate, ReplayCursor};
use crate::query_association as model;
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
