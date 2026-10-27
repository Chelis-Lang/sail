//! Constructive refinement of actual typed report records into the authority model.
//! The source-shared factory proves the concrete premises; no external report-valid
//! specification supplies them. Process/input/session observation remains separate.
use vstd::prelude::*;
use vstd::string::StringSliceAdditionalSpecFns;
use crate::authority_model as model;
use crate::acceptance::{self, confirmed, successful_report, string_bytes};
use crate::output::{EvidenceTierName, EstablishmentName, FunctionReport, Report, VerdictName};
use crate::authority::CompletionPolicy;

verus! {

/// An actual nonempty Cargo-side workspace selector is not always refused.
pub fn original_workspace_is_selected(arguments: &[String]) -> (selected: bool)
    requires arguments@.len() == 1, string_bytes(&arguments@[0]) == b"--workspace"@,
    ensures selected,
{
    acceptance::cargo_workspace_selected(arguments)
}

/// The same selector bytes after the real separator remain Verus arguments.
pub fn forwarded_workspace_is_not_selected(arguments: &[String]) -> (selected: bool)
    requires arguments@.len() == 2, string_bytes(&arguments@[0]) == b"--"@,
        string_bytes(&arguments@[1]) == b"--workspace"@,
    ensures !selected,
{
    proof { acceptance::initial_separator_excludes_workspace(arguments@); }
    acceptance::cargo_workspace_selected(arguments)
}

/// The legacy all-workspace spelling obeys the same original Cargo boundary.
pub fn forwarded_all_is_not_selected(arguments: &[String]) -> (selected: bool)
    requires arguments@.len() == 2, string_bytes(&arguments@[0]) == b"--"@,
        string_bytes(&arguments@[1]) == b"--all"@,
    ensures !selected,
{
    proof { acceptance::initial_separator_excludes_workspace(arguments@); }
    acceptance::cargo_workspace_selected(arguments)
}

/// The actual pre-execution constructor retains a nonempty original target
/// inventory and arguments. Native set/metadata completeness stays separate.
pub fn constructed_original_request(source: acceptance::RequestConstruction<'_>)
    -> (request: crate::project::ResolvedRequest)
    requires source.targets@.len() > 0, source.arguments@.len() > 0,
    ensures acceptance::request_construction_matches(&source,&request),
        request.targets@.len() == source.targets@.len(),
        request.args@.len() == source.arguments@.len(),
{
    acceptance::construct_resolved_request(source)
}

/// One genuine original dictionary entry yields one request row, using actual
/// insertion/iteration and the actual header constructor, without supplied rows.
pub fn constructed_original_dictionary_request(header: crate::request_targets::OriginalRequestHeader<'_>,
    original: (String, String, String)) -> (request: Option<crate::project::ResolvedRequest>)
    ensures request.is_some(), request.is_some() ==>
        crate::request_targets::original_header_matches(&header, &request.unwrap())
            && request.unwrap().targets@.len() == 1,
{
    let mut source = std::collections::BTreeSet::new();
    crate::request_targets::insert_original_target(&mut source, original);
    crate::request_targets::construct_original_request(header, &source)
}

/// Actual dictionary projection cannot turn original compile-only into a proof.
pub fn original_dictionary_compile_only_refuses(header: crate::request_targets::OriginalRequestHeader<'_>,
    source: &crate::request_targets::OriginalTargets, project: &crate::project::ProjectResult,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires header.arguments@.len() == 2,
        string_bytes(&header.arguments@[0]) == b"--"@,
        string_bytes(&header.arguments@[1]) == b"--no-verify"@,
    ensures refused,
{
    let Some(request) = crate::request_targets::construct_original_request(header, source)
        else { return false; };
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--no-verify");
        assert(string_bytes(&request.args@[0]) == b"--"@);
        assert(string_bytes(&request.args@[1]) == b"--no-verify"@);
        assert(acceptance::verification_disabled(request.args@));
    }
    acceptance::complete_request_ordered(&request, project, orders).is_none()
}

/// Actual dictionary projection retains the original partial selector too.
pub fn original_dictionary_partial_refuses(header: crate::request_targets::OriginalRequestHeader<'_>,
    source: &crate::request_targets::OriginalTargets, project: &crate::project::ProjectResult,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires header.arguments@.len() == 2,
        string_bytes(&header.arguments@[0]) == b"--"@,
        string_bytes(&header.arguments@[1]) == b"--verify-module=selected"@,
    ensures refused,
{
    let Some(request) = crate::request_targets::construct_original_request(header, source)
        else { return false; };
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--verify-module=selected");
        reveal_byteslit(b"--verify-module");
        acceptance::request_literal_lengths();
        assert(string_bytes(&request.args@[0]) == b"--"@);
        assert(string_bytes(&request.args@[1]) == b"--verify-module=selected"@);
        assert(b"--verify-module=selected"@.take(b"--verify-module"@.len() as int)
            =~= b"--verify-module"@);
        assert(sail_acceptance::authority::flag_matches(b"--verify-module=selected"@,
            b"--verify-module"@));
        assert(acceptance::focused_argument(string_bytes(&request.args@[1])));
        assert(acceptance::partial_command(request.args@));
    }
    acceptance::complete_request_ordered(&request, project, orders).is_none()
}

/// Every field of a real pre-execution target triple survives the constructor.
pub fn constructed_original_target(source: &(String,String,String))
    -> (target: crate::project::RequestTarget)
    requires string_bytes(&source.0).len() > 0,
        string_bytes(&source.1).len() > 0, string_bytes(&source.2).len() > 0,
    ensures target.package_id == source.0, target.target == source.1, target.kind == source.2,
{
    acceptance::construct_request_target(source)
}

/// Copying original compile-only arguments never constructs whole authority,
/// even if the supplied project later presents successful selected rows.
pub fn constructed_compile_only_refuses(source: acceptance::RequestConstruction<'_>,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires source.arguments@.len() == 2,
        string_bytes(&source.arguments@[0]) == b"--"@,
        string_bytes(&source.arguments@[1]) == b"--no-verify"@,
    ensures refused,
{
    let request = acceptance::construct_resolved_request(source);
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--no-verify");
        assert(string_bytes(&request.args@[0]) == b"--"@);
        assert(string_bytes(&request.args@[1]) == b"--no-verify"@);
        assert(acceptance::verification_disabled(request.args@));
    }
    acceptance::complete_request_ordered(&request,project,orders).is_none()
}

/// Original partial selectors cannot be erased by request construction and
/// thereby acquire a whole-request witness from later successful rows.
pub fn constructed_partial_refuses(source: acceptance::RequestConstruction<'_>,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires source.arguments@.len() == 2,
        string_bytes(&source.arguments@[0]) == b"--"@,
        string_bytes(&source.arguments@[1]) == b"--verify-module=selected"@,
    ensures refused,
{
    let request = acceptance::construct_resolved_request(source);
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--verify-module=selected");
        reveal_byteslit(b"--verify-module");
        acceptance::request_literal_lengths();
        assert(string_bytes(&request.args@[0]) == b"--"@);
        assert(string_bytes(&request.args@[1]) == b"--verify-module=selected"@);
        assert(b"--verify-module=selected"@.take(b"--verify-module"@.len() as int)
            =~= b"--verify-module"@);
        assert(sail_acceptance::authority::flag_matches(b"--verify-module=selected"@,
            b"--verify-module"@));
        assert(acceptance::focused_argument(string_bytes(&request.args@[1])));
        assert(acceptance::partial_command(request.args@));
    }
    acceptance::complete_request_ordered(&request,project,orders).is_none()
}

/// A genuine nonempty submitted argument survives the actual constructor.
pub fn normalized_original_submission(arguments: &[String]) -> (actual: Vec<String>)
    requires arguments@.len() == 1,
        string_bytes(&arguments@[0]) != b"--incremental"@,
    ensures actual@.len() == 1,
        string_bytes(&actual@[0]) == string_bytes(&arguments@[0]),
{
    let actual = acceptance::normalize_submitted_arguments(arguments);
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--incremental");
        acceptance::request_literal_lengths();
        reveal_with_fuel(acceptance::normalized_arguments, 3);
        acceptance::argument_byte_projection_properties(actual@);
        assert(acceptance::normalized_arguments(arguments@, arguments@.len())
            == seq![string_bytes(&arguments@[0])]);
    }
    actual
}

/// Removal is exactly prefix-scoped: the same bytes after `--` remain present.
pub fn forwarded_incremental_is_retained(arguments: &[String]) -> (actual: Vec<String>)
    requires arguments@.len() == 2,
        string_bytes(&arguments@[0]) == b"--"@,
        string_bytes(&arguments@[1]) == b"--incremental"@,
    ensures actual@.len() == 2,
        string_bytes(&actual@[0]) == b"--"@,
        string_bytes(&actual@[1]) == b"--incremental"@,
{
    let actual = acceptance::normalize_submitted_arguments(arguments);
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--incremental");
        acceptance::request_literal_lengths();
        assert(exists |i: int| 0 <= i < 1 && string_bytes(&arguments@[i]) == b"--"@);
        reveal_with_fuel(acceptance::normalized_arguments, 3);
        acceptance::argument_byte_projection_properties(actual@);
        assert(acceptance::normalized_arguments(arguments@, arguments@.len())
            == seq![b"--"@, b"--incremental"@]);
    }
    actual
}

/// Prefix removal cannot erase a later genuine argument or create a selector.
pub fn prefix_incremental_only_is_removed(arguments: &[String]) -> (actual: Vec<String>)
    requires arguments@.len() == 2,
        string_bytes(&arguments@[0]) == b"--incremental"@,
        string_bytes(&arguments@[1]) != b"--incremental"@,
    ensures actual@.len() == 1,
        string_bytes(&actual@[0]) == string_bytes(&arguments@[1]),
{
    let actual = acceptance::normalize_submitted_arguments(arguments);
    proof {
        reveal_byteslit(b"--");
        reveal_byteslit(b"--incremental");
        acceptance::request_literal_lengths();
        reveal_with_fuel(acceptance::normalized_arguments, 3);
        acceptance::argument_byte_projection_properties(actual@);
        assert(acceptance::normalized_arguments(arguments@, arguments@.len())
            == seq![string_bytes(&arguments@[1])]);
    }
    actual
}

/// A nonempty declared whole request reaches the real optional facet while
/// both returned data borrows remain the exact original pair.
pub fn positive_nonempty_completed_presentation(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (accepted: bool)
    requires acceptance::small_positive_request(request,project),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
    ensures accepted,
{
    let mut orders = Vec::new();
    orders.push(acceptance::ReportOrder {forward:Vec::new(),inverse:Vec::new()});
    let view = acceptance::completed_presentation_ordered(project,Some(request),&orders);
    let actual_project = view.project();
    let actual_request = view.request();
    proof {
        assert(actual_project == project);
        assert(actual_request == Some(request));
    }
    view.whole().is_some()
}

/// Missing original request never discards arbitrary recorded data or supplies
/// a whole-request witness, including nonempty failing/partial observations.
pub fn unbound_completed_presentation(project: &crate::project::ProjectResult)
    -> (view: acceptance::CompletedPresentation<'_>)
    ensures acceptance::completed_presentation_project(&view) == project,
        acceptance::completed_presentation_request(&view).is_none(),
        acceptance::completed_presentation_whole(&view).is_none(),
{
    let orders = Vec::new();
    acceptance::completed_presentation_ordered(project,None,&orders)
}

/// Recorded Historical/Unknown and non-successful data remain unchanged even
/// with a paired request; the Current whole facet cannot relabel those records.
pub fn recorded_completed_presentation<'a>(project: &'a crate::project::ProjectResult,
    request: &'a crate::project::ResolvedRequest, orders: &[acceptance::ReportOrder])
    -> (view: acceptance::CompletedPresentation<'a>)
    requires !(project.currentness is Current) || project.exit_code != Some(0i32) || project.error.is_some(),
    ensures acceptance::completed_presentation_project(&view) == project,
        acceptance::completed_presentation_request(&view) == Some(request),
        acceptance::completed_presentation_whole(&view).is_none(),
{
    acceptance::completed_presentation_ordered(project,Some(request),orders)
}

/// Partial scope keeps its exact original records and request without gaining
/// the separate whole-request facet.
pub fn partial_completed_presentation<'a>(project: &'a crate::project::ProjectResult,
    request: &'a crate::project::ResolvedRequest, orders: &[acceptance::ReportOrder])
    -> (view: acceptance::CompletedPresentation<'a>)
    requires acceptance::partial_command(request.args@),
    ensures acceptance::completed_presentation_project(&view) == project,
        acceptance::completed_presentation_request(&view) == Some(request),
        acceptance::completed_presentation_whole(&view).is_none(),
{
    acceptance::completed_presentation_ordered(project,Some(request),orders)
}

/// Fresh enum values witness both sides of the presentation rule without Copy
/// derives or an assumed positive report/solver observation.
pub fn presentation_cases() -> (passed: bool)
    ensures passed,
{
    matches!(acceptance::presentation_verdict(&VerdictName::Verified,&EstablishmentName::Provisional),VerdictName::Provisional)
        && matches!(acceptance::presentation_verdict(&VerdictName::Verified,&EstablishmentName::Confirmed),VerdictName::Verified)
        && matches!(acceptance::presentation_verdict(&VerdictName::Failed,&EstablishmentName::Confirmed),VerdictName::Failed)
        && matches!(acceptance::presentation_verdict(&VerdictName::Unknown,&EstablishmentName::Confirmed),VerdictName::Unknown)
        && matches!(acceptance::presentation_verdict(&VerdictName::Provisional,&EstablishmentName::Confirmed),VerdictName::Provisional)
        && matches!(acceptance::presentation_verdict(&VerdictName::Failed,&EstablishmentName::Provisional),VerdictName::Provisional)
        && matches!(acceptance::presentation_verdict(&VerdictName::Unknown,&EstablishmentName::Provisional),VerdictName::Provisional)
        && matches!(acceptance::presentation_verdict(&VerdictName::Provisional,&EstablishmentName::Provisional),VerdictName::Provisional)
}

/// Construct the two relevant states in an actual FunctionReport. No other
/// record field or authority premise is required to exercise its wrapper.
pub fn presentation_report_cases(mut report: FunctionReport) -> (passed: bool)
    ensures passed,
{
    report.verdict = VerdictName::Verified;
    report.establishment = EstablishmentName::Provisional;
    if !matches!(acceptance::present_function_verdict(&report),VerdictName::Provisional) { return false; }
    report.establishment = EstablishmentName::Confirmed;
    matches!(acceptance::present_function_verdict(&report),VerdictName::Verified)
}

/// The actual retained guard and readback classifier compose without allowing
/// a Current frame to upgrade a missing or mismatched original request.
pub fn retained_current_frame_refines_request(project: &mut crate::project::ProjectResult,
    request: Option<&crate::project::ResolvedRequest>, orders: &[acceptance::ReportOrder],
    observed: Option<&str>, complete: bool) -> (view: crate::jobs::Currentness)
    requires true,
    ensures acceptance::same_project_except_freshness_and_completion(old(project), final(project)),
        view is Current ==> old(project).currentness is Current,
        view is Current && final(project).exit_code == Some(0i32)
            && (exists |i: int| 0 <= i < final(project).targets@.len()
                && final(project).targets@[i].complete) ==>
            request.is_some() && acceptance::request_authorized(request.unwrap(), final(project)),
        view is Current ==> final(project).currentness is Current,
        view is Current ==> complete && observed.is_some()
            && observed.unwrap().spec_bytes() == acceptance::string_bytes(&old(project).input_revision),
{
    let _corresponding = acceptance::withhold_unbound_request_authority_ordered(project, request, orders);
    let view = acceptance::retained_currentness(&project.currentness, project.input_revision.as_str(), observed, complete);
    let _lost_readback = acceptance::apply_retained_currentness(project, &view);
    view
}

pub open spec fn project_engine_text(value: &Option<Vec<u8>>) -> Option<Seq<u8>> {
    match value { Some(text) => Some(text@), None => None }
}

pub open spec fn project_engine_terminal(t: &crate::authority::EngineTerminalFields)
    -> model::EngineTerminalObservation {
    model::EngineTerminalObservation { version: t.version, event: t.event@, phase: t.phase@,
        normal: t.normal, success: t.success }
}

pub open spec fn project_engine_protocol(p: &Option<crate::authority::EngineProtocolCandidate>)
    -> Option<model::EngineProtocolObservation> {
    match p {
        Some(protocol) => Some(model::EngineProtocolObservation { raw: protocol.raw@,
            versions: protocol.versions@, terminal: match &protocol.terminal {
                Some(t) => Some(project_engine_terminal(t)), None => None,
            } }),
        None => None,
    }
}

pub open spec fn project_engine_document(d: &crate::authority::EngineDocumentFields)
    -> model::EngineDocumentObservation {
    model::EngineDocumentObservation {
        functions: match d.functions { crate::authority::EngineFieldShape::Object => model::EngineFieldShape::Object,
            crate::authority::EngineFieldShape::Other => model::EngineFieldShape::Other },
        version: project_engine_text(&d.version), profile: project_engine_text(&d.profile),
        toolchain: project_engine_text(&d.toolchain), commit: project_engine_text(&d.commit),
        os: project_engine_text(&d.os), arch: project_engine_text(&d.arch),
        error: d.error, vir_error: d.vir_error,
        success: match d.success {
            crate::authority::EngineSuccessField::Missing => model::EngineSuccessField::Missing,
            crate::authority::EngineSuccessField::Boolean(value) => model::EngineSuccessField::Boolean(value),
            crate::authority::EngineSuccessField::Other => model::EngineSuccessField::Other,
        },
        verified: d.verified, errors: d.errors, whole: d.whole,
    }
}

pub open spec fn project_engine_completion(p: &Option<crate::authority::EngineProtocolCandidate>,
    d: &crate::authority::EngineDocumentCandidates, stdout: Seq<u8>) -> model::EngineCompletionObservation {
    model::EngineCompletionObservation { protocol: project_engine_protocol(p), stdout,
        documents: Seq::new(d.documents@.len(), |i: int| project_engine_document(&d.documents@[i])),
        remainder: d.remainder@, missing_feature: None }
}

pub proof fn engine_document_fields_refine(d: &crate::authority::EngineDocumentFields, success: bool)
    ensures crate::authority::engine_document_valid(d, success)
        == model::engine_document_complete(&project_engine_document(d), success),
{
    match d.success {
        crate::authority::EngineSuccessField::Missing => (),
        crate::authority::EngineSuccessField::Boolean(_) => (),
        crate::authority::EngineSuccessField::Other => (),
    }
}

/// A field-preserving projection, including every refusal-relevant type and
/// value. The raw JSON decoder's attribution to these fields remains native.
pub proof fn engine_completion_fields_refine(p: &Option<crate::authority::EngineProtocolCandidate>,
    d: &crate::authority::EngineDocumentCandidates, stdout: Seq<u8>, exit: Option<i32>)
    ensures crate::authority::engine_completion_valid(p, d, stdout, exit)
        == model::engine_protocol_complete(&project_engine_completion(p, d, stdout),
            match exit { Some(code) => model::ProcessEnd::Exit(code), None => model::ProcessEnd::Interrupted }),
{
    let projected = project_engine_completion(p, d, stdout);
    if d.documents@.len() == 1 {
        engine_document_fields_refine(&d.documents@[0], true);
        engine_document_fields_refine(&d.documents@[0], false);
        assert(projected.documents[0] == project_engine_document(&d.documents@[0]));
    }
    match p {
        None => (),
        Some(protocol) => {
            match &protocol.terminal { None => (), Some(terminal) => {
                assert(projected.protocol.unwrap().terminal.unwrap() == project_engine_terminal(terminal));
            } }
        }
    }
}

pub open spec fn project_missing_feature(f: &crate::authority::MissingEngineFeature) -> model::MissingFeatureObservation {
    model::MissingFeatureObservation { feature: f.feature()@,
        available: Seq::new(f.available()@.len(), |i: int| f.available()@[i]@),
        stderr: crate::authority::running_stderr(&crate::authority::observed_running(f.observed())).unwrap()@ }
}

pub proof fn missing_feature_fields_refine(f: &crate::authority::MissingEngineFeature)
    requires f.valid(),
    ensures model::missing_feature_valid(
        crate::authority::running_stdout(&crate::authority::observed_running(f.observed())).unwrap()@,
        &project_missing_feature(f)),
{
    let stdout = crate::authority::running_stdout(&crate::authority::observed_running(f.observed())).unwrap()@;
    let stderr = crate::authority::running_stderr(&crate::authority::observed_running(f.observed())).unwrap()@;
    reveal_byteslit(b"Please cargo build with `--features ");
    let name = f.feature()@;
    let projected = project_missing_feature(f);
    let pattern = crate::authority::engine_feature_pattern(name);
    assert(crate::authority::feature_refusal_fields(stdout, stderr, name, f.available()@));
    assert(name.len() > 0);
    assert forall |i: int| 0 <= i < name.len() implies
        ((48 <= name[i] <= 57) || (65 <= name[i] <= 90) || (97 <= name[i] <= 122)
            || name[i] == 95 || name[i] == 45) by {
        assert(crate::authority::engine_feature_byte(name[i]));
    }

    assert forall |i: int| 0 <= i < projected.available.len()
        implies projected.available[i] != name by {
        assert(projected.available[i] == f.available()@[i]@);
    }
    assert(pattern == b"Please cargo build with `--features "@ + name + seq![96u8]);
    assert(projected.feature == name);
    assert(projected.stderr == stderr);
    if crate::authority::engine_contains_pattern(stdout, pattern) {
        let at = choose |at: int| 0 <= at <= stdout.len() && crate::authority::engine_prefix_at(stdout, pattern, at);
        assert(model::feature_prefix(stdout, pattern, at));
        assert(exists |at: int| 0 <= at <= stdout.len() && model::feature_prefix(stdout, pattern, at));
    } else {
        let at = choose |at: int| 0 <= at <= stderr.len() && crate::authority::engine_prefix_at(stderr, pattern, at);
        assert(model::feature_prefix(stderr, pattern, at));
        assert(exists |at: int| 0 <= at <= stderr.len() && model::feature_prefix(stderr, pattern, at));
    }
    assert(model::missing_feature_valid(stdout, &projected));
}

/// The private concrete token preserves its actual normal exit zero and
/// projects either protocol refusal or a concrete raw missing-feature witness.
/// Attaching it to a target/run still needs the checked native event simulation.
pub proof fn checked_decline_preserves_status_and_refusal(declined: &crate::authority::DeclinedEngine)
    -> (out: model::EngineCompletionObservation)
    requires declined.valid(),
    ensures crate::authority::running_exit(&crate::authority::observed_running(declined.observed())).unwrap() == Some(0i32),
        out.protocol == project_engine_protocol(declined.protocol()),
        out.stdout == crate::authority::running_stdout(&crate::authority::observed_running(declined.observed())).unwrap()@,
        out.documents == project_engine_completion(declined.protocol(), declined.documents(), out.stdout).documents,
        out.remainder == declined.documents().remainder@,
        out.missing_feature == (match declined.missing() { None => None,
            Some(feature) => Some(project_missing_feature(&feature)) }),
        model::engine_observation_refuses(&out, model::ProcessEnd::Exit(0)),
        !model::engine_observation_usable(&out, model::ProcessEnd::Exit(0)),
        out.missing_feature.is_some() == declined.missing().is_some(),
{
    let stdout = crate::authority::running_stdout(&crate::authority::observed_running(declined.observed())).unwrap()@;
    engine_completion_fields_refine(declined.protocol(), declined.documents(), stdout, Some(0i32));
    let base = project_engine_completion(declined.protocol(), declined.documents(), stdout);
    let missing_feature = match declined.missing() {
        None => None,
        Some(feature) => { missing_feature_fields_refine(&feature); Some(project_missing_feature(&feature)) },
    };
    model::EngineCompletionObservation { missing_feature, ..base }
}

/// Reusing a PID cannot merge two different exclusively allocated launch slots.
/// These are supplied process observations, not a claim about an OS launch or
/// a compiler's input domain; the executable factory copies their exact origin.
pub fn owned_attempt_identity_keeps_reused_pid_slots_distinct(run: &[u8], unit: &crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32, exit: Option<i32>)
    -> (out: (crate::authority::AttemptIdentity, crate::authority::AttemptIdentity))
    requires pid != 0,
    ensures out.0.run_view() == run@, out.1.run_view() == run@,
        out.0.slot_view() == 0, out.1.slot_view() == 1,
        out.0.pid_view() == pid, out.1.pid_view() == pid,
        out.0.exit_view() == exit, out.1.exit_view() == exit,
        crate::phase_inputs::same_unit(out.0.unit_view(), *unit),
        crate::phase_inputs::same_unit(out.1.unit_view(), *unit),
        crate::phase_inputs::same_descriptor(out.0.descriptor_view(), descriptor),
        crate::phase_inputs::same_descriptor(out.1.descriptor_view(), descriptor),
{
    let second_descriptor = crate::phase_inputs::copy_descriptor(&descriptor);
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let first = ledger.reserve(descriptor); assert(first.is_some());
    let second = ledger.reserve(second_descriptor); assert(second.is_some());
    let first = first.unwrap().started(pid); assert(first.is_some());
    let second = second.unwrap().started(pid); assert(second.is_some());
    let first = first.unwrap().stdout_eof(Vec::new()).stderr_eof(Vec::new()).exited(exit).finish();
    let second = second.unwrap().stdout_eof(Vec::new()).stderr_eof(Vec::new()).exited(exit).finish();
    assert(first.is_some()); assert(second.is_some());
    (crate::authority::AttemptIdentity::from_observed(&first.unwrap()),
        crate::authority::AttemptIdentity::from_observed(&second.unwrap()))
}

/// The actual low-level reducer preserves the observed exit without promoting
/// it to verifier completion or inventing a document/process relation.
pub fn project_observed_process_end(observed: &crate::authority::ObservedAttempt)
    -> (out: model::ProcessEnd)
    requires crate::authority::observation_complete(&crate::authority::observed_running(observed)),
    ensures match crate::authority::running_exit(&crate::authority::observed_running(observed)).unwrap() {
        Some(code) => out == model::ProcessEnd::Exit(code), None => out == model::ProcessEnd::Interrupted,
    },
{
    match observed.exit() { Some(code) => model::ProcessEnd::Exit(code), None => model::ProcessEnd::Interrupted }
}

pub fn checked_process_observation<'a>(run: &'a [u8], unit: &'a crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32, stdout: Vec<u8>, stderr: Vec<u8>, exit: Option<i32>)
    -> (out: crate::authority::ObservedAttempt<'a>)
    requires pid != 0,
    ensures crate::authority::observation_complete(&crate::authority::observed_running(&out)),
        crate::authority::reserved_run(&crate::authority::running_reserved(&crate::authority::observed_running(&out))) == run,
        crate::authority::reserved_unit(&crate::authority::running_reserved(&crate::authority::observed_running(&out))) == unit,
        crate::authority::reserved_slot(&crate::authority::running_reserved(&crate::authority::observed_running(&out))) == 0,
        crate::authority::reserved_descriptor(&crate::authority::running_reserved(&crate::authority::observed_running(&out))) == descriptor,
        crate::authority::running_pid(&crate::authority::observed_running(&out)) == pid,
        crate::authority::running_stdout(&crate::authority::observed_running(&out)) == Some(stdout),
        crate::authority::running_stderr(&crate::authority::observed_running(&out)) == Some(stderr),
        crate::authority::running_exit(&crate::authority::observed_running(&out)) == Some(exit),
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(descriptor); assert(reserved.is_some());
    let running = reserved.unwrap().started(pid); assert(running.is_some());
    let observed = running.unwrap().stdout_eof(stdout).stderr_eof(stderr).exited(exit).finish();
    assert(observed.is_some()); observed.unwrap()
}

pub fn missing_process_wait_refused(run: &[u8], unit: &crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32) -> (refused: bool)
    requires pid != 0,
    ensures refused,
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(descriptor); assert(reserved.is_some());
    let running = reserved.unwrap().started(pid); assert(running.is_some());
    running.unwrap().stdout_eof(Vec::new()).stderr_eof(Vec::new()).finish().is_none()
}

pub fn recovering_finish_refuses_missing_events(run: &[u8], unit: &crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32) -> (refused: bool)
    requires pid != 0,
    ensures refused,
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(descriptor); assert(reserved.is_some());
    let running = reserved.unwrap().started(pid); assert(running.is_some());
    running.unwrap().exited(Some(0)).finish_or_output().is_err()
}

pub fn duplicate_process_stream_refused(run: &[u8], unit: &crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32) -> (refused: bool)
    requires pid != 0,
    ensures refused,
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(descriptor); assert(reserved.is_some());
    let running = reserved.unwrap().started(pid); assert(running.is_some());
    running.unwrap().stdout_eof(Vec::new()).stdout_eof(Vec::new())
        .stderr_eof(Vec::new()).exited(Some(0)).finish().is_none()
}

pub fn ambiguous_observations_preserve_output(run: &[u8], unit: &crate::authority::CompilerUnit,
    descriptor: crate::authority::ProcessDescriptor, pid: u32, stdout: Vec<u8>, extra: Vec<u8>, stderr: Vec<u8>)
    -> (out: (Vec<u8>, Vec<u8>, Option<i32>))
    requires pid != 0,
    ensures out.0 == stdout, out.1 == stderr, out.2 == Some(0),
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(descriptor); assert(reserved.is_some());
    let running = reserved.unwrap().started(pid); assert(running.is_some());
    let out = running.unwrap().stdout_eof(stdout).stdout_eof(extra).stderr_eof(stderr)
        .exited(Some(0)).finish_or_output();
    assert(out.is_err()); match out { Err(bytes) => (bytes.stdout, bytes.stderr, bytes.exit), Ok(_) => (Vec::new(), Vec::new(), None) }
}

pub fn changed_process_payload_refused(observed: &crate::authority::ObservedAttempt,
    stdout: &[u8], stderr: &[u8], exit: Option<i32>) -> (refused: bool)
    requires crate::authority::observation_complete(&crate::authority::observed_running(observed)),
        crate::authority::running_stdout(&crate::authority::observed_running(observed)).unwrap()@ != stdout@
            || crate::authority::running_stderr(&crate::authority::observed_running(observed)).unwrap()@ != stderr@
            || crate::authority::running_exit(&crate::authority::observed_running(observed)).unwrap() != exit,
    ensures refused,
{
    !crate::authority::observed_payload_matches(observed, stdout, stderr, exit)
}

pub fn attempt_slots_are_independent_of_pid(run: &[u8], unit: &crate::authority::CompilerUnit,
    first: crate::authority::ProcessDescriptor, second: crate::authority::ProcessDescriptor, pid: u32)
    requires pid != 0,
{
    let mut ledger = crate::authority::AttemptLedger::new(run, unit);
    let a = ledger.reserve(first); assert(a.is_some());
    let a = a.unwrap().started(pid); assert(a.is_some());
    let a = a.unwrap().stdout_eof(Vec::new()).stderr_eof(Vec::new()).exited(Some(1)).finish();
    assert(a.is_some());
    let b = ledger.reserve(second); assert(b.is_some());
    let b = b.unwrap().started(pid); assert(b.is_some());
    let b = b.unwrap().stdout_eof(Vec::new()).stderr_eof(Vec::new()).exited(Some(0)).finish();
    assert(b.is_some());
    let a = a.unwrap(); let b = b.unwrap();
    let apid = a.pid(); let bpid = b.pid();
    let areserved = a.reserved(); let breserved = b.reserved();
    let aslot = areserved.slot(); let bslot = breserved.slot();
    let arun = areserved.run(); let brun = breserved.run();
    let aunit = areserved.unit(); let bunit = breserved.unit();
    let asuccess = a.normal_success(); let bsuccess = b.normal_success();
    assert(apid == bpid);
    assert(aslot == 0 && bslot == 1);
    assert(arun == brun && aunit == bunit);
    assert(!asuccess && bsuccess);
}

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

/// A coherent successful partial report retains its exact recorded scope.
/// The ordinary zero-obligation case must not become an always-refuse path.
pub fn exact_partial_scope_factory(report: &Report, arguments: &[String]) -> (accepted: bool)
    requires arguments@ == report.command@, acceptance::partial_command(arguments@),
        report.engine.is_none(), acceptance::small_positive_report(report, CompletionPolicy::Receipt),
    ensures accepted,
{
    let forward: Vec<usize> = Vec::new();
    let inverse: Vec<usize> = Vec::new();
    proof {
        acceptance::small_exact_scope_candidate_definition(report, arguments@, forward@, inverse@);
    }
    acceptance::complete_exact_scope_report_ordered(report, arguments, &forward, &inverse).is_some()
}

/// A checked partial report cannot become whole authority, even when its
/// underlying crate summary happens to carry an explicit success flag.
pub fn exact_partial_scope_cannot_complete_whole(scope: &acceptance::ExactScopeReport,
    forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires acceptance::exact_scope_authority_valid(scope),
    ensures refused,
{
    let report = scope.report();
    let arguments = scope.arguments();
    proof { acceptance::exact_scope_cannot_authorize_whole(report, arguments@); }
    acceptance::complete_whole_report_ordered(report, forward, inverse).is_none()
}

/// Reordering, changing, adding or omitting any raw scope argument refuses.
pub fn exact_partial_scope_refuses_changed_arguments(report: &Report, arguments: &[String],
    forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires arguments@.len() != report.command@.len()
        || arguments@.map_values(|s: String| acceptance::string_bytes(&s))
            != report.command@.map_values(|s: String| acceptance::string_bytes(&s)),
    ensures refused,
{
    acceptance::complete_exact_scope_report_ordered(report, arguments, forward, inverse).is_none()
}

/// Existing partial receipts reject every engine report, including fallback.
pub fn exact_partial_scope_refuses_engine(report: &Report, arguments: &[String],
    forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires report.engine.is_some(),
    ensures refused,
{
    acceptance::complete_exact_scope_report_ordered(report, arguments, forward, inverse).is_none()
}

/// Compile-only success cannot become successful partial-scope evidence.
pub fn exact_partial_scope_refuses_compile_only(report: &Report, arguments: &[String],
    forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires acceptance::verification_disabled(arguments@),
    ensures refused,
{
    acceptance::complete_exact_scope_report_ordered(report, arguments, forward, inverse).is_none()
}

/// The independently supplied original request and a nonempty confirmed
/// partial report reach the actual request factory with one requested target.
pub fn positive_nonempty_exact_partial_request(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (accepted: bool)
    requires acceptance::single_exact_partial_request_example(request, project),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
    ensures accepted,
{
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    proof { acceptance::single_exact_partial_request_reaches_factory(request, project, orders@); }
    acceptance::complete_exact_partial_request_ordered(request, project, &orders).is_some()
}

/// Enabled zero-function partial reports remain valid under the same exact
/// scope contract; they cannot obtain whole-request authority.
pub fn positive_zero_function_exact_partial_request(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult) -> (accepted: bool)
    requires acceptance::single_exact_partial_request_example(request, project),
        project.targets@[0].report.crates@[0].functions@.len() == 0,
    ensures accepted,
{
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    proof { acceptance::single_exact_partial_request_reaches_factory(request, project, orders@); }
    acceptance::complete_exact_partial_request_ordered(request, project, &orders).is_some()
}

/// This constructive transfer uses a genuinely nonempty report and declared
/// input domain. The domain's physical completeness remains a separate join.
pub fn positive_nonempty_exact_partial_transfer(project: &mut crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    readback: &crate::inputs::GuardedReadback) -> (accepted: bool)
    requires acceptance::single_exact_partial_request_example(original, old(project)),
        old(project).targets@[0].report.crates@[0].functions@.len() == 1,
        acceptance::replay_request_payload(current, original),
        readback.unchanged_view(), readback.guard_view().is_some(),
        readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
        readback.comparison_view().expected_view().len() > 0,
    ensures accepted, acceptance::same_replay_evidence(old(project), final(project)),
        acceptance::exact_partial_request_authorized(current, final(project)),
        !acceptance::request_authorized(current, final(project)),
{
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    proof {
        acceptance::single_exact_partial_request_reaches_factory(original, project, orders@);
        acceptance::small_exact_partial_transfer_definition(original, project, current, orders@, readback);
    }
    acceptance::bind_replayed_exact_partial_project_ordered(project, original, current, &orders, readback)
}

/// Exact partial request tokens cannot call through to whole-request authority.
pub fn exact_partial_request_cannot_complete_whole(scope: &acceptance::ExactPartialRequest,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires acceptance::exact_partial_request_valid(scope),
    ensures refused,
{
    let request = scope.request();
    let project = scope.project();
    proof { acceptance::exact_partial_request_excludes_whole(request, project); }
    acceptance::complete_request_ordered(request, project, orders).is_none()
}

/// Partial request evidence also refuses the actual whole-selection consumer.
pub fn exact_partial_request_cannot_complete_selection(scope: &acceptance::ExactPartialRequest,
    orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires acceptance::exact_partial_request_valid(scope),
    ensures refused,
{
    let request = scope.request();
    let project = scope.project();
    proof { acceptance::exact_partial_request_excludes_whole(request, project); }
    acceptance::complete_selection_ordered(project, orders).is_none()
}

/// A correct partial report does not cover an omitted independently requested target.
pub fn exact_partial_request_refuses_missing_coverage(request: &crate::project::ResolvedRequest,
    project: &crate::project::ProjectResult, orders: &[acceptance::ReportOrder]) -> (refused: bool)
    requires !acceptance::resolved_coverage(request, project),
    ensures refused,
{
    acceptance::complete_exact_partial_request_ordered(request, project, orders).is_none()
}

/// Partial-scope reuse never accepts complete-target reuse/completion labels.
pub fn exact_partial_target_refuses_whole_labels(target: &crate::project::TargetResult,
    arguments: &[String], forward: &[usize], inverse: &[usize]) -> (refused: bool)
    requires target.complete || target.reused,
    ensures refused,
{
    acceptance::complete_exact_partial_target_ordered(target, arguments, forward, inverse).is_none()
}

/// Transfer cannot accept a changed/missing guard or a different bound revision.
pub fn exact_partial_transfer_refuses_bad_readback(project: &mut crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    orders: &[acceptance::ReportOrder], readback: &crate::inputs::GuardedReadback) -> (refused: bool)
    requires !readback.unchanged_view() || readback.guard_view().is_none()
        || readback.guard_view().unwrap().revision_view() != string_bytes(&current.input_revision),
    ensures refused, *final(project) == *old(project),
{
    !acceptance::bind_replayed_exact_partial_project_ordered(project, original, current, orders, readback)
}

/// A stored request cannot replace a missing live invocation observation.
pub fn retained_submission_refuses_missing_invocation(arguments: &[String], mode: &crate::jobs::Mode,
    request: &crate::project::ResolvedRequest) -> (refused: bool)
    requires true,
    ensures refused,
{
    acceptance::bind_retained_submission(arguments, mode, request, None).is_none()
}

/// Original submission correspondence remains insufficient after the current
/// canonical invocation resolves to another directory.
pub fn retained_submission_refuses_changed_invocation(arguments: &[String], mode: &crate::jobs::Mode,
    request: &crate::project::ResolvedRequest, current: &str) -> (refused: bool)
    requires current.spec_bytes() != string_bytes(&request.invocation),
    ensures refused,
{
    acceptance::bind_retained_submission(arguments, mode, request, Some(current)).is_none()
}

/// A coherent live invocation and actual stock submission reach the binding;
/// this operation establishes correspondence, not process or proof completion.
pub fn positive_retained_submission_invocation(request: &crate::project::ResolvedRequest,
    current: &str) -> (accepted: bool)
    requires request.args@.len() == 0, request.engine.is_none(), request.function.is_none(),
        current.spec_bytes().len() > 0,
        current.spec_bytes() == string_bytes(&request.invocation),
    ensures accepted,
{
    let arguments: Vec<String> = Vec::new();
    let mode = crate::jobs::Mode::Stock;
    acceptance::bind_retained_submission(&arguments, &mode, request, Some(current)).is_some()
}

/// Two separately supplied nonempty native byte paths reach the actual
/// binding factory only when both exact request strings preserve their bytes.
pub fn positive_nonempty_request_path_binding(request: &crate::project::ResolvedRequest,
    invocation: &[u8], workspace: &[u8]) -> (accepted: bool)
    requires invocation@.len() > 0, workspace@.len() > 0,
        acceptance::request_paths_correspond(request, invocation@, workspace@),
    ensures accepted,
{
    acceptance::bind_request_paths(request, invocation, workspace).is_some()
}

/// A lossy replacement, changed spelling or other byte mismatch cannot bind.
pub fn mismatched_request_path_binding_refused(request: &crate::project::ResolvedRequest,
    invocation: &[u8], workspace: &[u8]) -> (refused: bool)
    requires !acceptance::request_paths_correspond(request, invocation@, workspace@),
    ensures refused,
{
    acceptance::bind_request_paths(request, invocation, workspace).is_none()
}

/// A native pre-execution path mismatch withholds both whole and exact partial
/// authority, preserving observed reports and input currentness. Neither
/// complete-request factory can silently restore those missing labels.
pub fn mismatched_paths_withhold_whole_and_partial(project: &mut crate::project::ProjectResult,
    request: &crate::project::ResolvedRequest, orders: &[acceptance::ReportOrder],
    paths: Option<&acceptance::RequestPathBinding>) -> (withheld: bool)
    requires paths.is_none() || !acceptance::request_paths_correspond(request,
        acceptance::path_binding_invocation(paths.unwrap())@, acceptance::path_binding_workspace(paths.unwrap())@),
    ensures withheld, acceptance::same_project_except_path_authority(old(project), final(project)),
        final(project).currentness == old(project).currentness,
        forall |i: int| 0 <= i < final(project).targets@.len() ==>
            !final(project).targets@[i].complete && final(project).targets@[i].scope_reuse.is_none(),
{
    let matched = acceptance::withhold_unmatched_request_paths(project, request, paths);
    let whole = acceptance::complete_request_ordered(request, project, orders);
    if whole.is_some() {
        proof {
            acceptance::resolved_coverage_has_requested_target(request, project);
            let i = choose |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested;
            assert(!project.targets@[i].complete);
        }
    }
    let partial = acceptance::complete_exact_partial_request_ordered(request, project, orders);
    if partial.is_some() {
        proof {
            acceptance::resolved_coverage_has_requested_target(request, project);
            let i = choose |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested;
            assert(project.targets@[i].scope_reuse.is_none());
        }
    }
    !matched && whole.is_none() && partial.is_none()
}

/// A valid matching binding keeps the complete supplied project unchanged;
/// the path guard is not an always-withhold implementation.
pub fn matched_request_paths_preserve_observations(project: &mut crate::project::ProjectResult,
    request: &crate::project::ResolvedRequest, paths: &acceptance::RequestPathBinding) -> (matched: bool)
    requires acceptance::request_paths_correspond(request,
        acceptance::path_binding_invocation(paths)@, acceptance::path_binding_workspace(paths)@),
    ensures matched, *final(project) == *old(project),
{
    acceptance::withhold_unmatched_request_paths(project, request, Some(paths))
}

/// Constructive whole admission over a nonempty supplied report/input domain;
/// no physical compiler-input or complete solver inventory premise is implied.
pub fn positive_nonempty_declared_whole_replay(project: crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    invocation: &[u8], workspace: &[u8], readback: &crate::inputs::GuardedReadback)
    -> (out: Option<crate::project::ProjectResult>)
    requires acceptance::small_positive_request(original, &project),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
        acceptance::replay_request_payload(current, original),
        !acceptance::partial_command(current.args@),
        acceptance::request_paths_correspond(current, invocation@, workspace@),
        readback.unchanged_view(), readback.guard_view().is_some(),
        readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
        readback.comparison_view().expected_view().len() > 0,
    ensures out.is_some(), acceptance::same_replay_evidence(&project, &out.unwrap()),
        acceptance::request_authorized(current, &out.unwrap()),
{
    hide(acceptance::small_positive_request);
    hide(acceptance::request_authorized);
    hide(acceptance::replay_request_payload);
    hide(acceptance::same_replay_evidence);
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    let order_slice: &[acceptance::ReportOrder] = &orders;
    proof {
        acceptance::replay_constructive_request_definition(original, &project, current, order_slice, readback);
        assert(acceptance::replay_constructive_request(original, &project, current, order_slice, readback));
    }
    let Some(paths) = acceptance::bind_request_paths(current, invocation, workspace) else { return None; };
    let Some(admitted) = acceptance::admit_declared_replay_ordered(project, original, current, order_slice,
        Some(&paths), Some(readback)) else { return None; };
    Some(acceptance::present_declared_replay(admitted))
}

/// Exact partial admission is constructive and retains whole/selection refusal.
pub fn positive_nonempty_declared_partial_replay(project: crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    invocation: &[u8], workspace: &[u8], readback: &crate::inputs::GuardedReadback)
    -> (out: Option<crate::project::ProjectResult>)
    requires acceptance::single_exact_partial_request_example(original, &project),
        project.targets@[0].report.crates@[0].functions@.len() == 1,
        acceptance::replay_request_payload(current, original),
        acceptance::partial_command(current.args@),
        acceptance::request_paths_correspond(current, invocation@, workspace@),
        readback.unchanged_view(), readback.guard_view().is_some(),
        readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
        readback.comparison_view().expected_view().len() > 0,
    ensures out.is_some(), acceptance::same_replay_evidence(&project, &out.unwrap()),
        acceptance::exact_partial_request_authorized(current, &out.unwrap()),
        !acceptance::request_authorized(current, &out.unwrap()),
        !acceptance::supplied_selection_authorized(&out.unwrap()),
{
    let mut orders: Vec<acceptance::ReportOrder> = Vec::new();
    orders.push(acceptance::ReportOrder { forward: Vec::new(), inverse: Vec::new() });
    proof {
        acceptance::single_exact_partial_request_reaches_factory(original, &project, orders@);
        acceptance::small_exact_partial_transfer_definition(original, &project, current, orders@, readback);
    }
    let Some(paths) = acceptance::bind_request_paths(current, invocation, workspace) else { return None; };
    let Some(admitted) = acceptance::admit_declared_replay_ordered(project, original, current, &orders,
        Some(&paths), Some(readback)) else { return None; };
    Some(acceptance::present_declared_replay(admitted))
}

/// Neither flags nor a well-formed report replace missing typed witnesses.
pub fn declared_replay_missing_readback_refused(project: crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    orders: &[acceptance::ReportOrder], paths: Option<&acceptance::RequestPathBinding>) -> (refused: bool)
    ensures refused,
{
    acceptance::admit_declared_replay_ordered(project, original, current, orders, paths, None).is_none()
}

pub fn declared_replay_missing_native_paths_refused(project: crate::project::ProjectResult,
    original: &crate::project::ResolvedRequest, current: &crate::project::ResolvedRequest,
    orders: &[acceptance::ReportOrder], readback: &crate::inputs::GuardedReadback) -> (refused: bool)
    ensures refused,
{
    acceptance::admit_declared_replay_ordered(project, original, current, orders, None, Some(readback)).is_none()
}


/// Constructive byte witnesses reject an always-refusing footer parser without
/// requiring any report, verifier event or positive proof assumption.
pub fn empty_human_footer_examples() -> (accepted: bool)
    ensures accepted,
{
    proof {
        reveal_byteslit(b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in ");
        reveal_byteslit(b"    Finished `dev` profile [unoptimized] target(s) in ");
        reveal_byteslit(b"    Finished `release` profile [optimized] target(s) in ");
        reveal_byteslit(b"    Finished `release` profile [optimized + debuginfo] target(s) in ");
        reveal_byteslit(b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n");
        reveal_byteslit(b"    Finished `release` profile [optimized] target(s) in 12m 59s\n");
        reveal_byteslit(b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\nverified\n");
        reveal_byteslit(b"0.01s\n");
        reveal_byteslit(b"12m 59s\n");
        reveal_byteslit(b"0.01s\nverified\n");
        let dev = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in "@;
        let release = b"    Finished `release` profile [optimized] target(s) in "@;
        let first = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n"@;
        let second = b"    Finished `release` profile [optimized] target(s) in 12m 59s\n"@;
        assert(first.take(dev.len() as int) =~= dev);
        assert(first.skip(dev.len() as int) =~= b"0.01s\n"@);
        assert(second.take(release.len() as int) =~= release);
        assert(second.skip(release.len() as int) =~= b"12m 59s\n"@);
        assert(acceptance::cargo_duration_tail(b"0.01s\n"@, 1));
        assert(acceptance::cargo_duration(b"0.01s\n"@)) by { assert(acceptance::cargo_duration_tail(b"0.01s\n"@, 1)); }
        assert(acceptance::cargo_duration_tail(b"12m 59s\n"@, 2));
        assert(acceptance::cargo_duration(b"12m 59s\n"@)) by { assert(acceptance::cargo_duration_tail(b"12m 59s\n"@, 2)); }
        assert(acceptance::cargo_footer(first));
        assert(acceptance::cargo_footer(second));
        let trailing = b"0.01s\nverified\n"@;
        assert forall |end: int| !acceptance::cargo_duration_tail(trailing, end) by {
            if acceptance::cargo_duration_tail(trailing, end) {
                assert(end == trailing.len() - 5 || end == trailing.len() - 6);
                assert(!acceptance::decimal_byte(trailing[1]));
            }
        }
        let wrong = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\nverified\n"@;
        assert(wrong.skip(dev.len() as int) =~= trailing);
        assert(!acceptance::cargo_footer(wrong));
    }
    acceptance::empty_human_bytes(b"", b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n")
        && acceptance::empty_human_bytes(b"", b"    Finished `release` profile [optimized] target(s) in 12m 59s\n")
        && !acceptance::empty_human_bytes(b"verified", b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n")
        && !acceptance::empty_human_bytes(b"", b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\nverified\n")
}

/// A valid declared-domain human observation reaches the byte-only factory.
/// No complete-report/request predicate is a premise of this witness.
pub fn empty_human_replay_constructive<'a, 'b>(original: &'a crate::project::ResolvedRequest,
    current: &'a crate::project::ResolvedRequest, observed: &'a crate::project::ProjectResult,
    paths: &acceptance::RequestPathBinding<'a>, readback: &'a crate::inputs::GuardedReadback<'b>,
    stderr: &'a [u8]) -> (accepted: bool)
    requires acceptance::empty_human_observation(original, observed, true),
        acceptance::replay_request_payload(current, original), acceptance::cargo_footer(stderr@),
        acceptance::request_paths_correspond(current,
            acceptance::path_binding_invocation(paths)@, acceptance::path_binding_workspace(paths)@),
        readback.unchanged_view(), readback.guard_view().is_some(),
        readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
    ensures accepted,
{
    let admitted = acceptance::admit_empty_human_output(original, current, observed, true,
        b"", stderr, Some(paths), Some(readback));
    match admitted {
        Some(admitted) => {
            let actual = admitted.stderr();
            proof {
                assert(actual == stderr);
                reveal_byteslit(b"0.01s\n");
                assert(acceptance::cargo_duration_tail(b"0.01s\n"@, 1));
                assert(acceptance::cargo_duration(b"0.01s\n"@)) by {
                    assert(acceptance::cargo_duration_tail(b"0.01s\n"@, 1));
                }
            }
            acceptance::render_empty_human_output(&admitted, b"0.01s\n").is_some()
        },
        None => false,
    }
}

/// Even Current, exit-zero empty observations cannot establish the independently
/// requested whole proof. Currentness and human output remain separate facts.
pub proof fn empty_observation_has_no_whole_authority(request: &crate::project::ResolvedRequest,
    observed: &crate::project::ProjectResult)
    requires observed.targets@.len() == 0,
    ensures !acceptance::request_authorized(request, observed),
        !acceptance::supplied_selection_authorized(observed),
{
    if acceptance::resolved_coverage(request, observed) {
        acceptance::resolved_coverage_has_requested_target(request, observed);
    }
}

/// The real worker selector retains one original record behind earlier runs.
/// The selected record remains data; this witness constructs no proof facet.
pub fn original_worker_journal_preserves_nonempty_input(
    records: Vec<crate::project::RequestCapture>, worker: &str, invocation: &str,
    arguments: &[String], mode: &crate::jobs::Mode, project: &crate::project::ProjectResult,
) -> (out: Option<crate::project::ResolvedRequest>)
    requires crate::request_capture::constructive_original_capture(records@, worker,
        invocation, arguments@, mode, project),
    ensures out.is_some(), out.unwrap() == records@.last().request,
        crate::request_capture::selected_original_capture(records@, worker, invocation,
            arguments@, mode, project, &out.unwrap()),
{
    crate::request_capture::select_original_request(records, worker, invocation,
        arguments, mode, project)
}

/// No original matching run means no selected request, including an empty file.
pub fn unmatched_worker_journal_refuses(
    records: Vec<crate::project::RequestCapture>, worker: &str, invocation: &str,
    arguments: &[String], mode: &crate::jobs::Mode, project: &crate::project::ProjectResult,
) -> (refused: bool)
    requires forall |j: int| 0 <= j < records@.len() ==>
        !crate::request_capture::original_capture_run(&records@[j], project),
    ensures refused,
{
    crate::request_capture::select_original_request(records, worker, invocation,
        arguments, mode, project).is_none()
}

/// Even an ignored run's original worker/version header is mandatory.
pub fn invalid_worker_journal_header_refuses(
    records: Vec<crate::project::RequestCapture>, worker: &str, invocation: &str,
    arguments: &[String], mode: &crate::jobs::Mode, project: &crate::project::ProjectResult,
) -> (refused: bool)
    requires records@.len() > 0,
        !crate::request_capture::original_capture_header(&records@[0], worker),
    ensures refused,
{
    crate::request_capture::select_original_request(records, worker, invocation,
        arguments, mode, project).is_none()
}

/// Two records for the output run cannot silently choose either request.
pub fn duplicate_worker_journal_run_refuses(
    records: Vec<crate::project::RequestCapture>, worker: &str, invocation: &str,
    arguments: &[String], mode: &crate::jobs::Mode, project: &crate::project::ProjectResult,
) -> (refused: bool)
    requires records@.len() == 2,
        crate::request_capture::original_capture_run(&records@[0], project),
        crate::request_capture::original_capture_run(&records@[1], project),
    ensures refused,
{
    crate::request_capture::select_original_request(records, worker, invocation,
        arguments, mode, project).is_none()
}


/// A real nonempty enabled command keeps its original token unchanged.
pub fn enabled_report_argument_is_retained(original: &[String], effective: &[String])
    -> (out: Option<Vec<String>>)
    requires original@.len() == 1,
        string_bytes(&original@[0]) != b"--no-verify"@, effective@.len() == 0,
    ensures out.is_some(), out.unwrap()@ == original@,
{
    let out = crate::request_mode::construct_report_verification_arguments(original, Some(effective));
    proof {
        crate::request_mode::report_marker_literals();
        reveal_with_fuel(crate::request_mode::kept_report_arguments, 2);
    }
    out
}

/// A disabled package with no original separator gets exactly two markers.
pub fn disabled_report_adds_only_missing_markers(original: &[String], effective: &[String])
    -> (out: Option<Vec<String>>)
    requires original@.len() == 1, string_bytes(&original@[0]) != b"--"@,
        effective@.len() == 1, string_bytes(&effective@[0]) == b"--no-verify"@,
    ensures out.is_some(), out.unwrap()@.len() == 3,
        out.unwrap()@[0] == original@[0],
        string_bytes(&out.unwrap()@[1]) == b"--"@,
        string_bytes(&out.unwrap()@[2]) == b"--no-verify"@,
{
    let out = crate::request_mode::construct_report_verification_arguments(original, Some(effective));
    proof {
        crate::request_mode::report_marker_literals();
        reveal_with_fuel(crate::request_mode::kept_report_arguments, 2);
        assert(acceptance::options_disable_verification(effective@));
        assert(!crate::request_mode::has_report_separator(original@));
        assert(crate::request_mode::kept_report_arguments(original@, false, 1) =~= original@);
        assert(!acceptance::verification_disabled(original@));
        assert(out.unwrap()@.take(1) == original@);
        assert(out.unwrap()@[0] == out.unwrap()@.take(1)[0]);
    }
    out
}

/// Enabled package construction removes only the forwarded no-verify token.
pub fn enabled_report_keeps_nonempty_forwarded_payload(original: &[String], effective: &[String])
    -> (out: Option<Vec<String>>)
    requires original@.len() == 3, string_bytes(&original@[0]) == b"--"@,
        string_bytes(&original@[1]) == b"--no-verify"@,
        string_bytes(&original@[2]) != b"--no-verify"@, effective@.len() == 0,
    ensures out.is_some(), out.unwrap()@ == seq![original@[0], original@[2]],
{
    let out = crate::request_mode::construct_report_verification_arguments(original, Some(effective));
    proof {
        crate::request_mode::report_marker_literals();
        reveal_with_fuel(crate::request_mode::kept_report_arguments, 4);
        vstd::seq::lemma_seq_subrange_len(original@, 0, 1);
        vstd::seq::lemma_seq_subrange_index(original@, 0, 1, 0);
        vstd::seq::lemma_seq_subrange_len(original@, 0, 2);
        vstd::seq::lemma_seq_subrange_index(original@, 0, 2, 0);
        assert(exists |i: int| i == 0 && 0 <= i < original@.take(1).len()
            && string_bytes(&original@.take(1)[i]) == b"--"@);
        assert(exists |i: int| i == 0 && 0 <= i < original@.take(2).len()
            && string_bytes(&original@.take(2)[i]) == b"--"@);
        assert(crate::request_mode::has_report_separator(original@.take(1)));
        assert(crate::request_mode::has_report_separator(original@.take(2)));
        assert(crate::request_mode::kept_report_arguments(original@, true, 3)
            =~= seq![original@[0], original@[2]]);
    }
    out
}

/// A token after the effective Rust separator cannot disable this package.
pub fn forwarded_effective_flag_preserves_report_mode(original: &[String], effective: &[String])
    -> (out: Option<Vec<String>>)
    requires original@.len() == 1, string_bytes(&original@[0]) != b"--no-verify"@,
        effective@.len() == 2, string_bytes(&effective@[0]) == b"--"@,
        string_bytes(&effective@[1]) == b"--no-verify"@,
    ensures out.is_some(), out.unwrap()@ == original@,
        !acceptance::verification_disabled(out.unwrap()@),
{
    let out = crate::request_mode::construct_report_verification_arguments(original, Some(effective));
    proof {
        crate::request_mode::report_marker_literals();
        reveal_with_fuel(crate::request_mode::kept_report_arguments, 2);
    }
    out
}

/// Unknown effective options remain unknown on the actual report path.
pub fn missing_effective_options_cannot_enable_report(original: &[String]) -> (refused: bool)
    requires original@.len() > 0,
    ensures refused,
{
    crate::request_mode::construct_report_verification_arguments(original, None).is_none()
}

}
