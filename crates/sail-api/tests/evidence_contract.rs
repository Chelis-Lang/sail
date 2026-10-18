//! Comparisons preserve exact scope, removal limits and ambiguous identities.
use sail_api::evidence::Change;
use sail_api::output::{EstablishmentName, OutcomeName, Report, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};

fn project() -> Result<ProjectResult, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    Ok(ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "run".into(),
        workspace: "workspace".into(),
        context_sha256: "context".into(),
        input_revision: "before".into(),
        currentness: Currentness::Current,
        exit_code: Some(0),
        targets: vec![TargetResult {
            dependency_store: None,
            dependency_build: None,
            package_id: "demo".into(),
            target: "demo".into(),
            kind: vec!["lib".into()],
            establishing_run: "run".into(),
            reused: false,
            requested: true,
            complete: true,
            report,
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
        engine: None,
        no_reuse: None,
    })
}
fn success(mut project: ProjectResult) -> ProjectResult {
    let report = &mut project.targets[0].report;
    report.exit_code = Some(0);
    report.error = None;
    report.summary.failed = 0;
    report.summary.unknown = 0;
    report.summary.provisional = 0;
    for c in &mut report.crates {
        c.verus.success = Some(true);
        c.verus.errors = Some(0);
    }
    for f in report.crates.iter_mut().flat_map(|c| &mut c.functions) {
        f.establishment = EstablishmentName::Confirmed;
        f.verdict = VerdictName::Verified;
    }
    project
}

/// Unbound setups can produce different verdicts without establishing the
/// same runtime context. Neither a shared empty context nor a loader run's
/// nonempty context authorizes a proof-regression comparison.
#[test]
fn unbound_results_never_claim_comparable_proof_contexts() -> Result<(), Box<dyn std::error::Error>>
{
    use sail_api::project::{NoReuse, VerifiedAs};
    for verified_as in [VerifiedAs::PlainCargoVerus, VerifiedAs::Managed] {
        let mut before = success(project()?);
        let mut after = project()?;
        before.no_reuse = Some(NoReuse {
            code: "SAIL3112".into(),
            reason: "--tests".into(),
            verified_as,
        });
        after.no_reuse = Some(NoReuse {
            code: "SAIL3112".into(),
            reason: "--config build.rustflags=[]".into(),
            verified_as,
        });
        for empty in [false, true] {
            if empty {
                before.context_sha256.clear();
                after.context_sha256.clear();
            }
            for (left, right) in [(&before, &after), (&after, &before)] {
                let comparison = sail_api::evidence::compare(left, right);
                assert!(!comparison.compatible);
                assert_ne!(comparison.differences, []);
                assert!(comparison.differences.iter().all(|difference| {
                    difference.change == Change::Incomparable
                        && difference
                            .obligations
                            .iter()
                            .all(|obligation| obligation.change == Change::Incomparable)
                }));
            }
        }
    }
    let mut before = success(project()?);
    let mut after = project()?;
    before.context_sha256.clear();
    after.context_sha256.clear();
    assert!(!sail_api::evidence::compare(&before, &after).compatible);
    Ok(())
}
#[test]
fn exact_target_obligation_comparisons_preserve_scope_and_removal_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let before = success(project()?);
    let mut after = before.clone();
    after.input_revision = "after".into();
    let f = &mut after.targets[0].report.crates[0].functions[0];
    f.verdict = VerdictName::Failed;
    let Some(obligations) = f.obligations.as_mut() else {
        return Err("fixture requires attributed obligations".into());
    };
    let obligation = obligations
        .first_mut()
        .ok_or("fixture requires an obligation")?;
    let id = obligation.id.clone();
    obligation.outcome = OutcomeName::Failed;
    obligation.advisory = false;
    let mut before = before;
    before.targets[0].report.crates[0].functions[0]
        .obligations
        .as_mut()
        .ok_or("fixture requires obligations")?[0]
        .outcome = OutcomeName::Proved;
    let comparison = sail_api::evidence::compare(&before, &after);
    assert!(comparison.compatible);
    assert_eq!(comparison.differences[0].change, Change::Regressed);
    assert!(
        comparison.differences[0]
            .obligations
            .iter()
            .any(|diff| diff.id == id && diff.change == Change::Regressed)
    );
    after.targets[0].report.crates[0].functions.remove(0);
    after.targets[0].report.crates[0].verus.success = None;
    assert_eq!(
        sail_api::evidence::compare(&before, &after).differences[0].change,
        Change::NotObserved
    );
    after.targets[0].report.crates[0].verus.success = Some(true);
    assert_eq!(
        sail_api::evidence::compare(&before, &after).differences[0].change,
        Change::Removed
    );
    after.targets[0].kind = vec!["bin".into()];
    assert!(!sail_api::evidence::compare(&before, &after).compatible);
    Ok(())
}

#[test]
fn repeated_friendly_names_do_not_invent_order_dependent_regressions()
-> Result<(), Box<dyn std::error::Error>> {
    let mut before = success(project()?);
    let mut duplicate = before.targets[0].report.crates[0].functions[0].clone();
    duplicate.verdict = VerdictName::Failed;
    let name = duplicate.function.clone();
    let original_count = before.targets[0].report.crates[0].functions.len();
    before.targets[0].report.crates[0].functions.push(duplicate);
    let mut after = before.clone();
    after.targets[0].report.crates[0].functions.reverse();
    let compared = sail_api::evidence::compare(&before, &after);
    assert!(compared.compatible);
    assert_eq!(compared.differences.len(), original_count);
    let difference = compared
        .differences
        .iter()
        .find(|d| d.function == name)
        .ok_or("ambiguous function difference missing")?;
    assert!(
        compared
            .differences
            .iter()
            .filter(|d| d.function != name)
            .all(|d| d.change == Change::Unchanged)
    );
    assert_eq!(difference.change, sail_api::evidence::Change::Incomparable);
    assert_eq!(difference.before, None);
    assert_eq!(difference.after, None);
    assert_eq!(difference.obligations, []);
    assert!(
        compared
            .notes
            .iter()
            .any(|n| n.contains("Repeated friendly function names"))
    );
    Ok(())
}

/// A failing run whose successes Sail could not attribute one by one: every
/// non-failed function is `unknown`, and Verus's totals hold one error per
/// failed function.
fn failing_unattributed(mut project: ProjectResult) -> ProjectResult {
    let report = &mut project.targets[0].report;
    for c in &mut report.crates {
        c.verus.success = Some(false);
        c.verus.errors = Some(1);
        c.verus.verified = Some(100);
        c.verus.encountered_vir_error = Some(false);
    }
    for (i, f) in report
        .crates
        .iter_mut()
        .flat_map(|c| &mut c.functions)
        .enumerate()
    {
        f.establishment = EstablishmentName::Confirmed;
        f.verdict = if i == 0 {
            VerdictName::Failed
        } else {
            VerdictName::Unknown
        };
    }
    project
}

#[test]
fn functions_passing_by_verus_totals_compare_as_verified_without_changing_verdicts()
-> Result<(), Box<dyn std::error::Error>> {
    let before = failing_unattributed(success(project()?));
    let after = success(project()?);
    let fixed = before.targets[0].report.crates[0].functions[0]
        .function
        .clone();
    let compared = sail_api::evidence::compare(&before, &after);
    assert!(compared.compatible);
    for d in &compared.differences {
        if d.function == fixed {
            assert_eq!(d.change, Change::Recovered);
            assert_eq!(d.before, Some(VerdictName::Failed));
            assert!(!d.before_passed_by_verus_totals);
        } else {
            // Unknown by attribution, passed by Verus's totals: unchanged,
            // and the recorded verdict is still `unknown`.
            assert_eq!(d.change, Change::Unchanged, "{}", d.function);
            assert_eq!(d.before, Some(VerdictName::Unknown));
            assert_eq!(d.after, Some(VerdictName::Verified));
            assert!(d.before_passed_by_verus_totals);
            assert!(!d.after_passed_by_verus_totals);
        }
    }
    assert!(compared.notes.iter().any(|n| n.contains("Verus's totals")));
    let json = serde_json::to_value(&compared)?;
    assert!(
        json["differences"][0]
            .get("after_passed_by_verus_totals")
            .is_none()
    );

    // The same on the later side: a newly failing function with the rest
    // unattributed regresses alone; the others did not become unknown.
    let reversed = sail_api::evidence::compare(&after, &before);
    for d in &reversed.differences {
        let expected = if d.function == fixed {
            Change::Regressed
        } else {
            Change::Unchanged
        };
        assert_eq!(d.change, expected, "{}", d.function);
        assert_eq!(d.after_passed_by_verus_totals, d.function != fixed);
    }

    // An error Verus's totals do not account for: unknown stays unknown.
    let mut extra = before.clone();
    extra.targets[0].report.crates[0].verus.errors = Some(2);
    for d in &sail_api::evidence::compare(&extra, &after).differences {
        if d.function != fixed {
            assert_eq!(d.change, Change::Recovered, "{}", d.function);
            assert!(!d.before_passed_by_verus_totals);
        }
    }
    Ok(())
}

#[test]
fn repeated_friendly_names_compare_as_groups_only_when_every_instance_agrees()
-> Result<(), Box<dyn std::error::Error>> {
    // Two implementations share a friendly name (VI-0010).
    let mut base = project()?;
    let duplicate = base.targets[0].report.crates[0].functions[1].clone();
    let name = duplicate.function.clone();
    base.targets[0].report.crates[0].functions.push(duplicate);
    let before = failing_unattributed(success(base.clone()));
    let after = success(base);
    let compared = sail_api::evidence::compare(&before, &after);
    let group = compared
        .differences
        .iter()
        .find(|d| d.function == name)
        .ok_or("repeated name missing")?;
    // Both passed by Verus's totals before and verified after.
    assert_eq!(group.change, Change::Unchanged);
    assert_eq!(group.before, Some(VerdictName::Unknown));
    assert_eq!(group.after, Some(VerdictName::Verified));
    assert_eq!(group.obligations, []);
    assert!(compared.notes.iter().any(|n| n.contains(
        "1 repeated name(s), shared by 2 functions (VI-0010), were each compared as a group"
    )));
    assert!(
        !compared
            .notes
            .iter()
            .any(|n| n.contains("are incomparable"))
    );

    // A different number of instances stays incomparable.
    let mut more = after.clone();
    let extra = more.targets[0].report.crates[0].functions[1].clone();
    more.targets[0].report.crates[0].functions.push(extra);
    let compared = sail_api::evidence::compare(&before, &more);
    let group = compared
        .differences
        .iter()
        .find(|d| d.function == name)
        .ok_or("repeated name missing")?;
    assert_eq!(group.change, Change::Incomparable);
    assert_eq!(group.before, None);
    assert!(!group.before_passed_by_verus_totals);
    assert!(compared.notes.iter().any(|n| n.contains("1 such name(s)")));
    Ok(())
}
