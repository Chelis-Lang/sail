//! Modeled invocation lifecycle, exact coverage and sealed result authority.
//! Real process observation and production presentation refinement remain external.
#![forbid(unsafe_code)]
use vstd::prelude::*;
use crate::context::{ModelContext, ScopeKey, Mode, InputKey, ReadOutcome, context_wf, SolverId, SolverTranscript, transcript_wf};

verus! {

/// All target fields are exact bytes, never normalized names or hashes.
pub struct TargetId {
    pub package: Vec<u8>,
    pub cargo_name: Vec<u8>,
    pub kind: Vec<u8>,
    pub source: Vec<u8>,
    pub invocation: Vec<u8>,
}

pub open spec fn same_target(a: &TargetId, b: &TargetId) -> bool {
    a.package@ == b.package@ && a.cargo_name@ == b.cargo_name@
        && a.kind@ == b.kind@ && a.source@ == b.source@
        && a.invocation@ == b.invocation@
}

pub struct Scope {
    pub whole: bool,
    pub selection: Vec<u8>,
}

pub open spec fn same_scope(a: &Scope, b: &Scope) -> bool {
    a.whole == b.whole && a.selection@ == b.selection@
}

pub open spec fn scope_matches(scope: &Scope, context: ModelContext) -> bool {
    scope.whole == context.scope.whole && scope.selection@ == context.scope.selection
}

/// The input map, runtime, mode and request are shared exact modeled facts.
pub struct Request {
    pub targets: Vec<TargetId>,
    pub scope: Scope,
    pub context: Ghost<ModelContext>,
}

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum Verdict { Verified, Failed, Unknown, Provisional }

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum Establishment { Confirmed, Provisional, Unresolved }

/// Settled function evidence retains its explicit source tier.
pub enum EvidenceTier {
    Fresh, LocalCache, SharedCache, Bundled, Engine,
    /// Actual schema evidence strength, distinct from the older origin labels.
    ObservedStrength(u8),
}

pub struct Function {
    pub name: Vec<u8>,
    pub verdict: Verdict,
    pub establishment: Establishment,
    pub evidence_tier: Option<EvidenceTier>,
}

/// Summary candidates are checked against actual observed function records.
pub struct Summary {
    pub functions: usize,
    pub verified: usize,
    pub failed: usize,
    pub unknown: usize,
    pub provisional: usize,
}

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum VerifierEnd { Finished(u64), VirError, Malformed }

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum ProcessEnd { Exit(i32), Interrupted, Malformed }

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum Origin { Direct, Engine, Fallback(u64), RestartAfterDecline(u64) }

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum VerificationSetting { Enabled, CompileOnly }


/// Typed fields observed on the engine side channel, retaining original values.
pub struct EngineTerminalObservation {
    pub version: u64,
    pub event: Seq<u8>,
    pub phase: Seq<u8>,
    pub normal: Option<bool>,
    pub success: Option<bool>,
}

pub enum EngineFieldShape { Object, Other }
pub enum EngineSuccessField { Missing, Boolean(bool), Other }

/// Original verifier header/result field candidates, not a completion flag.
pub struct EngineDocumentObservation {
    pub functions: EngineFieldShape,
    pub version: Option<Seq<u8>>,
    pub profile: Option<Seq<u8>>,
    pub toolchain: Option<Seq<u8>>,
    pub commit: Option<Seq<u8>>,
    pub os: Option<Seq<u8>>,
    pub arch: Option<Seq<u8>>,
    pub error: Option<bool>,
    pub vir_error: Option<bool>,
    pub success: EngineSuccessField,
    pub verified: Option<u64>,
    pub errors: Option<u64>,
    pub whole: Option<bool>,
}

pub struct EngineProtocolObservation {
    pub raw: Seq<u8>,
    pub versions: Seq<Option<u64>>,
    pub terminal: Option<EngineTerminalObservation>,
}

/// One EOF-time observation of the original protocol/result candidates.
pub struct EngineCompletionObservation {
    pub protocol: Option<EngineProtocolObservation>,
    pub documents: Seq<EngineDocumentObservation>,
    pub stdout: Seq<u8>,
    pub remainder: Seq<u8>,
}

pub open spec fn engine_text_present(value: Option<Seq<u8>>) -> bool {
    value.is_some() && value.unwrap().len() > 0
}

pub open spec fn engine_document_complete(d: &EngineDocumentObservation, success: bool) -> bool {
    matches!(d.functions, EngineFieldShape::Object)
        && engine_text_present(d.version) && engine_text_present(d.profile)
        && engine_text_present(d.toolchain) && engine_text_present(d.commit)
        && engine_text_present(d.os) && engine_text_present(d.arch)
        && d.error.is_some() && d.vir_error.is_some()
        && (match d.success { EngineSuccessField::Boolean(value) => value == success,
            EngineSuccessField::Missing => true, EngineSuccessField::Other => false })
        && !(d.vir_error.unwrap() && success)
        && (success ==> d.error == Some(false) && d.errors == Some(0u64))
        && (d.vir_error.unwrap() || (d.verified.is_some() && d.errors.is_some() && d.whole.is_some()
            && (d.whole != Some(true) || matches!(d.success, EngineSuccessField::Boolean(_)))))
}

pub open spec fn engine_protocol_complete(o: &EngineCompletionObservation, end: ProcessEnd) -> bool {
    (end == ProcessEnd::Exit(0) || end == ProcessEnd::Exit(1)) && o.stdout.len() <= 33_554_432
        && o.protocol.is_some()
        && o.protocol.unwrap().raw.len() > 0 && o.protocol.unwrap().raw.len() <= 16_777_216
        && o.protocol.unwrap().raw[o.protocol.unwrap().raw.len() - 1] == 10u8
        && o.protocol.unwrap().versions.len() > 0
        && (forall |i: int| 0 <= i < o.protocol.unwrap().versions.len()
            ==> o.protocol.unwrap().versions[i] == Some(1u64))
        && o.protocol.unwrap().terminal.is_some()
        && o.protocol.unwrap().terminal.unwrap().version == 1
        && o.protocol.unwrap().terminal.unwrap().event == b"compiler-sealed"@
        && o.protocol.unwrap().terminal.unwrap().phase == b"full"@
        && o.protocol.unwrap().terminal.unwrap().normal == Some(true)
        && o.protocol.unwrap().terminal.unwrap().success.is_some()
        && (end == ProcessEnd::Exit(0)) == o.protocol.unwrap().terminal.unwrap().success.unwrap()
        && o.documents.len() == 1
        && engine_document_complete(&o.documents[0], o.protocol.unwrap().terminal.unwrap().success.unwrap())
        && forall |i: int| 0 <= i < o.remainder.len() ==>
            o.remainder[i] == 9 || o.remainder[i] == 10 || o.remainder[i] == 12
                || o.remainder[i] == 13 || o.remainder[i] == 32
}

/// Metadata captured when this particular modeled verifier process starts.
pub struct Invocation {
    pub target: TargetId,
    pub scope: Scope,
    pub context: Ghost<ModelContext>,
    pub run_id: u64,
    pub started_at: u64,
    pub origin: Origin,
    /// Declared verifier invocation mode, independent of process success.
    pub verification: VerificationSetting,
}

pub open spec fn invocation_wf(inv: &Invocation) -> bool {
    context_wf(inv.context@) && scope_matches(&inv.scope, inv.context@)
}

/// A parsed observation is still a candidate, not completed proof authority.
/// Real output attribution to the process is an explicit adapter obligation.
pub struct VerifierDocument {
    pub target: TargetId,
    pub scope: Scope,
    pub context: Ghost<ModelContext>,
    pub run_id: u64,
    pub functions: Vec<Function>,
    pub claimed_summary: Summary,
    pub terminal: VerifierEnd,
    /// Complete modeled solver-session observations for this document.
    pub transcripts: Ghost<Seq<SolverTranscript>>,
}

pub open spec fn good_function(f: &Function) -> bool {
    f.verdict == Verdict::Verified && f.establishment == Establishment::Confirmed
        && f.evidence_tier.is_some()
}

/// Successful counts are derived from the checked records, not trusted flags.
pub open spec fn derived_success_summary(functions: Seq<Function>, summary: &Summary) -> bool {
    summary.functions == functions.len() && summary.verified == functions.len()
        && summary.failed == 0 && summary.unknown == 0 && summary.provisional == 0
        && forall|i: int| #![auto] 0 <= i < functions.len() ==> good_function(&functions[i])
}

pub open spec fn document_good(doc: &VerifierDocument) -> bool {
    doc.terminal == VerifierEnd::Finished(0)
        && derived_success_summary(doc.functions@, &doc.claimed_summary)
}

pub open spec fn document_matches(inv: &Invocation, doc: &VerifierDocument) -> bool {
    doc.run_id == inv.run_id && doc.context@ == inv.context@
        && same_target(&doc.target, &inv.target) && same_scope(&doc.scope, &inv.scope)
}

pub open spec fn document_streams_valid(doc: &VerifierDocument) -> bool {
    forall|i: int| 0 <= i < doc.transcripts@.len() ==>
        transcript_wf(doc.transcripts@[i]) && doc.transcripts@[i].solver == doc.context@.solver
}

/// Derive the inventory from every observed document, preserving all sessions.
pub open spec fn observed_transcripts(documents: Seq<VerifierDocument>) -> Seq<SolverTranscript>
    decreases documents.len(),
{
    if documents.len() == 0 { Seq::empty() }
    else { observed_transcripts(documents.drop_last()) + documents.last().transcripts@ }
}

proof fn single_document_inventory(document: VerifierDocument)
    ensures observed_transcripts(seq![document]) == document.transcripts@,
{
    reveal_with_fuel(observed_transcripts, 2);
    assert(seq![document].drop_last() =~= Seq::<VerifierDocument>::empty());
    assert(seq![document].last() == document);
    assert(Seq::<SolverTranscript>::empty() + document.transcripts@ =~= document.transcripts@);
}

pub proof fn observed_transcripts_are_well_formed(documents: Seq<VerifierDocument>)
    requires forall|i: int| 0 <= i < documents.len() ==> document_streams_valid(&documents[i]),
    ensures forall|i: int| 0 <= i < observed_transcripts(documents).len()
        ==> transcript_wf(observed_transcripts(documents)[i]),
    decreases documents.len(),
{
    if documents.len() > 0 {
        observed_transcripts_are_well_formed(documents.drop_last());
        let before = observed_transcripts(documents.drop_last());
        let last = documents.last().transcripts@;
        assert(document_streams_valid(&documents.last()));
        assert forall|i: int| 0 <= i < (before + last).len()
            implies transcript_wf(#[trigger] (before + last)[i]) by {
            if i < before.len() { assert((before + last)[i] == before[i]); }
            else { assert((before + last)[i] == last[i - before.len()]); }
        }
    }
}

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum StreamState { Running, Interrupted, Malformed }

/// Independent verifier-child lifecycle slot; documents cannot create or replace it.
pub struct ChildStream {
    solver: SolverId,
    closed: Option<SolverTranscript>,
}

impl ChildStream {
    pub closed spec fn solver(&self) -> SolverId { self.solver }
    pub closed spec fn closed(&self) -> Option<SolverTranscript> { self.closed }
}

pub open spec fn child_stream_valid(stream: &ChildStream, solver: SolverId) -> bool {
    stream.solver() == solver && (stream.closed().is_some() ==>
        transcript_wf(stream.closed().unwrap()) && stream.closed().unwrap().solver == solver)
}

/// Ordered final inventory derives from started slots, never from verifier documents.
pub open spec fn child_stream_inventory(streams: Seq<ChildStream>) -> Seq<SolverTranscript> {
    Seq::new(streams.len(), |i: int| streams[i].closed().unwrap())
}

pub open spec fn every_child_closed(streams: Seq<ChildStream>) -> bool {
    forall|i: int| 0 <= i < streams.len() ==> streams[i].closed().is_some()
}

/// Private lifecycle state can be obtained only by modeled start/observation transitions.
pub struct RunningInvocation {
    invocation: Invocation,
    documents: Seq<VerifierDocument>,
    stream: StreamState,
    fallback_from: Option<EngineFailure>,
    declined_from: Option<EngineDecline>,
    completion: Option<EngineCompletionObservation>,
    children: Seq<ChildStream>,
}

/// A failed engine execution, not a caller-supplied fallback flag.
pub struct EngineFailure {
    invocation: Invocation,
    end: ProcessEnd,
    failed_at: u64,
}

/// A normally terminated engine with concretely unusable protocol evidence.
/// Its terminal status remains Exit(0); this is not an EngineFailure.
pub struct EngineDecline {
    invocation: Invocation,
    end: ProcessEnd,
    observed_at: u64,
    completion: EngineCompletionObservation,
}

impl EngineDecline {
    pub closed spec fn invocation(&self) -> Invocation { self.invocation }
    pub closed spec fn end(&self) -> ProcessEnd { self.end }
    pub closed spec fn point(&self) -> u64 { self.observed_at }
    pub closed spec fn completion(&self) -> EngineCompletionObservation { self.completion }
    pub closed spec fn valid(&self) -> bool {
        invocation_wf(&self.invocation) && self.invocation.origin == Origin::Engine
            && self.end == ProcessEnd::Exit(0) && self.observed_at >= self.invocation.started_at
            && !engine_protocol_complete(&self.completion, self.end)
    }
}

impl RunningInvocation {
    pub closed spec fn invocation(&self) -> Invocation { self.invocation }
    pub closed spec fn documents(&self) -> Seq<VerifierDocument> { self.documents }
    pub closed spec fn stream(&self) -> StreamState { self.stream }
    pub closed spec fn children(&self) -> Seq<ChildStream> { self.children }
    pub closed spec fn completion(&self) -> Option<EngineCompletionObservation> { self.completion }
}
impl EngineFailure {
    pub closed spec fn invocation(&self) -> Invocation { self.invocation }
    pub closed spec fn point(&self) -> u64 { self.failed_at }
}

pub closed spec fn failure_valid(f: &EngineFailure) -> bool {
    invocation_wf(&f.invocation) && f.invocation.origin == Origin::Engine
        && f.failed_at >= f.invocation.started_at
        && f.end != ProcessEnd::Exit(0)
}

/// Restarting a whole target may widen a failed partial selection, while every
/// non-selection request/input/runtime fact and the exact target remain fixed.
pub open spec fn fallback_subject_matches(failed: &Invocation, fresh: &Invocation) -> bool {
    same_target(&failed.target, &fresh.target)
        && failed.context@.request == fresh.context@.request
        && failed.context@.runtime == fresh.context@.runtime
        && failed.context@.solver == fresh.context@.solver
        && failed.context@.inputs == fresh.context@.inputs
        && failed.context@.mode == fresh.context@.mode
        && failed.verification == fresh.verification
}

pub closed spec fn origin_valid(run: &RunningInvocation) -> bool {
    match run.invocation.origin {
        Origin::Direct => run.fallback_from.is_none() && run.declined_from.is_none(),
        Origin::Engine => run.fallback_from.is_none() && run.declined_from.is_none(),
        Origin::Fallback(id) => run.fallback_from.is_some()
            && failure_valid(&run.fallback_from.unwrap())
            && run.fallback_from.unwrap().invocation.run_id == id
            && run.invocation.run_id > id && run.invocation.scope.whole
            && run.invocation.started_at >= run.fallback_from.unwrap().failed_at
            && fallback_subject_matches(&run.fallback_from.unwrap().invocation, &run.invocation)
            && run.declined_from.is_none(),
        Origin::RestartAfterDecline(id) => run.declined_from.is_some()
            && run.declined_from.unwrap().valid() && run.fallback_from.is_none()
            && run.declined_from.unwrap().invocation().run_id == id
            && run.invocation.run_id > id && run.invocation.scope.whole
            && run.invocation.started_at >= run.declined_from.unwrap().point()
            && fallback_subject_matches(&run.declined_from.unwrap().invocation(), &run.invocation),
    }
}

pub closed spec fn running_wf(run: &RunningInvocation) -> bool {
    invocation_wf(&run.invocation) && origin_valid(run)
        && forall|i: int| 0 <= i < run.children.len() ==>
            child_stream_valid(&run.children[i], run.invocation.context@.solver)
}

pub proof fn start(invocation: Invocation) -> (run: Option<RunningInvocation>)
    ensures run.is_some() == (invocation_wf(&invocation)
        && !matches!(invocation.origin, Origin::Fallback(_) | Origin::RestartAfterDecline(_))),
        run.is_some() ==> running_wf(&run.unwrap()),
        run.is_some() ==> run.unwrap().invocation() == invocation,
        run.is_some() ==> run.unwrap().documents().len() == 0,
        run.is_some() ==> run.unwrap().children().len() == 0,
        run.is_some() ==> run.unwrap().stream() == StreamState::Running,
        run.is_some() ==> run.unwrap().completion().is_none(),
{
    if !invocation_wf(&invocation) || matches!(invocation.origin, Origin::Fallback(_) | Origin::RestartAfterDecline(_)) {
        None
    } else {
        Some(RunningInvocation { invocation, documents: Seq::empty(),
            stream: StreamState::Running, fallback_from: None, declined_from: None,
            completion: None, children: Seq::empty() })
    }
}

pub proof fn observe(run: RunningInvocation, document: VerifierDocument) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents().push(document), out.stream() == run.stream(),
        out.children() == run.children(), out.completion() == run.completion(),
{
    RunningInvocation { documents: run.documents.push(document), ..run }
}

/// Preserve a single EOF-time protocol observation; a second observation is
/// ambiguous and cannot repair the first by overwriting it.
pub proof fn observe_engine_completion(run: RunningInvocation, observation: EngineCompletionObservation)
    -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.children() == run.children(),
        run.completion().is_none() ==> out.completion() == Some(observation)
            && out.stream() == run.stream(),
        run.completion().is_some() ==> out.completion() == run.completion()
            && out.stream() == StreamState::Malformed,
{
    if run.completion.is_some() {
        RunningInvocation { stream: StreamState::Malformed, ..run }
    } else { RunningInvocation { completion: Some(observation), ..run } }
}

/// Normal exit zero is preserved. The observed protocol fields themselves must
/// refuse completion; a caller cannot supply an arbitrary decline reason.
pub proof fn engine_declined(run: RunningInvocation, end: ProcessEnd, point: u64)
    -> (out: Option<EngineDecline>)
    requires running_wf(&run),
    ensures out.is_some() == (run.invocation().origin == Origin::Engine
        && end == ProcessEnd::Exit(0) && point >= run.invocation().started_at
        && run.completion().is_some() && !engine_protocol_complete(&run.completion().unwrap(), end)),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().invocation() == run.invocation(),
        out.is_some() ==> out.unwrap().end() == end && out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().completion() == run.completion().unwrap(),
{
    if run.invocation.origin == Origin::Engine && end == ProcessEnd::Exit(0)
        && point >= run.invocation.started_at && run.completion.is_some()
        && !engine_protocol_complete(&run.completion.unwrap(), end) {
        Some(EngineDecline { invocation: run.invocation, end, observed_at: point,
            completion: run.completion.unwrap() })
    } else { None }
}

pub proof fn start_declined_fallback(declined: EngineDecline, invocation: Invocation)
    -> (out: Option<RunningInvocation>)
    requires declined.valid(),
    ensures out.is_some() == (invocation_wf(&invocation) && invocation.scope.whole
        && invocation.origin == Origin::RestartAfterDecline(declined.invocation().run_id)
        && invocation.run_id > declined.invocation().run_id
        && invocation.started_at >= declined.point()
        && fallback_subject_matches(&declined.invocation(), &invocation)),
        out.is_some() ==> running_wf(&out.unwrap()),
        out.is_some() ==> out.unwrap().invocation() == invocation,
        out.is_some() ==> out.unwrap().documents().len() == 0,
        out.is_some() ==> out.unwrap().children().len() == 0,
        out.is_some() ==> out.unwrap().completion().is_none(),
        out.is_some() ==> out.unwrap().stream() == StreamState::Running,
{
    if invocation_wf(&invocation) && invocation.scope.whole
        && invocation.origin == Origin::RestartAfterDecline(declined.invocation.run_id)
        && invocation.run_id > declined.invocation.run_id && invocation.started_at >= declined.observed_at
        && fallback_subject_matches(&declined.invocation, &invocation) {
        Some(RunningInvocation { invocation, documents: Seq::empty(), stream: StreamState::Running,
            fallback_from: None, declined_from: Some(declined), completion: None, children: Seq::empty() })
    } else { None }
}

pub proof fn interrupt(run: RunningInvocation) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.stream() == StreamState::Interrupted,
        out.children() == run.children(),
{
    RunningInvocation { stream: StreamState::Interrupted, ..run }
}

pub proof fn malformed(run: RunningInvocation) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.stream() == StreamState::Malformed,
        out.children() == run.children(),
{
    RunningInvocation { stream: StreamState::Malformed, ..run }
}

/// A child process start records an independent mandatory slot before any document exists.
pub proof fn stream_started(run: RunningInvocation, solver: SolverId) -> (out: Option<RunningInvocation>)
    requires running_wf(&run),
    ensures out.is_some() == (run.stream() == StreamState::Running
        && solver == run.invocation().context@.solver),
        out.is_some() ==> running_wf(&out.unwrap()),
        out.is_some() ==> out.unwrap().invocation() == run.invocation(),
        out.is_some() ==> out.unwrap().documents() == run.documents(),
        out.is_some() ==> out.unwrap().completion() == run.completion(),
        out.is_some() ==> out.unwrap().stream() == run.stream(),
        out.is_some() ==> out.unwrap().children().len() == run.children().len() + 1,
        out.is_some() ==> out.unwrap().children().drop_last() == run.children(),
        out.is_some() ==> out.unwrap().children().last().solver() == solver,
        out.is_some() ==> out.unwrap().children().last().closed().is_none(),
{
    if run.stream == StreamState::Running && solver == run.invocation.context@.solver {
        Some(RunningInvocation { children: run.children.push(ChildStream { solver, closed: None }), ..run })
    } else { None }
}

/// Only the originally declared solver can close its started slot; it closes once.
pub proof fn stream_closed(run: RunningInvocation, index: int, transcript: SolverTranscript)
    -> (out: Option<RunningInvocation>)
    requires running_wf(&run),
    ensures out.is_some() == (run.stream() == StreamState::Running
        && 0 <= index < run.children().len() && run.children()[index].closed().is_none()
        && transcript_wf(transcript) && transcript.solver == run.children()[index].solver()),
        out.is_some() ==> running_wf(&out.unwrap()),
        out.is_some() ==> out.unwrap().invocation() == run.invocation(),
        out.is_some() ==> out.unwrap().documents() == run.documents(),
        out.is_some() ==> out.unwrap().completion() == run.completion(),
        out.is_some() ==> out.unwrap().stream() == run.stream(),
        out.is_some() ==> out.unwrap().children().len() == run.children().len(),
        out.is_some() ==> out.unwrap().children()[index].closed() == Some(transcript),
        out.is_some() ==> forall|i: int| 0 <= i < run.children().len() && i != index ==>
            out.unwrap().children()[i] == run.children()[i],
{
    if run.stream == StreamState::Running && 0 <= index < run.children.len()
        && run.children[index].closed.is_none() && transcript_wf(transcript)
        && transcript.solver == run.children[index].solver {
        let children = run.children.update(index,
            ChildStream { solver: run.children[index].solver, closed: Some(transcript) });
        Some(RunningInvocation { children, ..run })
    } else { None }
}

pub proof fn engine_failed(run: RunningInvocation, end: ProcessEnd, point: u64)
    -> (out: Option<EngineFailure>)
    requires running_wf(&run),
    ensures out.is_some() == (run.invocation().origin == Origin::Engine
        && end != ProcessEnd::Exit(0) && point >= run.invocation().started_at),
        out.is_some() ==> failure_valid(&out.unwrap()),
        out.is_some() ==> out.unwrap().invocation() == run.invocation(),
        out.is_some() ==> out.unwrap().point() == point,
{
    if run.invocation.origin == Origin::Engine && end != ProcessEnd::Exit(0)
        && point >= run.invocation.started_at {
        Some(EngineFailure { invocation: run.invocation, end, failed_at: point })
    } else { None }
}

pub proof fn start_fallback(failed: EngineFailure, invocation: Invocation)
    -> (out: Option<RunningInvocation>)
    requires failure_valid(&failed),
    ensures out.is_some() ==> running_wf(&out.unwrap()),
        out.is_some() ==> out.unwrap().invocation() == invocation,
        out.is_some() ==> out.unwrap().documents().len() == 0,
        out.is_some() ==> out.unwrap().children().len() == 0,
        out.is_some() ==> out.unwrap().stream() == StreamState::Running,
        out.is_some() == (invocation_wf(&invocation) && invocation.scope.whole
            && invocation.origin == Origin::Fallback(failed.invocation().run_id)
            && invocation.run_id > failed.invocation().run_id
            && invocation.started_at >= failed.point()
            && fallback_subject_matches(&failed.invocation(), &invocation)),
{
    if invocation_wf(&invocation) && invocation.scope.whole
        && invocation.origin == Origin::Fallback(failed.invocation.run_id)
        && invocation.run_id > failed.invocation.run_id
        && invocation.started_at >= failed.failed_at
        && fallback_subject_matches(&failed.invocation, &invocation) {
        Some(RunningInvocation { invocation, documents: Seq::empty(),
            stream: StreamState::Running, fallback_from: Some(failed), declined_from: None,
            completion: None, children: Seq::empty() })
    } else { None }
}

pub closed spec fn can_complete(run: &RunningInvocation, end: ProcessEnd, point: u64) -> bool {
    running_wf(run) && run.stream == StreamState::Running
        && run.invocation.verification == VerificationSetting::Enabled
        && (run.invocation.origin == Origin::Engine ==> run.completion.is_some()
            && engine_protocol_complete(&run.completion.unwrap(), end))
        && end == ProcessEnd::Exit(0) && point >= run.invocation.started_at
        && run.documents.len() > 0
        && every_child_closed(run.children)
        && observed_transcripts(run.documents) == child_stream_inventory(run.children)
        && forall|i: int| #![auto] 0 <= i < run.documents.len() ==>
            document_matches(&run.invocation, &run.documents[i]) && document_good(&run.documents[i])
                && document_streams_valid(&run.documents[i])
}

/// Private completed evidence stores the actual validated model run and termination.
pub struct CompletedInvocation {
    run: RunningInvocation,
    end: ProcessEnd,
    completed_at: u64,
}

impl CompletedInvocation {
    pub closed spec fn valid(&self) -> bool { can_complete(&self.run, self.end, self.completed_at) }
    pub closed spec fn context(&self) -> ModelContext { self.run.invocation.context@ }
    pub closed spec fn point(&self) -> u64 { self.completed_at }
    pub closed spec fn invocation(&self) -> Invocation { self.run.invocation }
    pub closed spec fn documents(&self) -> Seq<VerifierDocument> { self.run.documents }
    pub closed spec fn transcripts(&self) -> Seq<SolverTranscript> { observed_transcripts(self.run.documents) }
    pub closed spec fn children(&self) -> Seq<ChildStream> { self.run.children }
    pub closed spec fn predecessor(&self) -> Option<EngineFailure> { self.run.fallback_from }
    pub closed spec fn declined_predecessor(&self) -> Option<EngineDecline> { self.run.declined_from }
}

pub proof fn complete(run: RunningInvocation, end: ProcessEnd, point: u64)
    -> (out: Option<CompletedInvocation>)
    ensures out.is_some() == can_complete(&run, end, point),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().invocation() == run.invocation(),
        out.is_some() ==> out.unwrap().context() == run.invocation().context@,
        out.is_some() ==> out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().documents() == run.documents(),
        out.is_some() ==> out.unwrap().transcripts() == observed_transcripts(run.documents()),
{
    if can_complete(&run, end, point) {
        Some(CompletedInvocation { run, end, completed_at: point })
    } else { None }
}

#[derive(Structural, PartialEq, Eq, Copy, Clone)]
pub enum ExecutionPath {
    Direct, Engine, IndependentFallback(u64),
    /// Origin belongs to each sealed invocation, rather than one project flag.
    PerTarget,
}

/// Sealed project candidate; fields are derived from completed run evidence.
pub struct Candidate {
    context: ModelContext,
    reports: Seq<CompletedInvocation>,
    end: ProcessEnd,
    path: ExecutionPath,
    observed_at: u64,
}

pub open spec fn path_matches(path: ExecutionPath, report: &CompletedInvocation) -> bool {
    match path {
        ExecutionPath::Direct => report.invocation().origin == Origin::Direct,
        ExecutionPath::Engine => report.invocation().origin == Origin::Engine,
        ExecutionPath::IndependentFallback(id) => report.invocation().origin == Origin::Fallback(id),
        ExecutionPath::PerTarget => report.valid(),
    }
}

pub open spec fn project_records_valid(context: ModelContext, reports: Seq<CompletedInvocation>,
    path: ExecutionPath, point: u64) -> bool {
    context_wf(context) && forall|i: int| #![auto] 0 <= i < reports.len() ==>
        reports[i].valid() && reports[i].context() == context
        && reports[i].point() <= point && path_matches(path, &reports[i])
}

impl Candidate {
    pub closed spec fn context(&self) -> ModelContext { self.context }
    pub closed spec fn point(&self) -> u64 { self.observed_at }
    pub closed spec fn invocations(&self) -> Seq<CompletedInvocation> { self.reports }
    pub closed spec fn path(&self) -> ExecutionPath { self.path }
    pub closed spec fn valid(&self) -> bool {
        self.end == ProcessEnd::Exit(0)
            && project_records_valid(self.context, self.reports, self.path, self.observed_at)
    }
}

/// Engine-to-fallback selection admits only the fresh independent completion set.
pub proof fn finish_project(context: ModelContext, reports: Seq<CompletedInvocation>,
    path: ExecutionPath, end: ProcessEnd, point: u64) -> (out: Option<Candidate>)
    ensures out.is_some() == (end == ProcessEnd::Exit(0)
        && project_records_valid(context, reports, path, point)),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().context() == context,
        out.is_some() ==> out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().invocations() == reports,
        out.is_some() ==> out.unwrap().path() == path,
{
    if end == ProcessEnd::Exit(0) && project_records_valid(context, reports, path, point) {
        Some(Candidate { context, reports, end, path, observed_at: point })
    } else { None }
}

/// This constructor carries each completed invocation's actual modeled origin.
/// It cannot turn an engine candidate or an unsuccessful attempt into a report.
pub proof fn finish_project_per_target(context: ModelContext, reports: Seq<CompletedInvocation>,
    end: ProcessEnd, point: u64) -> (out: Option<Candidate>)
    ensures out.is_some() == (end == ProcessEnd::Exit(0)
        && project_records_valid(context, reports, ExecutionPath::PerTarget, point)),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().context() == context,
        out.is_some() ==> out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().invocations() == reports,
        out.is_some() ==> out.unwrap().path() == ExecutionPath::PerTarget,
{
    finish_project(context, reports, ExecutionPath::PerTarget, end, point)
}

/// Every fallback record preserves the failed target/context and starts later.
/// The predecessor is the private failed lifecycle token, never a supplied label.
pub proof fn completed_origin_is_justified(report: &CompletedInvocation)
    requires report.valid(),
    ensures match report.invocation().origin {
        Origin::Direct | Origin::Engine => report.predecessor().is_none(),
        Origin::Fallback(id) => report.predecessor().is_some()
            && failure_valid(&report.predecessor().unwrap())
            && report.predecessor().unwrap().invocation().run_id == id
            && report.invocation().run_id > id
            && report.invocation().started_at >= report.predecessor().unwrap().point()
            && fallback_subject_matches(&report.predecessor().unwrap().invocation(), &report.invocation())
            && report.invocation().scope.whole,
        Origin::RestartAfterDecline(id) => report.declined_predecessor().is_some()
            && report.declined_predecessor().unwrap().valid()
            && report.declined_predecessor().unwrap().invocation().run_id == id
            && report.invocation().run_id > id
            && report.invocation().started_at >= report.declined_predecessor().unwrap().point()
            && fallback_subject_matches(&report.declined_predecessor().unwrap().invocation(), &report.invocation())
            && report.invocation().scope.whole,
    },
{}

pub open spec fn distinct_requests(r: &Request) -> bool {
    forall|i: int, j: int| 0 <= i < j < r.targets.len()
        ==> !same_target(&r.targets[i], &r.targets[j])
}

pub open spec fn distinct_reports(c: &Candidate) -> bool {
    forall|i: int, j: int| 0 <= i < j < c.invocations().len()
        ==> !same_target(&c.invocations()[i].invocation().target, &c.invocations()[j].invocation().target)
}

pub open spec fn covered_at(r: &Request, c: &Candidate, i: int) -> bool {
    exists|j: int| #![auto] 0 <= j < c.invocations().len()
        && same_target(&r.targets[i], &c.invocations()[j].invocation().target)
        && same_scope(&r.scope, &c.invocations()[j].invocation().scope)
}

pub open spec fn covers(r: &Request, c: &Candidate) -> bool {
    forall|i: int| 0 <= i < r.targets.len() ==> #[trigger] covered_at(r, c, i)
}

pub open spec fn safe_scoped(r: &Request, c: &Candidate, now: ModelContext, point: u64) -> bool {
    c.valid() && r.context@ == c.context() && c.context() == now
        && point >= c.point() && context_wf(now) && scope_matches(&r.scope, now)
        && r.targets.len() > 0 && distinct_requests(r) && distinct_reports(c) && covers(r, c)
}

/// Immutable authority for this exact acceptance observation and completed evidence.
pub struct ValidatedResult<'a> {
    request: &'a Request,
    candidate: &'a Candidate,
    accepted_context: ModelContext,
    accepted_at: u64,
}

impl<'a> ValidatedResult<'a> {
    pub closed spec fn valid(&self) -> bool {
        safe_scoped(self.request, self.candidate, self.accepted_context, self.accepted_at)
    }
    pub closed spec fn whole(&self) -> bool { self.request.scope.whole }
    pub closed spec fn context(&self) -> ModelContext { self.accepted_context }
    pub closed spec fn point(&self) -> u64 { self.accepted_at }
    pub closed spec fn invocations(&self) -> Seq<CompletedInvocation> { self.candidate.invocations() }
    pub closed spec fn path(&self) -> ExecutionPath { self.candidate.path }
    pub closed spec fn request_context(&self) -> ModelContext { self.request.context@ }
    pub closed spec fn candidate_context(&self) -> ModelContext { self.candidate.context() }
    pub closed spec fn scope(&self) -> Scope { self.request.scope }
    pub closed spec fn requested_targets(&self) -> Seq<TargetId> { self.request.targets@ }
}

/// Context comparison is an explicit guarded model transition, not executable map equality.
pub proof fn validate_scoped<'a>(r: &'a Request, c: &'a Candidate, now: ModelContext, point: u64)
    -> (out: Option<ValidatedResult<'a>>)
    ensures out.is_some() == safe_scoped(r, c, now, point),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().whole() == r.scope.whole,
        out.is_some() ==> out.unwrap().context() == now && out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().invocations() == c.invocations(),
        out.is_some() ==> out.unwrap().path() == c.path(),
        out.is_some() ==> out.unwrap().scope().whole == r.scope.whole,
        out.is_some() ==> out.unwrap().requested_targets() == r.targets@,
{
    if safe_scoped(r, c, now, point) {
        Some(ValidatedResult { request: r, candidate: c, accepted_context: now, accepted_at: point })
    } else { None }
}

pub proof fn validate_current_complete<'a>(r: &'a Request, c: &'a Candidate, now: ModelContext, point: u64)
    -> (out: Option<ValidatedResult<'a>>)
    ensures out.is_some() == (r.scope.whole && safe_scoped(r, c, now, point)),
        out.is_some() ==> out.unwrap().valid() && out.unwrap().whole(),
        out.is_some() ==> out.unwrap().context() == now && out.unwrap().point() == point,
        out.is_some() ==> out.unwrap().path() == c.path(),
        out.is_some() ==> out.unwrap().invocations() == c.invocations(),
        out.is_some() ==> out.unwrap().scope().whole == r.scope.whole,
        out.is_some() ==> out.unwrap().requested_targets() == r.targets@,
{
    if !r.scope.whole { None } else { validate_scoped(r, c, now, point) }
}

/// Read-time retained-job projection rechecks this same sealed candidate at a fresh context.
pub proof fn project_job<'a>(r: &'a Request, c: &'a Candidate, now: ModelContext, point: u64)
    -> (out: Option<ValidatedResult<'a>>)
    ensures out.is_some() == (r.scope.whole && safe_scoped(r, c, now, point)),
        out.is_some() ==> out.unwrap().valid() && out.unwrap().context() == now,
{
    validate_current_complete(r, c, now, point)
}

/// The selected named function must occur in a requested target's completed records.
pub closed spec fn repair_selectable(result: &ValidatedResult, target_index: int, name: Seq<u8>) -> bool {
    0 <= target_index < result.requested_targets().len()
        && exists|j: int, d: int, k: int| 0 <= j < result.invocations().len()
            && same_target(&result.requested_targets()[target_index], &result.invocations()[j].invocation().target)
            && 0 <= d < result.invocations()[j].documents().len()
            && 0 <= k < result.invocations()[j].documents()[d].functions.len()
            && result.invocations()[j].documents()[d].functions[k].name@ == name
}

/// Every target in the requested repair scope must retain non-fallback origin.
/// Unrequested dependency records do not widen that supplied repair scope.
pub closed spec fn repair_target_origin_allowed(result: &ValidatedResult, target_index: int) -> bool {
    0 <= target_index < result.requested_targets().len()
        && forall|i: int, j: int| 0 <= i < result.invocations().len()
            && 0 <= j < result.requested_targets().len()
            && #[trigger] same_target(&result.requested_targets()[j], &result.invocations()[i].invocation().target)
            ==> match result.invocations()[i].invocation().origin {
                Origin::Fallback(_) | Origin::RestartAfterDecline(_) => false, _ => true,
            }
}

/// Repair and ordinary result authority consume the same checked completion token.
pub proof fn repair_accepted(result: &ValidatedResult, target_index: int, name: Seq<u8>) -> (accepted: bool)
    requires result.valid(),
    ensures accepted == (result.whole() && repair_selectable(result, target_index, name)
        && !matches!(result.path(), ExecutionPath::IndependentFallback(_))
        && repair_target_origin_allowed(result, target_index)),
{
    result.whole() && repair_selectable(result, target_index, name)
        && !matches!(result.path(), ExecutionPath::IndependentFallback(_))
        && repair_target_origin_allowed(result, target_index)
}

/// Both crashed-engine and normal-exit declined-engine restarts are excluded
/// from every requested target of repair authority.
pub proof fn repair_refuses_requested_fallback(result: &ValidatedResult, selected: int,
    report: int, requested: int, name: Seq<u8>)
    requires result.valid(), 0 <= report < result.invocations().len(),
        0 <= requested < result.requested_targets().len(),
        same_target(&result.requested_targets()[requested], &result.invocations()[report].invocation().target),
        match result.invocations()[report].invocation().origin {
            Origin::Fallback(_) | Origin::RestartAfterDecline(_) => true, _ => false,
        },
{
    assert(!repair_target_origin_allowed(result, selected));
    let accepted = repair_accepted(result, selected, name);
    assert(!accepted);
}

/// A report cannot cover two distinct requested targets.
pub proof fn injective_coverage(r: &Request, c: &Candidate, now: ModelContext, point: u64,
    i: int, j: int, k: int)
    requires safe_scoped(r, c, now, point), 0 <= i < r.targets.len(), 0 <= j < r.targets.len(),
        0 <= k < c.invocations().len(),
        same_target(&r.targets[i], &c.invocations()[k].invocation().target),
        same_target(&r.targets[j], &c.invocations()[k].invocation().target),
    ensures i == j,
{
    if i < j { assert(!same_target(&r.targets[i], &r.targets[j])); }
    if j < i { assert(!same_target(&r.targets[j], &r.targets[i])); }
}

/// Byte equality is executable; no digest injectivity is assumed.
pub fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (equal: bool)
    ensures equal == (a@ == b@),
{
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len()
        invariant i <= a.len(), a.len() == b.len(),
            forall|j: int| 0 <= j < i ==> a[j] == b[j],
        decreases a.len() - i,
    {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    assert(a@ =~= b@);
    true
}

pub fn target_equal(a: &TargetId, b: &TargetId) -> (equal: bool)
    ensures equal == same_target(a, b),
{
    bytes_equal(&a.package, &b.package) && bytes_equal(&a.cargo_name, &b.cargo_name)
        && bytes_equal(&a.kind, &b.kind) && bytes_equal(&a.source, &b.source)
        && bytes_equal(&a.invocation, &b.invocation)
}

pub fn scope_equal(a: &Scope, b: &Scope) -> (equal: bool)
    ensures equal == same_scope(a, b),
{
    a.whole == b.whole && bytes_equal(&a.selection, &b.selection)
}

/// The concrete record validator derives a passing summary from its checked functions.
pub fn validate_document(doc: &VerifierDocument) -> (valid: bool)
    ensures valid == document_good(doc),
{
    if doc.terminal != VerifierEnd::Finished(0)
        || doc.claimed_summary.functions != doc.functions.len()
        || doc.claimed_summary.verified != doc.functions.len()
        || doc.claimed_summary.failed != 0 || doc.claimed_summary.unknown != 0
        || doc.claimed_summary.provisional != 0 { return false; }
    let mut i = 0;
    while i < doc.functions.len()
        invariant i <= doc.functions.len(),
            forall|j: int| #![auto] 0 <= j < i ==> good_function(&doc.functions[j]),
        decreases doc.functions.len() - i,
    {
        let f = &doc.functions[i];
        if f.verdict != Verdict::Verified || f.establishment != Establishment::Confirmed
            || f.evidence_tier.is_none() {
            return false;
        }
        i += 1;
    }
    true
}

/// The completion constructor's validity is tied to actual recorded model events.
pub proof fn completion_has_validated_records(completed: &CompletedInvocation)
    requires completed.valid(),
    ensures invocation_wf(&completed.invocation()), completed.documents().len() > 0,
        completed.context() == completed.invocation().context@,
        completed.point() >= completed.invocation().started_at,
        forall|i: int| 0 <= i < completed.documents().len() ==>
            document_matches(&completed.invocation(), &completed.documents()[i])
                && document_good(&completed.documents()[i])
                && document_streams_valid(&completed.documents()[i]),
        completed.transcripts() == observed_transcripts(completed.documents()),
        every_child_closed(completed.children()),
        completed.transcripts() == child_stream_inventory(completed.children()),
        forall|i: int| 0 <= i < completed.children().len() ==>
            child_stream_valid(&completed.children()[i], completed.context().solver),
        forall|i: int| 0 <= i < completed.transcripts().len()
            ==> transcript_wf(completed.transcripts()[i])
                && completed.transcripts()[i].solver == completed.context().solver,
        match completed.invocation().origin {
            Origin::Fallback(id) => completed.invocation().run_id > id
                && completed.invocation().scope.whole,
            _ => true,
        },
{
    observed_transcripts_are_well_formed(completed.documents());
    assert forall|i: int| 0 <= i < completed.transcripts().len()
        implies transcript_wf(#[trigger] completed.transcripts()[i])
            && completed.transcripts()[i].solver == completed.context().solver by {
        assert(completed.children()[i].closed().is_some());
        assert(child_stream_valid(&completed.children()[i], completed.context().solver));
        assert(completed.transcripts()[i] == completed.children()[i].closed().unwrap());
    }
}

/// Every published authority preserves the exact input/runtime/request context.
pub proof fn acceptance_has_current_origin(result: &ValidatedResult)
    requires result.valid(),
    ensures context_wf(result.context()), result.invocations().len() > 0,
        result.candidate_context() == result.context(),
        result.request_context() == result.context(),
        scope_matches(&result.scope(), result.context()),
        forall|i: int| 0 <= i < result.invocations().len() ==>
            result.invocations()[i].valid()
                && result.invocations()[i].context() == result.context()
                && result.invocations()[i].point() <= result.point(),
{
    assert(covered_at(result.request, result.candidate, 0));
}

/// The modeled environment supplies a readable source fact, not an opaque input ID.
pub open spec fn example_context(whole: bool) -> ModelContext {
    ModelContext {
        request: seq![7u8], scope: ScopeKey { whole,
            selection: if whole { Seq::empty() } else { seq![9u8] } },
        runtime: seq![8u8], mode: Mode::Default,
        solver: SolverId { name: seq![1u8], version: seq![2u8], binary: seq![3u8], platform: seq![4u8] },
        inputs: Map::empty().insert(InputKey { role: 1, lexical: seq![4u8] },
            ReadOutcome::Present { resolved: seq![5u8], bytes: seq![6u8] }),
    }
}

pub open spec fn sample_target(target: &TargetId, name: u8) -> bool {
    target.package@ == seq![1u8] && target.cargo_name@ == seq![name]
        && target.kind@ == seq![3u8] && target.source@ == seq![4u8]
        && target.invocation@ == seq![5u8]
}

fn example_bytes(value: u8) -> (out: Vec<u8>)
    ensures out@ == seq![value],
{
    let mut out = Vec::new(); out.push(value); out
}

fn example_target(name: u8) -> (out: TargetId)
    ensures sample_target(&out, name),
{
    TargetId { package: example_bytes(1), cargo_name: example_bytes(name),
        kind: example_bytes(3), source: example_bytes(4), invocation: example_bytes(5) }
}

fn example_scope(whole: bool) -> (out: Scope)
    ensures out.whole == whole,
        out.selection@ == (if whole { Seq::<u8>::empty() } else { seq![9u8] }),
{
    Scope { whole, selection: if whole { Vec::new() } else { example_bytes(9) } }
}

fn example_request(whole: bool) -> (out: Request)
    ensures out.targets.len() == 1, sample_target(&out.targets[0], 2),
        scope_matches(&out.scope, example_context(whole)),
        out.context@ == example_context(whole),
{
    let mut targets = Vec::new(); targets.push(example_target(2));
    Request { targets, scope: example_scope(whole), context: Ghost(example_context(whole)) }
}

fn example_invocation(whole: bool, id: u64, origin: Origin) -> (out: Invocation)
    ensures invocation_wf(&out), out.context@ == example_context(whole),
        out.run_id == id, out.started_at == id, out.origin == origin,
        out.verification == VerificationSetting::Enabled,
        sample_target(&out.target, 2), scope_matches(&out.scope, example_context(whole)),
{
    Invocation { target: example_target(2), scope: example_scope(whole),
        context: Ghost(example_context(whole)), run_id: id, started_at: id, origin, verification: VerificationSetting::Enabled }
}

fn example_document(whole: bool, id: u64, has_function: bool,
    verdict: Verdict, establishment: Establishment, terminal: VerifierEnd) -> (out: VerifierDocument)
    ensures out.context@ == example_context(whole), out.run_id == id,
        sample_target(&out.target, 2), scope_matches(&out.scope, example_context(whole)),
        out.functions.len() == (if has_function { 1int } else { 0int }),
        has_function ==> out.functions[0].name@ == seq![12u8],
        has_function ==> out.functions[0].evidence_tier.is_some(),
        out.transcripts@.len() == 0,
        document_good(&out) == (terminal == VerifierEnd::Finished(0)
            && (!has_function || (verdict == Verdict::Verified && establishment == Establishment::Confirmed))),
{
    let mut functions = Vec::new();
    if has_function { functions.push(Function { name: example_bytes(12), verdict, establishment,
        evidence_tier: Some(EvidenceTier::Fresh) }); }
    assert forall|i: int| 0 <= i < functions.len() implies
        functions[i].verdict == verdict && functions[i].establishment == establishment
            && functions[i].evidence_tier.is_some() by {
        assert(i == 0);
    }
    let n = functions.len();
    let claimed_summary = Summary { functions: n,
        verified: if verdict == Verdict::Verified { n } else { 0 },
        failed: if verdict == Verdict::Failed { n } else { 0 },
        unknown: if verdict == Verdict::Unknown { n } else { 0 },
        provisional: if verdict == Verdict::Provisional { n } else { 0 } };
    let out = VerifierDocument { target: example_target(2), scope: example_scope(whole),
        context: Ghost(example_context(whole)), run_id: id,
        functions, claimed_summary, terminal, transcripts: Ghost(Seq::empty()) };
    if has_function && (verdict != Verdict::Verified || establishment != Establishment::Confirmed) {
        assert(!good_function(&out.functions[0]));
    }
    out
}

/// Fixture support explicitly emits each child-start and child-close event.
/// The lifecycle itself never manufactures this ledger from a completed document.
proof fn record_child_lifecycles(run: RunningInvocation, transcripts: Seq<SolverTranscript>)
    -> (out: RunningInvocation)
    requires running_wf(&run), run.stream() == StreamState::Running,
        run.children().len() == 0,
        forall|i: int| 0 <= i < transcripts.len() ==>
            transcript_wf(transcripts[i]) && transcripts[i].solver == run.invocation().context@.solver,
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.stream() == StreamState::Running,
        out.completion() == run.completion(),
        out.children().len() == transcripts.len(),
        every_child_closed(out.children()), child_stream_inventory(out.children()) == transcripts,
    decreases transcripts.len(),
{
    if transcripts.len() == 0 {
        assert(child_stream_inventory(run.children()) =~= transcripts);
        run
    } else {
        let current = record_child_lifecycles(run, transcripts.drop_last());
        let before = current.children();
        let i = transcripts.len() - 1;
        let started_option = stream_started(current, transcripts[i].solver);
        assert(started_option.is_some()); let started = started_option.unwrap();
        assert(started.children()[i].closed().is_none());
        assert(started.children()[i].solver() == transcripts[i].solver);
        let closed_option = stream_closed(started, i, transcripts[i]);
        assert(closed_option.is_some()); let closed = closed_option.unwrap();
        assert forall|k: int| 0 <= k < closed.children().len()
            implies closed.children()[k].closed().is_some() by {
            if k < i { assert(closed.children()[k] == before[k]); }
            else { assert(k == i); }
        }
        assert forall|k: int| 0 <= k < child_stream_inventory(closed.children()).len()
            implies #[trigger] child_stream_inventory(closed.children())[k] == transcripts[k] by {
            if k < i {
                assert(closed.children()[k] == before[k]);
                assert(child_stream_inventory(before)[k] == transcripts.drop_last()[k]);
            } else { assert(k == i); }
        }
        assert(child_stream_inventory(closed.children()) =~= transcripts);
        closed
    }
}

proof fn positive_engine_completion(whole: bool) -> (out: EngineCompletionObservation)
    ensures engine_protocol_complete(&out, ProcessEnd::Exit(0)),
{
    EngineCompletionObservation {
        protocol: Some(EngineProtocolObservation { raw: seq![10u8], versions: seq![Some(1u64)],
            terminal: Some(EngineTerminalObservation { version: 1, event: b"compiler-sealed"@,
                phase: b"full"@, normal: Some(true), success: Some(true) }) }),
        stdout: Seq::empty(), remainder: Seq::empty(),
        documents: seq![EngineDocumentObservation { functions: EngineFieldShape::Object,
            version: Some(seq![1u8]), profile: Some(seq![2u8]), toolchain: Some(seq![3u8]),
            commit: Some(seq![4u8]), os: Some(seq![5u8]), arch: Some(seq![6u8]),
            error: Some(false), vir_error: Some(false), success: EngineSuccessField::Boolean(true),
            verified: Some(1), errors: Some(0), whole: Some(whole) }],
    }
}

/// Positive fixture genuinely starts, observes and completes the modeled process.
proof fn single_completion(invocation: Invocation, document: VerifierDocument, point: u64)
    -> (out: CompletedInvocation)
    requires invocation_wf(&invocation), !matches!(invocation.origin, Origin::Fallback(_) | Origin::RestartAfterDecline(_)),
        invocation.verification == VerificationSetting::Enabled,
        document_matches(&invocation, &document), document_good(&document), document_streams_valid(&document),
        point >= invocation.started_at,
    ensures out.valid(), out.invocation() == invocation, out.context() == invocation.context@,
        out.point() == point, out.documents() == seq![document],
        out.transcripts() == document.transcripts@,
{
    let started = start(invocation);
    assert(started.is_some());
    assert(started.unwrap().documents() =~= Seq::<VerifierDocument>::empty());
    let with_children = record_child_lifecycles(started.unwrap(), document.transcripts@);
    let with_children = if invocation.origin == Origin::Engine {
        let protocol = positive_engine_completion(invocation.scope.whole);
        // Child lifecycle construction does not alter the initially absent record.
        assert(with_children.completion().is_none());
        observe_engine_completion(with_children, protocol)
    } else { with_children };
    let observed = observe(with_children, document);
    assert(observed.documents() == seq![document]);
    single_document_inventory(document);
    assert(observed_transcripts(observed.documents()) == document.transcripts@);
    assert(can_complete(&observed, ProcessEnd::Exit(0), point));
    let completed = complete(observed, ProcessEnd::Exit(0), point);
    assert(completed.is_some());
    assert(observed_transcripts(seq![document].drop_last()) == Seq::<SolverTranscript>::empty());
    assert(Seq::<SolverTranscript>::empty() + document.transcripts@ =~= document.transcripts@);
    completed.unwrap()
}

pub fn positive_nonempty_lifecycle_control() {
    let request = example_request(true);
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let valid = validate_document(&document);
    assert(valid);
    proof {
        let completed = single_completion(invocation, document, 2);
        let reports = seq![completed];
        let candidate_option = finish_project(example_context(true), reports, ExecutionPath::Direct,
            ProcessEnd::Exit(0), 3);
        assert(candidate_option.is_some());
        let candidate = candidate_option.unwrap();
        assert(candidate.invocations() == seq![completed]);
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0));
        assert(covers(&request, &candidate));
        let accepted = validate_current_complete(&request, &candidate, example_context(true), 4);
        assert(accepted.is_some());
        acceptance_has_current_origin(&accepted.unwrap());
        let result = accepted.unwrap();
        assert(result.requested_targets() == request.targets@);
        assert(result.invocations()[0].documents() == seq![document]);
        assert(result.invocations()[0].documents()[0].functions[0].name@ == seq![12u8]);
        assert(repair_selectable(&result, 0, seq![12u8]));
        assert(repair_target_origin_allowed(&result, 0));
        let repair = repair_accepted(&accepted.unwrap(), 0, seq![12u8]);
        assert(repair);
    }
}

pub fn positive_zero_function_lifecycle_control() {
    let request = example_request(true);
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, false, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let valid = validate_document(&document); assert(valid);
    proof {
        let completed = single_completion(invocation, document, 2);
        let candidate_option = finish_project(example_context(true), seq![completed], ExecutionPath::Direct,
            ProcessEnd::Exit(0), 3);
        assert(candidate_option.is_some());
        let candidate = candidate_option.unwrap();
        assert(candidate.invocations() == seq![completed]);
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0));
        assert(covers(&request, &candidate));
        let accepted = validate_current_complete(&request, &candidate, example_context(true), 4);
        assert(accepted.is_some());
    }
}

/// Reusable non-vacuous witness for the composed model, with the supplied exact
/// input context and the complete observed solver inventory passed through.
pub fn example_candidate(Ghost(context): Ghost<ModelContext>,
    Ghost(transcripts): Ghost<Seq<SolverTranscript>>) -> (pair: (Request, Ghost<Candidate>))
    requires context_wf(context), context.scope.whole, context.scope.selection.len() == 0,
        forall|i: int| 0 <= i < transcripts.len() ==>
            transcript_wf(transcripts[i]) && transcripts[i].solver == context.solver,
    ensures pair.0.context@ == context, pair.0.scope.whole, pair.0.targets.len() == 1,
        safe_scoped(&pair.0, &pair.1@, context, 0),
        pair.1@.invocations().len() == 1,
        pair.1@.invocations()[0].documents().len() == 1,
        pair.1@.invocations()[0].documents()[0].functions.len() == 1,
        pair.1@.invocations()[0].documents()[0].functions[0].name@ == seq![12u8],
        pair.1@.invocations()[0].transcripts() == transcripts,
{
    let mut targets = Vec::new(); targets.push(example_target(2));
    let request = Request { targets, scope: example_scope(true), context: Ghost(context) };
    let invocation = Invocation { target: example_target(2), scope: example_scope(true),
        context: Ghost(context), run_id: 1, started_at: 0, origin: Origin::Direct, verification: VerificationSetting::Enabled };
    let mut functions = Vec::new();
    functions.push(Function { name: example_bytes(12), verdict: Verdict::Verified,
        establishment: Establishment::Confirmed, evidence_tier: Some(EvidenceTier::Fresh) });
    let document = VerifierDocument { target: example_target(2), scope: example_scope(true),
        context: Ghost(context), run_id: 1, functions,
        claimed_summary: Summary { functions: 1, verified: 1, failed: 0, unknown: 0, provisional: 0 },
        terminal: VerifierEnd::Finished(0), transcripts: Ghost(transcripts) };
    let checked = validate_document(&document); assert(checked);
    assert(context_wf(context));
    assert(invocation.context@ == context);
    assert(context.scope.selection =~= Seq::<u8>::empty());
    assert(scope_matches(&invocation.scope, context));
    assert(invocation_wf(&invocation));
    let ghost candidate = {
        let completed = single_completion(invocation, document, 0);
        let candidate_option = finish_project(context, seq![completed], ExecutionPath::Direct,
            ProcessEnd::Exit(0), 0);
        assert(candidate_option.is_some());
        let candidate = candidate_option.unwrap();
        assert(candidate.invocations() == seq![completed]);
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0));
        assert(covers(&request, &candidate));
        candidate
    };
    (request, Ghost(candidate))
}

/// No output, cancelled streams and malformed streams cannot be completed.
pub proof fn incomplete_or_stopped_refused(run: &RunningInvocation, end: ProcessEnd, point: u64)
    requires run.documents().len() == 0 || run.stream() != StreamState::Running,
    ensures !can_complete(run, end, point),
{}

/// A supplied document for another input/runtime/request cannot complete this run.
pub proof fn context_mismatch_refused(run: &RunningInvocation, index: int, end: ProcessEnd, point: u64)
    requires 0 <= index < run.documents().len(),
        run.documents()[index].context@ != run.invocation().context@,
    ensures !can_complete(run, end, point),
{
    assert(!document_matches(&run.invocation(), &run.documents()[index]));
}

pub proof fn failed_unknown_provisional_refused(run: &RunningInvocation, document: int,
    function: int, end: ProcessEnd, point: u64)
    requires 0 <= document < run.documents().len(),
        0 <= function < run.documents()[document].functions.len(),
        !good_function(&run.documents()[document].functions[function]),
    ensures !can_complete(run, end, point),
{
    assert(!document_good(&run.documents()[document]));
}

pub proof fn malformed_or_error_document_refused(run: &RunningInvocation, index: int,
    end: ProcessEnd, point: u64)
    requires 0 <= index < run.documents().len(),
        run.documents()[index].terminal != VerifierEnd::Finished(0),
    ensures !can_complete(run, end, point),
{}

pub proof fn stale_context_refused(r: &Request, c: &Candidate, now: ModelContext, point: u64)
    requires c.context() != now,
    ensures !safe_scoped(r, c, now, point),
{}

pub fn interrupted_and_context_mismatch_controls() {
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let initial_option = start(invocation); assert(initial_option.is_some());
        let initial = initial_option.unwrap();
        let empty = complete(initial, ProcessEnd::Exit(0), 2); assert(empty.is_none());
        let observed = observe(initial, document);
        let interrupted = interrupt(observed);
        let stopped = complete(interrupted, ProcessEnd::Exit(0), 2); assert(stopped.is_none());
        let malformed = malformed(observed);
        let bad = complete(malformed, ProcessEnd::Exit(0), 2); assert(bad.is_none());
        let wrong_context = ModelContext { runtime: seq![99u8], ..example_context(true) };
        let wrong_document = VerifierDocument { context: Ghost(wrong_context), ..document };
        let wrong_run = observe(initial, wrong_document);
        assert(wrong_run.documents()[0].context@ == wrong_context);
        assert(wrong_run.invocation().context@ == example_context(true));
        assert(wrong_context.runtime[0] != example_context(true).runtime[0]);
        context_mismatch_refused(&wrong_run, 0, ProcessEnd::Exit(0), 2);
        let wrong = complete(wrong_run, ProcessEnd::Exit(0), 2); assert(wrong.is_none());
    }
}

fn reject_document(verdict: Verdict, establishment: Establishment, terminal: VerifierEnd)
    requires verdict != Verdict::Verified || establishment != Establishment::Confirmed
        || terminal != VerifierEnd::Finished(0),
{
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, verdict, establishment, terminal);
    let checked = validate_document(&document); assert(!checked);
    proof {
        let initial_option = start(invocation); assert(initial_option.is_some());
        let initial = initial_option.unwrap();
        let observed = observe(initial, document);
        assert(!document_good(&observed.documents()[0]));
        let completed = complete(observed, ProcessEnd::Exit(0), 2); assert(completed.is_none());
    }
}

pub fn concrete_failed_unknown_provisional_controls() {
    reject_document(Verdict::Failed, Establishment::Confirmed, VerifierEnd::Finished(1));
    reject_document(Verdict::Unknown, Establishment::Unresolved, VerifierEnd::Finished(0));
    reject_document(Verdict::Provisional, Establishment::Provisional, VerifierEnd::Finished(0));
    reject_document(Verdict::Verified, Establishment::Unresolved, VerifierEnd::Finished(0));
    reject_document(Verdict::Verified, Establishment::Confirmed, VerifierEnd::Malformed);
    reject_document(Verdict::Verified, Establishment::Confirmed, VerifierEnd::VirError);
}

pub fn missing_sibling_and_ambiguous_reports_controls() {
    let mut request = example_request(true);
    request.targets.push(example_target(3));
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let completed = single_completion(invocation, document, 2);
        let candidate_option = finish_project(example_context(true), seq![completed],
            ExecutionPath::Direct, ProcessEnd::Exit(0), 3);
        assert(candidate_option.is_some());
        let candidate = candidate_option.unwrap();
        assert(candidate.invocations() == seq![completed]);
        assert forall|j: int| 0 <= j < candidate.invocations().len() implies
            !same_target(&request.targets[1], &candidate.invocations()[j].invocation().target) by {
            assert(j == 0);
            assert(request.targets[1].cargo_name@[0] == 3);
            assert(candidate.invocations()[j].invocation().target.cargo_name@[0] == 2);
        }
        assert(!covered_at(&request, &candidate, 1));
        assert(!covers(&request, &candidate));
        let missing = validate_current_complete(&request, &candidate, example_context(true), 4);
        assert(missing.is_none());
        let duplicated_option = finish_project(example_context(true), seq![completed, completed],
            ExecutionPath::Direct, ProcessEnd::Exit(0), 3);
        assert(duplicated_option.is_some());
        let duplicated = duplicated_option.unwrap();
        assert(duplicated.invocations() == seq![completed, completed]);
        assert(same_target(&duplicated.invocations()[0].invocation().target,
            &duplicated.invocations()[1].invocation().target));
        if distinct_reports(&duplicated) {
            assert(!same_target(&duplicated.invocations()[0].invocation().target,
                &duplicated.invocations()[1].invocation().target));
        }
        assert(!distinct_reports(&duplicated));
        let ambiguous = validate_current_complete(&request, &duplicated, example_context(true), 4);
        assert(ambiguous.is_none());
    }
}

pub fn retained_job_stale_and_revert_controls() {
    let request = example_request(true);
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let completed = single_completion(invocation, document, 2);
        let candidate_option = finish_project(example_context(true), seq![completed],
            ExecutionPath::Direct, ProcessEnd::Exit(0), 3);
        assert(candidate_option.is_some());
        let candidate = candidate_option.unwrap();
        let changed = ModelContext { inputs: example_context(true).inputs.insert(
            InputKey { role: 1, lexical: seq![4u8] },
            ReadOutcome::Present { resolved: seq![5u8], bytes: seq![99u8] }), ..example_context(true) };
        let key = InputKey { role: 1, lexical: seq![4u8] };
        assert(changed.inputs[key] == ReadOutcome::Present { resolved: seq![5u8], bytes: seq![99u8] });
        assert(candidate.context().inputs[key] == ReadOutcome::Present { resolved: seq![5u8], bytes: seq![6u8] });
        assert(seq![6u8][0] != seq![99u8][0]);
        assert(seq![6u8] != seq![99u8]);
        assert(changed.inputs[key] != candidate.context().inputs[key]);
        assert(changed != candidate.context());
        let historical = project_job(&request, &candidate, changed, 4);
        assert(historical.is_none());
        assert(candidate.invocations() == seq![completed]);
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0)); assert(covers(&request, &candidate));
        let restored = project_job(&request, &candidate, example_context(true), 5);
        assert(restored.is_some());
    }
}

pub fn partial_scope_never_whole_control() {
    let request = example_request(false);
    let invocation = example_invocation(false, 1, Origin::Direct);
    let document = example_document(false, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let completed = single_completion(invocation, document, 2);
        let candidate_option = finish_project(example_context(false), seq![completed], ExecutionPath::Direct,
            ProcessEnd::Exit(0), 3);
        assert(candidate_option.is_some()); let candidate = candidate_option.unwrap();
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0)); assert(covers(&request, &candidate));
        let scoped = validate_scoped(&request, &candidate, example_context(false), 4);
        assert(scoped.is_some());
        let complete = validate_current_complete(&request, &candidate, example_context(false), 4);
        assert(complete.is_none());
    }
}

/// One project can contain independently completed direct, engine and fallback
/// targets without discarding their individual origins or accepting failed work.
pub fn positive_mixed_target_origins_control() {
    let mut request = example_request(true);
    request.targets.push(example_target(3));
    request.targets.push(example_target(4));
    let direct = example_invocation(true, 1, Origin::Direct);
    let direct_document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let mut engine = example_invocation(true, 2, Origin::Engine);
    engine.target = example_target(3);
    let mut engine_document = example_document(true, 2, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    engine_document.target = example_target(3);
    let mut failed = example_invocation(false, 3, Origin::Engine);
    failed.target = example_target(4);
    let mut fallback = example_invocation(true, 5, Origin::Fallback(3));
    fallback.target = example_target(4);
    let mut fallback_document = example_document(true, 5, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    fallback_document.target = example_target(4);
    proof {
        let direct_done = single_completion(direct, direct_document, 2);
        let engine_done = single_completion(engine, engine_document, 3);
        let failed_start = start(failed);
        assert(failed_start.is_some());
        let failed_end = engine_failed(failed_start.unwrap(), ProcessEnd::Interrupted, 4);
        assert(failed_end.is_some());
        let swapped_target = Invocation { target: direct.target, ..fallback };
        assert(failed_end.unwrap().invocation().target.cargo_name@[0] == 4u8);
        assert(swapped_target.target.cargo_name@[0] == 2u8);
        assert(!same_target(&failed_end.unwrap().invocation().target, &swapped_target.target));
        assert(!fallback_subject_matches(&failed_end.unwrap().invocation(), &swapped_target));
        let wrong_target = start_fallback(failed_end.unwrap(), swapped_target);
        assert(wrong_target.is_none());
        let changed_context = ModelContext { runtime: seq![99u8], ..example_context(true) };
        let swapped_runtime = Invocation { context: Ghost(changed_context), ..fallback };
        assert(failed_end.unwrap().invocation().context@.runtime[0] == 8u8);
        assert(swapped_runtime.context@.runtime[0] == 99u8);
        assert(failed_end.unwrap().invocation().context@.runtime != swapped_runtime.context@.runtime);
        assert(!fallback_subject_matches(&failed_end.unwrap().invocation(), &swapped_runtime));
        let wrong_runtime = start_fallback(failed_end.unwrap(), swapped_runtime);
        assert(wrong_runtime.is_none());
        let fallback_start = start_fallback(failed_end.unwrap(), fallback);
        assert(fallback_start.is_some());
        let fallback_observed = observe(fallback_start.unwrap(), fallback_document);
        assert(fallback_observed.documents() == seq![fallback_document]);
        single_document_inventory(fallback_document);
        assert(child_stream_inventory(fallback_observed.children()) =~= Seq::<SolverTranscript>::empty());
        assert(observed_transcripts(fallback_observed.documents()) =~= Seq::<SolverTranscript>::empty());
        let fallback_end = complete(fallback_observed, ProcessEnd::Exit(0), 6);
        assert(fallback_end.is_some());
        let fallback_done = fallback_end.unwrap();
        completed_origin_is_justified(&fallback_done);
        assert(fallback_done.predecessor().unwrap().invocation().run_id == 3);
        let reports = seq![direct_done, engine_done, fallback_done];
        let finished = finish_project_per_target(example_context(true), reports, ProcessEnd::Exit(0), 7);
        assert(finished.is_some());
        let candidate = finished.unwrap();
        assert(candidate.invocations() == reports);
        assert forall|i: int| 0 <= i < request.targets.len()
            implies covered_at(&request, &candidate, i) by {
            assert(same_target(&request.targets[i], &candidate.invocations()[i].invocation().target));
            assert(same_scope(&request.scope, &candidate.invocations()[i].invocation().scope));
        }
        assert forall|i: int| 0 <= i < request.targets.len()
            implies request.targets[i].cargo_name@[0] as int == i + 2 by {
            if i == 0 {} else if i == 1 {} else { assert(i == 2); }
        }
        assert forall|i: int, j: int| 0 <= i < j < request.targets.len()
            implies !same_target(&request.targets[i], &request.targets[j]) by {
            assert(request.targets[i].cargo_name@[0] != request.targets[j].cargo_name@[0]);
        }
        assert(distinct_requests(&request));
        assert forall|i: int, j: int| 0 <= i < j < candidate.invocations().len()
            implies !same_target(&candidate.invocations()[i].invocation().target,
                &candidate.invocations()[j].invocation().target) by {
            assert(same_target(&request.targets[i], &candidate.invocations()[i].invocation().target));
            assert(same_target(&request.targets[j], &candidate.invocations()[j].invocation().target));
            assert(!same_target(&request.targets[i], &request.targets[j]));
        }
        assert(distinct_reports(&candidate));
        let accepted = validate_current_complete(&request, &candidate, example_context(true), 8);
        assert(accepted.is_some());
        let result = accepted.unwrap();
        acceptance_has_current_origin(&result);
        assert(repair_selectable(&result, 2, seq![12u8]));
        assert(!repair_target_origin_allowed(&result, 2));
        let rejected_repair = repair_accepted(&result, 2, seq![12u8]);
        assert(!rejected_repair);
        // A fresh mixed result cannot use the failed engine's partial record in
        // place of the independently completed whole fallback target.
        let omitted = finish_project_per_target(example_context(true), seq![direct_done, engine_done],
            ProcessEnd::Exit(0), 7);
        assert(omitted.is_some());
        let missing = omitted.unwrap();
        assert forall|i: int| 0 <= i < missing.invocations().len() implies
            !same_target(&request.targets[2], &missing.invocations()[i].invocation().target) by {}
        assert(!covered_at(&request, &missing, 2));
        let refused = validate_current_complete(&request, &missing, example_context(true), 8);
        assert(refused.is_none());
    }
}

/// The failed engine cannot be borrowed to justify another target or execution
/// context, even when both candidates have individually well-formed metadata.
pub proof fn fallback_subject_swap_refused(failed: EngineFailure, fresh: Invocation)
    requires failure_valid(&failed), !fallback_subject_matches(&failed.invocation(), &fresh),
{
    let started = start_fallback(failed, fresh);
    assert(started.is_none());
}

/// An exit-zero engine with an unusable side channel restarts independently;
/// it never masquerades as a process failure or leaks old documents.
pub fn normal_zero_decline_fresh_fallback_control() {
    let request = example_request(true);
    let engine = example_invocation(true, 1, Origin::Engine);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let fallback = example_invocation(true, 3, Origin::RestartAfterDecline(1));
    let fresh_document = example_document(true, 3, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let started = start(engine);
        assert(started.is_some());
        let observed = observe(started.unwrap(), document);
        let valid_protocol = positive_engine_completion(true);
        let missing_terminal = EngineCompletionObservation { protocol: None, ..valid_protocol };
        let observed = observe_engine_completion(observed, missing_terminal);
        let rejected_completion = complete(observed, ProcessEnd::Exit(0), 2);
        assert(rejected_completion.is_none());
        let not_a_crash = engine_failed(observed, ProcessEnd::Exit(0), 2);
        assert(not_a_crash.is_none());
        let declined = engine_declined(observed, ProcessEnd::Exit(0), 2);
        assert(declined.is_some());
        assert(declined.unwrap().end() == ProcessEnd::Exit(0));
        let wrong_status = engine_declined(observed, ProcessEnd::Exit(1), 2);
        assert(wrong_status.is_none());
        // Repeated metadata cannot overwrite the first unusable observation.
        let overwritten = observe_engine_completion(observed, valid_protocol);
        assert(overwritten.stream() == StreamState::Malformed);
        let still_rejected = complete(overwritten, ProcessEnd::Exit(0), 2);
        assert(still_rejected.is_none());
        let restarted = start_declined_fallback(declined.unwrap(), fallback);
        assert(restarted.is_some());
        assert(restarted.unwrap().documents() =~= Seq::<VerifierDocument>::empty());
        let contaminated = observe(restarted.unwrap(), document);
        assert(!document_matches(&contaminated.invocation(), &document));
        let leaked = complete(contaminated, ProcessEnd::Exit(0), 4);
        assert(leaked.is_none());
        let fresh = observe(restarted.unwrap(), fresh_document);
        assert(fresh.documents() == seq![fresh_document]);
        single_document_inventory(fresh_document);
        assert(observed_transcripts(fresh.documents()) =~= Seq::<SolverTranscript>::empty());
        assert(child_stream_inventory(fresh.children()) =~= Seq::<SolverTranscript>::empty());
        let completed = complete(fresh, ProcessEnd::Exit(0), 4);
        assert(completed.is_some());
        completed_origin_is_justified(&completed.unwrap());
        assert(completed.unwrap().declined_predecessor().is_some());
        let finished = finish_project_per_target(example_context(true), seq![completed.unwrap()], ProcessEnd::Exit(0), 5);
        assert(finished.is_some());
        let candidate = finished.unwrap();
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0));
        let accepted = validate_current_complete(&request, &candidate, example_context(true), 6);
        assert(accepted.is_some());
        assert(repair_selectable(&accepted.unwrap(), 0, seq![12u8]));
        assert(!repair_target_origin_allowed(&accepted.unwrap(), 0));
        let repair = repair_accepted(&accepted.unwrap(), 0, seq![12u8]);
        assert(!repair);
        // Consistent complete engine evidence is never relabeled as a decline.
        let good_started = start(engine);
        assert(good_started.is_some());
        let good_observed = observe_engine_completion(good_started.unwrap(), valid_protocol);
        let good_decline = engine_declined(good_observed, ProcessEnd::Exit(0), 2);
        assert(good_decline.is_none());
    }
}

/// A failed partial engine run cannot leak its output into the fresh full fallback.
pub fn independent_fallback_control() {
    let request = example_request(true);
    let engine_invocation = example_invocation(false, 1, Origin::Engine);
    let partial_document = example_document(false, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let fallback_invocation = example_invocation(true, 3, Origin::Fallback(1));
    let full_document = example_document(true, 3, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let engine_option = start(engine_invocation); assert(engine_option.is_some());
        let engine = observe(engine_option.unwrap(), partial_document);
        let failure_option = engine_failed(engine, ProcessEnd::Exit(1), 2);
        assert(failure_option.is_some());
        let fallback_option = start_fallback(failure_option.unwrap(), fallback_invocation);
        assert(fallback_option.is_some()); let fallback = fallback_option.unwrap();
        let empty = complete(fallback, ProcessEnd::Exit(0), 4); assert(empty.is_none());
        let contaminated = observe(fallback, partial_document);
        assert(contaminated.documents()[0].run_id == 1);
        assert(contaminated.invocation().run_id == 3);
        let leaked = complete(contaminated, ProcessEnd::Exit(0), 4); assert(leaked.is_none());
        assert(fallback.documents() =~= Seq::<VerifierDocument>::empty());
        let fresh = observe(fallback, full_document);
        single_document_inventory(full_document);
        assert(fresh.documents() == seq![full_document]);
        assert(child_stream_inventory(fresh.children()) =~= Seq::<SolverTranscript>::empty());
        assert(observed_transcripts(fresh.documents()) =~= Seq::<SolverTranscript>::empty());
        let completed_option = complete(fresh, ProcessEnd::Exit(0), 4);
        assert(completed_option.is_some()); let completed = completed_option.unwrap();
        let candidate_option = finish_project(example_context(true), seq![completed],
            ExecutionPath::IndependentFallback(1), ProcessEnd::Exit(0), 5);
        assert(candidate_option.is_some()); let candidate = candidate_option.unwrap();
        assert(same_target(&request.targets[0], &candidate.invocations()[0].invocation().target));
        assert(same_scope(&request.scope, &candidate.invocations()[0].invocation().scope));
        assert(covered_at(&request, &candidate, 0)); assert(covers(&request, &candidate));
        let ordinary = validate_current_complete(&request, &candidate, example_context(true), 6);
        assert(ordinary.is_some());
        let repair = repair_accepted(&ordinary.unwrap(), 0, seq![12u8]);
        assert(!repair);
        let old_partial = single_completion(engine_invocation, partial_document, 2);
        assert(!path_matches(ExecutionPath::IndependentFallback(1), &old_partial));
        let old_reports = seq![old_partial];
        assert(old_reports.len() == 1);
        assert(old_reports[0] == old_partial);
        if project_records_valid(example_context(true), old_reports, ExecutionPath::IndependentFallback(1), 5) {
            assert(path_matches(ExecutionPath::IndependentFallback(1), &old_reports[0]));
        }
        let mixed = finish_project(example_context(true), seq![old_partial],
            ExecutionPath::IndependentFallback(1), ProcessEnd::Exit(0), 5);
        assert(mixed.is_none());
    }
}

pub fn missing_tier_and_inconsistent_summary_controls() {
    let mut missing_tier = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    missing_tier.functions[0].evidence_tier = None;
    let missing = validate_document(&missing_tier); assert(!missing);
    let mut inconsistent = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    inconsistent.claimed_summary.verified = 0;
    let bad = validate_document(&inconsistent); assert(!bad);
}

/// A started child remains mandatory even when both supplied document/query
/// inventories omit it; an open child and a closed-but-omitted child both refuse.
pub fn child_stream_omission_and_open_controls() {
    let invocation = example_invocation(true, 1, Origin::Direct);
    let document = example_document(true, 1, true, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    proof {
        let initial_option = start(invocation); assert(initial_option.is_some());
        let initial = initial_option.unwrap();
        assert(initial.documents() =~= Seq::<VerifierDocument>::empty());
        let started_option = stream_started(initial, example_context(true).solver);
        assert(started_option.is_some()); let started = started_option.unwrap();
        let observed = observe(started, document);
        assert(observed.children().len() == 1);
        assert(observed.children()[0].closed().is_none());
        assert(!every_child_closed(observed.children()));
        let open = complete(observed, ProcessEnd::Exit(0), 2); assert(open.is_none());
        let transcript = SolverTranscript { solver: example_context(true).solver,
            raw: seq![42u8], endpoints: seq![1nat], replies: seq![seq![17u8]] };
        let closed_option = stream_closed(observed, 0, transcript);
        assert(closed_option.is_some()); let closed = closed_option.unwrap();
        assert(closed.documents().len() == 1);
        single_document_inventory(document);
        assert(observed_transcripts(closed.documents()).len() == 0);
        assert(child_stream_inventory(closed.children()).len() == 1);
        let omitted = complete(closed, ProcessEnd::Exit(0), 2); assert(omitted.is_none());
        let wrong_solver = SolverId { binary: seq![99u8], ..example_context(true).solver };
        assert(wrong_solver.binary[0] != example_context(true).solver.binary[0]);
        let wrong = stream_started(initial, wrong_solver); assert(wrong.is_none());
        let wrong_close = stream_closed(observed, 0,
            SolverTranscript { solver: wrong_solver, ..transcript });
        assert(wrong_close.is_none());
    }
}

/// The independent ledger cannot be replaced by the document inventory.
pub proof fn completed_inventory_matches_started_children(completed: &CompletedInvocation)
    requires completed.valid(),
    ensures every_child_closed(completed.children()),
        completed.transcripts() == child_stream_inventory(completed.children()),
        forall|i: int| 0 <= i < completed.children().len() ==>
            completed.children()[i].solver() == completed.context().solver
                && completed.children()[i].closed().is_some(),
{}

/// Even a document that claims a plausible final inventory cannot close a live child.
pub proof fn open_child_cannot_complete(run: &RunningInvocation, index: int, end: ProcessEnd, point: u64)
    requires 0 <= index < run.children().len(), run.children()[index].closed().is_none(),
    ensures !can_complete(run, end, point),
{
    assert(!every_child_closed(run.children()));
}

/// Compile-only terminal success has no proof authority, even with zero functions.
pub fn compile_only_zero_function_completion_refused()
{
    let invocation = example_invocation(true, 1, Origin::Direct);
    let compile_only = Invocation { verification: VerificationSetting::CompileOnly, ..invocation };
    let document = example_document(true, 1, false, Verdict::Verified,
        Establishment::Confirmed, VerifierEnd::Finished(0));
    let ghost checked = {
        let started = start(compile_only);
        assert(started.is_some());
        let observed = observe(started.unwrap(), document);
        assert(observed.documents().len() == 1);
        assert(document_good(&observed.documents()[0]));
        assert(observed.documents()[0].functions.len() == 0);
        let completion = complete(observed, ProcessEnd::Exit(0), 2);
        assert(completion.is_none());
    };
}


} // verus!
