//! Inductive query/reply association model, checked independently of Sail.
//! This is specification code, not a claim that the production adapters refine it.
#![forbid(unsafe_code)]

pub use crate::context::SolverId as Solver;
use crate::context::{
    InputKey, Mode, ModelContext, ReadOutcome, ScopeKey, SolverTranscript, context_wf,
};
use vstd::prelude::*;

verus! {

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

/// Observations belong to the identified live solver process. Their physical
/// source and parser boundary refinement remain named external obligations.
pub struct LiveRun {
    pub context: ModelContext,
    pub solver: Solver,
    pub raw: Seq<u8>,
    pub output: ReplyBuffer,
    pub pairer: Pairer,
    pub input_closed: bool,
    pub stdout_closed: bool,
    pub stderr_closed: bool,
    pub exited: bool,
    pub exit_ok: bool,
    pub quiet: bool,
}

pub enum RunAction {
    Input(Seq<u8>),
    Command(nat),
    Output(Seq<u8>),
    Frame(nat),
    Pair,
    CloseInput,
    CloseOutput,
    CloseStderr,
    Exit(bool),
    Stderr,
    Stop,
}

pub open spec fn run_init(context: ModelContext, solver: Solver) -> LiveRun {
    LiveRun {
        context, solver, raw: Seq::empty(), output: buffer_init(), pairer: pairer_init(),
        input_closed: false, stdout_closed: false, stderr_closed: false, exited: false, exit_ok: false, quiet: true,
    }
}

pub open spec fn run_wf(run: LiveRun) -> bool {
    pairer_wf(run.pairer) && buffer_wf(run.output)
    && run.pairer.replies == run.output.complete
    && forall|i: int| 0 <= i < run.pairer.commands.len() ==>
        run.pairer.commands[i].solver == run.solver
        && run.pairer.commands[i].mode == run.context.mode
        && run.pairer.commands[i].endpoint <= run.raw.len()
        && run.pairer.commands[i].prefix == run.raw.take(run.pairer.commands[i].endpoint as int)
}

pub open spec fn run_step(run: LiveRun, action: RunAction) -> LiveRun {
    match action {
        RunAction::Input(bytes) => if !run.input_closed && !run.exited {
            LiveRun { raw: run.raw + bytes, ..run }
        } else { run },
        RunAction::Command(endpoint) => if !run.exited && 0 < endpoint <= run.raw.len() {
            let binding = Binding {
                solver: run.solver, mode: run.context.mode,
                prefix: run.raw.take(endpoint as int), endpoint,
            };
            LiveRun { pairer: enqueue_command(run.pairer, binding), ..run }
        } else { run },
        RunAction::Output(bytes) => if !run.stdout_closed {
            LiveRun { output: receive_fragment(run.output, bytes), ..run }
        } else { run },
        RunAction::Frame(endpoint) => if run.output.consumed < endpoint <= run.output.received.len() {
            let bytes = run.output.received.subrange(run.output.consumed as int, endpoint as int);
            LiveRun {
                output: finish_reply(run.output, endpoint),
                pairer: enqueue_reply(run.pairer, bytes), ..run
            }
        } else { run },
        RunAction::Pair => if can_pair(run.pairer) {
            LiveRun { pairer: pair_one(run.pairer), ..run }
        } else { run },
        RunAction::CloseInput => LiveRun { input_closed: true, ..run },
        RunAction::CloseOutput => LiveRun { stdout_closed: true, ..run },
        RunAction::CloseStderr => LiveRun { stderr_closed: true, ..run },
        RunAction::Exit(ok) => if !run.exited {
            LiveRun { exited: true, exit_ok: ok, ..run }
        } else { run },
        RunAction::Stderr => if !run.stderr_closed { LiveRun { quiet: false, ..run } } else { run },
        RunAction::Stop => LiveRun { pairer: stop_pairer(run.pairer), ..run },
    }
}

pub proof fn run_initially_valid(context: ModelContext, solver: Solver)
    ensures run_wf(run_init(context, solver)),
{
    pairer_initially_valid();
    empty_buffer_is_valid();
}

pub proof fn every_run_transition_preserves(run: LiveRun, action: RunAction)
    requires run_wf(run),
    ensures run_wf(run_step(run, action)),
        run_step(run, action).context == run.context,
        run_step(run, action).solver == run.solver,
{
    match action {
        RunAction::Input(bytes) => if !run.input_closed && !run.exited {
            let next = run_step(run, action);
            assert forall|i: int| 0 <= i < run.pairer.commands.len() implies
                next.pairer.commands[i].solver == next.solver
                && next.pairer.commands[i].mode == next.context.mode
                && next.pairer.commands[i].endpoint <= next.raw.len()
                && next.pairer.commands[i].prefix == next.raw.take(next.pairer.commands[i].endpoint as int) by {
                assert((run.raw + bytes).take(run.pairer.commands[i].endpoint as int)
                    =~= run.raw.take(run.pairer.commands[i].endpoint as int));
            }
        },
        RunAction::Command(endpoint) => if !run.exited && 0 < endpoint <= run.raw.len() {
            let binding = Binding { solver: run.solver, mode: run.context.mode,
                prefix: run.raw.take(endpoint as int), endpoint };
            command_arrival_preserves(run.pairer, binding);
            let next = run_step(run, action);
            assert forall|i: int| 0 <= i < next.pairer.commands.len() implies
                next.pairer.commands[i].solver == next.solver
                && next.pairer.commands[i].mode == next.context.mode
                && next.pairer.commands[i].endpoint <= next.raw.len()
                && next.pairer.commands[i].prefix == next.raw.take(next.pairer.commands[i].endpoint as int) by {
                if i < run.pairer.commands.len() {} else { assert(i == run.pairer.commands.len()); }
            }
        },
        RunAction::Output(bytes) => if !run.stdout_closed {
            response_fragment_preserves_bytes(run.output, bytes);
        },
        RunAction::Frame(endpoint) => if run.output.consumed < endpoint <= run.output.received.len() {
            let bytes = run.output.received.subrange(run.output.consumed as int, endpoint as int);
            completed_reply_preserves_all_bytes(run.output, endpoint);
            reply_arrival_preserves(run.pairer, bytes);
        },
        RunAction::Pair => if can_pair(run.pairer) { pairing_preserves_order(run.pairer); },
        RunAction::Stop => stop_is_permanent(run.pairer),
        _ => {},
    }
}

pub open spec fn run_replay(context: ModelContext, solver: Solver, actions: Seq<RunAction>) -> LiveRun
    decreases actions.len(),
{
    if actions.len() == 0 { run_init(context, solver) }
    else { run_step(run_replay(context, solver, actions.drop_last()), actions.last()) }
}

pub proof fn replayed_run_is_valid(context: ModelContext, solver: Solver, actions: Seq<RunAction>)
    ensures run_wf(run_replay(context, solver, actions)),
        run_replay(context, solver, actions).context == context,
        run_replay(context, solver, actions).solver == solver,
    decreases actions.len(),
{
    if actions.len() == 0 { run_initially_valid(context, solver); }
    else {
        replayed_run_is_valid(context, solver, actions.drop_last());
        every_run_transition_preserves(run_replay(context, solver, actions.drop_last()), actions.last());
    }
}

pub open spec fn run_complete(run: LiveRun) -> bool {
    run.input_closed && run.stdout_closed && run.stderr_closed && run.exited && run.exit_ok
    && run.quiet
    && !run.pairer.stopped
    && run.output.consumed == run.output.received.len()
    && run.pairer.paired == run.pairer.commands.len()
    && run.pairer.paired == run.pairer.replies.len()
}

/// The origin witness is ghost provenance from trusted process observations,
/// not an on-disk certificate that the current implementation can check.
pub struct Origin {
    pub run_id: nat,
    pub context: ModelContext,
    pub solver: Solver,
    pub actions: Seq<RunAction>,
    pub index: nat,
}

pub open spec fn origin_run(origin: Origin) -> LiveRun {
    run_replay(origin.context, origin.solver, origin.actions)
}

#[verifier::opaque]
pub open spec fn origin_valid(origin: Origin) -> bool {
    context_wf(origin.context)
    && run_complete(origin_run(origin))
    && origin.index < origin_run(origin).pairer.entries.len()
}

pub open spec fn origin_entry(origin: Origin) -> Entry
    recommends origin_valid(origin),
{
    origin_run(origin).pairer.entries[origin.index as int]
}

pub proof fn origin_has_complete_trusted_run(origin: Origin)
    requires origin_valid(origin),
    ensures
        run_wf(origin_run(origin)),
        origin_run(origin).input_closed,
        origin_run(origin).stdout_closed && origin_run(origin).stderr_closed,
        origin_run(origin).exited && origin_run(origin).exit_ok,
        origin_run(origin).quiet && !origin_run(origin).pairer.stopped,
        origin_entry(origin).binding == origin_run(origin).pairer.commands[origin.index as int],
        origin_entry(origin).reply == origin_run(origin).output.complete[origin.index as int],
        origin_entry(origin).binding.solver == origin.solver,
        origin_entry(origin).binding.mode == origin.context.mode,
        binding_wf(origin_entry(origin).binding),
        flatten(origin_run(origin).output.complete) == origin_run(origin).output.received,
{
    reveal(origin_valid);
    replayed_run_is_valid(origin.context, origin.solver, origin.actions);
    let run = origin_run(origin);
    assert(run.output.received.take(run.output.received.len() as int) =~= run.output.received);
}

pub struct Claim {
    pub entry: Entry,
    pub origin: Origin,
}

pub open spec fn claim_valid(claim: Claim) -> bool {
    origin_valid(claim.origin) && claim.entry == origin_entry(claim.origin)
}

pub open spec fn claim_from_origin(origin: Origin) -> Claim {
    Claim { entry: origin_entry(origin), origin }
}

/// The candidate payload and framing are untrusted. `claim` represents the
/// protected writer provenance that must refine the concrete record checks.
pub struct DiskFrame {
    pub claim: Claim,
    pub payload: Entry,
    pub complete: bool,
    pub final_name: bool,
}

pub open spec fn frame_valid(frame: DiskFrame) -> bool {
    frame.complete && frame.final_name && claim_valid(frame.claim)
    && frame.payload == frame.claim.entry
}

pub open spec fn recovered_claim(frame: DiskFrame) -> Claim {
    Claim { entry: frame.payload, origin: frame.claim.origin }
}

/// Only modeled trusted I/O events can extend a live-run observation history.
pub struct ObservedRun {
    pub context: ModelContext,
    pub solver: Solver,
    pub actions: Seq<RunAction>,
}

pub struct Store {
    pub runs: Seq<ObservedRun>,
    pub recorded: Seq<Origin>,
    pub available: Seq<Claim>,
    pub pending: Seq<Claim>,
    pub temporary: Seq<DiskFrame>,
    pub durable: Seq<DiskFrame>,
}

pub open spec fn origin_observed(store: Store, origin: Origin) -> bool {
    origin.run_id < store.runs.len()
    && origin.context == store.runs[origin.run_id as int].context
    && origin.solver == store.runs[origin.run_id as int].solver
    && origin.actions.len() <= store.runs[origin.run_id as int].actions.len()
    && origin.actions == store.runs[origin.run_id as int].actions.take(origin.actions.len() as int)
}

pub open spec fn store_origin_admitted(store: Store, origin: Origin) -> bool {
    store.recorded.contains(origin) && origin_valid(origin) && origin_observed(store, origin)
}

pub open spec fn store_wf(store: Store) -> bool {
    (forall|i: int| 0 <= i < store.recorded.len() ==> origin_valid(store.recorded[i]) && origin_observed(store, store.recorded[i]))
    && (forall|i: int| 0 <= i < store.available.len() ==>
        claim_valid(store.available[i]) && store.recorded.contains(store.available[i].origin))
    && (forall|i: int| 0 <= i < store.pending.len() ==>
        claim_valid(store.pending[i]) && store.recorded.contains(store.pending[i].origin))
}

/// An arbitrary initial disk image has no authority. A valid initial/recovered
/// entry must be connected to prior trusted publication in the modeled history.
pub open spec fn store_init(disk: Seq<DiskFrame>) -> Store {
    Store { runs: Seq::empty(), recorded: Seq::empty(), available: Seq::empty(),
        pending: Seq::empty(), temporary: Seq::empty(), durable: disk }
}

pub enum StoreAction {
    StartRun(ModelContext, Solver),
    ObserveRun(nat, RunAction),
    Publish(nat, nat),
    StartWrite,
    CompleteWrite,
    CommitWrite,
    Crash,
    Recover(nat),
    ReplaceDisk(nat, DiskFrame),
    ReplaceTemporary(nat, DiskFrame),
    ImportShared(DiskFrame),
    ImportBundled(DiskFrame),
}

pub open spec fn publication_origin(store: Store, run_index: nat, reply_index: nat) -> Origin {
    let run = store.runs[run_index as int];
    Origin { run_id: run_index, context: run.context, solver: run.solver, actions: run.actions, index: reply_index }
}

pub open spec fn can_recover(store: Store, index: nat) -> bool {
    index < store.durable.len()
    && frame_valid(store.durable[index as int])
    && store.recorded.contains(store.durable[index as int].claim.origin)
}

pub open spec fn store_step(store: Store, action: StoreAction) -> Store {
    match action {
        StoreAction::StartRun(context, solver) => Store {
            runs: store.runs.push(ObservedRun { context, solver, actions: Seq::empty() }), ..store },
        StoreAction::ObserveRun(index, event) => if index < store.runs.len() {
            let run = store.runs[index as int];
            Store { runs: store.runs.update(index as int, ObservedRun { actions: run.actions.push(event), ..run }), ..store }
        } else { store },
        StoreAction::Publish(run_index, reply_index) => if run_index < store.runs.len() {
            let origin = publication_origin(store, run_index, reply_index);
            if origin_valid(origin) {
                let claim = claim_from_origin(origin);
                Store { recorded: store.recorded.push(origin), available: store.available.push(claim),
                    pending: store.pending.push(claim), ..store }
            } else { store }
        } else { store },
        StoreAction::StartWrite => if store.pending.len() > 0 {
            let claim = store.pending[0];
            let frame = DiskFrame { claim, payload: claim.entry, complete: false, final_name: false };
            Store { pending: store.pending.drop_first(), temporary: store.temporary.push(frame), ..store }
        } else { store },
        StoreAction::CompleteWrite => if store.temporary.len() > 0 {
            let frame = DiskFrame { complete: true, ..store.temporary[0] };
            Store { temporary: store.temporary.update(0, frame), ..store }
        } else { store },
        StoreAction::CommitWrite => if store.temporary.len() > 0 {
            let old = store.temporary[0];
            let frame = DiskFrame { final_name: true, ..old };
            if frame_valid(frame) && store.recorded.contains(frame.claim.origin) {
                Store { temporary: store.temporary.drop_first(), durable: store.durable.push(frame), ..store }
            } else { store }
        } else { store },
        StoreAction::Crash => Store { available: Seq::empty(), pending: Seq::empty(), temporary: Seq::empty(), ..store },
        StoreAction::Recover(index) => if can_recover(store, index) {
            Store { available: store.available.push(recovered_claim(store.durable[index as int])), ..store }
        } else { store },
        StoreAction::ReplaceDisk(index, frame) => if index < store.durable.len() {
            Store { durable: store.durable.update(index as int, frame), available: Seq::empty(),
                pending: Seq::empty(), temporary: Seq::empty(), ..store }
        } else { store },
        StoreAction::ReplaceTemporary(index, frame) => if index < store.temporary.len() {
            Store { temporary: store.temporary.update(index as int, frame), ..store }
        } else { store },
        StoreAction::ImportShared(_) | StoreAction::ImportBundled(_) => store,
    }
}

pub proof fn sequence_push_retains<T>(items: Seq<T>, item: T, extra: T)
    requires items.contains(item),
    ensures items.push(extra).contains(item),
{
    let index = choose|i: int| 0 <= i < items.len() && items[i] == item;
    assert(items.push(extra)[index] == item);
}

pub proof fn empty_or_untrusted_initial_disk_is_safe(disk: Seq<DiskFrame>)
    ensures store_wf(store_init(disk)), store_init(disk).available.len() == 0,
        store_init(disk).recorded.len() == 0,
{}

pub proof fn every_store_transition_preserves(store: Store, action: StoreAction)
    requires store_wf(store),
    ensures store_wf(store_step(store, action)),
{
    match action {
        StoreAction::StartRun(context, solver) => {
            let next = store_step(store, action);
            assert forall|i: int| 0 <= i < next.recorded.len() implies
                origin_valid(next.recorded[i]) && origin_observed(next, next.recorded[i]) by {
                assert(next.recorded[i].run_id < store.runs.len());
                assert(next.runs[next.recorded[i].run_id as int] == store.runs[next.recorded[i].run_id as int]);
            }
        },
        StoreAction::ObserveRun(index, event) => if index < store.runs.len() {
            let next = store_step(store, action);
            assert forall|i: int| 0 <= i < next.recorded.len() implies
                origin_valid(next.recorded[i]) && origin_observed(next, next.recorded[i]) by {
                let origin = next.recorded[i];
                if origin.run_id == index {
                    let before = store.runs[index as int].actions;
                    assert(before.push(event).take(origin.actions.len() as int)
                        =~= before.take(origin.actions.len() as int));
                } else {
                    assert(next.runs[origin.run_id as int] == store.runs[origin.run_id as int]);
                }
            }
        },
        StoreAction::Publish(run_index, reply_index) => if run_index < store.runs.len() {
            let origin = publication_origin(store, run_index, reply_index);
            if origin_valid(origin) {
                let claim = claim_from_origin(origin);
                let next = store_step(store, action);
                assert(next.recorded[next.recorded.len() - 1] == origin);
                assert(next.recorded.contains(origin));
                assert forall|i: int| 0 <= i < next.recorded.len() implies
                    origin_valid(next.recorded[i]) && origin_observed(next, next.recorded[i]) by {
                    if i < store.recorded.len() {} else {
                        assert(i == store.recorded.len());
                        assert(store.runs[run_index as int].actions.take(origin.actions.len() as int)
                            =~= store.runs[run_index as int].actions);
                    }
                }
                assert forall|i: int| 0 <= i < next.available.len() implies
                    claim_valid(next.available[i]) && next.recorded.contains(next.available[i].origin) by {
                    if i < store.available.len() {
                        sequence_push_retains(store.recorded, store.available[i].origin, origin);
                    } else { assert(i == store.available.len()); }
                }
                assert forall|i: int| 0 <= i < next.pending.len() implies
                    claim_valid(next.pending[i]) && next.recorded.contains(next.pending[i].origin) by {
                    if i < store.pending.len() {
                        sequence_push_retains(store.recorded, store.pending[i].origin, origin);
                    } else { assert(i == store.pending.len()); }
                }
            }
        },
        StoreAction::StartWrite => if store.pending.len() > 0 {
            assert forall|i: int| 0 <= i < store.pending.drop_first().len()
                implies claim_valid(store.pending.drop_first()[i])
                    && store.recorded.contains(store.pending.drop_first()[i].origin) by {
                assert(store.pending.drop_first()[i] == store.pending[i+1]);
            }
        },
        StoreAction::Recover(index) => if can_recover(store, index) {
            let next = store_step(store, action);
            assert forall|i: int| 0 <= i < next.available.len() implies
                claim_valid(next.available[i]) && next.recorded.contains(next.available[i].origin) by {
                if i < store.available.len() {} else { assert(i == store.available.len()); }
            }
        },
        _ => {},
    }
}

pub open spec fn store_replay(disk: Seq<DiskFrame>, actions: Seq<StoreAction>) -> Store
    decreases actions.len(),
{
    if actions.len() == 0 { store_init(disk) }
    else { store_step(store_replay(disk, actions.drop_last()), actions.last()) }
}

pub proof fn replayed_store_has_only_valid_origins(disk: Seq<DiskFrame>, actions: Seq<StoreAction>)
    ensures store_wf(store_replay(disk, actions)),
    decreases actions.len(),
{
    if actions.len() == 0 { empty_or_untrusted_initial_disk_is_safe(disk); }
    else {
        replayed_store_has_only_valid_origins(disk, actions.drop_last());
        every_store_transition_preserves(store_replay(disk, actions.drop_last()), actions.last());
    }
}

pub proof fn rejected_recovery_cannot_add_evidence(store: Store, index: nat)
    requires index < store.durable.len(),
        !frame_valid(store.durable[index as int])
            || !store.recorded.contains(store.durable[index as int].claim.origin),
    ensures store_step(store, StoreAction::Recover(index)) == store,
{}

pub proof fn crash_cannot_publish_pending_or_temporary(store: Store)
    ensures
        store_step(store, StoreAction::Crash).available.len() == 0,
        store_step(store, StoreAction::Crash).pending.len() == 0,
        store_step(store, StoreAction::Crash).temporary.len() == 0,
        store_step(store, StoreAction::Crash).durable == store.durable,
        store_step(store, StoreAction::Crash).recorded == store.recorded,
{}

pub proof fn shared_and_bundled_origins_are_excluded(store: Store, frame: DiskFrame)
    ensures
        store_step(store, StoreAction::ImportShared(frame)) == store,
        store_step(store, StoreAction::ImportBundled(frame)) == store,
{}

pub proof fn ledger_member_has_observed_origin(store: Store, origin: Origin)
    requires store_wf(store), store.recorded.contains(origin),
    ensures origin_valid(origin), origin_observed(store, origin), store_origin_admitted(store, origin),
{
    let index = choose|i: int| 0 <= i < store.recorded.len() && store.recorded[i] == origin;
}

/// Every visible store claim has an exact reply from a complete clean trusted
/// observation trace. No premise states that an arbitrary initial cache is true.
pub proof fn stored_reply_has_admitted_origin(disk: Seq<DiskFrame>, actions: Seq<StoreAction>, index: int)
    requires 0 <= index < store_replay(disk, actions).available.len(),
    ensures
        claim_valid(store_replay(disk, actions).available[index]),
        store_origin_admitted(store_replay(disk, actions), store_replay(disk, actions).available[index].origin),
        store_replay(disk, actions).available[index].entry
            == origin_entry(store_replay(disk, actions).available[index].origin),
{
    replayed_store_has_only_valid_origins(disk, actions);
    ledger_member_has_observed_origin(store_replay(disk, actions), store_replay(disk, actions).available[index].origin);
}
/// A session starts cache-serving and can move to live replay only once.
/// Cache contents are derived from the checked store history; they are not
/// supplied as an arbitrary sequence of supposedly truthful entries.
pub struct Session {
    pub context: ModelContext,
    pub solver: Solver,
    pub raw: Seq<u8>,
    pub commands: Seq<Binding>,
    pub initial_disk: Seq<DiskFrame>,
    pub cache_history: Seq<StoreAction>,
    pub cache: Seq<Claim>,
    pub origin_ledger: Seq<Origin>,
    pub delivered: Seq<Entry>,
    pub cached: nat,
    pub live: bool,
    pub connected: bool,
    pub solver_input: Seq<u8>,
    pub skip: nat,
    pub seen: nat,
    pub live_observed: Seq<Seq<u8>>,
}

pub open spec fn entry_has_admitted_origin(cache: Seq<Claim>, ledger: Seq<Origin>, entry: Entry) -> bool {
    exists|claim: Claim| cache.contains(claim) && claim_valid(claim) && ledger.contains(claim.origin) && claim.entry == entry
}

pub open spec fn cached_reply_has_origin(s: Session, index: int) -> bool {
    0 <= index < s.cached && entry_has_admitted_origin(s.cache, s.origin_ledger, s.delivered[index])
}

/// Fresh replies bind an ordered observation of the active trusted solver;
/// reused replies additionally carry a complete clean publication origin.
pub open spec fn session_reply_has_origin(s: Session, index: int) -> bool {
    0 <= index < s.delivered.len()
    && if index < s.cached { cached_reply_has_origin(s, index) }
    else { s.live && index < s.live_observed.len() && s.delivered[index].reply == s.live_observed[index] }
}

pub open spec fn session_wf(s: Session) -> bool {
    context_wf(s.context)
    && s.cache == store_replay(s.initial_disk, s.cache_history).available
    && s.origin_ledger == store_replay(s.initial_disk, s.cache_history).recorded
    && (forall|i: int| 0 <= i < s.cache.len() ==> claim_valid(s.cache[i]) && s.origin_ledger.contains(s.cache[i].origin))
    && s.live_observed.len() == s.seen
    && (forall|i: int| s.cached <= i < s.delivered.len() ==> s.delivered[i].reply == s.live_observed[i])
    && s.delivered.len() <= s.commands.len()
    && s.cached <= s.delivered.len()
    && (forall|i: int| 0 <= i < s.commands.len() ==>
        binding_wf(s.commands[i])
        && s.commands[i].solver == s.solver
        && s.commands[i].mode == s.context.mode
        && s.commands[i].endpoint <= s.raw.len()
        && s.commands[i].prefix == s.raw.take(s.commands[i].endpoint as int))
    && (forall|i: int| 0 <= i < s.delivered.len() ==>
        s.delivered[i].binding == s.commands[i])
    && (forall|i: int| 0 <= i < s.cached ==>
        entry_has_admitted_origin(s.cache, s.origin_ledger, s.delivered[i]))
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

pub open spec fn session_init(context: ModelContext, solver: Solver,
    initial_disk: Seq<DiskFrame>, cache_history: Seq<StoreAction>) -> Session {
    Session {
        context, solver, initial_disk, cache_history,
        cache: store_replay(initial_disk, cache_history).available,
        origin_ledger: store_replay(initial_disk, cache_history).recorded,
        raw: Seq::empty(), commands: Seq::empty(), delivered: Seq::empty(),
        cached: 0, live: false, connected: true, solver_input: Seq::empty(),
        skip: 0, seen: 0, live_observed: Seq::empty(),
    }
}

pub proof fn session_initially_valid(context: ModelContext, solver: Solver,
    initial_disk: Seq<DiskFrame>, cache_history: Seq<StoreAction>)
    requires context_wf(context),
    ensures session_wf(session_init(context, solver, initial_disk, cache_history)),
{
    replayed_store_has_only_valid_origins(initial_disk, cache_history);
}

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
        && next.commands[i].mode == next.context.mode
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
        solver: s.solver, mode: s.context.mode,
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
        && next.commands[i].mode == next.context.mode
        && next.commands[i].endpoint <= next.raw.len()
        && next.commands[i].prefix == next.raw.take(next.commands[i].endpoint as int) by {
        if i < s.commands.len() {} else { assert(i == s.commands.len()); }
    }
}

pub open spec fn can_serve(s: Session, claim: Claim) -> bool {
    !s.live && s.connected
    && s.delivered.len() < s.commands.len()
    && s.cache.contains(claim) && claim_valid(claim) && s.origin_ledger.contains(claim.origin)
    && claim.entry.binding == s.commands[s.delivered.len() as int]
}

pub open spec fn serve_cached(s: Session, claim: Claim) -> Session {
    Session { delivered: s.delivered.push(claim.entry), cached: s.cached + 1, ..s }
}

pub proof fn cache_serve_preserves_association(s: Session, claim: Claim)
    requires session_wf(s), can_serve(s, claim),
    ensures session_wf(serve_cached(s, claim)),
{
    let next = serve_cached(s, claim);
    assert forall|i: int| 0 <= i < next.delivered.len()
        implies next.delivered[i].binding == next.commands[i] by {
        if i < s.delivered.len() {} else { assert(i == s.delivered.len()); }
    }
    assert forall|i: int| 0 <= i < next.cached implies cached_reply_has_origin(next, i) by {
        if i < s.cached {
            assert(entry_has_admitted_origin(s.cache, s.origin_ledger, s.delivered[i]));
            let previous = choose|c: Claim| s.cache.contains(c) && claim_valid(c) && s.origin_ledger.contains(c.origin) && c.entry == s.delivered[i];
            assert(next.cache.contains(previous) && claim_valid(previous) && next.origin_ledger.contains(previous.origin) && previous.entry == next.delivered[i]);
        } else {
            assert(i == s.cached);
            assert(next.cache.contains(claim) && claim_valid(claim) && next.origin_ledger.contains(claim.origin) && claim.entry == next.delivered[i]);
        }
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
pub proof fn late_cache_reply_cannot_be_served(s: Session, claim: Claim, failed: bool)
    requires session_wf(s), !s.live,
    ensures !can_serve(begin_fallback(s, failed), claim),
{}

pub open spec fn live_reply(s: Session, bytes: Seq<u8>) -> Session
    recommends s.live, s.seen < s.commands.len(),
{
    let entry = Entry { binding: s.commands[s.seen as int], reply: bytes };
    Session {
        delivered: if s.seen < s.skip { s.delivered } else { s.delivered.push(entry) },
        seen: s.seen + 1,
        live_observed: s.live_observed.push(bytes),
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
    assert forall|i: int| 0 <= i < next.cached
        implies entry_has_admitted_origin(next.cache, next.origin_ledger, next.delivered[i]) by {
        assert(i < s.delivered.len());
        assert(next.delivered[i] == s.delivered[i]);
    }
    if s.seen >= s.skip {
        assert(s.delivered.len() == s.seen);
        assert forall|i: int| 0 <= i < next.delivered.len()
            implies next.delivered[i].binding == next.commands[i] by {
            if i < s.delivered.len() {} else { assert(i == s.seen); }
        }
        assert forall|i: int| next.cached <= i < next.delivered.len()
            implies next.delivered[i].reply == next.live_observed[i] by {
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
        s.delivered[index].binding.mode == s.context.mode,
        s.delivered[index].binding.endpoint <= s.raw.len(),
        s.delivered[index].binding.prefix
            == s.raw.take(s.delivered[index].binding.endpoint as int),
{}

/// Unsupported or inapplicable actions leave the modeled state unchanged.
pub enum Action {
    Input(Seq<u8>),
    Command(nat),
    Cache(Claim),
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
        step(s, action).context == s.context, step(s, action).solver == s.solver,
        step(s, action).initial_disk == s.initial_disk, step(s, action).cache_history == s.cache_history,
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
    && context_wf(states[0].context)
    && states[0] == session_init(states[0].context, states[0].solver, states[0].initial_disk, states[0].cache_history)
    && forall|i: int| 0 <= i < actions.len() ==> states[i + 1] == step(states[i], actions[i])
}

pub proof fn all_finite_executions_preserve(states: Seq<Session>, actions: Seq<Action>, index: nat)
    requires execution(states, actions), index < states.len(),
    ensures session_wf(states[index as int]),
        states[index as int].context == states[0].context,
        states[index as int].solver == states[0].solver,
    decreases index,
{
    if index == 0 {
        session_initially_valid(states[0].context, states[0].solver, states[0].initial_disk, states[0].cache_history);
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
            == states[state_index as int].context.mode,
        states[state_index as int].delivered[reply_index].binding.prefix
            == states[state_index as int].raw.take(
                states[state_index as int].delivered[reply_index].binding.endpoint as int),
{
    all_finite_executions_preserve(states, actions, state_index);
    every_delivered_reply_binds_its_requested_prefix(states[state_index as int], reply_index);
}

pub proof fn every_reused_reply_has_clean_origin(s: Session, index: int)
    requires session_wf(s), 0 <= index < s.cached,
    ensures cached_reply_has_origin(s, index),
        exists|origin: Origin| store_origin_admitted(store_replay(s.initial_disk, s.cache_history), origin)
            && origin_entry(origin) == s.delivered[index],
{
    assert(entry_has_admitted_origin(s.cache, s.origin_ledger, s.delivered[index]));
    let claim = choose|c: Claim| s.cache.contains(c) && claim_valid(c) && s.origin_ledger.contains(c.origin) && c.entry == s.delivered[index];
    replayed_store_has_only_valid_origins(s.initial_disk, s.cache_history);
    ledger_member_has_observed_origin(store_replay(s.initial_disk, s.cache_history), claim.origin);
    assert(store_origin_admitted(store_replay(s.initial_disk, s.cache_history), claim.origin)
        && origin_entry(claim.origin) == s.delivered[index]);
}

pub proof fn executed_reply_has_admitted_origin(
    states: Seq<Session>, actions: Seq<Action>, state_index: nat, reply_index: int,
)
    requires execution(states, actions), state_index < states.len(),
        0 <= reply_index < states[state_index as int].delivered.len(),
    ensures
        session_reply_has_origin(states[state_index as int], reply_index),
        reply_index < states[state_index as int].cached ==>
            exists|origin: Origin| store_origin_admitted(store_replay(
                states[state_index as int].initial_disk, states[state_index as int].cache_history), origin)
                && origin_entry(origin) == states[state_index as int].delivered[reply_index],
{
    all_finite_executions_preserve(states, actions, state_index);
    let state = states[state_index as int];
    if reply_index < state.cached { every_reused_reply_has_clean_origin(state, reply_index); }
}

pub open spec fn session_complete(s: Session) -> bool {
    s.delivered.len() == s.commands.len()
}

pub open spec fn session_transcript(s: Session) -> SolverTranscript {
    SolverTranscript {
        solver: s.solver,
        raw: s.raw,
        endpoints: s.commands.map(|i: int, binding: Binding| binding.endpoint),
        replies: s.delivered.map(|i: int, entry: Entry| entry.reply),
    }
}

pub proof fn executed_transcript_projects_observations(
    states: Seq<Session>, actions: Seq<Action>, state_index: nat,
)
    requires execution(states, actions), state_index < states.len(),
    ensures
        session_wf(states[state_index as int]),
        session_transcript(states[state_index as int]).solver == states[state_index as int].solver,
        session_transcript(states[state_index as int]).raw == states[state_index as int].raw,
        session_transcript(states[state_index as int]).endpoints.len() == states[state_index as int].commands.len(),
        session_transcript(states[state_index as int]).replies.len() == states[state_index as int].delivered.len(),
        session_complete(states[state_index as int]) ==>
            session_transcript(states[state_index as int]).endpoints.len()
                == session_transcript(states[state_index as int]).replies.len(),
        forall|i: int| 0 <= i < states[state_index as int].commands.len() ==>
            session_transcript(states[state_index as int]).endpoints[i]
                == states[state_index as int].commands[i].endpoint
            && states[state_index as int].commands[i].prefix
                == session_transcript(states[state_index as int]).raw.take(
                    session_transcript(states[state_index as int]).endpoints[i] as int),
        forall|i: int| 0 <= i < states[state_index as int].delivered.len() ==>
            session_transcript(states[state_index as int]).replies[i]
                == states[state_index as int].delivered[i].reply
            && session_reply_has_origin(states[state_index as int], i),
{
    all_finite_executions_preserve(states, actions, state_index);
    let s = states[state_index as int];
    assert forall|i: int| 0 <= i < s.delivered.len() implies
        session_transcript(s).replies[i] == s.delivered[i].reply
        && session_reply_has_origin(s, i) by {
        if i < s.cached { every_reused_reply_has_clean_origin(s, i); }
    }
}

pub proof fn store_replay_after_push(disk: Seq<DiskFrame>, actions: Seq<StoreAction>, action: StoreAction)
    ensures store_replay(disk, actions.push(action)) == store_step(store_replay(disk, actions), action),
{
    assert(actions.push(action).drop_last() =~= actions);
}

pub proof fn run_replay_after_push(context: ModelContext, solver: Solver, actions: Seq<RunAction>, action: RunAction)
    ensures run_replay(context, solver, actions.push(action)) == run_step(run_replay(context, solver, actions), action),
{
    assert(actions.push(action).drop_last() =~= actions);
}

pub open spec fn observing_history(context: ModelContext, solver: Solver, events: Seq<RunAction>) -> Seq<StoreAction>
    decreases events.len(),
{
    if events.len() == 0 { seq![StoreAction::StartRun(context, solver)] }
    else { observing_history(context, solver, events.drop_last()).push(StoreAction::ObserveRun(0, events.last())) }
}

pub proof fn observing_history_captures_exact_run(context: ModelContext, solver: Solver, events: Seq<RunAction>)
    ensures
        store_replay(Seq::empty(), observing_history(context, solver, events)).runs.len() == 1,
        store_replay(Seq::empty(), observing_history(context, solver, events)).runs[0].context == context,
        store_replay(Seq::empty(), observing_history(context, solver, events)).runs[0].solver == solver,
        store_replay(Seq::empty(), observing_history(context, solver, events)).runs[0].actions == events,
        store_replay(Seq::empty(), observing_history(context, solver, events)).available.len() == 0,
        store_replay(Seq::empty(), observing_history(context, solver, events)).recorded.len() == 0,
        store_replay(Seq::empty(), observing_history(context, solver, events)).pending.len() == 0,
        store_replay(Seq::empty(), observing_history(context, solver, events)).temporary.len() == 0,
        store_replay(Seq::empty(), observing_history(context, solver, events)).durable.len() == 0,
    decreases events.len(),
{
    if events.len() == 0 {
        reveal_with_fuel(store_replay, 2);
    } else {
        let before = events.drop_last();
        observing_history_captures_exact_run(context, solver, before);
        store_replay_after_push(Seq::empty(), observing_history(context, solver, before), StoreAction::ObserveRun(0, events.last()));
        assert(before.push(events.last()) =~= events);
    }
}

pub open spec fn example_solver() -> Solver {
    Solver { name: seq![90u8, 51u8], version: seq![49u8], binary: seq![0u8], platform: seq![1u8] }
}

pub open spec fn example_context(request: u8, source: u8) -> ModelContext {
    ModelContext {
        request: seq![request], scope: ScopeKey { whole: true, selection: Seq::empty() },
        runtime: seq![7u8], mode: Mode::Default,
        inputs: Map::empty().insert(InputKey { role: 1, lexical: seq![1u8] },
            ReadOutcome::Present { resolved: seq![2u8], bytes: seq![source] }),
    }
}

pub proof fn example_context_is_readable(request: u8, source: u8)
    ensures context_wf(example_context(request, source)), example_context(request, source).inputs.dom().len() == 1,
{
    let context = example_context(request, source);
    let only = InputKey { role: 1, lexical: seq![1u8] };
    assert forall|key: InputKey| context.inputs.dom().contains(key)
        implies crate::context::readable(context.inputs[key]) by { assert(key == only); }
}

pub open spec fn example_run_events() -> Seq<RunAction> {
    seq![RunAction::Input(seq![1u8]), RunAction::Command(1),
        RunAction::Output(seq![11u8]), RunAction::Frame(1), RunAction::Pair,
        RunAction::CloseInput, RunAction::CloseOutput, RunAction::CloseStderr, RunAction::Exit(true)]
}

pub open spec fn example_origin() -> Origin {
    Origin { run_id: 0, context: example_context(1, 31), solver: example_solver(),
        actions: example_run_events(), index: 0 }
}

pub proof fn positive_origin_is_complete_and_nonempty()
    ensures origin_valid(example_origin()),
        origin_entry(example_origin()).binding.prefix == seq![1u8],
        origin_entry(example_origin()).binding == (Binding {
            solver: example_solver(), mode: Mode::Default, prefix: seq![1u8], endpoint: 1 }),
        origin_entry(example_origin()).reply == seq![11u8],
{
    example_context_is_readable(1, 31);
    reveal(origin_valid);
    reveal_with_fuel(run_replay, 10);
    assert(origin_run(example_origin()).pairer.entries.len() == 1);
    assert(origin_run(example_origin()).pairer.entries[0].binding.prefix =~= seq![1u8]);
}

#[verifier::opaque]
pub open spec fn example_recovery_history() -> Seq<StoreAction> {
    observing_history(example_context(1, 31), example_solver(), example_run_events())
        .push(StoreAction::Publish(0, 0)).push(StoreAction::StartWrite)
        .push(StoreAction::CompleteWrite).push(StoreAction::CommitWrite)
        .push(StoreAction::Crash).push(StoreAction::Recover(0))
}

pub proof fn positive_published_write_crash_recovery()
    ensures
        store_replay(Seq::empty(), example_recovery_history()).available.len() == 1,
        store_replay(Seq::empty(), example_recovery_history()).durable.len() == 1,
        store_origin_admitted(store_replay(Seq::empty(), example_recovery_history()), example_origin()),
        store_replay(Seq::empty(), example_recovery_history()).available[0] == claim_from_origin(example_origin()),
        store_replay(Seq::empty(), example_recovery_history()).recorded.len() == 1,
        store_replay(Seq::empty(), example_recovery_history()).recorded[0] == example_origin(),
        store_replay(Seq::empty(), example_recovery_history()).durable[0].claim == claim_from_origin(example_origin()),
        store_replay(Seq::empty(), example_recovery_history()).durable[0].payload == origin_entry(example_origin()),
{
    reveal(example_recovery_history);
    positive_origin_is_complete_and_nonempty();
    let observed = observing_history(example_context(1, 31), example_solver(), example_run_events());
    observing_history_captures_exact_run(example_context(1, 31), example_solver(), example_run_events());
    assert(publication_origin(store_replay(Seq::empty(), observed), 0, 0) == example_origin());
    let published = observed.push(StoreAction::Publish(0, 0));
    store_replay_after_push(Seq::empty(), observed, StoreAction::Publish(0, 0));
    assert(store_replay(Seq::empty(), published).pending.len() == 1);
    assert(store_replay(Seq::empty(), published).recorded[0] == example_origin());
    assert(store_replay(Seq::empty(), published).recorded.contains(example_origin()));
    let started = published.push(StoreAction::StartWrite);
    store_replay_after_push(Seq::empty(), published, StoreAction::StartWrite);
    assert(store_replay(Seq::empty(), started).temporary.len() == 1);
    let written = started.push(StoreAction::CompleteWrite);
    store_replay_after_push(Seq::empty(), started, StoreAction::CompleteWrite);
    assert(store_replay(Seq::empty(), written).temporary[0].complete);
    let committed = written.push(StoreAction::CommitWrite);
    store_replay_after_push(Seq::empty(), written, StoreAction::CommitWrite);
    assert(store_replay(Seq::empty(), committed).durable.len() == 1);
    let crashed = committed.push(StoreAction::Crash);
    store_replay_after_push(Seq::empty(), committed, StoreAction::Crash);
    assert(store_replay(Seq::empty(), crashed).available.len() == 0);
    let recovered = crashed.push(StoreAction::Recover(0));
    store_replay_after_push(Seq::empty(), crashed, StoreAction::Recover(0));
    replayed_store_has_only_valid_origins(Seq::empty(), recovered);
    assert(store_replay(Seq::empty(), recovered).available.len() == 1);
}

pub proof fn positive_unpublished_boot_record_is_refused()
{
    positive_origin_is_complete_and_nonempty();
    let claim = claim_from_origin(example_origin());
    let frame = DiskFrame { claim, payload: claim.entry, complete: true, final_name: true };
    assert(frame_valid(frame));
    let boot = store_init(seq![frame]);
    assert(!boot.recorded.contains(frame.claim.origin));
    rejected_recovery_cannot_add_evidence(boot, 0);
    assert(store_step(boot, StoreAction::Recover(0)).available.len() == 0);
}

pub proof fn positive_partial_or_corrupt_recovery_is_refused()
{
    positive_origin_is_complete_and_nonempty();
    positive_published_write_crash_recovery();
    let good = store_replay(Seq::empty(), example_recovery_history());
    let original = good.durable[0];
    let partial = DiskFrame { complete: false, ..original };
    let partial_disk = store_step(good, StoreAction::ReplaceDisk(0, partial));
    assert(!frame_valid(partial_disk.durable[0]));
    rejected_recovery_cannot_add_evidence(partial_disk, 0);
    assert(store_step(partial_disk, StoreAction::Recover(0)).available.len() == 0);
    let changed = Entry { binding: original.payload.binding, reply: seq![99u8] };
    let corrupt = DiskFrame { payload: changed, ..original };
    assert(original.claim.entry.reply == seq![11u8]);
    singleton_bytes_are_distinct(99, 11);
    assert(changed.reply[0] == 99 && original.claim.entry.reply[0] == 11);
    assert(corrupt.payload != corrupt.claim.entry);
    assert(!frame_valid(corrupt));
    let corrupt_disk = store_step(good, StoreAction::ReplaceDisk(0, corrupt));
    rejected_recovery_cannot_add_evidence(corrupt_disk, 0);
    assert(store_step(corrupt_disk, StoreAction::Recover(0)).available.len() == 0);
}

pub proof fn positive_crash_before_rename_loses_only_unpublished_write()
{
    positive_origin_is_complete_and_nonempty();
    let observed = observing_history(example_context(1, 31), example_solver(), example_run_events());
    observing_history_captures_exact_run(example_context(1, 31), example_solver(), example_run_events());
    assert(publication_origin(store_replay(Seq::empty(), observed), 0, 0) == example_origin());
    let published = observed.push(StoreAction::Publish(0, 0));
    store_replay_after_push(Seq::empty(), observed, StoreAction::Publish(0, 0));
    assert(store_replay(Seq::empty(), published).pending.len() == 1);
    let started = published.push(StoreAction::StartWrite);
    store_replay_after_push(Seq::empty(), published, StoreAction::StartWrite);
    let writing = store_replay(Seq::empty(), started);
    assert(writing.temporary.len() == 1 && !writing.temporary[0].complete);
    assert(writing.durable.len() == 0);
    let crashed_history = started.push(StoreAction::Crash);
    store_replay_after_push(Seq::empty(), started, StoreAction::Crash);
    let crashed = store_replay(Seq::empty(), crashed_history);
    crash_cannot_publish_pending_or_temporary(writing);
    assert(crashed.durable.len() == 0);
    assert(store_step(crashed, StoreAction::Recover(0)).available.len() == 0);
}

pub proof fn run_noise_never_becomes_quiet(run: LiveRun, action: RunAction)
    requires !run.quiet,
    ensures !run_step(run, action).quiet,
{}

pub proof fn noised_run_replay_is_never_clean(context: ModelContext, solver: Solver, tail: Seq<RunAction>)
    ensures !run_replay(context, solver, seq![RunAction::Stderr] + tail).quiet,
    decreases tail.len(),
{
    if tail.len() == 0 {
        assert(seq![RunAction::Stderr] + tail =~= seq![RunAction::Stderr]);
        reveal_with_fuel(run_replay, 2);
    } else {
        let before = tail.drop_last();
        noised_run_replay_is_never_clean(context, solver, before);
        let prefix = seq![RunAction::Stderr] + before;
        assert(seq![RunAction::Stderr] + tail =~= prefix.push(tail.last()));
        run_replay_after_push(context, solver, prefix, tail.last());
        run_noise_never_becomes_quiet(run_replay(context, solver, prefix), tail.last());
    }
}

pub proof fn positive_noisy_live_run_cannot_publish()
{
    let context = example_context(1, 31);
    let solver = example_solver();
    let noisy = seq![RunAction::Stderr] + example_run_events();
    let history = observing_history(context, solver, noisy);
    observing_history_captures_exact_run(context, solver, noisy);
    let before = store_replay(Seq::empty(), history);
    let origin = publication_origin(before, 0, 0);
    assert(origin.actions == noisy);
    noised_run_replay_is_never_clean(context, solver, example_run_events());
    assert(!origin_run(origin).quiet);
    reveal(origin_valid);
    assert(!origin_valid(origin));
    assert(store_step(before, StoreAction::Publish(0, 0)).available.len() == 0);
    assert(store_step(before, StoreAction::Publish(0, 0)).recorded.len() == 0);
}

pub open spec fn session_states(context: ModelContext, solver: Solver, disk: Seq<DiskFrame>,
    cache_history: Seq<StoreAction>, actions: Seq<Action>) -> Seq<Session>
    decreases actions.len(),
{
    if actions.len() == 0 { seq![session_init(context, solver, disk, cache_history)] }
    else {
        let before = session_states(context, solver, disk, cache_history, actions.drop_last());
        before.push(step(before.last(), actions.last()))
    }
}

pub proof fn constructed_session_history_is_execution(context: ModelContext, solver: Solver,
    disk: Seq<DiskFrame>, cache_history: Seq<StoreAction>, actions: Seq<Action>)
    requires context_wf(context),
    ensures
        execution(session_states(context, solver, disk, cache_history, actions), actions),
        session_states(context, solver, disk, cache_history, actions).len() == actions.len() + 1,
        session_states(context, solver, disk, cache_history, actions)[0]
            == session_init(context, solver, disk, cache_history),
    decreases actions.len(),
{
    let states = session_states(context, solver, disk, cache_history, actions);
    if actions.len() == 0 {} else {
        let before = actions.drop_last();
        constructed_session_history_is_execution(context, solver, disk, cache_history, before);
        let previous = session_states(context, solver, disk, cache_history, before);
        assert(states =~= previous.push(step(previous.last(), actions.last())));
        assert forall|i: int| 0 <= i < actions.len()
            implies states[i + 1] == step(states[i], actions[i]) by {
            if i < before.len() {
                assert(previous[i + 1] == step(previous[i], before[i]));
                assert(states[i] == previous[i] && states[i + 1] == previous[i + 1]);
                assert(actions[i] == before[i]);
            } else { assert(i == before.len()); }
        }
    }
}

pub open spec fn example_session_actions() -> Seq<Action> {
    seq![Action::Input(seq![1u8]), Action::Command(1), Action::Cache(claim_from_origin(example_origin())),
        Action::Input(seq![2u8]), Action::Command(2), Action::Fallback(true),
        Action::Reply(seq![11u8]), Action::Reply(seq![22u8])]
}

pub open spec fn example_session_states() -> Seq<Session> {
    let initial = session_init(example_context(2, 32), example_solver(), Seq::empty(), example_recovery_history());
    let a = step(initial, Action::Input(seq![1u8]));
    let b = step(a, Action::Command(1));
    let c = step(b, Action::Cache(claim_from_origin(example_origin())));
    let d = step(c, Action::Input(seq![2u8]));
    let e = step(d, Action::Command(2));
    let f = step(e, Action::Fallback(true));
    let g = step(f, Action::Reply(seq![11u8]));
    let h = step(g, Action::Reply(seq![22u8]));
    seq![initial, a, b, c, d, e, f, g, h]
}

pub proof fn positive_cache_then_live_has_actual_origins()
    ensures
        execution(example_session_states(), example_session_actions()),
        example_session_states().len() == 9,
        session_complete(example_session_states()[8]),
        example_session_states()[8].cached == 1,
        session_transcript(example_session_states()[8]).raw == seq![1u8, 2u8],
        session_transcript(example_session_states()[8]).endpoints == seq![1nat, 2nat],
        session_transcript(example_session_states()[8]).replies == seq![seq![11u8], seq![22u8]],
        session_reply_has_origin(example_session_states()[8], 0),
        session_reply_has_origin(example_session_states()[8], 1),
        example_origin().context.inputs != example_session_states()[8].context.inputs,
{
    positive_published_write_crash_recovery();
    positive_origin_is_complete_and_nonempty();
    example_context_is_readable(2, 32);
    let context = example_context(2, 32);
    let solver = example_solver();
    let history = example_recovery_history();
    let initial = session_init(context, solver, Seq::empty(), history);
    session_initially_valid(context, solver, Seq::empty(), history);
    let claim = claim_from_origin(example_origin());
    assert(initial.cache.len() == 1 && initial.cache[0] == claim);
    assert(initial.cache.contains(claim));
    assert(initial.origin_ledger[0] == example_origin());
    assert(initial.origin_ledger.contains(example_origin()));
    let a = input_fragment(initial, seq![1u8]);
    fragments_preserve_exact_input(initial, seq![1u8]);
    assert(a.context == context);
    let b = observe_command(a, 1);
    command_observation_preserves(a, 1);
    assert(b.context == context);
    assert(b.commands[0].solver == solver && b.commands[0].mode == Mode::Default);
    assert(b.commands[0].endpoint == 1);
    assert(b.commands[0].prefix =~= seq![1u8]);
    assert(b.commands[0] == claim.entry.binding);
    assert(can_serve(b, claim));
    let c = serve_cached(b, claim);
    cache_serve_preserves_association(b, claim);
    assert(c.context == context);
    let d = input_fragment(c, seq![2u8]);
    fragments_preserve_exact_input(c, seq![2u8]);
    assert(d.context == context);
    let e = observe_command(d, 2);
    command_observation_preserves(d, 2);
    assert(e.context == context);
    let f = begin_fallback(e, true);
    fallback_replays_exact_prefix(e, true);
    assert(f.context == context);
    let g = live_reply(f, seq![11u8]);
    replay_suppresses_exactly_served_replies(f, seq![11u8]);
    assert(g.context == context);
    let h = live_reply(g, seq![22u8]);
    replay_suppresses_exactly_served_replies(g, seq![22u8]);
    assert(h.context == context);
    assert(h.delivered.len() == 2 && h.cached == 1);
    assert(h.delivered[0].reply == seq![11u8] && h.delivered[1].reply == seq![22u8]);
    assert(example_session_states() =~= seq![initial, a, b, c, d, e, f, g, h]);
    assert forall|i: int| 0 <= i < example_session_actions().len() implies
        example_session_states()[i + 1] == step(example_session_states()[i], example_session_actions()[i]) by {
        if i == 0 {} else if i == 1 {} else if i == 2 {} else if i == 3 {}
        else if i == 4 {} else if i == 5 {} else if i == 6 {} else { assert(i == 7); }
    }
    assert(execution(example_session_states(), example_session_actions()));
    assert(example_session_states()[8] == h);
    executed_reply_has_admitted_origin(example_session_states(), example_session_actions(), 8, 0);
    executed_reply_has_admitted_origin(example_session_states(), example_session_actions(), 8, 1);
    let key = InputKey { role: 1, lexical: seq![1u8] };
    assert(example_origin().context.inputs[key] == ReadOutcome::Present { resolved: seq![2u8], bytes: seq![31u8] });
    assert(h.context.inputs[key] == ReadOutcome::Present { resolved: seq![2u8], bytes: seq![32u8] });
    singleton_bytes_are_distinct(31, 32);
    assert(example_origin().context.inputs[key] != h.context.inputs[key]);
    assert(session_transcript(h).endpoints =~= seq![1nat, 2nat]);
    assert(session_transcript(h).replies =~= seq![seq![11u8], seq![22u8]]);
}

pub proof fn singleton_bytes_are_distinct(a: u8, b: u8)
    requires a != b,
    ensures seq![a] != seq![b],
{
    assert(seq![a][0] == a && seq![b][0] == b);
}

} // verus!

fn main() {}
