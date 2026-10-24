//! Conditional composition of the actual checked model components.
#![forbid(unsafe_code)]

use vstd::prelude::*;
use crate::context::{ModelContext, SolverTranscript, SolverId, InputKey, ReadOutcome,
    ScopeKey, Mode, context_wf, transcript_wf};
use crate::query_association as query;
use crate::input_binding as input;
use crate::authority as authority;

verus! {

/// An actual finite modeled shim execution, not a supplied success flag.
pub struct QueryExecution {
    pub states: Seq<query::Session>,
    pub actions: Seq<query::Action>,
}

/// One execution for every solver stream observed by one verifier invocation.
pub struct InvocationQueries {
    pub sessions: Seq<QueryExecution>,
}

pub open spec fn final_session(execution: QueryExecution) -> query::Session
    recommends execution.states.len() > 0,
{
    execution.states.last()
}

pub open spec fn complete_query_execution(
    execution: QueryExecution, context: ModelContext, transcript: SolverTranscript,
) -> bool {
    query::execution(execution.states, execution.actions)
        && final_session(execution).context == context
        && final_session(execution).delivered.len() == final_session(execution).commands.len()
        && query::session_transcript(final_session(execution)) == transcript
}

/// Exact ordered inventory correspondence prevents an empty trace from
/// substituting for an invocation that actually observed solver streams.
pub open spec fn covers_solver_inventory(
    candidate: &authority::Candidate, executions: Seq<InvocationQueries>, context: ModelContext,
) -> bool {
    executions.len() == candidate.invocations().len()
        && forall|i: int| #![auto] 0 <= i < candidate.invocations().len() ==>
            executions[i].sessions.len() == candidate.invocations()[i].transcripts().len()
            && forall|j: int| #![auto] 0 <= j < executions[i].sessions.len() ==>
                complete_query_execution(executions[i].sessions[j], context,
                    candidate.invocations()[i].transcripts()[j])
}

/// Admission is defined over checked evidence, actual model contexts and
/// complete trace inventories. No digest identity or admission goal is assumed.
pub open spec fn can_accept(
    request: &authority::Request, candidate: &authority::Candidate,
    observed: &input::InputEvidence, world: input::InputWorld,
    executions: Seq<InvocationQueries>, point: u64,
) -> bool {
    observed.valid_at(world) && world.point == point as nat
        && world.context.mode == Mode::Default
        && request.scope.whole
        && authority::safe_scoped(request, candidate, world.context, point)
        && covers_solver_inventory(candidate, executions, world.context)
}

/// Only the guarded constructor can create a complete acceptance token.
pub struct Accepted<'a> {
    request: &'a authority::Request,
    candidate: &'a authority::Candidate,
    observed: &'a input::InputEvidence,
    world: input::InputWorld,
    executions: Seq<InvocationQueries>,
    point: u64,
    authority: authority::ValidatedResult<'a>,
}

impl<'a> Accepted<'a> {
    pub closed spec fn valid(&self) -> bool {
        can_accept(self.request, self.candidate, self.observed,
            self.world, self.executions, self.point)
            && self.authority.valid() && self.authority.whole()
            && self.authority.context() == self.world.context
            && self.authority.point() == self.point
            && self.authority.invocations() == self.candidate.invocations()
            && self.authority.requested_targets() == self.request.targets@
    }
    pub closed spec fn context(&self) -> ModelContext { self.world.context }
    pub closed spec fn point(&self) -> u64 { self.point }
    pub closed spec fn requested_targets(&self) -> nat { self.request.targets.len() as nat }
    pub closed spec fn observation(&self) -> &'a input::InputEvidence { self.observed }
    pub closed spec fn request(&self) -> &'a authority::Request { self.request }
    pub closed spec fn candidate(&self) -> &'a authority::Candidate { self.candidate }
    pub closed spec fn query_executions(&self) -> Seq<InvocationQueries> { self.executions }
    pub closed spec fn whole(&self) -> bool { self.authority.whole() }
}

pub proof fn accept_current_complete<'a>(
    request: &'a authority::Request, candidate: &'a authority::Candidate,
    observed: &'a input::InputEvidence, world: input::InputWorld,
    executions: Seq<InvocationQueries>, point: u64,
) -> (out: Option<Accepted<'a>>)
    ensures out.is_some() == can_accept(request, candidate, observed, world, executions, point),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().context() == world.context,
        out.is_some() ==> out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().query_executions() == executions,
        out.is_some() ==> out.unwrap().request() == request,
        out.is_some() ==> out.unwrap().candidate() == candidate,
        out.is_some() ==> out.unwrap().observation() == observed,
        out.is_some() ==> out.unwrap().requested_targets() == request.targets.len(),
{
    if !observed.valid_at(world) { return None; }
    if world.point != point as nat { return None; }
    if world.context.mode != Mode::Default { return None; }
    if !covers_solver_inventory(candidate, executions, world.context) { return None; }
    let checked = authority::validate_current_complete(request, candidate, world.context, point);
    if checked.is_none() { return None; }
    Some(Accepted { request, candidate, observed, world, executions, point,
        authority: checked.unwrap() })
}

pub proof fn composed_acceptance_binds_actual_observation(value: &Accepted)
    requires value.valid(),
    ensures
        value.observation().view() == value.context(),
        value.observation().point() == value.point() as nat,
        context_wf(value.context()),
        value.context().mode == Mode::Default,
        authority::safe_scoped(value.request(), value.candidate(), value.context(), value.point()),
        value.request().scope.whole,
        covers_solver_inventory(value.candidate(), value.query_executions(), value.context()),
{
    input::input_evidence_facts(value.observed, value.world);
}

/// Every reply in every declared verifier solver stream is justified by the
/// concrete query model, including cache-origin and exact-prefix association.
pub proof fn composed_reply_has_exact_origin(value: &Accepted, invocation: int, session: int, reply: int)
    requires value.valid(),
        0 <= invocation < value.query_executions().len(),
        0 <= session < value.query_executions()[invocation].sessions.len(),
        0 <= reply < final_session(value.query_executions()[invocation].sessions[session]).delivered.len(),
    ensures
        query::session_reply_has_origin(final_session(value.query_executions()[invocation].sessions[session]), reply),
        final_session(value.query_executions()[invocation].sessions[session]).delivered[reply].binding
            == final_session(value.query_executions()[invocation].sessions[session]).commands[reply],
        final_session(value.query_executions()[invocation].sessions[session]).context == value.context(),
{
    let execution = value.executions[invocation].sessions[session];
    let index = (execution.states.len() - 1) as nat;
    query::executed_reply_has_exact_binding(execution.states, execution.actions, index, reply);
    query::executed_reply_has_admitted_origin(execution.states, execution.actions, index, reply);
}

/// The composed model safety statement: exact current observations, complete
/// requested coverage, completed record origins and exhaustive child ledgers.
pub proof fn composed_model_safety(value: &Accepted)
    requires value.valid(),
    ensures
        value.observation().view() == value.context(),
        value.observation().point() == value.point() as nat,
        value.context().mode == Mode::Default,
        authority::covers(value.request(), value.candidate()),
        authority::distinct_requests(value.request()),
        authority::distinct_reports(value.candidate()),
        covers_solver_inventory(value.candidate(), value.query_executions(), value.context()),
        forall|i: int| #![auto] 0 <= i < value.candidate().invocations().len() ==>
            value.candidate().invocations()[i].valid()
            && value.candidate().invocations()[i].context() == value.context()
            && value.candidate().invocations()[i].transcripts()
                == authority::child_stream_inventory(value.candidate().invocations()[i].children())
            && authority::every_child_closed(value.candidate().invocations()[i].children()),
{
    composed_acceptance_binds_actual_observation(value);
    authority::acceptance_has_current_origin(&value.authority);
    assert forall|i: int| 0 <= i < value.candidate.invocations().len() implies
        value.candidate.invocations()[i].valid()
        && value.candidate.invocations()[i].context() == value.context()
        && value.candidate.invocations()[i].transcripts()
            == authority::child_stream_inventory(value.candidate.invocations()[i].children())
        && authority::every_child_closed(value.candidate.invocations()[i].children()) by {
        authority::completion_has_validated_records(&value.candidate.invocations()[i]);
    }
}

/// Retained success obtains authority only by rechecking a fresh observation.
pub open spec fn fresh_projection_allowed(old: &Accepted, observed: &input::InputEvidence,
    world: input::InputWorld, point: u64) -> bool {
    point > old.point() && observed.observed_after(old.point() as nat)
        && can_accept(old.request(), old.candidate(), observed, world, old.query_executions(), point)
}

pub proof fn refresh_retained<'a>(old: &Accepted<'a>, observed: &'a input::InputEvidence,
    world: input::InputWorld, point: u64) -> (out: Option<Accepted<'a>>)
    requires old.valid(),
    ensures out.is_some() == fresh_projection_allowed(old, observed, world, point),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().context() == world.context,
        out.is_some() ==> out.unwrap().point() == point,
        out.is_some() ==> point > old.point(),
        out.is_some() ==> observed.observed_after(old.point() as nat),
        out.is_some() ==> out.unwrap().requested_targets() == old.requested_targets(),
{
    if point <= old.point() { return None; }
    if !observed.observed_after(old.point() as nat) { return None; }
    accept_current_complete(old.request, old.candidate, observed, world, old.executions, point)
}

/// Private presentation fields prevent arbitrary report data from becoming
/// model authority. Actual CLI/JSON/MCP adapters are separate refinements.
pub struct Presentation {
    context: ModelContext,
    point: u64,
    targets: nat,
    current: bool,
    complete: bool,
    successful: bool,
}

impl Presentation {
    pub closed spec fn current_complete_success(&self) -> bool {
        self.current && self.complete && self.successful
    }
    pub closed spec fn context(&self) -> ModelContext { self.context }
    pub closed spec fn point(&self) -> u64 { self.point }
    pub closed spec fn targets(&self) -> nat { self.targets }
}

pub proof fn present<'a>(value: &Accepted<'a>, observed: &'a input::InputEvidence,
    world: input::InputWorld, point: u64) -> (view: Option<Presentation>)
    requires value.valid(),
    ensures view.is_some() == fresh_projection_allowed(value, observed, world, point),
        view.is_some() ==> view.unwrap().current_complete_success(),
        view.is_some() ==> view.unwrap().context() == world.context,
        view.is_some() ==> view.unwrap().point() == point && point > value.point(),
        view.is_some() ==> view.unwrap().targets() == value.requested_targets(),
{
    let current = refresh_retained(value, observed, world, point);
    if current.is_none() { return None; }
    let fresh = current.unwrap();
    Some(Presentation { context: fresh.context(), point: fresh.point(),
        targets: fresh.requested_targets(), current: true, complete: true, successful: true })
}

pub proof fn historical_view(value: &Accepted) -> (view: Presentation)
    requires value.valid(),
    ensures !view.current_complete_success(), view.context() == value.context(),
        view.point() == value.point(),
{
    Presentation { context: value.context(), point: value.point(), targets: value.requested_targets(),
        current: false, complete: true, successful: true }
}

pub proof fn repair_projection<'a>(value: &Accepted<'a>, observed: &'a input::InputEvidence,
    world: input::InputWorld, point: u64, target_index: int, name: Seq<u8>) -> (accepted: bool)
    requires value.valid(),
    ensures accepted ==> point > value.point() && observed.valid_at(world),
{
    let current = refresh_retained(value, observed, world, point);
    if current.is_none() { return false; }
    authority::repair_accepted(&current.unwrap().authority, target_index, name)
}

pub open spec fn example_context() -> ModelContext {
    ModelContext {
        request: seq![1u8], scope: ScopeKey { whole: true, selection: Seq::empty() },
        runtime: seq![2u8], mode: Mode::Default, solver: example_solver(),
        inputs: Map::empty().insert(InputKey { role: 0, lexical: seq![3u8] },
            ReadOutcome::Present { resolved: seq![4u8], bytes: seq![5u8] }),
    }
}

pub open spec fn example_solver() -> SolverId {
    SolverId { name: seq![1u8], version: seq![2u8], binary: seq![3u8], platform: seq![4u8] }
}

pub open spec fn example_execution(context: ModelContext) -> QueryExecution {
    let initial = query::session_init(context, context.solver, Seq::empty(), Seq::empty());
    let a0 = query::Action::Input(seq![40u8, 41u8]);
    let s1 = query::step(initial, a0);
    let a1 = query::Action::Command(2);
    let s2 = query::step(s1, a1);
    let a2 = query::Action::Fallback(false);
    let s3 = query::step(s2, a2);
    let a3 = query::Action::Reply(seq![117u8, 110u8, 115u8, 97u8, 116u8]);
    let s4 = query::step(s3, a3);
    QueryExecution { states: seq![initial, s1, s2, s3, s4], actions: seq![a0, a1, a2, a3] }
}

pub proof fn example_execution_is_complete(context: ModelContext)
    requires context_wf(context),
    ensures query::execution(example_execution(context).states, example_execution(context).actions),
        final_session(example_execution(context)).context == context,
        final_session(example_execution(context)).commands.len() == 1,
        final_session(example_execution(context)).delivered.len() == 1,
        transcript_wf(query::session_transcript(final_session(example_execution(context)))),
{
    let execution = example_execution(context);
    assert forall|i: int| 0 <= i < execution.actions.len()
        implies execution.states[i + 1] == query::step(execution.states[i], execution.actions[i]) by {
        if i == 0 {} else if i == 1 {} else if i == 2 {} else { assert(i == 3); }
    }
    query::all_finite_executions_preserve(execution.states, execution.actions, 4);
    query::executed_transcript_projects_observations(execution.states, execution.actions, 4);
    assert(query::session_transcript(final_session(execution)).endpoints.len() == 1);
}

/// A nonempty input, target, function and solver stream construct success
/// through the real component factories; the safety theorem is not vacuous.
pub fn positive_nonempty_composition() {
    let ghost context = example_context();
    let ghost execution = example_execution(context);
    proof { example_execution_is_complete(context); }
    let ghost transcript = query::session_transcript(final_session(execution));
    let ghost transcripts = seq![transcript];
    let pair = authority::example_candidate(Ghost(context), Ghost(transcripts));
    proof {
        let world = input::InputWorld { context, epoch: 0, point: 0 };
        let key = InputKey { role: 0, lexical: seq![3u8] };
        let required = seq![key];
        assert forall|item: InputKey| context.inputs.dom().contains(item)
            <==> required.to_set().contains(item) by {
            assert(required.len() == 1 && required[0] == key);
            assert(required.to_set().contains(item) == required.contains(item));
            assert(required.contains(item) == (item == key));
        }
        assert(context.inputs.dom() =~= required.to_set());
        let evidence = input::capture_unchanged(world, required);
        let queries = seq![InvocationQueries { sessions: seq![execution] }];
        let candidate = pair.1@;
        assert(covers_solver_inventory(&candidate, queries, context));
        let accepted = accept_current_complete(&pair.0, &candidate, &evidence, world, queries, 0);
        assert(accepted.is_some());
        let value = accepted.unwrap();
        composed_acceptance_binds_actual_observation(&value);
        composed_reply_has_exact_origin(&value, 0, 0, 0);
        let historical = historical_view(&value);
        assert(!historical.current_complete_success());
        let later = input::InputWorld { context, epoch: 0, point: 1 };
        let fresh = input::capture_unchanged(later, required);
        let view = present(&value, &fresh, later, 1);
        assert(view.is_some());
        assert(view.unwrap().current_complete_success() && view.unwrap().targets() == 1);
        let stale_view = present(&value, &evidence, world, 0);
        assert(stale_view.is_none());
    }
}

/// The complete admission also accepts justified cache reuse followed by a
/// fresh reply, even when the origin caller had different effective inputs.
pub fn positive_cached_and_live_composition() {
    let ghost context = query::example_context(2, 32);
    let ghost execution = QueryExecution {
        states: query::example_session_states(), actions: query::example_session_actions(),
    };
    proof {
        query::positive_cache_then_live_has_actual_origins();
        query::example_context_is_readable(2, 32);
        query::executed_transcript_projects_observations(execution.states, execution.actions, 8);
    }
    let ghost transcript = query::session_transcript(final_session(execution));
    let ghost transcripts = seq![transcript];
    let pair = authority::example_candidate(Ghost(context), Ghost(transcripts));
    proof {
        let world = input::InputWorld { context, epoch: 0, point: 0 };
        let key = InputKey { role: 1, lexical: seq![1u8] };
        let required = seq![key];
        assert forall|item: InputKey| context.inputs.dom().contains(item)
            <==> required.to_set().contains(item) by {
            assert(required.len() == 1 && required[0] == key);
            assert(required.to_set().contains(item) == required.contains(item));
            assert(required.contains(item) == (item == key));
        }
        assert(context.inputs.dom() =~= required.to_set());
        let evidence = input::capture_unchanged(world, required);
        let queries = seq![InvocationQueries { sessions: seq![execution] }];
        let candidate = pair.1@;
        assert(covers_solver_inventory(&candidate, queries, context));
        let accepted = accept_current_complete(&pair.0, &candidate, &evidence, world, queries, 0);
        assert(accepted.is_some());
        let value = accepted.unwrap();
        composed_reply_has_exact_origin(&value, 0, 0, 0);
        composed_reply_has_exact_origin(&value, 0, 0, 1);
        assert(final_session(execution).cached == 1);
        assert(query::example_origin().context.inputs != context.inputs);
    }
}

pub proof fn mismatched_input_context_is_refused(request: &authority::Request,
    candidate: &authority::Candidate, observed: &input::InputEvidence,
    world: input::InputWorld, executions: Seq<InvocationQueries>, point: u64)
    requires observed.view() != world.context,
    ensures !can_accept(request, candidate, observed, world, executions, point),
{
    if can_accept(request, candidate, observed, world, executions, point) {
        input::input_evidence_facts(observed, world);
    }
}

pub proof fn missing_solver_stream_is_refused(candidate: &authority::Candidate,
    executions: Seq<InvocationQueries>, context: ModelContext, invocation: int)
    requires 0 <= invocation < candidate.invocations().len(),
        0 <= invocation < executions.len(),
        executions[invocation].sessions.len() < candidate.invocations()[invocation].transcripts().len(),
    ensures !covers_solver_inventory(candidate, executions, context),
{}

pub proof fn incomplete_query_cannot_cover(execution: QueryExecution,
    context: ModelContext, transcript: SolverTranscript)
    requires execution.states.len() > 0,
        final_session(execution).delivered.len() < final_session(execution).commands.len(),
    ensures !complete_query_execution(execution, context, transcript),
{}

pub proof fn opt_in_mode_requires_another_acceptance_relation(request: &authority::Request,
    candidate: &authority::Candidate, observed: &input::InputEvidence,
    world: input::InputWorld, executions: Seq<InvocationQueries>, point: u64)
    requires world.context.mode != Mode::Default,
    ensures !can_accept(request, candidate, observed, world, executions, point),
{}

}
