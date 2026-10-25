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

}
