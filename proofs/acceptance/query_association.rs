//! Inductive query/reply association model, checked independently of Sail.
//! This is specification code, not a claim that the production adapters refine it.
#![forbid(unsafe_code)]

use vstd::prelude::*;

verus! {

pub struct Solver {
    pub name: Seq<u8>,
    pub version: Seq<u8>,
    pub binary: Seq<u8>,
    pub platform: Seq<u8>,
}

pub enum Mode {
    Default,
    OptIn(Seq<u8>),
}

/// Identity uses exact bytes, without a hash-injectivity axiom.
pub struct Binding {
    pub solver: Solver,
    pub mode: Mode,
    pub prefix: Seq<u8>,
    pub endpoint: nat,
}

pub struct Entry {
    pub binding: Binding,
    pub reply: Seq<u8>,
}

pub open spec fn binding_wf(binding: Binding) -> bool {
    binding.endpoint == binding.prefix.len()
}

/// Response framing preserves all bytes, including whitespace. The parser
/// supplies boundaries; delimiter recognition is not proved by this model.
pub struct ReplyBuffer {
    pub received: Seq<u8>,
    pub consumed: nat,
    pub complete: Seq<Seq<u8>>,
}

pub open spec fn flatten(chunks: Seq<Seq<u8>>) -> Seq<u8>
    decreases chunks.len(),
{
    if chunks.len() == 0 { Seq::empty() }
    else { flatten(chunks.drop_last()) + chunks.last() }
}

pub proof fn flatten_after_push(chunks: Seq<Seq<u8>>, bytes: Seq<u8>)
    ensures flatten(chunks.push(bytes)) == flatten(chunks) + bytes,
{
    assert(chunks.push(bytes).drop_last() =~= chunks);
}

pub open spec fn buffer_wf(buffer: ReplyBuffer) -> bool {
    buffer.consumed <= buffer.received.len()
    && flatten(buffer.complete) == buffer.received.take(buffer.consumed as int)
}

pub open spec fn buffer_init() -> ReplyBuffer {
    ReplyBuffer { received: Seq::empty(), consumed: 0, complete: Seq::empty() }
}

pub proof fn empty_buffer_is_valid()
    ensures buffer_wf(buffer_init()),
{
    assert(flatten(Seq::<Seq<u8>>::empty()) == Seq::<u8>::empty());
    assert(Seq::<u8>::empty().take(0) =~= Seq::<u8>::empty());
}

pub open spec fn receive_fragment(buffer: ReplyBuffer, bytes: Seq<u8>) -> ReplyBuffer {
    ReplyBuffer { received: buffer.received + bytes, ..buffer }
}

pub proof fn response_fragment_preserves_bytes(buffer: ReplyBuffer, bytes: Seq<u8>)
    requires buffer_wf(buffer),
    ensures buffer_wf(receive_fragment(buffer, bytes)),
{
    assert((buffer.received + bytes).take(buffer.consumed as int)
        =~= buffer.received.take(buffer.consumed as int));
}

pub open spec fn finish_reply(buffer: ReplyBuffer, endpoint: nat) -> ReplyBuffer {
    ReplyBuffer {
        complete: buffer.complete.push(buffer.received.subrange(buffer.consumed as int, endpoint as int)),
        consumed: endpoint,
        ..buffer
    }
}

pub proof fn completed_reply_preserves_all_bytes(buffer: ReplyBuffer, endpoint: nat)
    requires buffer_wf(buffer), buffer.consumed < endpoint <= buffer.received.len(),
    ensures buffer_wf(finish_reply(buffer, endpoint)),
{
    let bytes = buffer.received.subrange(buffer.consumed as int, endpoint as int);
    flatten_after_push(buffer.complete, bytes);
    assert(buffer.received.take(buffer.consumed as int) + bytes
        =~= buffer.received.take(endpoint as int));
}

/// The two arrival queues can grow independently. `paired` is their common
/// consumed prefix; neither side is consumed while the other side is absent.
pub struct Pairer {
    pub commands: Seq<Binding>,
    pub replies: Seq<Seq<u8>>,
    pub paired: nat,
    pub entries: Seq<Entry>,
    pub stopped: bool,
}

pub open spec fn pairer_wf(s: Pairer) -> bool {
    s.paired <= s.commands.len()
    && s.paired <= s.replies.len()
    && s.entries.len() == s.paired
    && (forall|i: int| 0 <= i < s.commands.len() ==> binding_wf(s.commands[i]))
    && (forall|i: int| 0 <= i < s.entries.len() ==>
        s.entries[i].binding == s.commands[i] && s.entries[i].reply == s.replies[i])
}

pub open spec fn pairer_init() -> Pairer {
    Pairer {
        commands: Seq::empty(), replies: Seq::empty(), paired: 0,
        entries: Seq::empty(), stopped: false,
    }
}

pub open spec fn enqueue_command(s: Pairer, binding: Binding) -> Pairer {
    Pairer { commands: s.commands.push(binding), ..s }
}

pub open spec fn enqueue_reply(s: Pairer, reply: Seq<u8>) -> Pairer {
    Pairer { replies: s.replies.push(reply), ..s }
}

pub open spec fn stop_pairer(s: Pairer) -> Pairer {
    Pairer { stopped: true, ..s }
}

pub open spec fn can_pair(s: Pairer) -> bool {
    !s.stopped && s.paired < s.commands.len() && s.paired < s.replies.len()
}

pub open spec fn pair_one(s: Pairer) -> Pairer
    recommends can_pair(s),
{
    let entry = Entry {
        binding: s.commands[s.paired as int],
        reply: s.replies[s.paired as int],
    };
    Pairer { entries: s.entries.push(entry), paired: s.paired + 1, ..s }
}

pub proof fn pairer_initially_valid()
    ensures pairer_wf(pairer_init()),
{}

pub proof fn command_arrival_preserves(s: Pairer, binding: Binding)
    requires pairer_wf(s), binding_wf(binding),
    ensures pairer_wf(enqueue_command(s, binding)),
{
    let next = enqueue_command(s, binding);
    assert forall|i: int| 0 <= i < next.commands.len()
        implies binding_wf(next.commands[i]) by {
        if i < s.commands.len() {} else { assert(i == s.commands.len()); }
    }
}

pub proof fn reply_arrival_preserves(s: Pairer, reply: Seq<u8>)
    requires pairer_wf(s),
    ensures pairer_wf(enqueue_reply(s, reply)),
{}

pub proof fn pairing_preserves_order(s: Pairer)
    requires pairer_wf(s), can_pair(s),
    ensures pairer_wf(pair_one(s)),
{
    let next = pair_one(s);
    assert forall|i: int| 0 <= i < next.entries.len()
        implies next.entries[i].binding == next.commands[i]
            && next.entries[i].reply == next.replies[i] by {
        if i < s.paired {} else { assert(i == s.paired); }
    }
}

pub proof fn stop_is_permanent(s: Pairer)
    requires pairer_wf(s),
    ensures pairer_wf(stop_pairer(s)), !can_pair(stop_pairer(s)),
{}

/// Publication requires a complete, clean, equally paired session. The
/// adapter must establish `clean` from actual EOF/exit/stderr observations.
pub open spec fn published(s: Pairer, clean: bool) -> Seq<Entry> {
    if clean && !s.stopped
        && s.paired == s.commands.len() && s.paired == s.replies.len() {
        s.entries
    } else {
        Seq::empty()
    }
}

pub proof fn publication_has_ordered_origin(s: Pairer, clean: bool)
    requires pairer_wf(s),
    ensures
        forall|i: int| 0 <= i < published(s, clean).len() ==>
            published(s, clean)[i].binding == s.commands[i]
            && published(s, clean)[i].reply == s.replies[i],
        !clean || s.stopped || s.paired != s.commands.len()
            || s.paired != s.replies.len() ==> published(s, clean).len() == 0,
{}

/// A session starts cache-serving and can move to live replay only once.
/// `cache` is the admitted store: every entry needs the origin theorem above
/// or an explicit recovery/trusted-writer contract before entering this model.
pub struct Session {
    pub solver: Solver,
    pub mode: Mode,
    pub raw: Seq<u8>,
    pub commands: Seq<Binding>,
    pub cache: Seq<Entry>,
    pub delivered: Seq<Entry>,
    pub cached: nat,
    pub live: bool,
    pub connected: bool,
    pub solver_input: Seq<u8>,
    pub skip: nat,
    pub seen: nat,
}

pub open spec fn session_wf(s: Session) -> bool {
    s.delivered.len() <= s.commands.len()
    && s.cached <= s.delivered.len()
    && (forall|i: int| 0 <= i < s.commands.len() ==>
        binding_wf(s.commands[i])
        && s.commands[i].solver == s.solver
        && s.commands[i].mode == s.mode
        && s.commands[i].endpoint <= s.raw.len()
        && s.commands[i].prefix == s.raw.take(s.commands[i].endpoint as int))
    && (forall|i: int| 0 <= i < s.delivered.len() ==>
        s.delivered[i].binding == s.commands[i])
    && (forall|i: int| 0 <= i < s.cached ==>
        s.cache.contains(s.delivered[i]))
    && (if s.live {
        s.solver_input == s.raw
        && s.skip == s.cached
        && s.seen <= s.commands.len()
        && s.delivered.len() == s.skip + if s.seen > s.skip { s.seen - s.skip } else { 0 }
    } else {
        s.connected && s.delivered.len() == s.cached
        && s.skip == 0 && s.seen == 0 && s.solver_input.len() == 0
    })
}

pub open spec fn session_init(solver: Solver, mode: Mode, cache: Seq<Entry>) -> Session {
    Session {
        solver, mode, cache, raw: Seq::empty(), commands: Seq::empty(),
        delivered: Seq::empty(), cached: 0, live: false, connected: true,
        solver_input: Seq::empty(), skip: 0, seen: 0,
    }
}

pub proof fn session_initially_valid(solver: Solver, mode: Mode, cache: Seq<Entry>)
    ensures session_wf(session_init(solver, mode, cache)),
{}

/// A fragment is added unchanged to both the raw stream and a running solver.
pub open spec fn input_fragment(s: Session, bytes: Seq<u8>) -> Session {
    Session {
        raw: s.raw + bytes,
        solver_input: if s.live { s.solver_input + bytes } else { s.solver_input },
        ..s
    }
}

pub proof fn fragments_preserve_exact_input(s: Session, bytes: Seq<u8>)
    requires session_wf(s),
    ensures session_wf(input_fragment(s, bytes)),
{
    let next = input_fragment(s, bytes);
    assert forall|i: int| 0 <= i < next.commands.len() implies
        binding_wf(next.commands[i])
        && next.commands[i].solver == next.solver
        && next.commands[i].mode == next.mode
        && next.commands[i].endpoint <= next.raw.len()
        && next.commands[i].prefix == next.raw.take(next.commands[i].endpoint as int) by {
        assert((s.raw + bytes).take(s.commands[i].endpoint as int)
            =~= s.raw.take(s.commands[i].endpoint as int));
    }
}

/// A completed output-producing command binds every byte up to its endpoint.
/// Parsing/classifying which endpoints produce replies is an adapter obligation.
pub open spec fn observe_command(s: Session, endpoint: nat) -> Session {
    let binding = Binding {
        solver: s.solver, mode: s.mode,
        prefix: s.raw.take(endpoint as int), endpoint,
    };
    Session { commands: s.commands.push(binding), ..s }
}

pub proof fn command_observation_preserves(s: Session, endpoint: nat)
    requires session_wf(s), endpoint <= s.raw.len(),
    ensures session_wf(observe_command(s, endpoint)),
{
    let next = observe_command(s, endpoint);
    assert forall|i: int| 0 <= i < next.commands.len() implies
        binding_wf(next.commands[i])
        && next.commands[i].solver == next.solver
        && next.commands[i].mode == next.mode
        && next.commands[i].endpoint <= next.raw.len()
        && next.commands[i].prefix == next.raw.take(next.commands[i].endpoint as int) by {
        if i < s.commands.len() {} else { assert(i == s.commands.len()); }
    }
}

pub open spec fn can_serve(s: Session, entry: Entry) -> bool {
    !s.live && s.connected
    && s.delivered.len() < s.commands.len()
    && s.cache.contains(entry)
    && entry.binding == s.commands[s.delivered.len() as int]
}

pub open spec fn serve_cached(s: Session, entry: Entry) -> Session {
    Session { delivered: s.delivered.push(entry), cached: s.cached + 1, ..s }
}

pub proof fn cache_serve_preserves_association(s: Session, entry: Entry)
    requires session_wf(s), can_serve(s, entry),
    ensures session_wf(serve_cached(s, entry)),
{
    let next = serve_cached(s, entry);
    assert forall|i: int| 0 <= i < next.delivered.len()
        implies next.delivered[i].binding == next.commands[i] by {
        if i < s.delivered.len() {} else { assert(i == s.delivered.len()); }
    }
    assert forall|i: int| 0 <= i < next.cached
        implies next.cache.contains(next.delivered[i]) by {
        if i < s.cached {} else { assert(i == s.cached); }
    }
}

pub open spec fn begin_fallback(s: Session, connection_failed: bool) -> Session {
    Session {
        live: true, connected: s.connected && !connection_failed,
        solver_input: s.raw, skip: s.cached, seen: 0, ..s
    }
}

pub proof fn fallback_replays_exact_prefix(s: Session, connection_failed: bool)
    requires session_wf(s), !s.live,
    ensures
        session_wf(begin_fallback(s, connection_failed)),
        begin_fallback(s, connection_failed).solver_input == s.raw,
        begin_fallback(s, connection_failed).skip == s.delivered.len(),
        connection_failed ==> !begin_fallback(s, connection_failed).connected,
{}

/// No cache-origin event is admissible after replay begins, including a late
/// response from a failed connection. There is no transition back to cache mode.
pub proof fn late_cache_reply_cannot_be_served(s: Session, entry: Entry, failed: bool)
    requires session_wf(s), !s.live,
    ensures !can_serve(begin_fallback(s, failed), entry),
{}

pub open spec fn live_reply(s: Session, bytes: Seq<u8>) -> Session
    recommends s.live, s.seen < s.commands.len(),
{
    let entry = Entry { binding: s.commands[s.seen as int], reply: bytes };
    Session {
        delivered: if s.seen < s.skip { s.delivered } else { s.delivered.push(entry) },
        seen: s.seen + 1,
        ..s
    }
}

pub proof fn replay_suppresses_exactly_served_replies(s: Session, bytes: Seq<u8>)
    requires session_wf(s), s.live, s.seen < s.commands.len(),
    ensures
        session_wf(live_reply(s, bytes)),
        s.seen < s.skip ==> live_reply(s, bytes).delivered == s.delivered,
        s.seen >= s.skip ==> live_reply(s, bytes).delivered
            == s.delivered.push(Entry { binding: s.commands[s.seen as int], reply: bytes }),
{
    let next = live_reply(s, bytes);
    if s.seen >= s.skip {
        assert(s.delivered.len() == s.seen);
        assert forall|i: int| 0 <= i < next.delivered.len()
            implies next.delivered[i].binding == next.commands[i] by {
            if i < s.delivered.len() {} else { assert(i == s.seen); }
        }
    }
}

/// This is the acceptance conclusion for any state reached by the proved
/// transitions. Induction is over an arbitrary finite transition sequence.
pub proof fn every_delivered_reply_binds_its_requested_prefix(s: Session, index: int)
    requires session_wf(s), 0 <= index < s.delivered.len(),
    ensures
        s.delivered[index].binding == s.commands[index],
        s.delivered[index].binding.solver == s.solver,
        s.delivered[index].binding.mode == s.mode,
        s.delivered[index].binding.endpoint <= s.raw.len(),
        s.delivered[index].binding.prefix
            == s.raw.take(s.delivered[index].binding.endpoint as int),
{}

/// Unsupported or inapplicable actions leave the modeled state unchanged.
pub enum Action {
    Input(Seq<u8>),
    Command(nat),
    Cache(Entry),
    Fallback(bool),
    Reply(Seq<u8>),
}

pub open spec fn step(s: Session, action: Action) -> Session {
    match action {
        Action::Input(bytes) => input_fragment(s, bytes),
        Action::Command(endpoint) => if endpoint <= s.raw.len() {
            observe_command(s, endpoint)
        } else { s },
        Action::Cache(entry) => if can_serve(s, entry) { serve_cached(s, entry) } else { s },
        Action::Fallback(failed) => if !s.live { begin_fallback(s, failed) } else { s },
        Action::Reply(bytes) => if s.live && s.seen < s.commands.len() {
            live_reply(s, bytes)
        } else { s },
    }
}

pub proof fn every_transition_preserves(s: Session, action: Action)
    requires session_wf(s),
    ensures session_wf(step(s, action)),
{
    match action {
        Action::Input(bytes) => fragments_preserve_exact_input(s, bytes),
        Action::Command(endpoint) => if endpoint <= s.raw.len() {
            command_observation_preserves(s, endpoint);
        },
        Action::Cache(entry) => if can_serve(s, entry) {
            cache_serve_preserves_association(s, entry);
        },
        Action::Fallback(failed) => if !s.live {
            fallback_replays_exact_prefix(s, failed);
        },
        Action::Reply(bytes) => if s.live && s.seen < s.commands.len() {
            replay_suppresses_exactly_served_replies(s, bytes);
        },
    }
}

pub open spec fn execution(states: Seq<Session>, actions: Seq<Action>) -> bool {
    states.len() == actions.len() + 1
    && states[0] == session_init(states[0].solver, states[0].mode, states[0].cache)
    && forall|i: int| 0 <= i < actions.len() ==> states[i + 1] == step(states[i], actions[i])
}

pub proof fn all_finite_executions_preserve(states: Seq<Session>, actions: Seq<Action>, index: nat)
    requires execution(states, actions), index < states.len(),
    ensures session_wf(states[index as int]),
    decreases index,
{
    if index == 0 {
        session_initially_valid(states[0].solver, states[0].mode, states[0].cache);
    } else {
        all_finite_executions_preserve(states, actions, (index - 1) as nat);
        every_transition_preserves(states[index - 1], actions[index - 1]);
    }
}

/// The main session theorem quantifies over every finite action history and
/// every delivered reply, without assuming the final state's invariant.
pub proof fn executed_reply_has_exact_binding(
    states: Seq<Session>, actions: Seq<Action>, state_index: nat, reply_index: int,
)
    requires
        execution(states, actions), state_index < states.len(),
        0 <= reply_index < states[state_index as int].delivered.len(),
    ensures
        states[state_index as int].delivered[reply_index].binding
            == states[state_index as int].commands[reply_index],
        states[state_index as int].delivered[reply_index].binding.solver
            == states[state_index as int].solver,
        states[state_index as int].delivered[reply_index].binding.mode
            == states[state_index as int].mode,
        states[state_index as int].delivered[reply_index].binding.prefix
            == states[state_index as int].raw.take(
                states[state_index as int].delivered[reply_index].binding.endpoint as int),
{
    all_finite_executions_preserve(states, actions, state_index);
    every_delivered_reply_binds_its_requested_prefix(states[state_index as int], reply_index);
}

pub open spec fn example_solver() -> Solver {
    Solver { name: seq![90u8, 51u8], version: seq![49u8], binary: seq![0u8], platform: seq![1u8] }
}

/// Reply-before-expect arrival must not consume or shift either queue.
pub proof fn positive_pairer_arrival_and_publication()
    ensures exists|entry: Entry| entry.binding.prefix == seq![1u8] && entry.reply == seq![2u8],
{
    let initial = pairer_init();
    pairer_initially_valid();
    let with_reply = enqueue_reply(initial, seq![2u8]);
    reply_arrival_preserves(initial, seq![2u8]);
    assert(!can_pair(with_reply));
    let binding = Binding { solver: example_solver(), mode: Mode::Default, prefix: seq![1u8], endpoint: 1 };
    let ready = enqueue_command(with_reply, binding);
    command_arrival_preserves(with_reply, binding);
    assert(can_pair(ready));
    let paired = pair_one(ready);
    pairing_preserves_order(ready);
    publication_has_ordered_origin(paired, true);
    assert(published(paired, true).len() == 1);
    assert(published(paired, false).len() == 0);
    let entry = published(paired, true)[0];
    assert(entry.binding.prefix == seq![1u8] && entry.reply == seq![2u8]);
}

/// A cached first response is emitted once. Its live replay is suppressed,
/// the second response is emitted, and a late cache event is rejected.
pub proof fn positive_cache_then_fallback()
{
    let binding = Binding { solver: example_solver(), mode: Mode::Default, prefix: seq![1u8], endpoint: 1 };
    let entry = Entry { binding, reply: seq![11u8] };
    let initial = session_init(example_solver(), Mode::Default, seq![entry]);
    session_initially_valid(example_solver(), Mode::Default, seq![entry]);
    let first_input = input_fragment(initial, seq![1u8]);
    fragments_preserve_exact_input(initial, seq![1u8]);
    let first = observe_command(first_input, 1);
    command_observation_preserves(first_input, 1);
    assert(first.commands.len() == 1);
    assert(first.commands[0].prefix =~= seq![1u8]);
    assert(first.commands[0] == binding);
    assert(first.cache.len() == 1);
    assert(first.cache[0] == entry);
    assert(first.cache.contains(entry));
    assert(can_serve(first, entry));
    let served = serve_cached(first, entry);
    cache_serve_preserves_association(first, entry);
    let second_input = input_fragment(served, seq![2u8]);
    fragments_preserve_exact_input(served, seq![2u8]);
    let second = observe_command(second_input, 2);
    command_observation_preserves(second_input, 2);
    let replay = begin_fallback(second, true);
    fallback_replays_exact_prefix(second, true);
    assert(replay.solver_input == seq![1u8, 2u8]);
    assert(replay.skip == 1);
    let suppressed = live_reply(replay, seq![11u8]);
    replay_suppresses_exactly_served_replies(replay, seq![11u8]);
    assert(suppressed.delivered.len() == 1);
    let complete = live_reply(suppressed, seq![22u8]);
    replay_suppresses_exactly_served_replies(suppressed, seq![22u8]);
    assert(complete.delivered.len() == 2);
    assert(complete.delivered[0].reply == seq![11u8]);
    assert(complete.delivered[1].reply == seq![22u8]);
    assert(!can_serve(complete, entry));
    assert(!complete.connected);
}

} // verus!

fn main() {}
