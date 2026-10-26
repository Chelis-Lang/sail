//! Constructive refinement of actual typed report records into the authority model.
//! The source-shared factory proves the concrete premises; no external report-valid
//! specification supplies them. Process/input/session observation remains separate.
use vstd::prelude::*;
use crate::authority_model as model;
use crate::acceptance::{self, confirmed, successful_report, string_bytes};
use crate::output::{EvidenceTierName, EstablishmentName, FunctionReport, Report, VerdictName};
use crate::authority::CompletionPolicy;

verus! {

/// A decoded effective configuration refines the model setting only after all
/// tokens have been checked against the independent raw common/package values.
pub fn project_effective_verification(common: &Option<String>, package: &Option<String>,
    boundary: usize, arguments: &[String]) -> (out: Option<model::VerificationSetting>)
    ensures out.is_some() ==> acceptance::driver_option_sources(common, package, arguments@, boundary as int),
        out.is_some() ==> ((out.unwrap() == model::VerificationSetting::Enabled)
            == !acceptance::options_disable_verification(arguments@)),
{
    if !acceptance::effective_driver_arguments_match(common, package, boundary, arguments) { return None; }
    Some(if acceptance::verification_enabled_options(arguments) {
        model::VerificationSetting::Enabled
    } else { model::VerificationSetting::CompileOnly })
}

pub fn packed_empty_token_positive(value: &str, tokens: &[String]) -> (accepted: bool)
    requires vstd::utf8::encode_utf8(value@) == b"__VERUS_DRIVER_ARGS_SEP__"@,
        tokens@.len() == 1, string_bytes(&tokens@[0]).len() == 0,
    ensures accepted,
{
    acceptance::packed_driver_arguments_match(value, tokens)
}

pub open spec fn model_verdict(v: VerdictName) -> model::Verdict {
    match v {
        VerdictName::Verified => model::Verdict::Verified,
        VerdictName::Failed => model::Verdict::Failed,
        VerdictName::Unknown => model::Verdict::Unknown,
        VerdictName::Provisional => model::Verdict::Provisional,
    }
}

/// The request factory supplies the same concrete function-to-model relation,
/// now with an independently retained target inventory and invocation identity.
pub fn checked_request_refines_function(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder],
    target_index: usize, crate_index: usize, function_index: usize) -> (out: Option<model::Function>)
    requires target_index < project.targets@.len(), project.targets@[target_index as int].requested,
        crate_index < project.targets@[target_index as int].report.crates@.len(),
        function_index < project.targets@[target_index as int].report.crates@[crate_index as int].functions@.len(),
    ensures out.is_some() ==> acceptance::request_authorized(request, project),
        out.is_some() ==> model::good_function(&out.unwrap()),
        out.is_some() ==> function_refines(
            &project.targets@[target_index as int].report.crates@[crate_index as int].functions@[function_index as int], &out.unwrap()),
{
    if acceptance::complete_request_ordered(request, project, orders).is_none() { return None; }
    let actual = &project.targets[target_index].report.crates[crate_index].functions[function_index];
    assert(confirmed(actual));
    Some(project_function(actual))
}

/// Original-mode and argument correspondence remains attached to the actual
/// request/project records when projecting a confirmed function into the model.
pub fn checked_submitted_request_refines_function(arguments: &[String], mode: &crate::jobs::Mode,
    request: &crate::project::ResolvedRequest, project: &crate::project::ProjectResult,
    orders: &[acceptance::ReportOrder], target_index: usize, crate_index: usize, function_index: usize)
    -> (out: Option<model::Function>)
    requires target_index < project.targets@.len(), project.targets@[target_index as int].requested,
        crate_index < project.targets@[target_index as int].report.crates@.len(),
        function_index < project.targets@[target_index as int].report.crates@[crate_index as int].functions@.len(),
    ensures out.is_some() ==> model::good_function(&out.unwrap()),
        out.is_some() ==> acceptance::request_authorized(request, project),
        out.is_some() ==> exists |origin: acceptance::SubmissionKind|
            acceptance::submission_correspondence(arguments@, mode, request, origin),
        out.is_some() ==> function_refines(
            &project.targets@[target_index as int].report.crates@[crate_index as int].functions@[function_index as int], &out.unwrap()),
{
    let Some(authority) = acceptance::complete_submitted_request_ordered(arguments, mode, request, project, orders) else { return None; };
    assert(acceptance::submission_correspondence(arguments@, mode, request, acceptance::submitted_kind(&authority)));
    let actual = &project.targets[target_index].report.crates[crate_index].functions[function_index];
    assert(confirmed(actual));
    Some(project_function(actual))
}

pub fn changed_original_arguments_refused(arguments: &[String], mode: &crate::jobs::Mode,
    request: &crate::project::ResolvedRequest, project: &crate::project::ProjectResult,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires request.args@.map_values(|s: String| string_bytes(&s)) != acceptance::normalized_arguments(arguments@, arguments@.len()),
    ensures refused,
{
    acceptance::complete_submitted_request_ordered(arguments, mode, request, project, orders).is_none()
}

pub fn changed_original_mode_refused(arguments: &[String],
    request: &crate::project::ResolvedRequest, project: &crate::project::ProjectResult,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires request.engine.is_some() || request.function.is_some(),
    ensures refused,
{
    let mode = crate::jobs::Mode::Stock;
    acceptance::complete_submitted_request_ordered(arguments, &mode, request, project, orders).is_none()
}

pub fn changed_repair_selection_refused(mode: &crate::jobs::Mode,
    request: &crate::project::ResolvedRequest) -> (refused: bool)
    requires request.function.is_some(), match mode {
        crate::jobs::Mode::Repair(selection) => !acceptance::repair_selection_matches(request,
            string_bytes(&selection.package), &selection.target, string_bytes(&selection.kind), string_bytes(&selection.function)),
        _ => false,
    },
    ensures refused,
{
    acceptance::classify_submission_mode(mode, request).is_none()
}

pub fn positive_nonempty_submitted_factory(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (accepted: bool)
    requires acceptance::small_positive_request(request, project), request.args@.len() == 0,
        request.engine.is_none(), request.function.is_none(),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
    ensures accepted,
{
    let arguments: Vec<String> = Vec::new();
    let mode = crate::jobs::Mode::Stock;
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    acceptance::complete_submitted_request_ordered(&arguments, &mode, request, project, &orders).is_some()
}

pub fn swapped_request_refused(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires string_bytes(&request.context_sha256) != string_bytes(&project.context_sha256)
        || string_bytes(&request.input_revision) != string_bytes(&project.input_revision)
        || string_bytes(&request.run_id) != string_bytes(&project.run_id),
    ensures refused,
{
    acceptance::complete_request_ordered(request, project, orders).is_none()
}

pub fn stale_request_refused(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires !(project.currentness is Current),
    ensures refused,
{
    acceptance::complete_request_ordered(request, project, orders).is_none()
}

pub fn compile_only_original_request_refused(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires acceptance::verification_disabled(request.args@),
    ensures refused,
{
    acceptance::complete_request_ordered(request, project, orders).is_none()
}

/// A nonempty coherent concrete request reaches the same checked factory;
/// rejecting every candidate would fail this constructive positive control.
pub fn positive_nonempty_request_factory(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (accepted: bool)
    requires acceptance::small_positive_request(request, project),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
    ensures accepted,
{
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    let authority = acceptance::complete_request_ordered(request, project, &orders);
    assert(authority.is_some());
    authority.is_some()
}

pub fn missing_requested_scope_refused(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires !acceptance::resolved_coverage(request, project),
    ensures refused,
{
    acceptance::complete_request_ordered(request, project, orders).is_none()
}

/// Raw selection bytes are compared to the original selection, rather than
/// confusing a package selector with the canonical resolved Cargo package ID.
pub fn swapped_repair_function_refused(request: &crate::project::ResolvedRequest,
    package: &str, target: &Option<String>, kind: &str, function: &str) -> (refused: bool)
    requires request.function.is_none()
        || string_bytes(&request.function.unwrap()) != vstd::utf8::encode_utf8(function@),
    ensures refused,
{
    !acceptance::repair_request_matches(request, package, target, kind, function)
}

pub fn matching_repair_selection_accepts(request: &crate::project::ResolvedRequest,
    package: &str, target: &Option<String>, kind: &str, function: &str) -> (accepted: bool)
    requires request.function.is_some(),
        string_bytes(&request.function.unwrap()) == vstd::utf8::encode_utf8(function@),
        string_bytes(&request.repair_package) == vstd::utf8::encode_utf8(package@),
        string_bytes(&request.repair_kind) == vstd::utf8::encode_utf8(kind@),
        match (&request.repair_target, target) {
            (None, None) => true,
            (Some(a), Some(b)) => string_bytes(a) == string_bytes(b),
            _ => false,
        },
    ensures accepted,
{
    acceptance::repair_request_matches(request, package, target, kind, function)
}

pub fn missing_repair_function_refused(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (refused: bool)
    requires !acceptance::repair_function_present(request, project),
    ensures refused,
{
    !acceptance::requested_function_present(request, project)
}

pub proof fn checked_repair_presence_has_confirmed_evidence(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult)
    requires acceptance::request_authorized(request, project),
        acceptance::repair_function_present(request, project),
    ensures exists |t: int, c: int, f: int| acceptance::repair_function_at(request, project, t, c, f)
        && confirmed(&project.targets@[t].report.crates@[c].functions@[f]),
{
    let (t, c, f) = choose |t: int, c: int, f: int| acceptance::repair_function_at(request, project, t, c, f);
    assert(confirmed(&project.targets@[t].report.crates@[c].functions@[f]));
}

pub open spec fn model_establishment(e: EstablishmentName) -> model::Establishment {
    match e {
        EstablishmentName::Confirmed => model::Establishment::Confirmed,
        EstablishmentName::Provisional => model::Establishment::Provisional,
    }
}

pub open spec fn model_tier(t: Option<EvidenceTierName>) -> Option<model::EvidenceTier> {
    match t {
        None => None,
        Some(EvidenceTierName::Sampled) => Some(model::EvidenceTier::ObservedStrength(0)),
        Some(EvidenceTierName::Bounded) => Some(model::EvidenceTier::ObservedStrength(1)),
        Some(EvidenceTierName::SmtVerified) => Some(model::EvidenceTier::ObservedStrength(2)),
        Some(EvidenceTierName::CertificateChecked) => Some(model::EvidenceTier::ObservedStrength(3)),
        Some(EvidenceTierName::KernelChecked) => Some(model::EvidenceTier::ObservedStrength(4)),
    }
}

pub open spec fn function_refines(actual: &FunctionReport, abstracted: &model::Function) -> bool {
    abstracted.name@ == string_bytes(&actual.function)
        && abstracted.verdict == model_verdict(actual.verdict)
        && abstracted.establishment == model_establishment(actual.establishment)
        && abstracted.evidence_tier == model_tier(actual.evidence_tier)
}

/// Execute the field mapping, including every verdict and actual evidence tier.
pub fn project_function(actual: &FunctionReport) -> (out: model::Function)
    ensures function_refines(actual, &out),
        confirmed(actual) ==> model::good_function(&out),
{
    let name = crate::authority::copy_bytes(actual.function.as_str().as_bytes());
    let verdict = match actual.verdict {
        VerdictName::Verified => model::Verdict::Verified,
        VerdictName::Failed => model::Verdict::Failed,
        VerdictName::Unknown => model::Verdict::Unknown,
        VerdictName::Provisional => model::Verdict::Provisional,
    };
    let establishment = match actual.establishment {
        EstablishmentName::Confirmed => model::Establishment::Confirmed,
        EstablishmentName::Provisional => model::Establishment::Provisional,
    };
    let evidence_tier = match actual.evidence_tier {
        None => None,
        Some(EvidenceTierName::Sampled) => Some(model::EvidenceTier::ObservedStrength(0)),
        Some(EvidenceTierName::Bounded) => Some(model::EvidenceTier::ObservedStrength(1)),
        Some(EvidenceTierName::SmtVerified) => Some(model::EvidenceTier::ObservedStrength(2)),
        Some(EvidenceTierName::CertificateChecked) => Some(model::EvidenceTier::ObservedStrength(3)),
        Some(EvidenceTierName::KernelChecked) => Some(model::EvidenceTier::ObservedStrength(4)),
    };
    model::Function { name, verdict, establishment, evidence_tier }
}

/// The actual checked factory supplies confirmation for any selected actual record.
pub fn checked_factory_refines_function(report: &Report, policy: CompletionPolicy,
    forward: &[usize], inverse: &[usize], crate_index: usize, function_index: usize)
    -> (out: Option<model::Function>)
    requires crate_index < report.crates@.len(),
        function_index < report.crates@[crate_index as int].functions@.len(),
    ensures out.is_some() ==> model::good_function(&out.unwrap()),
        out.is_some() ==> function_refines(
            &report.crates@[crate_index as int].functions@[function_index as int], &out.unwrap()),
        out.is_some() ==> successful_report(report, policy),
        out.is_some() ==> !acceptance::verification_disabled(report.command@),
{
    let authority = acceptance::complete_report_ordered(report, policy, forward, inverse);
    if authority.is_none() { return None; }
    let actual = &report.crates[crate_index].functions[function_index];
    assert(confirmed(actual));
    Some(project_function(actual))
}

/// A real nonempty function inventory can pass the concrete factory.
/// No SMT-obligation inventory is required when the verifier legitimately used none.
pub fn positive_nonempty_factory(report: &Report) -> (accepted: bool)
    requires acceptance::small_positive_report(report, CompletionPolicy::Receipt),
        report.crates@[0].functions@.len() == 1,
    ensures accepted,
{
    let forward: Vec<usize> = Vec::new();
    let inverse: Vec<usize> = Vec::new();
    let authority = acceptance::complete_report_ordered(report, CompletionPolicy::Receipt, &forward, &inverse);
    assert(authority.is_some());
    let function = project_function(&report.crates[0].functions[0]);
    assert(model::good_function(&function));
    authority.is_some()
}

/// Legitimate zero-function crate summaries have their own constructive control.
pub fn positive_zero_function_factory(report: &Report) -> (accepted: bool)
    requires acceptance::small_positive_report(report, CompletionPolicy::Receipt),
        report.crates@[0].functions@.len() == 0,
    ensures accepted,
{
    let forward: Vec<usize> = Vec::new();
    let inverse: Vec<usize> = Vec::new();
    acceptance::complete_report_ordered(report, CompletionPolicy::Receipt, &forward, &inverse).is_some()
}

/// A selected whole-target claim cannot be minted from a focused actual command.
pub fn focused_report_refused(report: &Report, forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires acceptance::partial_command(report.command@),
    ensures refused,
{
    acceptance::complete_whole_report_ordered(report, forward, inverse).is_none()
}

/// An explicit observed process interruption cannot become report authority.
pub fn interrupted_report_refused(report: &Report, forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires report.exit_code.is_none(),
    ensures refused,
{
    acceptance::complete_report_ordered(report, CompletionPolicy::Receipt, forward, inverse).is_none()
}

/// Compile-only success is refused for every proof-authority policy.
pub fn compile_only_report_refused(report: &Report, policy: CompletionPolicy,
    forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires acceptance::verification_disabled(report.command@),
    ensures refused,
{
    acceptance::complete_report_ordered(report, policy, forward, inverse).is_none()
}

}
