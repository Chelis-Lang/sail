use super::*;
use crate::output::{AnswerName, SourceName};

fn function(name: &str) -> Function {
    Function {
        package_id: "demo@0.1.0/path".to_owned(),
        target: "demo".to_owned(),
        target_kinds: vec!["lib".to_owned()],
        function: format!("demo::{name}"),
    }
}
fn check(answer: AnswerName, failure: Failure) -> Check {
    Check {
        answer: Some(answer),
        reason_unknown: (answer == AnswerName::Unknown)
            .then(|| "(:reason-unknown \"(incomplete quantifiers)\")".to_owned()),
        source: Some(SourceName::Solved),
        rlimit_limit: Some(100),
        rlimit_used: Some(10),
        failure,
        model: None,
    }
}
fn diagnostic(function: Option<Function>, message: &str) -> Diagnostic {
    let original = crate::output::Diagnostic {
        level: "error".to_owned(),
        code: None,
        message: message.to_owned(),
        spans: Vec::new(),
        rendered: format!("error: {message}\n"),
    };
    Diagnostic {
        function,
        level: original.level.clone(),
        code: original.code.clone(),
        message: original.message.clone(),
        original,
    }
}
fn input() -> Classification {
    let context = Context {
        baseline_sha256: "a".repeat(64),
        runtime_sha256: "b".repeat(64),
        solver_sha256: "c".repeat(64),
        options_sha256: "d".repeat(64),
    };
    let target = function("f");
    let baseline = Observation {
        context: context.clone(),
        source_sha256: context.baseline_sha256.clone(),
        scope: Scope::Full,
        completed: true,
        unviable: false,
        functions_complete: true,
        functions: vec![FunctionObservation {
            identity: target.clone(),
            verified: true,
            unstable: false,
            checks_complete: true,
            checks: vec![check(AnswerName::Unsat, Failure::None)],
        }],
        diagnostics: Vec::new(),
    };
    let mut mutant = baseline.clone();
    mutant.source_sha256 = "e".repeat(64);
    mutant.functions[0].verified = false;
    mutant.functions[0].checks = vec![check(AnswerName::Unknown, Failure::Verifier)];
    mutant.diagnostics = vec![diagnostic(Some(target.clone()), "assertion failed")];
    Classification {
        mutation: Mutation {
            id: "exec-001".to_owned(),
            file: "src/lib.rs".into(),
            function: Some(target),
            class: ExecClass::Constant,
            start: 0,
            end: 1,
            before: "1".to_owned(),
            after: "2".to_owned(),
            source_sha256: "e".repeat(64),
        },
        context,
        repeats: MIN_REPEATS,
        baseline: vec![baseline.clone(), baseline],
        attempts: vec![mutant.clone(), mutant],
    }
}

#[test]
fn stable_quantified_unknown_rejection_preserves_raw_evidence() {
    let input = input();
    let before = input.clone();
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
    assert_eq!(input, before);
    assert_eq!(
        input.attempts[0].functions[0].checks[0].answer,
        Some(AnswerName::Unknown)
    );
}
#[test]
fn native_sat_is_a_separate_subclass_and_a_model_alone_is_insufficient() {
    let mut input = input();
    for run in &mut input.attempts {
        run.functions[0].checks[0].model = Some("(model (define-fun x () Int 2))".to_owned());
    }
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
    for run in &mut input.attempts {
        run.functions[0].checks[0].answer = Some(AnswerName::Sat);
    }
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::CounterexampleConfirmed)
    );
    for run in &mut input.attempts {
        run.functions[0].checks[0].source = None;
    }
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
}
#[test]
fn failed_near_limit_unstable_and_unrecorded_twins_never_score() {
    for reason in [
        Reason::BaselineFailed,
        Reason::BaselineNearLimit,
        Reason::BaselineUnstable,
        Reason::BaselineHeadroomMissing,
    ] {
        let mut input = input();
        for run in &mut input.baseline {
            match reason {
                Reason::BaselineFailed => run.functions[0].verified = false,
                Reason::BaselineNearLimit => run.functions[0].checks[0].rlimit_used = Some(51),
                Reason::BaselineUnstable => run.functions[0].unstable = true,
                Reason::BaselineHeadroomMissing => run.functions[0].checks_complete = false,
                _ => {}
            }
        }
        assert_eq!(classify(&input), Outcome::Inconclusive(reason));
    }
}
#[test]
fn half_limit_boundary_is_exact_even_at_u64_maximum() {
    let mut input = input();
    for run in &mut input.baseline {
        run.functions[0].checks[0].rlimit_limit = Some(u64::MAX);
        run.functions[0].checks[0].rlimit_used = Some(u64::MAX / 2);
    }
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
    for run in &mut input.baseline {
        run.functions[0].checks[0].rlimit_used = Some(u64::MAX);
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::BaselineNearLimit)
    );
}
#[test]
fn every_repetition_needs_its_own_non_resource_evidence() {
    let mut input = input();
    input.attempts[1].functions[0].checks[0].failure = Failure::ResourceLimit;
    assert_eq!(classify(&input), Outcome::Inconclusive(Reason::Unstable));
    for run in &mut input.attempts {
        run.functions[0].checks[0].failure = Failure::ResourceLimit;
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::ResourceOnly)
    );
    for run in &mut input.attempts {
        run.functions[0].checks[0].failure = Failure::Verifier;
        run.functions[0].checks[0].reason_unknown = Some("max. resource limit exceeded".to_owned());
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::ResourceOnly)
    );
}
#[test]
fn attribution_and_physical_context_cannot_be_inferred_or_swapped() {
    let mut input = input();
    for run in &mut input.attempts {
        run.diagnostics[0].function = None;
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::DiagnosticUnattributed)
    );
    input.attempts[0].context.solver_sha256 = "f".repeat(64);
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::IdentityMismatch)
    );
}
#[test]
fn repetitions_keep_diagnostic_multiplicity_but_not_message_order() {
    let mut input = input();
    for run in &mut input.attempts {
        run.diagnostics
            .push(diagnostic(Some(function("f")), "second failure"));
    }
    input.attempts[1].diagnostics.reverse();
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
    let diagnostic = input.attempts[0].diagnostics[0].clone();
    input.attempts[1].diagnostics.push(diagnostic);
    assert_eq!(classify(&input), Outcome::Inconclusive(Reason::Unstable));
}
#[test]
fn baseline_failures_remain_visible_without_blocking_an_unchanged_survivor() {
    let mut input = input();
    let other = FunctionObservation {
        identity: function("other"),
        verified: false,
        unstable: false,
        checks_complete: true,
        checks: vec![check(AnswerName::Unknown, Failure::ResourceLimit)],
    };
    for run in &mut input.baseline {
        run.functions.push(other.clone());
    }
    for run in &mut input.attempts {
        run.functions[0].verified = true;
        run.functions[0].checks = vec![check(AnswerName::Unsat, Failure::None)];
        run.diagnostics.clear();
        run.functions.push(other.clone());
    }
    assert_eq!(classify(&input), Outcome::Survived);
    for run in &mut input.attempts {
        let mut new = other.clone();
        new.identity = function("new_failure");
        run.functions.push(new);
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::CollateralFailure)
    );
}
#[test]
fn scoped_observations_never_become_confirmed_rejections_or_survivors() {
    let mut input = input();
    for run in &mut input.attempts {
        run.scope = Scope::Scoped;
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::FullConfirmationMissing)
    );
    for run in &mut input.attempts {
        run.functions[0].verified = true;
        run.functions[0].checks = vec![check(AnswerName::Unsat, Failure::None)];
        run.diagnostics.clear();
    }
    assert_eq!(classify(&input), Outcome::SurvivedScoped);
}
#[test]
fn incomplete_duplicate_and_contradictory_inputs_fail_closed() {
    let mut input = input();
    input.attempts[1].completed = false;
    assert_eq!(classify(&input), Outcome::Inconclusive(Reason::Incomplete));
    input.attempts[1].completed = true;
    let function = input.attempts[0].functions[0].clone();
    input.attempts[1].functions.push(function);
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::InconsistentEvidence)
    );
    input.attempts[1].functions.pop();
    for run in &mut input.attempts {
        run.functions[0].verified = true;
    }
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::InconsistentEvidence)
    );
}
#[test]
fn bounds_and_empty_scores_remain_explicit() {
    assert!(Bounds::default().validate().is_ok());
    let bounds = Bounds {
        repeats: 1,
        ..Bounds::default()
    };
    assert!(bounds.validate().is_err());
    let values = [
        Outcome::Rejected(Detection::VerifierRejected),
        Outcome::Rejected(Detection::CounterexampleConfirmed),
        Outcome::Survived,
        Outcome::SurvivedScoped,
        Outcome::Inconclusive(Reason::ResourceOnly),
        Outcome::Unviable,
        Outcome::Unsupported("recursive".to_owned()),
    ];
    let summary = summarize(&values, 3, 2);
    assert_eq!(summary.score, Some(0.5));
    assert_eq!(
        (
            summary.rejected,
            summary.counterexample_confirmed,
            summary.verifier_rejected
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (
            summary.omitted,
            summary.cancelled,
            summary.unviable,
            summary.unsupported
        ),
        (3, 2, 1, 1)
    );
    assert_eq!(summarize([], 1, 1).score, None);
}

#[test]
fn absent_or_unattributed_failures_do_not_claim_resource_exhaustion() {
    for missing in [true, false] {
        let mut input = input();
        for run in &mut input.attempts {
            if missing {
                run.functions[0].checks.clear();
            } else {
                run.functions[0].checks[0].failure = Failure::Unattributed;
            }
        }
        assert_eq!(
            classify(&input),
            Outcome::Inconclusive(Reason::FailureUnattributed)
        );
    }
}

#[test]
fn contradictory_verified_observations_never_score() {
    for baseline in [false, true] {
        for kind in ["sat", "unknown", "error", "failure"] {
            let mut input = input();
            let runs = if baseline {
                &mut input.baseline
            } else {
                &mut input.attempts
            };
            for run in runs {
                let function = &mut run.functions[0];
                function.verified = true;
                function.checks = vec![check(AnswerName::Unsat, Failure::None)];
                run.diagnostics.clear();
                match kind {
                    "sat" => function.checks[0].answer = Some(AnswerName::Sat),
                    "unknown" => function.checks[0].answer = Some(AnswerName::Unknown),
                    "error" => run
                        .diagnostics
                        .push(diagnostic(Some(function.identity.clone()), "failed")),
                    _ => function.checks[0].failure = Failure::Verifier,
                }
            }
            assert_eq!(
                classify(&input),
                Outcome::Inconclusive(Reason::InconsistentEvidence),
                "baseline={baseline} kind={kind}"
            );
        }
    }
}

#[test]
fn full_confirmation_retains_scoped_conflicts_and_interruption() {
    for full_survives in [false, true] {
        let mut input = input();
        let mut scoped = input.attempts.clone();
        for run in &mut scoped {
            run.scope = Scope::Scoped;
        }
        let survivors = if full_survives {
            &mut input.attempts
        } else {
            &mut scoped
        };
        for run in survivors {
            run.functions[0].verified = true;
            run.functions[0].checks = vec![check(AnswerName::Unsat, Failure::None)];
            run.diagnostics.clear();
        }
        input.attempts.extend(scoped);
        assert_eq!(classify(&input), Outcome::Inconclusive(Reason::Unstable));
    }
    for incomplete in [true, false] {
        let mut input = input();
        let mut scoped = input.attempts.clone();
        for run in &mut scoped {
            run.scope = Scope::Scoped;
        }
        if incomplete {
            scoped[0].completed = false;
        } else {
            scoped[0].diagnostics[0].message = "different".to_owned();
            scoped[0].diagnostics[0].original.message = "different".to_owned();
        }
        input.attempts.extend(scoped);
        assert_eq!(
            classify(&input),
            Outcome::Inconclusive(if incomplete {
                Reason::Incomplete
            } else {
                Reason::Unstable
            })
        );
    }
}

#[test]
fn scope_specific_diagnostics_can_differ_without_changing_target_outcomes() {
    let mut input = input();
    let mut scoped = input.attempts.clone();
    for run in &mut scoped {
        run.scope = Scope::Scoped;
        run.diagnostics = vec![diagnostic(
            Some(function("f")),
            "different focused diagnostic",
        )];
    }
    input.attempts.extend(scoped);
    assert_eq!(
        classify(&input),
        Outcome::Rejected(Detection::VerifierRejected)
    );
}

#[test]
fn full_confirmations_do_not_erase_scoped_outcome_preconditions() {
    for (kind, reason) in [
        ("missing diagnostic", Reason::DiagnosticUnattributed),
        ("unattributed diagnostic", Reason::DiagnosticUnattributed),
        ("unsat rejection", Reason::InconsistentEvidence),
    ] {
        let mut input = input();
        let mut scoped = input.attempts.clone();
        for run in &mut scoped {
            run.scope = Scope::Scoped;
            match kind {
                "missing diagnostic" => run.diagnostics.clear(),
                "unattributed diagnostic" => run.diagnostics[0].function = None,
                _ => run.functions[0].checks[0].answer = Some(AnswerName::Unsat),
            }
        }
        input.attempts.extend(scoped);
        assert_eq!(classify(&input), Outcome::Inconclusive(reason), "{kind}");
    }
    let mut input = input();
    for run in &mut input.attempts {
        run.functions[0].verified = true;
        run.functions[0].checks = vec![check(AnswerName::Unsat, Failure::None)];
        run.diagnostics.clear();
    }
    let mut scoped = input.attempts.clone();
    for run in &mut scoped {
        run.scope = Scope::Scoped;
        run.functions.push(FunctionObservation {
            identity: function("new_failure"),
            verified: false,
            unstable: false,
            checks_complete: true,
            checks: vec![check(AnswerName::Unknown, Failure::Verifier)],
        });
        run.diagnostics.push(diagnostic(
            Some(function("new_failure")),
            "new collateral failure",
        ));
    }
    input.attempts.extend(scoped);
    assert_eq!(
        classify(&input),
        Outcome::Inconclusive(Reason::CollateralFailure)
    );
}
