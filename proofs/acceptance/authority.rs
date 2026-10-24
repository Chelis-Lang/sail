//! Pure scope/result acceptance model, not production integration.
//! All identity fields are exact bytes, never digest or normalized names.
#![forbid(unsafe_code)]

use vstd::prelude::*;

verus! {

/// Exact, separately represented compiler target identity.
pub struct TargetId {
    /// Exact package/manifest identity bytes.
    pub package: Vec<u8>,
    /// Exact Cargo name before any Rust crate-name normalization.
    pub cargo_name: Vec<u8>,
    /// Target kind bytes.
    pub kind: Vec<u8>,
    /// Source/compiler-input identity bytes.
    pub source: Vec<u8>,
    /// Exact effective invocation bytes.
    pub invocation: Vec<u8>,
}

/// Logical target equality preserves every identity field.
pub open spec fn same_target(a: &TargetId, b: &TargetId) -> bool {
    a.package@ == b.package@ && a.cargo_name@ == b.cargo_name@
        && a.kind@ == b.kind@ && a.source@ == b.source@
        && a.invocation@ == b.invocation@
}

/// Whole-target scope or an exact partial selection.
pub struct Scope {
    /// Whether this is a complete whole-target invocation.
    pub whole: bool,
    /// Exact ordered selection encoding supplied by the request adapter.
    pub selection: Vec<u8>,
}

/// Scope equality preserves kind and the complete ordered selection.
pub open spec fn same_scope(a: &Scope, b: &Scope) -> bool {
    a.whole == b.whole && a.selection@ == b.selection@
}

/// Untrusted request inventory supplied by the request adapter.
pub struct Request {
    /// Every requested target, including targets whose compilation fails.
    pub targets: Vec<TargetId>,
    /// Exact effective verification selection.
    pub scope: Scope,
}

/// Projected function evidence; its trusted origin is a refinement obligation.
pub struct Function {
    /// Whether the observed verdict is verified.
    pub verified: bool,
    /// Whether establishment is confirmed rather than provisional.
    pub confirmed: bool,
    /// Whether a settled verdict has its required evidence tier.
    pub evidence_tier_present: bool,
}

/// Untrusted report projection for one target invocation.
pub struct TargetReport {
    /// Exact observed target identity.
    pub target: TargetId,
    /// Exact effective verification selection.
    pub scope: Scope,
    /// Claimed completion of this exact invocation, requiring origin refinement.
    pub complete: bool,
    /// Whether the process terminated normally with exit zero.
    pub normal_exit_zero: bool,
    /// Whether the report has an infrastructure/launch error.
    pub report_error: bool,
    /// Number of complete verifier documents whose facts were projected.
    pub verifier_documents: usize,
    /// Whether all projected verifier documents explicitly succeeded.
    pub verifier_success: bool,
    /// Explicit error count, absent when no complete count was observed.
    pub verifier_errors: Option<u64>,
    /// Whether any verifier document reports a VIR error.
    pub vir_error: bool,
    /// Whether an engine failure caused stock fallback.
    pub engine_fallback: bool,
    /// Actual report-checker violation count; deriving it is not proved here.
    pub invariant_violations: usize,
    /// Reported function total.
    pub summary_functions: usize,
    /// Reported verified count.
    pub summary_verified: usize,
    /// Reported failed count.
    pub summary_failed: usize,
    /// Reported unresolved count.
    pub summary_unknown: usize,
    /// Reported provisional count.
    pub summary_provisional: usize,
    /// All function records projected from the invocation.
    pub functions: Vec<Function>,
}

/// Untrusted project/result observations, never authority by themselves.
pub struct Candidate {
    /// Current observation justified by the separate input-binding boundary.
    pub current: bool,
    /// Whether the process terminated normally with exit zero.
    pub normal_exit_zero: bool,
    /// Whether the project run has an error.
    pub project_error: bool,
    /// Whether requested focus/target selection was unavailable or ambiguous.
    pub selection_error: bool,
    /// Whether the batch engine faulted before independent fallback.
    pub engine_fault: bool,
    /// All observed target reports, including failed and duplicate observations.
    pub reports: Vec<TargetReport>,
}

/// A function has all facts required for confirmed verified acceptance.
pub open spec fn good_function(f: &Function) -> bool {
    f.verified && f.confirmed && f.evidence_tier_present
}

/// A report is consistent, complete and confirmed successful.
pub open spec fn good_report(r: &TargetReport) -> bool {
    r.complete && r.normal_exit_zero && !r.report_error
        && r.verifier_documents > 0 && r.verifier_success
        && r.verifier_errors == Some(0) && !r.vir_error
        && !r.engine_fallback && r.invariant_violations == 0
        && r.summary_functions == r.functions.len()
        && r.summary_verified == r.functions.len()
        && r.summary_failed == 0 && r.summary_unknown == 0
        && r.summary_provisional == 0
        && forall|i: int| #![auto] 0 <= i < r.functions.len() ==> good_function(&r.functions[i])
}

/// Distinct requested positions cannot denote the same target.
pub open spec fn distinct_requests(r: &Request) -> bool {
    forall|i: int, j: int| 0 <= i < j < r.targets.len()
        ==> !same_target(&r.targets[i], &r.targets[j])
}

/// Ambiguous duplicate target observations are refused.
pub open spec fn distinct_reports(c: &Candidate) -> bool {
    forall|i: int, j: int| 0 <= i < j < c.reports.len()
        ==> !same_target(&c.reports[i].target, &c.reports[j].target)
}

/// One requested target has exact matching completed evidence.
pub open spec fn covered_at(r: &Request, c: &Candidate, i: int) -> bool {
    exists|j: int| #![auto] 0 <= j < c.reports.len()
        && same_target(&r.targets[i], &c.reports[j].target)
        && same_scope(&r.scope, &c.reports[j].scope)
        && good_report(&c.reports[j])
}

/// Every requested target is accounted for.
pub open spec fn covers(r: &Request, c: &Candidate) -> bool {
    forall|i: int| 0 <= i < r.targets.len() ==> #[trigger] covered_at(r, c, i)
}

/// Independent logical acceptance relation for current exact scoped success.
pub open spec fn safe_scoped(r: &Request, c: &Candidate) -> bool {
    c.current && c.normal_exit_zero && !c.project_error
        && !c.selection_error && !c.engine_fault
        && r.targets.len() > 0
        && (r.scope.whole == (r.scope.selection.len() == 0))
        && distinct_requests(r) && distinct_reports(c) && covers(r, c)
}

/// Executable byte equality with its complete sequence-equality contract.
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

/// Executable exact target identity comparison.
pub fn target_equal(a: &TargetId, b: &TargetId) -> (equal: bool)
    ensures equal == same_target(a, b),
{
    bytes_equal(&a.package, &b.package) && bytes_equal(&a.cargo_name, &b.cargo_name)
        && bytes_equal(&a.kind, &b.kind) && bytes_equal(&a.source, &b.source)
        && bytes_equal(&a.invocation, &b.invocation)
}

/// Executable complete scope comparison.
pub fn scope_equal(a: &Scope, b: &Scope) -> (equal: bool)
    ensures equal == same_scope(a, b),
{
    a.whole == b.whole && bytes_equal(&a.selection, &b.selection)
}

/// Validate every report fact and each confirmed function, including summaries.
pub fn report_valid(r: &TargetReport) -> (valid: bool)
    ensures valid == good_report(r),
{
    if !r.complete || !r.normal_exit_zero || r.report_error
        || r.verifier_documents == 0 || !r.verifier_success
        || r.verifier_errors != Some(0) || r.vir_error || r.engine_fallback
        || r.invariant_violations != 0
        || r.summary_functions != r.functions.len()
        || r.summary_verified != r.functions.len()
        || r.summary_failed != 0 || r.summary_unknown != 0
        || r.summary_provisional != 0 { return false; }
    let mut i = 0;
    while i < r.functions.len()
        invariant i <= r.functions.len(),
            forall|j: int| #![auto] 0 <= j < i ==> good_function(&r.functions[j]),
        decreases r.functions.len() - i,
    {
        let f = &r.functions[i];
        if !f.verified || !f.confirmed || !f.evidence_tier_present { return false; }
        i += 1;
    }
    true
}

/// Refuse duplicate requested target identities.
pub fn requests_distinct(r: &Request) -> (distinct: bool)
    ensures distinct == distinct_requests(r),
{
    let mut i = 0;
    while i < r.targets.len()
        invariant i <= r.targets.len(),
            forall|a: int, b: int| 0 <= a < i && a < b < r.targets.len()
                ==> !same_target(&r.targets[a], &r.targets[b]),
        decreases r.targets.len() - i,
    {
        let mut j = i + 1;
        while j < r.targets.len()
            invariant i < r.targets.len(), i < j <= r.targets.len(),
                forall|b: int| #![auto] i < b < j ==> !same_target(&r.targets[i as int], &r.targets[b]),
            decreases r.targets.len() - j,
        {
            if target_equal(&r.targets[i], &r.targets[j]) { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Refuse ambiguous duplicate observed target identities.
pub fn reports_distinct(c: &Candidate) -> (distinct: bool)
    ensures distinct == distinct_reports(c),
{
    let mut i = 0;
    while i < c.reports.len()
        invariant i <= c.reports.len(),
            forall|a: int, b: int| 0 <= a < i && a < b < c.reports.len()
                ==> !same_target(&c.reports[a].target, &c.reports[b].target),
        decreases c.reports.len() - i,
    {
        let mut j = i + 1;
        while j < c.reports.len()
            invariant i < c.reports.len(), i < j <= c.reports.len(),
                forall|b: int| #![auto] i < b < j
                    ==> !same_target(&c.reports[i as int].target, &c.reports[b].target),
            decreases c.reports.len() - j,
        {
            if target_equal(&c.reports[i].target, &c.reports[j].target) { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Search exact matching successful evidence for one request position.
pub fn target_covered(r: &Request, c: &Candidate, index: usize) -> (covered: bool)
    requires index < r.targets.len(),
    ensures covered == covered_at(r, c, index as int),
{
    let mut j = 0;
    while j < c.reports.len()
        invariant index < r.targets.len(), j <= c.reports.len(),
            forall|k: int| #![auto] 0 <= k < j ==> !(same_target(&r.targets[index as int], &c.reports[k].target)
                && same_scope(&r.scope, &c.reports[k].scope) && good_report(&c.reports[k])),
        decreases c.reports.len() - j,
    {
        let report = &c.reports[j];
        if target_equal(&r.targets[index], &report.target)
            && scope_equal(&r.scope, &report.scope) && report_valid(report) {
            return true;
        }
        j += 1;
    }
    false
}

/// No deserialization or public constructor exists for this authority token.
/// Borrowing the request/candidate keeps the checked model snapshot immutable.
pub struct ValidatedResult<'a> {
    request: &'a Request,
    candidate: &'a Candidate,
}

impl<'a> ValidatedResult<'a> {
    /// Authority means this exact borrowed snapshot satisfies acceptance.
    pub closed spec fn valid(&self) -> bool {
        safe_scoped(self.request, self.candidate)
    }

    /// Whether the immutable request establishes whole-target authority.
    pub closed spec fn whole(&self) -> bool {
        self.request.scope.whole
    }
}

/// The sole scoped authority factory; validates all candidate observations.
pub fn validate_scoped<'a>(r: &'a Request, c: &'a Candidate) -> (out: Option<ValidatedResult<'a>>)
    ensures out.is_some() == safe_scoped(r, c),
        out.is_some() ==> out.unwrap().valid(),
        out.is_some() ==> out.unwrap().whole() == r.scope.whole,
{
    if !c.current || !c.normal_exit_zero || c.project_error || c.selection_error || c.engine_fault
        || r.targets.len() == 0
        || r.scope.whole != (r.scope.selection.len() == 0)
        || !requests_distinct(r) || !reports_distinct(c) { return None; }
    let mut i = 0;
    while i < r.targets.len()
        invariant i <= r.targets.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] covered_at(r, c, j),
        decreases r.targets.len() - i,
    {
        if !target_covered(r, c, i) { return None; }
        i += 1;
    }
    Some(ValidatedResult { request: r, candidate: c })
}

/// Whole-target authority additionally refuses every partial request.
pub fn validate_current_complete<'a>(r: &'a Request, c: &'a Candidate) -> (out: Option<ValidatedResult<'a>>)
    ensures out.is_some() == (r.scope.whole && safe_scoped(r, c)),
        out.is_some() ==> out.unwrap().valid() && out.unwrap().whole(),
{
    if !r.scope.whole { return None; }
    validate_scoped(r, c)
}

/// One report cannot cover two distinct requested targets.
pub proof fn injective_coverage(r: &Request, c: &Candidate, i: int, j: int, k: int)
    requires safe_scoped(r, c), 0 <= i < r.targets.len(), 0 <= j < r.targets.len(),
        0 <= k < c.reports.len(),
        same_target(&r.targets[i], &c.reports[k].target),
        same_target(&r.targets[j], &c.reports[k].target),
    ensures i == j,
{
    if i < j { assert(!same_target(&r.targets[i], &r.targets[j])); }
    if j < i { assert(!same_target(&r.targets[j], &r.targets[i])); }
}

fn example_bytes(value: u8) -> (out: Vec<u8>)
    ensures out@ == seq![value],
{
    let mut out = Vec::new();
    out.push(value);
    out
}

fn example_target() -> (out: TargetId)
    ensures out.package@ == seq![1u8], out.cargo_name@ == seq![2u8],
        out.kind@ == seq![3u8], out.source@ == seq![4u8], out.invocation@ == seq![5u8],
{
    TargetId { package: example_bytes(1), cargo_name: example_bytes(2),
        kind: example_bytes(3), source: example_bytes(4), invocation: example_bytes(5) }
}

fn example_scope(whole: bool) -> (out: Scope)
    ensures out.whole == whole,
        out.selection@ == (if whole { Seq::<u8>::empty() } else { seq![9u8] }),
{
    Scope { whole, selection: if whole { Vec::new() } else { example_bytes(9) } }
}

/// Construct a concrete nonempty successful run, without assuming its existence.
fn example(whole: bool) -> (pair: (Request, Candidate))
    ensures safe_scoped(&pair.0, &pair.1), pair.0.scope.whole == whole,
        pair.0.targets.len() == 1, pair.1.reports.len() == 1,
        pair.1.reports[0].functions.len() == 1,
{
    let mut functions = Vec::new();
    functions.push(Function { verified: true, confirmed: true, evidence_tier_present: true });
    let report = TargetReport {
        target: example_target(), scope: example_scope(whole), complete: true,
        normal_exit_zero: true, report_error: false, verifier_documents: 1,
        verifier_success: true, verifier_errors: Some(0), vir_error: false,
        engine_fallback: false, invariant_violations: 0,
        summary_functions: 1, summary_verified: 1, summary_failed: 0,
        summary_unknown: 0, summary_provisional: 0, functions,
    };
    let mut reports = Vec::new();
    reports.push(report);
    let c = Candidate { current: true, normal_exit_zero: true, project_error: false,
        selection_error: false, engine_fault: false, reports };
    let mut targets = Vec::new();
    targets.push(example_target());
    let r = Request { targets, scope: example_scope(whole) };
    assert(good_report(&c.reports[0]));
    assert(same_target(&r.targets[0], &c.reports[0].target));
    assert(same_scope(&r.scope, &c.reports[0].scope));
    assert(covered_at(&r, &c, 0));
    assert forall|i: int| 0 <= i < r.targets.len() implies #[trigger] covered_at(&r, &c, i) by {
        assert(i == 0);
    }
    (r, c)
}

/// A nonempty whole-target request really obtains authority.
pub fn positive_control() {
    let (r, c) = example(true);
    let accepted = validate_current_complete(&r, &c);
    assert(accepted.is_some());
}

/// The same successful evidence can establish its partial scope, never a whole proof.
pub fn partial_scope_control() {
    let (r, c) = example(false);
    let scoped = validate_scoped(&r, &c);
    assert(scoped.is_some());
    let complete = validate_current_complete(&r, &c);
    assert(complete.is_none());
}

/// Missing requested evidence cannot become success through an empty all() check.
pub fn missing_target_control() {
    let (r, mut c) = example(true);
    c.reports = Vec::new();
    assert(!covered_at(&r, &c, 0));
    assert(!covers(&r, &c));
    let accepted = validate_current_complete(&r, &c);
    assert(accepted.is_none());
}

/// A stale successful report cannot authorize Current.
pub fn historical_control() {
    let (r, mut c) = example(true);
    c.current = false;
    let accepted = validate_current_complete(&r, &c);
    assert(accepted.is_none());
}

/// A successful summary cannot hide an unconfirmed function.
pub fn unconfirmed_control() {
    let (r, mut c) = example(true);
    c.reports[0].functions[0].confirmed = false;
    assert(!good_report(&c.reports[0]));
    assert(!covered_at(&r, &c, 0));
    assert(!covers(&r, &c));
    let accepted = validate_current_complete(&r, &c);
    assert(accepted.is_none());
}

/// Summary fields must agree with the complete successful function collection.
pub fn inconsistent_summary_control() {
    let (r, mut c) = example(true);
    c.reports[0].summary_verified = 0;
    assert(!good_report(&c.reports[0]));
    assert(!covered_at(&r, &c, 0));
    assert(!covers(&r, &c));
    let accepted = validate_current_complete(&r, &c);
    assert(accepted.is_none());
}

} // verus!

fn main() {}
