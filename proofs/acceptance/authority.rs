//! Modeled invocation lifecycle, exact coverage and sealed result authority.
//! Real process observation and production presentation refinement remain external.
#![forbid(unsafe_code)]
use vstd::prelude::*;
use crate::context::{ModelContext, ScopeKey, Mode, InputKey, ReadOutcome, context_wf, SolverTranscript, transcript_wf};

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
pub enum EvidenceTier { Fresh, LocalCache, SharedCache, Bundled, Engine }

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
pub enum Origin { Direct, Engine, Fallback(u64) }

/// Metadata captured when this particular modeled verifier process starts.
pub struct Invocation {
    pub target: TargetId,
    pub scope: Scope,
    pub context: Ghost<ModelContext>,
    pub run_id: u64,
    pub started_at: u64,
    pub origin: Origin,
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
    forall|i: int| 0 <= i < doc.transcripts@.len() ==> transcript_wf(doc.transcripts@[i])
}

/// Derive the inventory from every observed document, preserving all sessions.
pub open spec fn observed_transcripts(documents: Seq<VerifierDocument>) -> Seq<SolverTranscript>
    decreases documents.len(),
{
    if documents.len() == 0 { Seq::empty() }
    else { observed_transcripts(documents.drop_last()) + documents.last().transcripts@ }
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

/// Private lifecycle state can be obtained only by modeled start/observation transitions.
pub struct RunningInvocation {
    invocation: Invocation,
    documents: Seq<VerifierDocument>,
    stream: StreamState,
    fallback_from: Option<EngineFailure>,
}

/// A failed engine execution, not a caller-supplied fallback flag.
pub struct EngineFailure {
    invocation: Invocation,
    end: ProcessEnd,
    failed_at: u64,
}

impl RunningInvocation {
    pub closed spec fn invocation(&self) -> Invocation { self.invocation }
    pub closed spec fn documents(&self) -> Seq<VerifierDocument> { self.documents }
    pub closed spec fn stream(&self) -> StreamState { self.stream }
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

pub closed spec fn origin_valid(run: &RunningInvocation) -> bool {
    match run.invocation.origin {
        Origin::Direct => run.fallback_from.is_none(),
        Origin::Engine => run.fallback_from.is_none(),
        Origin::Fallback(id) => run.fallback_from.is_some()
            && failure_valid(&run.fallback_from.unwrap())
            && run.fallback_from.unwrap().invocation.run_id == id
            && run.invocation.run_id > id && run.invocation.scope.whole
            && run.invocation.context@.request == run.fallback_from.unwrap().invocation.context@.request
            && run.invocation.context@.mode == run.fallback_from.unwrap().invocation.context@.mode,
    }
}

pub closed spec fn running_wf(run: &RunningInvocation) -> bool {
    invocation_wf(&run.invocation) && origin_valid(run)
}

pub proof fn start(invocation: Invocation) -> (run: Option<RunningInvocation>)
    ensures run.is_some() == (invocation_wf(&invocation)
        && !matches!(invocation.origin, Origin::Fallback(_))),
        run.is_some() ==> running_wf(&run.unwrap()),
        run.is_some() ==> run.unwrap().invocation() == invocation,
        run.is_some() ==> run.unwrap().documents().len() == 0,
        run.is_some() ==> run.unwrap().stream() == StreamState::Running,
{
    if !invocation_wf(&invocation) || matches!(invocation.origin, Origin::Fallback(_)) {
        None
    } else {
        Some(RunningInvocation { invocation, documents: Seq::empty(),
            stream: StreamState::Running, fallback_from: None })
    }
}

pub proof fn observe(run: RunningInvocation, document: VerifierDocument) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents().push(document), out.stream() == run.stream(),
{
    RunningInvocation { documents: run.documents.push(document), ..run }
}

pub proof fn interrupt(run: RunningInvocation) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.stream() == StreamState::Interrupted,
{
    RunningInvocation { stream: StreamState::Interrupted, ..run }
}

pub proof fn malformed(run: RunningInvocation) -> (out: RunningInvocation)
    requires running_wf(&run),
    ensures running_wf(&out), out.invocation() == run.invocation(),
        out.documents() == run.documents(), out.stream() == StreamState::Malformed,
{
    RunningInvocation { stream: StreamState::Malformed, ..run }
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
        out.is_some() ==> out.unwrap().stream() == StreamState::Running,
        out.is_some() == (invocation_wf(&invocation) && invocation.scope.whole
            && invocation.origin == Origin::Fallback(failed.invocation().run_id)
            && invocation.run_id > failed.invocation().run_id
            && invocation.started_at >= failed.point()
            && invocation.context@.request == failed.invocation().context@.request
            && invocation.context@.mode == failed.invocation().context@.mode),
{
    if invocation_wf(&invocation) && invocation.scope.whole
        && invocation.origin == Origin::Fallback(failed.invocation.run_id)
        && invocation.run_id > failed.invocation.run_id
        && invocation.started_at >= failed.failed_at
        && invocation.context@.request == failed.invocation.context@.request
        && invocation.context@.mode == failed.invocation.context@.mode {
        Some(RunningInvocation { invocation, documents: Seq::empty(),
            stream: StreamState::Running, fallback_from: Some(failed) })
    } else { None }
}

pub closed spec fn can_complete(run: &RunningInvocation, end: ProcessEnd, point: u64) -> bool {
    running_wf(run) && run.stream == StreamState::Running
        && end == ProcessEnd::Exit(0) && point >= run.invocation.started_at
        && run.documents.len() > 0
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
pub enum ExecutionPath { Direct, Engine, IndependentFallback(u64) }

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

/// Repair and ordinary result authority consume the same checked completion token.
pub proof fn repair_accepted(result: &ValidatedResult, target_index: int, name: Seq<u8>) -> (accepted: bool)
    requires result.valid(),
    ensures accepted == (result.whole() && repair_selectable(result, target_index, name)
        && !matches!(result.path(), ExecutionPath::IndependentFallback(_))),
{
    result.whole() && repair_selectable(result, target_index, name)
        && !matches!(result.path(), ExecutionPath::IndependentFallback(_))
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
        forall|i: int| 0 <= i < completed.transcripts().len()
            ==> transcript_wf(completed.transcripts()[i]),
        match completed.invocation().origin {
            Origin::Fallback(id) => completed.invocation().run_id > id
                && completed.invocation().scope.whole,
            _ => true,
        },
{
    observed_transcripts_are_well_formed(completed.documents());
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
        sample_target(&out.target, 2), scope_matches(&out.scope, example_context(whole)),
{
    Invocation { target: example_target(2), scope: example_scope(whole),
        context: Ghost(example_context(whole)), run_id: id, started_at: id, origin }
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

/// Positive fixture genuinely starts, observes and completes the modeled process.
proof fn single_completion(invocation: Invocation, document: VerifierDocument, point: u64)
    -> (out: CompletedInvocation)
    requires invocation_wf(&invocation), !matches!(invocation.origin, Origin::Fallback(_)),
        document_matches(&invocation, &document), document_good(&document), document_streams_valid(&document),
        point >= invocation.started_at,
    ensures out.valid(), out.invocation() == invocation, out.context() == invocation.context@,
        out.point() == point, out.documents() == seq![document],
        out.transcripts() == document.transcripts@,
{
    let started = start(invocation);
    assert(started.is_some());
    let observed = observe(started.unwrap(), document);
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
        forall|i: int| 0 <= i < transcripts.len() ==> transcript_wf(transcripts[i]),
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
        context: Ghost(context), run_id: 1, started_at: 0, origin: Origin::Direct };
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
        let fresh = observe(fallback, full_document);
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

} // verus!
