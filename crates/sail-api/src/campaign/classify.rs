//! Pure ADR 0111 interpretation of finalized producer evidence.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Classification, Context, Detection, Diagnostic, Failure, Function, FunctionObservation,
    MIN_REPEATS, Observation, Outcome, Reason, Scope, Summary,
};
use crate::output::AnswerName;

/// Classifies a mutant without changing any original backend observation.
/// Full-unit confirmation is mandatory for a final rejection or survivor.
#[must_use]
pub fn classify(input: &Classification) -> Outcome {
    checked(input).unwrap_or_else(Outcome::Inconclusive)
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn context_valid(context: &Context) -> bool {
    [
        &context.baseline_sha256,
        &context.runtime_sha256,
        &context.solver_sha256,
        &context.options_sha256,
    ]
    .into_iter()
    .all(|v| digest(v))
}
fn function_valid(function: &Function) -> bool {
    !function.package_id.is_empty()
        && !function.target.is_empty()
        && !function.function.is_empty()
        && !function.target_kinds.is_empty()
        && function.target_kinds.iter().all(|k| !k.is_empty())
        && function.target_kinds.windows(2).all(|w| w[0] < w[1])
}
fn valid_observation(
    observation: &Observation,
    context: &Context,
    source: &str,
) -> Result<(), Reason> {
    if &observation.context != context || observation.source_sha256 != source {
        return Err(Reason::IdentityMismatch);
    }
    let mut seen = BTreeSet::new();
    for function in &observation.functions {
        if !function_valid(&function.identity) || !seen.insert(&function.identity) {
            return Err(Reason::InconsistentEvidence);
        }
    }
    if observation.unviable
        && observation.completed
        && !observation.diagnostics.iter().any(|d| d.level == "error")
    {
        return Err(Reason::InconsistentEvidence);
    }
    if observation.diagnostics.iter().any(|d| {
        d.level != d.original.level
            || d.code != d.original.code
            || d.message != d.original.message
            || d.function.as_ref().is_some_and(|f| !function_valid(f))
    }) {
        return Err(Reason::InconsistentEvidence);
    }
    Ok(())
}
fn target<'a>(
    observation: &'a Observation,
    function: &Function,
) -> Result<&'a FunctionObservation, Reason> {
    observation
        .functions
        .iter()
        .find(|f| &f.identity == function)
        .ok_or(Reason::AmbiguousFunction)
}
fn effort(check: &super::Check) -> sail_ledger::Effort {
    sail_ledger::Effort {
        limit: check.rlimit_limit,
        used: check.rlimit_used,
        reason: check.reason_unknown.clone(),
    }
}
fn headroom(function: &FunctionObservation) -> Result<(), Reason> {
    if function.unstable {
        return Err(Reason::BaselineUnstable);
    }
    if !function.verified {
        return Err(Reason::BaselineFailed);
    }
    if !function.checks_complete {
        return Err(Reason::BaselineHeadroomMissing);
    }
    for check in &function.checks {
        if check.failure != Failure::None || effort(check).exhausted() {
            return Err(Reason::BaselineFailed);
        }
        let (Some(used), Some(limit)) = (check.rlimit_used, check.rlimit_limit) else {
            return Err(Reason::BaselineHeadroomMissing);
        };
        if check.source.is_none() || check.answer != Some(AnswerName::Unsat) {
            return Err(Reason::BaselineHeadroomMissing);
        }
        // Division avoids the ledger's separately tracked u64 multiply overflow.
        if limit > 0 && used > limit / 2 {
            return Err(Reason::BaselineNearLimit);
        }
    }
    Ok(())
}
fn detection(function: &FunctionObservation) -> Option<Detection> {
    function
        .checks
        .iter()
        .any(|c| c.failure == Failure::Verifier && !effort(c).exhausted())
        .then(|| {
            if function.checks.iter().any(|c| {
                c.failure == Failure::Verifier
                    && !effort(c).exhausted()
                    && c.answer == Some(AnswerName::Sat)
                    && c.source.is_some()
            }) {
                Detection::CounterexampleConfirmed
            } else {
                Detection::VerifierRejected
            }
        })
}

type Message<'a> = (Option<&'a Function>, &'a str, Option<&'a str>, &'a str);
fn messages(diagnostics: &[Diagnostic]) -> Vec<Message<'_>> {
    let mut messages: Vec<_> = diagnostics
        .iter()
        .map(|d| {
            (
                d.function.as_ref(),
                d.level.as_str(),
                d.code.as_deref(),
                d.message.as_str(),
            )
        })
        .collect();
    messages.sort_unstable();
    messages
}
fn stable(runs: &[&Observation], function: &Function) -> bool {
    let Some(first) = runs.first() else {
        return false;
    };
    let verdicts = |r: &Observation| -> BTreeMap<Function, bool> {
        r.functions
            .iter()
            .map(|f| (f.identity.clone(), f.verified))
            .collect()
    };
    runs.iter().skip(1).all(|run| {
        verdicts(first) == verdicts(run)
            && messages(&first.diagnostics) == messages(&run.diagnostics)
            && target(first, function).ok().and_then(detection)
                == target(run, function).ok().and_then(detection)
            && first.unviable == run.unviable
    })
}
fn complete(runs: &[&Observation]) -> bool {
    runs.iter()
        .all(|r| r.completed && (r.unviable || r.functions_complete))
}
fn failures(observation: &Observation) -> BTreeSet<&Function> {
    observation
        .functions
        .iter()
        .filter(|f| !f.verified)
        .map(|f| &f.identity)
        .collect()
}
fn outcome(
    run: &Observation,
    baseline: &Observation,
    function: &Function,
    scope: Scope,
) -> Result<Outcome, Reason> {
    if run.unviable {
        return Ok(Outcome::Unviable);
    }
    let observed = target(run, function)?;
    if observed.unstable {
        return Err(Reason::Unstable);
    }
    if observed.verified {
        if observed.checks.iter().any(|c| c.failure != Failure::None) {
            return Err(Reason::InconsistentEvidence);
        }
        if !failures(run).is_subset(&failures(baseline)) {
            return Err(Reason::CollateralFailure);
        }
        return Ok(if scope == Scope::Full {
            Outcome::Survived
        } else {
            Outcome::SurvivedScoped
        });
    }
    if !run
        .diagnostics
        .iter()
        .any(|d| d.function.as_ref() == Some(function) && d.level == "error")
    {
        return Err(Reason::DiagnosticUnattributed);
    }
    if observed
        .checks
        .iter()
        .any(|c| c.failure == Failure::Verifier && c.answer == Some(AnswerName::Unsat))
    {
        return Err(Reason::InconsistentEvidence);
    }
    let evidence = detection(observed).ok_or_else(|| {
        if !observed.checks.is_empty()
            && observed.checks.iter().all(|check| {
                matches!(
                    check.failure,
                    Failure::ResourceLimit | Failure::Timeout | Failure::Cancelled
                ) || effort(check).exhausted()
            })
        {
            Reason::ResourceOnly
        } else {
            Reason::FailureUnattributed
        }
    })?;
    if scope != Scope::Full {
        return Err(Reason::FullConfirmationMissing);
    }
    Ok(Outcome::Rejected(evidence))
}

fn checked(input: &Classification) -> Result<Outcome, Reason> {
    let function = input
        .mutation
        .function
        .as_ref()
        .ok_or(Reason::AmbiguousFunction)?;
    if !function_valid(function)
        || !context_valid(&input.context)
        || !digest(&input.mutation.source_sha256)
        || input.mutation.source_sha256 == input.context.baseline_sha256
        || input.mutation.id.is_empty()
        || input.mutation.before == input.mutation.after
        || input.mutation.start > input.mutation.end
        || input.mutation.end.saturating_sub(input.mutation.start)
            != u64::try_from(input.mutation.before.len()).unwrap_or(u64::MAX)
        || input.mutation.file.as_os_str().is_empty()
        || input
            .mutation
            .file
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
        || !(MIN_REPEATS..=20).contains(&input.repeats)
    {
        return Err(Reason::IdentityMismatch);
    }
    for run in &input.baseline {
        valid_observation(run, &input.context, &input.context.baseline_sha256)?;
    }
    for run in &input.attempts {
        valid_observation(run, &input.context, &input.mutation.source_sha256)?;
    }
    let required = usize::try_from(input.repeats).map_err(|_| Reason::IdentityMismatch)?;
    let baseline: Vec<_> = input
        .baseline
        .iter()
        .filter(|r| r.scope == Scope::Full)
        .collect();
    if baseline.len() < required {
        return Err(Reason::BaselineMissing);
    }
    if !complete(&baseline) || baseline.iter().any(|r| r.unviable) {
        return Err(Reason::BaselineFailed);
    }
    if !stable(&baseline, function) {
        return Err(Reason::BaselineUnstable);
    }
    for run in &baseline {
        headroom(target(run, function)?)?;
    }
    let full: Vec<_> = input
        .attempts
        .iter()
        .filter(|r| r.scope == Scope::Full)
        .collect();
    let scoped: Vec<_> = input
        .attempts
        .iter()
        .filter(|r| r.scope == Scope::Scoped)
        .collect();
    let (attempts, scope) = if full.len() >= required {
        (full, Scope::Full)
    } else if full.is_empty() && scoped.len() >= required {
        (scoped, Scope::Scoped)
    } else {
        return Err(Reason::FullConfirmationMissing);
    };
    if !complete(&attempts) {
        return Err(Reason::Incomplete);
    }
    if !stable(&attempts, function) {
        return Err(Reason::Unstable);
    }
    for run in &attempts {
        if !run.unviable && target(run, function).is_err() {
            return Err(Reason::AmbiguousFunction);
        }
    }
    let first = attempts.first().ok_or(Reason::Incomplete)?;
    let base = baseline.first().ok_or(Reason::BaselineMissing)?;
    let expected = outcome(first, base, function, scope)?;
    for run in attempts.iter().skip(1) {
        if outcome(run, base, function, scope)? != expected {
            return Err(Reason::Unstable);
        }
    }
    Ok(expected)
}

/// Produces one bounded score view with the required evidence breakdown.
/// Omitted and cancelled counts describe explicit runner dispositions, not
/// synthetic proof outcomes. Scoped survivors remain labelled separately.
#[must_use]
pub fn summarize<'a>(
    outcomes: impl IntoIterator<Item = &'a Outcome>,
    omitted: u64,
    cancelled: u64,
) -> Summary {
    let mut summary = Summary {
        omitted,
        cancelled,
        ..Summary::default()
    };
    for outcome in outcomes {
        match outcome {
            Outcome::Rejected(evidence) => {
                summary.rejected = summary.rejected.saturating_add(1);
                match evidence {
                    Detection::CounterexampleConfirmed => {
                        summary.counterexample_confirmed =
                            summary.counterexample_confirmed.saturating_add(1);
                    }
                    Detection::VerifierRejected => {
                        summary.verifier_rejected = summary.verifier_rejected.saturating_add(1);
                    }
                }
            }
            Outcome::Survived => summary.survived = summary.survived.saturating_add(1),
            Outcome::SurvivedScoped => {
                summary.survived_scoped = summary.survived_scoped.saturating_add(1);
            }
            Outcome::Inconclusive(reason) => {
                let n = summary.inconclusive.entry(*reason).or_default();
                *n = n.saturating_add(1);
            }
            Outcome::Unviable => summary.unviable = summary.unviable.saturating_add(1),
            Outcome::Unsupported(_) => summary.unsupported = summary.unsupported.saturating_add(1),
        }
    }
    let denominator = summary
        .rejected
        .saturating_add(summary.survived)
        .saturating_add(summary.survived_scoped);
    if denominator != 0 {
        #[allow(
            clippy::cast_precision_loss,
            reason = "bounded inventory counts are represented as a display ratio, not evidence"
        )]
        {
            summary.score = Some(summary.rejected as f64 / denominator as f64);
        }
    }
    summary
}
