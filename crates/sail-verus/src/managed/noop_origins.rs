//! Per-target original proof producers retained by unchanged-call replay.
//!
//! A saved Current envelope cannot stand in for an available original process
//! history. Input closure, current request resolution and OS read atomicity are
//! separate joins; these checks retain the ordinary receipt admission policy.

use super::super::receipts::{self, PhaseDigests, Receipt};
use sail_api::project::{ProjectResult, TargetResult};
use std::collections::BTreeSet;
use std::path::Path;

fn needed(target: &TargetResult) -> bool {
    target.complete || target.scope_reuse.is_some()
}

pub(super) fn capture(context: &Path, project: &ProjectResult) -> Option<Vec<Receipt>> {
    let directory = context.join("receipts");
    let receipts = project
        .targets
        .iter()
        .filter(|target| needed(target))
        .map(|target| {
            receipts::read(&receipts::target_path(
                &directory,
                &target.package_id,
                &target.target,
                &target.kind,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    intact(&receipts, project).then_some(receipts)
}

pub(super) fn intact(origins: &[Receipt], project: &ProjectResult) -> bool {
    let targets = project
        .targets
        .iter()
        .filter(|target| needed(target))
        .collect::<Vec<_>>();
    if targets.len() != origins.len() {
        return false;
    }
    let digests = PhaseDigests::default();
    digests.prefetch(origins.iter().flat_map(|receipt| {
        receipt
            .artifact_hashes
            .keys()
            .chain(receipt.invocation.dependencies.keys())
            .map(std::path::PathBuf::as_path)
    }));
    let mut identities = BTreeSet::new();
    targets.into_iter().zip(origins).all(|(target, receipt)| {
        identities.insert((&target.package_id, &target.target, &target.kind))
            && matches(receipt, target, project, &digests)
    })
}

fn matches(
    receipt: &Receipt,
    target: &TargetResult,
    project: &ProjectResult,
    digests: &PhaseDigests,
) -> bool {
    !(target.complete && target.scope_reuse.is_some())
        && receipt.package_id == target.package_id
        && receipt.name == target.target
        && receipt.kind == target.kind
        && !receipt.input_revision.is_empty()
        && receipt.invocation.establishing_run() == target.establishing_run
        && receipt.invocation.report == target.report
        && (if target.complete {
            sail_api::acceptance::complete_target(target).is_some()
        } else {
            target.scope_reuse.is_some()
        })
        && receipt.valid_scoped_in(
            &project.context_sha256,
            digests,
            target.scope_reuse.as_deref(),
        )
}
