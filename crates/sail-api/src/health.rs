//! Observed per-check proof effort; this view never changes verification verdicts.
//!
//! Missing measurements remain missing. No stability or fragility measurement
//! is inferred from one run, a whole-function timing, or a cache hit count.

use serde::{Deserialize, Serialize};

use crate::output::ObligationReport;

/// Recorded consumption of one check with a finite positive resource limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consumption {
    /// Actual check effort, including values above the requested limit.
    pub used: u64,
    /// Requested finite positive limit, never a whole-function approximation.
    pub limit: u64,
}

impl Consumption {
    /// Compares exact fractions without floating point or u64 multiplication.
    #[must_use]
    pub fn compare(self, other: Self) -> std::cmp::Ordering {
        (u128::from(self.used) * u128::from(other.limit))
            .cmp(&(u128::from(other.used) * u128::from(self.limit)))
    }
}

/// Health measurements for one original function report, not a proof tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionHealth {
    /// An obligation inventory was recorded; false differs from an empty one.
    pub attributed: bool,
    /// Every recorded check has answer, provenance, requested limit and effort.
    /// This says nothing about repeated-run stability or proof success.
    pub complete: bool,
    /// Original checks, including advisory checks.
    pub checks: u64,
    /// Checks with finite positive limits and actual recorded effort.
    pub measured_checks: u64,
    /// Checks whose measured effort exceeds half their finite limit (ADR 0009).
    pub near_limit_checks: u64,
    /// Checks whose original reason-unknown identifies resource exhaustion.
    pub exhausted_checks: u64,
    /// Checks explicitly requesting an unlimited resource budget (limit zero).
    pub unlimited_checks: u64,
    /// Checks missing an answer, source, requested limit or actual effort.
    pub unrecorded_checks: u64,
    /// Highest exact finite check-consumption fraction; absent without one.
    pub peak: Option<Consumption>,
}

impl FunctionHealth {
    /// Derives measurements only from original attributed per-check evidence.
    #[must_use]
    pub fn from_obligations(obligations: Option<&[ObligationReport]>) -> Self {
        let mut health = Self {
            attributed: obligations.is_some(),
            complete: obligations.is_some(),
            checks: 0,
            measured_checks: 0,
            near_limit_checks: 0,
            exhausted_checks: 0,
            unlimited_checks: 0,
            unrecorded_checks: 0,
            peak: None,
        };
        for check in obligations
            .into_iter()
            .flatten()
            .flat_map(|item| &item.checks)
        {
            health.checks = health.checks.saturating_add(1);
            let effort = sail_ledger::Effort {
                limit: check.rlimit_limit,
                used: check.rlimit_used,
                reason: check.reason_unknown.clone(),
            };
            if effort.exhausted() {
                health.exhausted_checks = health.exhausted_checks.saturating_add(1);
            }
            if check.answer.is_none()
                || check.source.is_none()
                || check.rlimit_limit.is_none()
                || check.rlimit_used.is_none()
            {
                health.complete = false;
                health.unrecorded_checks = health.unrecorded_checks.saturating_add(1);
            }
            if check.rlimit_limit == Some(0) {
                health.unlimited_checks = health.unlimited_checks.saturating_add(1);
            }
            if let (Some(used), Some(limit)) = (check.rlimit_used, check.rlimit_limit)
                && limit > 0
            {
                health.measured_checks = health.measured_checks.saturating_add(1);
                if effort.near_limit() {
                    health.near_limit_checks = health.near_limit_checks.saturating_add(1);
                }
                let observed = Consumption { used, limit };
                if health
                    .peak
                    .is_none_or(|peak| observed.compare(peak).is_gt())
                {
                    health.peak = Some(observed);
                }
            }
        }
        health
    }
}

/// One exactly identified function in a retained health view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Cargo package/name/kinds, including same-named library and binary targets.
    pub target: crate::proof::Target,
    /// Whether the original invocation requested this target.
    pub requested: bool,
    /// Original invocation that established this target's report.
    pub establishing_run: String,
    /// Whether this job reused the original complete target report.
    pub reused: bool,
    /// Original qualified compiler identity.
    pub function: String,
    /// Original ordinary verdict, including unknown and provisional.
    pub verdict: crate::output::VerdictName,
    /// Original establishment; a health measurement cannot confirm a proof.
    pub establishment: crate::output::EstablishmentName,
    /// Original ADR 0009 instability flag.
    pub unstable: bool,
    /// Original answer provenance.
    pub provenance: crate::output::ProvenanceReport,
    /// Derived only from this report's original checks.
    pub health: FunctionHealth,
    /// Explicit existing replay measurement, absent when it was not measured.
    pub fragility: Option<crate::output::Fragility>,
}

/// Bounded read-only health view, shared by CLI, JSON and MCP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// Always `sail-proof-health`.
    pub kind: String,
    /// Independent health-view contract version, currently one.
    pub version: u32,
    /// Retained job when read from the service; absent for a result file.
    pub job_id: Option<String>,
    /// Original producing invocation.
    pub run_id: String,
    /// Original workspace.
    pub workspace: String,
    /// Original full configuration identity, for interpreting comparisons.
    pub context_sha256: String,
    /// Original declared input revision.
    pub input_revision: String,
    /// Original result currentness; historical evidence stays historical.
    pub currentness: crate::project::Currentness,
    /// Original project process exit; view generation never replaces it.
    pub exit_code: Option<i32>,
    /// Every requested-target function, including entries omitted by the bound.
    pub total_functions: u64,
    /// Entries omitted from this bounded view, never silently dropped.
    pub omitted: u64,
    /// Ranked by exhaustion, near-limit checks, then exact finite consumption.
    pub entries: Vec<Entry>,
    /// Original infrastructure/setup failure, when present.
    pub error: Option<crate::output::ErrorInfo>,
    /// Explicit immutable baseline name when one was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_name: Option<String>,
    /// Separately labelled earlier evidence, never merged into current answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Box<Self>>,
}

impl View {
    /// Reads existing per-check evidence; no solver is started and no verdict
    /// or provisional establishment is changed. Older v2 reports need no new
    /// measurement: their obligations supply only what was actually retained.
    ///
    /// # Errors
    /// An unsupported document, duplicate identity or limit outside 1..1000.
    pub fn of(result: &crate::project::ProjectResult, limit: usize) -> Result<Self, String> {
        if result.kind != "sail-project-result"
            || result.schema_version != crate::project::SCHEMA_VERSION
            || !(1..=1000).contains(&limit)
        {
            return Err("health requires a supported project result and limit 1..1000".into());
        }
        let mut entries = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut targets = std::collections::BTreeSet::new();
        for target in result.targets.iter().filter(|target| target.requested) {
            if target.report.schema_version != crate::output::SCHEMA_VERSION {
                return Err("health does not support this embedded report version".into());
            }
            let identity = crate::proof::Target {
                package_id: target.package_id.clone(),
                name: target.target.clone(),
                kind: target.kind.clone(),
            };
            if !targets.insert(identity.clone()) {
                return Err("health cannot merge duplicate target identities".into());
            }
            for function in target.report.crates.iter().flat_map(|item| &item.functions) {
                if !seen.insert((identity.clone(), &function.function)) {
                    return Err("health cannot merge duplicate target/function identities".into());
                }
                entries.push(Entry {
                    target: identity.clone(),
                    requested: target.requested,
                    establishing_run: target.establishing_run.clone(),
                    reused: target.reused,
                    function: function.function.clone(),
                    verdict: function.verdict,
                    establishment: function.establishment,
                    unstable: function.unstable,
                    provenance: function.provenance.clone(),
                    health: FunctionHealth::from_obligations(function.obligations.as_deref()),
                    fragility: function.fragility,
                });
            }
        }
        entries.sort_by(compare_entries);
        let total_functions = u64::try_from(entries.len()).map_err(|error| error.to_string())?;
        entries.truncate(limit);
        let displayed = u64::try_from(entries.len()).map_err(|error| error.to_string())?;
        Ok(Self {
            kind: "sail-proof-health".into(),
            version: 1,
            job_id: None,
            run_id: result.run_id.clone(),
            workspace: result.workspace.clone(),
            context_sha256: result.context_sha256.clone(),
            input_revision: result.input_revision.clone(),
            currentness: result.currentness,
            exit_code: result.exit_code,
            total_functions,
            omitted: total_functions - displayed,
            entries,
            error: result.error.clone(),
            baseline_name: None,
            baseline: None,
        })
    }
}

fn compare_entries(left: &Entry, right: &Entry) -> std::cmp::Ordering {
    right
        .health
        .exhausted_checks
        .cmp(&left.health.exhausted_checks)
        .then_with(|| {
            right
                .health
                .near_limit_checks
                .cmp(&left.health.near_limit_checks)
        })
        .then_with(|| match (left.health.peak, right.health.peak) {
            (Some(left), Some(right)) => right.compare(left),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, None) => std::cmp::Ordering::Equal,
        })
        .then_with(|| (&left.target, &left.function).cmp(&(&right.target, &right.function)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{AnswerName, CheckReport, OutcomeName, SourceName};

    fn obligation(checks: Vec<CheckReport>) -> ObligationReport {
        ObligationReport {
            id: "demo::f#Function-Def/0".into(),
            kind: "Function-Def".into(),
            ordinal: 0,
            spinoff: None,
            advisory: false,
            span: None,
            outcome: OutcomeName::Proved,
            unstable: false,
            checks,
            counterexample: None,
        }
    }

    fn check(used: Option<u64>, limit: Option<u64>) -> CheckReport {
        CheckReport {
            answer: Some(AnswerName::Unsat),
            rlimit_limit: limit,
            rlimit_used: used,
            reason_unknown: None,
            source: Some(SourceName::Solved),
        }
    }

    #[test]
    fn exact_half_threshold_and_fraction_order_survive_u64_boundaries() {
        let maximum = u64::MAX;
        let obligations = [obligation(vec![
            check(Some(maximum / 2), Some(maximum)),
            check(Some(maximum / 2 + 1), Some(maximum)),
            check(Some(1), Some(2)),
            check(Some(2), Some(3)),
        ])];
        let health = FunctionHealth::from_obligations(Some(&obligations));
        assert_eq!(health.near_limit_checks, 2);
        assert_eq!(health.peak, Some(Consumption { used: 2, limit: 3 }));
        assert!(health.complete);
        assert_eq!(
            Consumption {
                used: maximum,
                limit: maximum - 1
            }
            .compare(Consumption {
                used: maximum - 1,
                limit: maximum
            }),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn missing_unlimited_and_empty_measurements_are_distinct() {
        let absent = FunctionHealth::from_obligations(None);
        let empty = FunctionHealth::from_obligations(Some(&[]));
        assert!(!absent.attributed);
        assert!(!absent.complete);
        assert!(empty.attributed);
        assert!(empty.complete);
        let mut exhausted = check(Some(5), Some(10));
        exhausted.answer = Some(AnswerName::Unknown);
        exhausted.reason_unknown = Some("(:reason-unknown \"timeout\")".into());
        let obligations = [obligation(vec![
            check(Some(100), Some(0)),
            check(None, Some(20)),
            check(Some(0), None),
            exhausted,
        ])];
        let health = FunctionHealth::from_obligations(Some(&obligations));
        assert!(!health.complete);
        assert_eq!(health.checks, 4);
        assert_eq!(health.unrecorded_checks, 2);
        assert_eq!(health.unlimited_checks, 1);
        assert_eq!(health.measured_checks, 1);
        assert_eq!(health.exhausted_checks, 1);
        assert_eq!(health.near_limit_checks, 0);
        assert_eq!(health.peak, Some(Consumption { used: 5, limit: 10 }));
    }
}
