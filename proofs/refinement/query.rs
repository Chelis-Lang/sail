//! Simulation of the source-shared production queue and replay operations.
//!
//! The maps interpret owned Rust payloads as exact model bindings/bytes. The
//! shim's parser, key derivation and process observations must establish that
//! interpretation; this module does not replace those obligations with axioms.
use crate::query_core::{BoundPair, CorrelatedQueue, PublicationBatch, PublicationGate, PrefixFeed, RawTranscript, ReplayCursor};
use crate::query_association as model;
use vstd::prelude::*;

verus! {

/// An inductive history of actual comparison inputs, distinct from the stdout
/// cursor's frame count and from any solver-semantic equivalence relation.
pub open spec fn comparisons_match<Owner>(
    replay: &crate::query_replay::Replay<'_,Owner>, actual: Seq<Seq<u8>>,
) -> bool {
    replay.well_formed() && replay.checked_view() == actual.len()
        && forall|i:int| 0 <= i < actual.len() ==>
            replay.served_view()[i].reproduces(actual[i])
}

pub fn initial_comparison<'a,Owner>(served: &'a [crate::query_replay::ServedReply], owner: Owner)
    -> (replay: crate::query_replay::Replay<'a,Owner>)
    ensures comparisons_match(&replay,Seq::empty()), replay.owner_view() == owner,
{
    crate::query_replay::Replay::new(served,owner)
}

/// Calls the actual shared mechanical comparison and retains each successful
/// fresh byte input at its original served position in the inductive relation.
pub fn comparison_step<Owner>(replay: &mut crate::query_replay::Replay<'_,Owner>,
    actual: &[u8], Ghost(previous): Ghost<Seq<Seq<u8>>>) -> (accepted: bool)
    requires comparisons_match(old(replay),previous),
    ensures accepted ==> comparisons_match(final(replay),previous.push(actual@)),
        !accepted ==> comparisons_match(final(replay),previous),
        final(replay).owner_view() == old(replay).owner_view(),
{
    let accepted = replay.compare(actual);
    proof {
        if accepted {
            assert forall|i:int| 0 <= i < previous.push(actual@).len()
                implies replay.served_view()[i].reproduces(previous.push(actual@)[i]) by {
                if i < previous.len() {} else { assert(i == previous.len()); }
            }
        }
    }
    accepted
}

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


/// Exact received bytes and the already-fed contiguous prefix, never a digest.
pub open spec fn transcript_refines(raw: &RawTranscript, received: Seq<u8>, fed: Seq<u8>) -> bool {
    raw.view() == received && raw.fed_view() == fed.len()
        && fed.len() <= received.len() && fed == received.take(fed.len() as int)
}

pub fn transcript_initially_refines() -> (raw: RawTranscript)
    ensures transcript_refines(&raw, Seq::empty(), Seq::empty()), raw.active(),
{
    let raw = RawTranscript::empty();
    proof { assert(raw.view().take(0) =~= Seq::<u8>::empty()); }
    raw
}

/// Arbitrary input fragmentation extends only received bytes; it cannot drop,
/// reorder or re-feed an already consumed prefix.
pub fn transcript_append_refines(raw: &mut RawTranscript, bytes: &[u8],
    Ghost(received): Ghost<Seq<u8>>, Ghost(fed): Ghost<Seq<u8>>)
    requires transcript_refines(old(raw), received, fed),
    ensures transcript_refines(final(raw), received + bytes@, fed),
        final(raw).active() == old(raw).active(),
{
    raw.append(bytes);
    proof { assert((received + bytes@).take(fed.len() as int) =~= received.take(fed.len() as int)); }
}

/// The returned model binding is constructed from the actual accepted bytes.
/// Caller context metadata is retained exactly; matching it to the launched
/// solver/header remains a separate process/encoder refinement obligation.
pub fn command_bytes_refine_binding<'a>(raw: &'a mut RawTranscript,
    offset: usize, command: &[u8], Ghost(received): Ghost<Seq<u8>>,
    Ghost(fed): Ghost<Seq<u8>>, Ghost(context): Ghost<crate::context::ModelContext>)
    -> (result: (Option<PrefixFeed<'a>>, Ghost<Option<model::Binding>>))
    requires transcript_refines(old(raw), received, fed),
    ensures result.0.is_some() == result.1@.is_some(),
        result.0.is_some() == (old(raw).active() && command@.len() > 0
            && old(raw).fed_view() <= offset
            && offset as nat + command@.len() <= old(raw).view().len()
            && old(raw).view().subrange(offset as int, offset as int + command@.len() as int) == command@
            && crate::query_core::trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)),
        result.0.is_some() ==> (
            final(raw).active()
            && model::binding_wf(result.1@.unwrap())
            && result.1@.unwrap().solver == context.solver
            && result.1@.unwrap().mode == context.mode
            && result.1@.unwrap().endpoint == offset as nat + command@.len()
            && result.1@.unwrap().prefix == received.take(result.1@.unwrap().endpoint as int)
            && fed + result.0.unwrap().view() == result.1@.unwrap().prefix
            && transcript_refines(final(raw), received, result.1@.unwrap().prefix)
        ),
        result.0.is_none() ==> (!final(raw).active()
            && transcript_refines(final(raw), received, fed)),
{
    let feed = raw.command(offset, command);
    let ghost binding = if feed.is_some() {
        Some(model::Binding { solver: context.solver, mode: context.mode,
            prefix: fed + feed.unwrap().view(), endpoint: feed.unwrap().to() })
    } else { None };
    proof {
        if feed.is_some() {
            let end = feed.unwrap().to();
            assert(fed + feed.unwrap().view() =~= received.take(end as int));
        }
    }
    (feed, Ghost(binding))
}

/// The final feed completes the exact same raw bytes used by solver replay.
pub fn tail_bytes_complete_replay<'a>(raw: &'a mut RawTranscript,
    Ghost(received): Ghost<Seq<u8>>, Ghost(fed): Ghost<Seq<u8>>)
    -> (feed: Option<PrefixFeed<'a>>)
    requires transcript_refines(old(raw), received, fed),
    ensures feed.is_some() == (old(raw).active() && old(raw).fed_view() <= old(raw).view().len()),
        feed.is_some() ==> fed + feed.unwrap().view() == received,
        feed.is_some() ==> transcript_refines(final(raw), received, received),
        feed.is_none() ==> !final(raw).active(),
{
    let feed = raw.tail();
    proof {
        if feed.is_some() {
            assert(fed + feed.unwrap().view() =~= received);
            assert(received.take(received.len() as int) =~= received);
        }
    }
    feed
}

/// Nonempty comments, CR/LF, quoted parentheses and fragmented commands all
/// reach the hash stream unchanged, in the exact order used for solver replay.
pub fn fragmented_prefixes_preserve_all_bytes() -> (hashed: Vec<u8>)
    ensures hashed@ == seq![b' ', b';', b'h', b'e', b'a', b'd', b'\r', b'\n', b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'a', b';', b'(', b')', b'"', b')', b'\t', b';', b'g', b'a', b'p', b'\n', b'(', b'c', b'h', b'e', b'c', b'k', b'-', b's', b'a', b't', b')', b'\n', b';', b't', b'a', b'i', b'l'],
{
    let mut raw = transcript_initially_refines();
    let mut hashed = Vec::new();
    let ghost context = model::example_context(1, 2);
    let first = vec![b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'a', b';', b'(', b')', b'"', b')'];
    let second = vec![b'(', b'c', b'h', b'e', b'c', b'k', b'-', b's', b'a', b't', b')'];
    let chunk1 = vec![b' ', b';', b'h', b'e', b'a', b'd', b'\r'];
    let chunk2 = vec![b'\n', b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'a', b';', b'(', b')', b'"', b')'];
    let chunk3 = vec![b'\t', b';', b'g', b'a', b'p', b'\n', b'(', b'c', b'h', b'e', b'c', b'k', b'-'];
    let chunk4 = vec![b's', b'a', b't', b')', b'\n', b';', b't', b'a', b'i', b'l'];
    transcript_append_refines(&mut raw, &chunk1, Ghost(Seq::<u8>::empty()), Ghost(Seq::<u8>::empty()));
    let ghost r1 = chunk1@;
    transcript_append_refines(&mut raw, &chunk2, Ghost(r1), Ghost(Seq::<u8>::empty()));
    let ghost r2 = r1 + chunk2@;
    proof {
        reveal_with_fuel(crate::query_core::trivia_after, 20);
        assert(r2.subrange(8, 8 + first@.len() as int) =~= first@);
    }
    let (feed, Ghost(binding)) = command_bytes_refine_binding(&mut raw, 8, &first,
        Ghost(r2), Ghost(Seq::<u8>::empty()), Ghost(context));
    let feed = match feed { Some(feed) => feed, None => return Vec::new() };
    hashed.extend_from_slice(feed.bytes());
    let ghost fed1 = binding.unwrap().prefix;
    proof { assert(fed1 =~= r2); assert(hashed@ == fed1); }
    transcript_append_refines(&mut raw, &chunk3, Ghost(r2), Ghost(fed1));
    let ghost r3 = r2 + chunk3@;
    transcript_append_refines(&mut raw, &chunk4, Ghost(r3), Ghost(fed1));
    let ghost r4 = r3 + chunk4@;
    let offset = 8 + first.len() + 6;
    proof {
        reveal_with_fuel(crate::query_core::trivia_after, 20);
        assert(r4.subrange(offset as int, offset as int + second@.len() as int) =~= second@);
    }
    let (feed, Ghost(binding)) = command_bytes_refine_binding(&mut raw, offset, &second,
        Ghost(r4), Ghost(fed1), Ghost(context));
    let feed = match feed { Some(feed) => feed, None => return Vec::new() };
    hashed.extend_from_slice(feed.bytes());
    let ghost fed2 = binding.unwrap().prefix;
    proof { assert(hashed@ == fed2); }
    let tail = tail_bytes_complete_replay(&mut raw, Ghost(r4), Ghost(fed2));
    let tail = match tail { Some(tail) => tail, None => return Vec::new() };
    hashed.extend_from_slice(tail.bytes());
    proof { assert(hashed@ == r4); assert(r4 =~= seq![b' ', b';', b'h', b'e', b'a', b'd', b'\r', b'\n', b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'a', b';', b'(', b')', b'"', b')', b'\t', b';', b'g', b'a', b'p', b'\n', b'(', b'c', b'h', b'e', b'c', b'k', b'-', b's', b'a', b't', b')', b'\n', b';', b't', b'a', b'i', b'l']); }
    hashed
}

/// A skipped nontrivia command, wrong item bytes or overlap cannot be relabeled
/// as a clean prefix, and appending later input cannot restore validity.
pub fn malformed_prefix_candidates_are_permanently_refused() {
    let prefix = vec![b'(', b's', b'e', b't', b'-', b'o', b'p', b't', b'i', b'o', b'n', b' ', b':', b'p', b'r', b'i', b'n', b't', b'-', b's', b'u', b'c', b'c', b'e', b's', b's', b' ', b'f', b'a', b'l', b's', b'e', b')', b'\n'];
    let command = vec![b'(', b'c', b'h', b'e', b'c', b'k', b'-', b's', b'a', b't', b')'];
    let mut omitted = RawTranscript::empty();
    omitted.append(&prefix);
    omitted.append(&command);
    let offset = prefix.len();
    proof { reveal_with_fuel(crate::query_core::trivia_after, 2); }
    let rejected = omitted.command(offset, &command);
    proof { assert(rejected.is_none()); }
    let newline = vec![b'\n'];
    omitted.append(&newline);
    let again = omitted.command(0, &prefix);
    proof { assert(again.is_none()); assert(!omitted.active()); }
    let mut changed = RawTranscript::empty();
    changed.append(&command);
    let wrong = vec![b'(', b'c', b'h', b'e', b'c', b'k', b'-', b'b', b'a', b'd', b')'];
    let rejected = changed.command(0, &wrong);
    proof { assert(rejected.is_none()); }
    let mut overflow = RawTranscript::empty();
    let byte = vec![b'x'];
    overflow.append(&byte);
    let rejected = overflow.command(usize::MAX, &byte);
    proof { assert(rejected.is_none()); }
}


/// A first-newline partition is unique even when the reply contains newlines.
/// This relates actual byte framing; it grants no meaning to header metadata.
pub proof fn record_partition_unique(header: Seq<u8>, reply: Seq<u8>,
    decoded_header: Seq<u8>, decoded_reply: Seq<u8>)
    requires crate::query_core::record_header(header),
        crate::query_core::record_header(decoded_header),
        header + seq![b'\n'] + reply == decoded_header + seq![b'\n'] + decoded_reply,
    ensures decoded_header == header, decoded_reply == reply,
{
    let whole = header + seq![b'\n'] + reply;
    if header.len() < decoded_header.len() {
        assert(whole[header.len() as int] == b'\n');
        assert(whole[header.len() as int] == decoded_header[header.len() as int]);
    } else if decoded_header.len() < header.len() {
        assert(whole[decoded_header.len() as int] == b'\n');
        assert(whole[decoded_header.len() as int] == header[decoded_header.len() as int]);
    }
    assert(header.len() == decoded_header.len());
    assert(whole.take(header.len() as int) =~= header);
    assert(whole.take(decoded_header.len() as int) =~= decoded_header);
    assert(header == decoded_header);
    assert(whole.subrange(header.len() as int + 1, whole.len() as int) =~= reply);
    assert(whole.subrange(decoded_header.len() as int + 1, whole.len() as int) =~= decoded_reply);
    assert(reply == decoded_reply);
}

/// The actual writer encoding and actual reader split preserve arbitrary payload
/// bytes. Metadata interpretation, digest computation and OS writes stay outside.
pub fn record_bytes_roundtrip(header: Vec<u8>, reply: &[u8])
    -> (decoded: Option<(Vec<u8>, Vec<u8>)>)
    ensures decoded.is_some() == crate::query_core::record_header(header@),
        decoded.is_some() ==> decoded.unwrap().0@ == header@
            && decoded.unwrap().1@ == reply@,
{
    let ghost original_header = header@;
    let encoded = crate::query_core::encode_record(header, reply);
    let encoded = match encoded { Some(encoded) => encoded, None => return None };
    let parts = crate::query_core::split_record(&encoded);
    proof { assert(!crate::query_core::record_header(encoded@)) by {
        assert(encoded@[original_header.len() as int] == b'\n');
    } }
    let (header, decoded_reply) = match parts { Some(parts) => parts, None => return None };
    proof { record_partition_unique(original_header, reply@, header@, decoded_reply@); }
    let mut header_copy = Vec::new();
    header_copy.extend_from_slice(header);
    let mut reply_copy = Vec::new();
    reply_copy.extend_from_slice(decoded_reply);
    Some((header_copy, reply_copy))
}

/// Nonempty multiline/binary reply survives exact framing; an embedded newline
/// in a purported serialized header is refused before it can relabel payloads.
pub fn record_payload_and_header_refusal_witness() {
    let header = vec![b'{', b'}'];
    let reply = vec![b'u', b'n', b's', b'a', b't', b'\n', 0u8, 255u8];
    let decoded = record_bytes_roundtrip(header, &reply);
    proof {
        assert(decoded.is_some());
        assert(decoded.unwrap().0@ == seq![b'{', b'}']);
        assert(decoded.unwrap().1@ == reply@);
    }
    let malformed_header = vec![b'{', b'\n', b'}'];
    let refused = crate::query_core::encode_record(malformed_header, &reply);
    proof { assert(refused.is_none()); }
    let unterminated = vec![b'{', b'}'];
    let refused = crate::query_core::split_record(&unterminated);
    proof { assert(refused.is_none()); }
}


/// A completed raw prefix resets lexical nesting before the next exact bytes.
pub proof fn scan_reset_join(left: Seq<u8>, right: Seq<u8>, comments: bool)
    requires crate::query_core::frame_scan(left, comments) == (0, crate::query_core::FrameLex::Plain),
    ensures crate::query_core::frame_scan(left + right, comments) == crate::query_core::frame_scan(right, comments),
    decreases right.len(),
{
    if right.len() == 0 { assert(left + right =~= left); }
    else {
        scan_reset_join(left, right.drop_last(), comments);
        assert((left + right).drop_last() =~= left + right.drop_last());
        assert((left + right).last() == right.last());
        reveal_with_fuel(crate::query_core::frame_scan, 1);
    }
}

/// Actual semicolon-trivia bytes preserve zero nesting, including quoted or
/// parenthesis bytes hidden in comments and CR/LF split across fragments.
pub proof fn trivia_framing_state(bytes: Seq<u8>, comment: bool)
    requires crate::query_core::trivia_after(bytes, false) == Some(comment),
    ensures crate::query_core::frame_scan(bytes, true)
        == (0, if comment { crate::query_core::FrameLex::Comment } else { crate::query_core::FrameLex::Plain }),
    decreases bytes.len(),
{
    if bytes.len() > 0 {
        let prefix = bytes.drop_last();
        let byte = bytes.last();
        crate::query_core::trivia_join(prefix, seq![byte], false);
        assert(prefix + seq![byte] =~= bytes);
        match crate::query_core::trivia_after(prefix, false) {
            Some(previous) => {
                trivia_framing_state(prefix, previous);
                reveal_with_fuel(crate::query_core::trivia_after, 2);
                reveal_with_fuel(crate::query_core::frame_scan, 1);
            },
            None => {},
        }
    }
}

/// Two forms cannot be substituted for one admitted command/reply frame.
pub proof fn complete_forms_cannot_coalesce(first: Seq<u8>, suffix: Seq<u8>, comments: bool)
    requires crate::query_core::single_list(first, comments), suffix.len() > 0,
    ensures !crate::query_core::single_list(first + suffix, comments),
{
    assert((first + suffix).take(first.len() as int) =~= first);
    if crate::query_core::single_list(first + suffix, comments) {
        assert(crate::query_core::frame_scan((first + suffix).take(first.len() as int), comments).0 > 0);
    }
}

/// The actual framed-command operation extends the exact model binding and
/// preserves the inductive top-level lexical boundary of the entire prefix.
pub fn framed_prefix_refines_binding<'a>(raw: &'a mut RawTranscript, offset: usize,
    command: &'a [u8], Ghost(received): Ghost<Seq<u8>>, Ghost(fed): Ghost<Seq<u8>>,
    Ghost(context): Ghost<crate::context::ModelContext>)
    -> (result: (Option<(PrefixFeed<'a>, &'a [u8])>, Ghost<Option<model::Binding>>))
    requires transcript_refines(old(raw), received, fed),
        crate::query_core::frame_scan(fed, true) == (0, crate::query_core::FrameLex::Plain),
    ensures result.0.is_some() == result.1@.is_some(),
        result.0.is_some() ==> (
            model::binding_wf(result.1@.unwrap())
            && result.1@.unwrap().solver == context.solver && result.1@.unwrap().mode == context.mode
            && result.1@.unwrap().endpoint == offset as nat + command@.len()
            && result.1@.unwrap().prefix == received.take(result.1@.unwrap().endpoint as int)
            && fed + result.0.unwrap().0.view() == result.1@.unwrap().prefix
            && transcript_refines(final(raw), received, result.1@.unwrap().prefix)
            && final(raw).active() && crate::query_core::single_list(command@, true)
            && crate::query_core::frame_scan(result.1@.unwrap().prefix, true) == (0, crate::query_core::FrameLex::Plain)
            && exists|a: int, b: int| crate::query_core::head_span(command@, a, b)
                && result.0.unwrap().1@ == command@.subrange(a, b)),
        result.0.is_none() ==> !final(raw).active() && transcript_refines(final(raw), received, fed),
        old(raw).active() && crate::query_core::single_list(command@, true) && crate::query_core::symbol_first(command@[1])
            && fed.len() <= offset && offset as nat + command@.len() <= received.len()
            && received.subrange(offset as int, offset as int + command@.len() as int) == command@
            && crate::query_core::trivia_after(received.subrange(fed.len() as int, offset as int), false) == Some(false)
            ==> result.0.is_some(),
{
    let accepted = crate::query_core::framed_command(raw, offset, command);
    let ghost binding = if accepted.is_some() {
        Some(model::Binding { solver: context.solver, mode: context.mode,
            prefix: fed + accepted.unwrap().0.view(), endpoint: accepted.unwrap().0.to() })
    } else { None };
    proof {
        if accepted.is_some() {
            let gap = received.subrange(fed.len() as int, offset as int);
            trivia_framing_state(gap, false);
            scan_reset_join(fed, gap, true);
            scan_reset_join(fed + gap, command@, true);
            assert(binding.unwrap().prefix =~= fed + gap + command@);
            assert(binding.unwrap().prefix =~= received.take(binding.unwrap().endpoint as int));
        }
    }
    (accepted, Ghost(binding))
}

/// Checked one-byte induction avoids repeatedly unfolding an entire literal trace.
pub proof fn scan_after_byte(prefix: Seq<u8>, byte: u8, comments: bool)
    ensures crate::query_core::frame_scan(prefix.push(byte), comments)
        == (crate::query_core::frame_scan(prefix, comments).0
            + if crate::query_core::structural(crate::query_core::frame_scan(prefix, comments).1, byte) && byte == b'(' { 1int }
            else if crate::query_core::structural(crate::query_core::frame_scan(prefix, comments).1, byte) && byte == b')' { -1int } else { 0int },
            crate::query_core::frame_lex(crate::query_core::frame_scan(prefix, comments).1, byte, comments)),
{
    assert(prefix.push(byte).drop_last() =~= prefix);
    assert(prefix.push(byte).last() == byte);
    reveal_with_fuel(crate::query_core::frame_scan, 1);
}

/// A concrete nonempty witness of the generic lexical framing theorem.
pub proof fn quoted_frame_grammar(bytes: Seq<u8>, comments: bool)
    requires bytes == seq![b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'(', b')', b'"', b'"', b'x', b'"', b')'],
    ensures crate::query_core::single_list(bytes, comments),
{
    assert(bytes.take(0) =~= Seq::<u8>::empty());
    scan_after_byte(bytes.take(0), bytes[0], comments);
    assert(bytes.take(1) =~= bytes.take(0).push(bytes[0]));
    assert(crate::query_core::frame_scan(bytes.take(1), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(1), bytes[1], comments);
    assert(bytes.take(2) =~= bytes.take(1).push(bytes[1]));
    assert(crate::query_core::frame_scan(bytes.take(2), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(2), bytes[2], comments);
    assert(bytes.take(3) =~= bytes.take(2).push(bytes[2]));
    assert(crate::query_core::frame_scan(bytes.take(3), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(3), bytes[3], comments);
    assert(bytes.take(4) =~= bytes.take(3).push(bytes[3]));
    assert(crate::query_core::frame_scan(bytes.take(4), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(4), bytes[4], comments);
    assert(bytes.take(5) =~= bytes.take(4).push(bytes[4]));
    assert(crate::query_core::frame_scan(bytes.take(5), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(5), bytes[5], comments);
    assert(bytes.take(6) =~= bytes.take(5).push(bytes[5]));
    assert(crate::query_core::frame_scan(bytes.take(6), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(6), bytes[6], comments);
    assert(bytes.take(7) =~= bytes.take(6).push(bytes[6]));
    assert(crate::query_core::frame_scan(bytes.take(7), comments) == (1, crate::query_core::FrameLex::String));
    scan_after_byte(bytes.take(7), bytes[7], comments);
    assert(bytes.take(8) =~= bytes.take(7).push(bytes[7]));
    assert(crate::query_core::frame_scan(bytes.take(8), comments) == (1, crate::query_core::FrameLex::String));
    scan_after_byte(bytes.take(8), bytes[8], comments);
    assert(bytes.take(9) =~= bytes.take(8).push(bytes[8]));
    assert(crate::query_core::frame_scan(bytes.take(9), comments) == (1, crate::query_core::FrameLex::String));
    scan_after_byte(bytes.take(9), bytes[9], comments);
    assert(bytes.take(10) =~= bytes.take(9).push(bytes[9]));
    assert(crate::query_core::frame_scan(bytes.take(10), comments) == (1, crate::query_core::FrameLex::Quote));
    scan_after_byte(bytes.take(10), bytes[10], comments);
    assert(bytes.take(11) =~= bytes.take(10).push(bytes[10]));
    assert(crate::query_core::frame_scan(bytes.take(11), comments) == (1, crate::query_core::FrameLex::String));
    scan_after_byte(bytes.take(11), bytes[11], comments);
    assert(bytes.take(12) =~= bytes.take(11).push(bytes[11]));
    assert(crate::query_core::frame_scan(bytes.take(12), comments) == (1, crate::query_core::FrameLex::String));
    scan_after_byte(bytes.take(12), bytes[12], comments);
    assert(bytes.take(13) =~= bytes.take(12).push(bytes[12]));
    assert(crate::query_core::frame_scan(bytes.take(13), comments) == (1, crate::query_core::FrameLex::Quote));
    scan_after_byte(bytes.take(13), bytes[13], comments);
    assert(bytes.take(14) =~= bytes.take(13).push(bytes[13]));
    assert(crate::query_core::frame_scan(bytes.take(14), comments) == (0, crate::query_core::FrameLex::Plain));
    assert forall|i: int| 1 <= i < bytes.len() implies crate::query_core::frame_scan(bytes.take(i), comments).0 > 0 by {
        if i == 1 {}
        else if i == 2 {}
        else if i == 3 {}
        else if i == 4 {}
        else if i == 5 {}
        else if i == 6 {}
        else if i == 7 {}
        else if i == 8 {}
        else if i == 9 {}
        else if i == 10 {}
        else if i == 11 {}
        else if i == 12 {}
        else if i == 13 {}
    }
    assert(bytes.take(bytes.len() as int) =~= bytes);
}

/// A concrete nonempty witness of the generic lexical framing theorem.
pub proof fn bar_frame_grammar(bytes: Seq<u8>, comments: bool)
    requires bytes == seq![b'(', b'a', b' ', b'|', b')', b'|', b')'],
    ensures crate::query_core::single_list(bytes, comments),
{
    assert(bytes.take(0) =~= Seq::<u8>::empty());
    scan_after_byte(bytes.take(0), bytes[0], comments);
    assert(bytes.take(1) =~= bytes.take(0).push(bytes[0]));
    assert(crate::query_core::frame_scan(bytes.take(1), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(1), bytes[1], comments);
    assert(bytes.take(2) =~= bytes.take(1).push(bytes[1]));
    assert(crate::query_core::frame_scan(bytes.take(2), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(2), bytes[2], comments);
    assert(bytes.take(3) =~= bytes.take(2).push(bytes[2]));
    assert(crate::query_core::frame_scan(bytes.take(3), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(3), bytes[3], comments);
    assert(bytes.take(4) =~= bytes.take(3).push(bytes[3]));
    assert(crate::query_core::frame_scan(bytes.take(4), comments) == (1, crate::query_core::FrameLex::Symbol));
    scan_after_byte(bytes.take(4), bytes[4], comments);
    assert(bytes.take(5) =~= bytes.take(4).push(bytes[4]));
    assert(crate::query_core::frame_scan(bytes.take(5), comments) == (1, crate::query_core::FrameLex::Symbol));
    scan_after_byte(bytes.take(5), bytes[5], comments);
    assert(bytes.take(6) =~= bytes.take(5).push(bytes[5]));
    assert(crate::query_core::frame_scan(bytes.take(6), comments) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(6), bytes[6], comments);
    assert(bytes.take(7) =~= bytes.take(6).push(bytes[6]));
    assert(crate::query_core::frame_scan(bytes.take(7), comments) == (0, crate::query_core::FrameLex::Plain));
    assert forall|i: int| 1 <= i < bytes.len() implies crate::query_core::frame_scan(bytes.take(i), comments).0 > 0 by {
        if i == 1 {}
        else if i == 2 {}
        else if i == 3 {}
        else if i == 4 {}
        else if i == 5 {}
        else if i == 6 {}
    }
    assert(bytes.take(bytes.len() as int) =~= bytes);
}

/// A concrete nonempty witness of the generic lexical framing theorem.
pub proof fn comment_frame_grammar(bytes: Seq<u8>)
    requires bytes == seq![b'(', b'a', b' ', b';', b')', b'\r', b'\n', b')'],
    ensures crate::query_core::single_list(bytes, true),
{
    assert(bytes.take(0) =~= Seq::<u8>::empty());
    scan_after_byte(bytes.take(0), bytes[0], true);
    assert(bytes.take(1) =~= bytes.take(0).push(bytes[0]));
    assert(crate::query_core::frame_scan(bytes.take(1), true) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(1), bytes[1], true);
    assert(bytes.take(2) =~= bytes.take(1).push(bytes[1]));
    assert(crate::query_core::frame_scan(bytes.take(2), true) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(2), bytes[2], true);
    assert(bytes.take(3) =~= bytes.take(2).push(bytes[2]));
    assert(crate::query_core::frame_scan(bytes.take(3), true) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(3), bytes[3], true);
    assert(bytes.take(4) =~= bytes.take(3).push(bytes[3]));
    assert(crate::query_core::frame_scan(bytes.take(4), true) == (1, crate::query_core::FrameLex::Comment));
    scan_after_byte(bytes.take(4), bytes[4], true);
    assert(bytes.take(5) =~= bytes.take(4).push(bytes[4]));
    assert(crate::query_core::frame_scan(bytes.take(5), true) == (1, crate::query_core::FrameLex::Comment));
    scan_after_byte(bytes.take(5), bytes[5], true);
    assert(bytes.take(6) =~= bytes.take(5).push(bytes[5]));
    assert(crate::query_core::frame_scan(bytes.take(6), true) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(6), bytes[6], true);
    assert(bytes.take(7) =~= bytes.take(6).push(bytes[6]));
    assert(crate::query_core::frame_scan(bytes.take(7), true) == (1, crate::query_core::FrameLex::Plain));
    scan_after_byte(bytes.take(7), bytes[7], true);
    assert(bytes.take(8) =~= bytes.take(7).push(bytes[7]));
    assert(crate::query_core::frame_scan(bytes.take(8), true) == (0, crate::query_core::FrameLex::Plain));
    assert forall|i: int| 1 <= i < bytes.len() implies crate::query_core::frame_scan(bytes.take(i), true).0 > 0 by {
        if i == 1 {}
        else if i == 2 {}
        else if i == 3 {}
        else if i == 4 {}
        else if i == 5 {}
        else if i == 6 {}
        else if i == 7 {}
    }
    assert(bytes.take(bytes.len() as int) =~= bytes);
}

/// An actual fragmented receive, checked lexical boundary and exact model
/// binding preserve comment/quote bytes, rather than only checking opaque tags.
pub fn framed_fragment_binding_and_response_witness() -> (feed_bytes: Vec<u8>)
    ensures feed_bytes@ == seq![b';', b'x', b'(', b'\r', b'\n', b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'(', b')', b'"', b'"', b'x', b'"', b')'],
{
    let command = vec![b'(', b'e', b'c', b'h', b'o', b' ', b'"', b'(', b')', b'"', b'"', b'x', b'"', b')'];
    let prefix = vec![b';', b'x', b'(', b'\r', b'\n'];
    let mut raw = RawTranscript::empty();
    raw.append(&prefix);
    raw.append(&command[..10]);
    raw.append(&command[10..]);
    let ghost received = prefix@ + command@;
    let ghost context = model::example_context(1, 2);
    proof {
        quoted_frame_grammar(command@, true);
        reveal_with_fuel(crate::query_core::trivia_after, 6);
        assert(raw.view() =~= received);
        assert(received.subrange(prefix@.len() as int, received.len() as int) =~= command@);
        assert(received.subrange(0, prefix@.len() as int) =~= prefix@);
        assert(received.take(0) =~= Seq::<u8>::empty());
    }
    let (framed, Ghost(binding)) = framed_prefix_refines_binding(&mut raw, prefix.len(), &command,
        Ghost(received), Ghost(Seq::<u8>::empty()), Ghost(context));
    let (feed, _) = match framed { Some(framed) => framed, None => return Vec::new() };
    let mut feed_bytes = Vec::new();
    feed_bytes.extend_from_slice(feed.bytes());
    proof { assert(feed_bytes@ =~= received); }
    let bar = vec![b'(', b'a', b' ', b'|', b')', b'|', b')'];
    let comment = vec![b'(', b'a', b' ', b';', b')', b'\r', b'\n', b')'];
    proof { bar_frame_grammar(bar@, true); comment_frame_grammar(comment@); }
    let bar_ok = crate::query_core::complete_list(&bar, true);
    let comment_ok = crate::query_core::complete_list(&comment, true);
    proof { assert(bar_ok); assert(comment_ok); }
    let mut response = Vec::new();
    response.extend_from_slice(&bar); response.push(b'\n');
    proof {
        bar_frame_grammar(bar@, false);
        assert(response@.subrange(0, bar@.len() as int) =~= bar@);
        assert(crate::query_core::response_span(response@, 0, bar@.len() as int));
        assert(crate::query_core::response_shape(response@));
    }
    let response_ok = crate::query_core::response_frame(&response);
    let status = vec![b'u', b'n', b's', b'a', b't', b'\r', b'\n'];
    proof {
        assert(crate::query_core::response_span(status@, 0, 5));
        assert(crate::query_core::response_shape(status@));
    }
    let status_ok = crate::query_core::response_frame(&status);
    proof { assert(response_ok); assert(status_ok); }
    feed_bytes
}

/// Actual admission refuses two balanced replies pretending to be one, as well
/// as an atom lacking its line terminator. No injected assertion is needed.
pub fn coalesced_and_partial_response_witness() {
    let form = vec![b'(', b'a', b' ', b'|', b')', b'|', b')'];
    let mut coalesced = Vec::new(); coalesced.extend_from_slice(&form); coalesced.extend_from_slice(&form);
    proof {
        bar_frame_grammar(form@, false);
        complete_forms_cannot_coalesce(form@, form@, false);
        assert(coalesced@ == form@ + form@);
        assert(crate::query_core::response_trim(coalesced@, 0, coalesced@.len() as int));
        crate::query_core::response_at_trim(coalesced@, 0, coalesced@.len() as int);
        assert(coalesced@.subrange(0, coalesced@.len() as int) =~= coalesced@);
        assert(!crate::query_core::response_span(coalesced@, 0, coalesced@.len() as int));
    }
    let refused = crate::query_core::response_frame(&coalesced);
    proof { assert(!refused); }
    let partial = vec![b'u', b'n', b's', b'a', b't'];
    proof {
        assert(crate::query_core::response_trim(partial@, 0, partial@.len() as int));
        crate::query_core::response_at_trim(partial@, 0, partial@.len() as int);
    }
    let refused = crate::query_core::response_frame(&partial);
    proof { assert(!refused); }
}


/// The actual typed identity's byte interpretation, without physical-launch
/// authority or a digest-injectivity premise.
pub open spec fn typed_model_solver(solver: &crate::SolverIdentity) -> crate::context::SolverId {
    crate::context::SolverId {
        name: vstd::utf8::encode_utf8(solver.name@),
        version: vstd::utf8::encode_utf8(solver.version@),
        binary: solver.binary_sha256@,
        platform: vstd::utf8::encode_utf8(solver.platform@),
    }
}

pub open spec fn typed_model_mode(mode: &crate::Mode) -> crate::context::Mode {
    match mode {
        crate::Mode::Default => crate::context::Mode::Default,
        crate::Mode::OptIn(name) => crate::context::Mode::OptIn(vstd::utf8::encode_utf8(name@)),
    }
}

pub open spec fn typed_model_fields(solver: &crate::SolverIdentity, mode: &crate::Mode)
    -> (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)
{
    (vstd::utf8::encode_utf8(solver.name@), vstd::utf8::encode_utf8(solver.version@),
        solver.binary_sha256@, vstd::utf8::encode_utf8(solver.platform@),
        match mode { crate::Mode::Default => None,
            crate::Mode::OptIn(name) => Some(vstd::utf8::encode_utf8(name@)) })
}

/// Calls actual typed construction, original input append, checked command and
/// checkpoint freeze. Its model binding is derived from those same fields and
/// bytes rather than an independently supplied ModelContext.
pub fn typed_model_command<Host: crate::key_prefix::HashHost>(
    host: Host, solver: &crate::SolverIdentity, mode: &crate::Mode,
    input: &[u8], start: usize, command: &[u8],
) -> (out: (Option<crate::query_prefix::FrozenCommand<Host>>, Ghost<Option<model::Binding>>))
    requires host.bytes().len() == 0,
    ensures out.0.is_some() == out.1@.is_some(),
        out.0.is_some() ==> (
            out.0.unwrap().coherent()
            && out.0.unwrap().command_view() == command@
            && out.0.unwrap().offset_view() == start
            && model::binding_wf(out.1@.unwrap())
            && out.1@.unwrap().solver == typed_model_solver(solver)
            && out.1@.unwrap().mode == typed_model_mode(mode)
            && out.1@.unwrap().endpoint == out.0.unwrap().endpoint_view()
            && out.1@.unwrap().endpoint == start as nat + command@.len()
            && out.1@.unwrap().endpoint <= input@.len()
            && out.1@.unwrap().prefix == input@.take(out.1@.unwrap().endpoint as int)
            && out.0.unwrap().material_view()
                == crate::key_prefix::header(typed_model_fields(solver,mode).0,
                    typed_model_fields(solver,mode).1,typed_model_fields(solver,mode).2,
                    typed_model_fields(solver,mode).3,typed_model_fields(solver,mode).4)
                    + out.1@.unwrap().prefix
            && out.0.unwrap().key_view() == Host::digest(crate::key_prefix::end(
                out.0.unwrap().material_view(),out.1@.unwrap().endpoint as u64))),
        (exists|a:int,b:int| crate::query_core::head_span(command@,a,b))
            && start as nat + command@.len() <= input@.len()
            && input@.subrange(start as int,start as int+command@.len() as int) == command@
            && crate::query_core::trivia_after(input@.subrange(0,start as int),false) == Some(false)
            ==> out.0.is_some(),
{
    let mut prefix = crate::prefix_factory::new_query_prefix(host,solver,mode);
    let ghost fields = typed_model_fields(solver,mode);
    prefix.append(input);
    let accepted = prefix.command(start,command);
    match accepted {
        Some(frame) => {
            let frozen = frame.freeze();
            let ghost binding = model::Binding {
                solver: typed_model_solver(solver), mode: typed_model_mode(mode),
                prefix: input@.take(frozen.endpoint_view() as int),
                endpoint: frozen.endpoint_view(),
            };
            proof {
                crate::query_prefix::coherent_prefix_bound(&prefix);
                assert(prefix.matches(fields));
                assert(prefix.view() == input@);
                assert(prefix.fed() == frozen.endpoint_view());
                assert(frozen.endpoint_view() <= input@.len());
                assert(input@.take(frozen.endpoint_view() as int).len() == frozen.endpoint_view());
                assert(binding.prefix.len() == binding.endpoint);
            }
            (Some(frozen),Ghost(Some(binding)))
        },
        None => (None,Ghost(None)),
    }
}

/// Constructive universal admission for a real supported nonempty command.
/// The only environment boundary is the existing narrow hash primitive.
pub fn typed_model_supported_command_constructive<Host: crate::key_prefix::HashHost>(
    host: Host, solver: &crate::SolverIdentity, mode: &crate::Mode, command: &[u8],
) -> (out: crate::query_prefix::FrozenCommand<Host>)
    requires host.bytes().len() == 0, command@.len() > 0,
        exists|a:int,b:int| crate::query_core::head_span(command@,a,b),
    ensures out.coherent(), out.command_view() == command@, out.offset_view() == 0,
        out.endpoint_view() == command@.len(),
        out.material_view() == crate::key_prefix::header(typed_model_fields(solver,mode).0,
            typed_model_fields(solver,mode).1,typed_model_fields(solver,mode).2,
            typed_model_fields(solver,mode).3,typed_model_fields(solver,mode).4)+command@,
{
    proof {
        assert(command@.subrange(0,0) =~= Seq::<u8>::empty());
        assert(command@.subrange(0,command@.len() as int) =~= command@);
        assert(crate::query_core::trivia_after(Seq::<u8>::empty(),false) == Some(false));
    }
    let (accepted,Ghost(binding)) = typed_model_command(host,solver,mode,command,0,command);
    accepted.unwrap()
}

}
