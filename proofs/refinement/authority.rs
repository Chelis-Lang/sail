//! Constructive refinement of actual typed report records into the authority model.
//! The source-shared factory proves the concrete premises; no external report-valid
//! specification supplies them. Process/input/session observation remains separate.
use vstd::prelude::*;
use crate::authority_model as model;
use crate::acceptance::{self, confirmed, successful_report, string_bytes};
use crate::output::{EvidenceTierName, EstablishmentName, FunctionReport, Report, VerdictName};
use crate::authority::CompletionPolicy;

verus! {

pub open spec fn model_verdict(v: VerdictName) -> model::Verdict {
    match v {
        VerdictName::Verified => model::Verdict::Verified,
        VerdictName::Failed => model::Verdict::Failed,
        VerdictName::Unknown => model::Verdict::Unknown,
        VerdictName::Provisional => model::Verdict::Provisional,
    }
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
