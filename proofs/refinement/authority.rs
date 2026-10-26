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

}
