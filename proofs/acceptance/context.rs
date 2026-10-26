//! Shared exact observation context for the acceptance proof models.
#![forbid(unsafe_code)]

use vstd::prelude::*;

verus! {

/// Distinct observation namespaces plus the original lexical identity.
pub struct InputKey {
    pub role: u8,
    pub lexical: Seq<u8>,
}

/// Absence is observed data; unreadable is a refusal, never absence.
pub enum ReadOutcome {
    Present { resolved: Seq<u8>, bytes: Seq<u8> },
    Absent,
    Unreadable,
}

/// Default and opt-in evidence inhabit distinct logical modes.
pub enum Mode {
    Default,
    OptIn(Seq<u8>),
}

/// Identity of the actual modeled solver, separate from the caller context.
pub struct SolverId {
    pub name: Seq<u8>,
    pub version: Seq<u8>,
    pub binary: Seq<u8>,
    pub platform: Seq<u8>,
}

/// Complete observed solver stream inventory for one verifier invocation.
/// Input capture/translation and concrete response framing remain adapters.
pub struct SolverTranscript {
    pub solver: SolverId,
    pub raw: Seq<u8>,
    pub endpoints: Seq<nat>,
    pub replies: Seq<Seq<u8>>,
}

pub open spec fn transcript_wf(transcript: SolverTranscript) -> bool {
    transcript.endpoints.len() == transcript.replies.len()
        && forall|i: int| 0 <= i < transcript.endpoints.len()
            ==> transcript.endpoints[i] <= transcript.raw.len()
}

/// Exact selection structure; no encoding injectivity is assumed.
pub struct ScopeKey {
    pub whole: bool,
    pub selection: Seq<u8>,
}

/// Exact modeled effective inputs, request, selection and runtime.
/// The external compiler-input closure must still refine this supplied map.
pub struct ModelContext {
    pub request: Seq<u8>,
    pub scope: ScopeKey,
    pub runtime: Seq<u8>,
    pub mode: Mode,
    pub solver: SolverId,
    pub inputs: Map<InputKey, ReadOutcome>,
}

pub open spec fn scope_wf(scope: ScopeKey) -> bool {
    scope.whole == (scope.selection.len() == 0)
}

pub open spec fn readable(value: ReadOutcome) -> bool {
    !matches!(value, ReadOutcome::Unreadable)
}

pub open spec fn context_wf(context: ModelContext) -> bool {
    scope_wf(context.scope)
        && forall|key: InputKey| context.inputs.dom().contains(key)
            ==> readable(context.inputs[key])
}

pub open spec fn same_execution(a: ModelContext, b: ModelContext) -> bool {
    a.request == b.request && a.scope == b.scope
        && a.runtime == b.runtime && a.mode == b.mode && a.solver == b.solver
        && a.inputs == b.inputs
}

pub proof fn exact_context_equality(a: ModelContext, b: ModelContext)
    ensures same_execution(a, b) == (a == b),
{}

pub proof fn unreadable_cannot_be_current(context: ModelContext, key: InputKey)
    requires context.inputs.dom().contains(key),
        context.inputs[key] == ReadOutcome::Unreadable,
    ensures !context_wf(context),
{}

}
