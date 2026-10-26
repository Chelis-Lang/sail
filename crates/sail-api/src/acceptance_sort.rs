//! Untrusted candidate ordering for the checked authority factory.
//!
//! This module cannot construct `CompleteReport`: its fields are private to
//! `acceptance`. A malformed, incomplete or incorrectly sorted candidate is
//! rejected by the checked factory against the actual immutable report.

use sail_acceptance::authority::CompletionPolicy;

use crate::acceptance::{
    CompleteReport, CompleteRequest, CompleteSelection, ReportOrder, SubmittedAuthority,
    complete_report_ordered, complete_request_ordered, complete_selection_ordered,
    complete_submitted_request_ordered, complete_target_ordered, complete_whole_report_ordered,
    withhold_unbound_request_authority_ordered,
};
use crate::jobs::Mode;
use crate::output::Report;
use crate::project::{ProjectResult, ResolvedRequest, TargetResult};

/// Rechecks retained original-request authority with candidate obligation orders.
#[must_use]
pub fn withhold_unbound_request_authority(
    project: &mut ProjectResult,
    request: Option<&ResolvedRequest>,
) -> bool {
    let orders: Vec<_> = if request.is_some()
        && matches!(project.currentness, crate::project::Currentness::Current)
        && project.exit_code == Some(0)
    {
        project
            .targets
            .iter()
            .map(|target| candidate_order(&target.report))
            .collect()
    } else {
        Vec::new()
    };
    withhold_unbound_request_authority_ordered(project, request, &orders)
}

/// Checks original submission correspondence and actual complete request authority.
#[must_use]
pub fn complete_submitted_request<'a>(
    arguments: &'a [String],
    mode: &'a Mode,
    request: &'a ResolvedRequest,
    project: &'a ProjectResult,
) -> Option<SubmittedAuthority<'a>> {
    let orders: Vec<_> = project
        .targets
        .iter()
        .map(|target| candidate_order(&target.report))
        .collect();
    complete_submitted_request_ordered(arguments, mode, request, project, &orders)
}

/// Validates the complete original request without trusting result-selected scope.
#[must_use]
pub fn complete_request<'a>(
    request: &'a ResolvedRequest,
    project: &'a ProjectResult,
) -> Option<CompleteRequest<'a>> {
    let orders: Vec<_> = project
        .targets
        .iter()
        .map(|target| candidate_order(&target.report))
        .collect();
    complete_request_ordered(request, project, &orders)
}

/// Establishes receipt/repair report authority through the checked factory.
#[must_use]
pub fn complete_report(report: &Report, policy: CompletionPolicy) -> Option<CompleteReport<'_>> {
    let order = candidate_order(report);
    complete_report_ordered(report, policy, &order.forward, &order.inverse)
}

/// Checks complete whole-target scope through the same checked factory.
#[must_use]
pub fn complete_whole_report(report: &Report) -> Option<CompleteReport<'_>> {
    let order = candidate_order(report);
    complete_whole_report_ordered(report, &order.forward, &order.inverse)
}

/// Revalidates target completion before deriving a positive public presentation.
#[must_use]
pub fn complete_target(target: &TargetResult) -> Option<CompleteReport<'_>> {
    let order = candidate_order(&target.report);
    complete_target_ordered(target, &order.forward, &order.inverse)
}

/// Checks a retained envelope's actual selected reports through the checked factory.
#[must_use]
pub fn complete_selection(project: &ProjectResult) -> Option<CompleteSelection<'_>> {
    let orders: Vec<_> = project
        .targets
        .iter()
        .map(|target| {
            if target.requested {
                candidate_order(&target.report)
            } else {
                ReportOrder {
                    forward: Vec::new(),
                    inverse: Vec::new(),
                }
            }
        })
        .collect();
    complete_selection_ordered(project, &orders)
}

fn candidate_order(report: &Report) -> ReportOrder {
    let ids: Vec<_> = report
        .crates
        .iter()
        .flat_map(|c| &c.functions)
        .flat_map(|f| f.obligations.iter().flatten())
        .map(|o| o.id.as_bytes())
        .collect();
    let mut forward: Vec<_> = (0..ids.len()).collect();
    forward.sort_unstable_by(|a, b| ids[*a].cmp(ids[*b]));
    let mut inverse = vec![0; forward.len()];
    for (position, index) in forward.iter().copied().enumerate() {
        inverse[index] = position;
    }
    ReportOrder { forward, inverse }
}
