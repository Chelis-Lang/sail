//! The M7 repair acceptance gate, applied to recorded samples at report time.
//!
//! Per catalogue (one project), the gate passes only when:
//! 1. every catalogue cycle has an M6 and an M7 sample in at least the
//!    catalogue's minimum number of rounds (`SAIL7108`);
//! 2. no measured sample has a violation ([`violations`]);
//! 3. each round's M6 and M7 full outcomes are identical (`SAIL7107`);
//! 4. the median over complete rounds of the M6 whole-loop total, divided
//!    by the same median for M7, is at least the catalogue minimum
//!    (`SAIL7109`). A whole-loop total is the sum of a round's cycle wall
//!    times for one arm.
//!
//! Warm-ups are never judged. A failing sample is reported, never dropped.

use std::collections::BTreeSet;

use serde::Serialize;

use super::{Arm, RepairFile, Sample};
use crate::cold::median;

/// A protocol or correctness refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    /// Stable code.
    pub code: &'static str,
    /// What was observed.
    pub message: String,
}

fn v(code: &'static str, message: impl Into<String>) -> Violation {
    Violation {
        code,
        message: message.into(),
    }
}

/// Every violation of one measured sample, in a stable order.
#[must_use]
pub fn violations(s: &Sample) -> Vec<Violation> {
    let mut out = Vec::new();
    for p in &s.phases {
        if p.timed_out {
            out.push(v(
                "SAIL7101",
                format!("{} exceeded the harness deadline", p.name),
            ));
        } else if let Some(e) = &p.error {
            out.push(v("SAIL7101", format!("{}: {e}", p.name)));
        } else if p.exit_code.is_none() {
            out.push(v("SAIL7101", format!("{} has no exit code", p.name)));
        }
    }
    if s.phases.is_empty() {
        out.push(v("SAIL7101", "no invocation was recorded"));
    }
    let f = &s.facts;
    if f.full_currentness.as_deref() != Some("current") {
        out.push(v(
            "SAIL7102",
            format!(
                "full evidence is {}, not current",
                f.full_currentness.as_deref().unwrap_or("unavailable")
            ),
        ));
    }
    let want_verified = s.expected == "verified";
    if s.arm == Some(Arm::M7) {
        if f.compiler_invocations != Some(1) {
            out.push(v(
                "SAIL7103",
                format!(
                    "{} compiler drivers, not one",
                    f.compiler_invocations
                        .map_or_else(|| "unreported".to_owned(), |n| n.to_string())
                ),
            ));
        }
        let mut batch = Vec::new();
        if f.batched != Some(true) {
            batch.push("not batched");
        }
        if f.engine_manifest != Some(true) {
            batch.push("no engine manifest");
        }
        if f.focus_present != Some(true) {
            batch.push("no focused evidence");
        }
        if f.selection_error.is_some() {
            batch.push("focus selection error");
        }
        if !batch.is_empty() {
            out.push(v(
                "SAIL7104",
                format!("not a real source-engine batch: {}", batch.join(", ")),
            ));
        }
        match &s.preparation {
            None => out.push(v("SAIL7105", "preparation telemetry unavailable")),
            Some(p) if !p.enabled => out.push(v("SAIL7105", "preparation reuse not enabled")),
            Some(p) if !p.within_budget => {
                out.push(v("SAIL7105", "preparation outside the accepted budget"));
            }
            Some(_) => {}
        }
        if f.accepted != Some(want_verified) {
            out.push(v(
                "SAIL7106",
                format!(
                    "repair accepted={}, catalogue expects {}",
                    f.accepted
                        .map_or_else(|| "unreported".to_owned(), |a| a.to_string()),
                    s.expected
                ),
            ));
        }
    } else if f.verified != Some(want_verified) {
        out.push(v(
            "SAIL7106",
            format!(
                "full verification {}, catalogue expects {}",
                match f.verified {
                    Some(true) => "verified",
                    Some(false) => "failed",
                    None => "unavailable",
                },
                s.expected
            ),
        ));
    }
    if f.outcome_sha256.is_none() {
        out.push(v(
            "SAIL7106",
            format!(
                "no comparable full outcome: {}",
                f.outcome_error.as_deref().unwrap_or("unknown")
            ),
        ));
    }
    out
}

/// A measured sample that fails the gate.
#[derive(Debug, Clone, Serialize)]
pub struct Failing {
    /// Round.
    pub round: usize,
    /// Cycle id.
    pub cycle: String,
    /// Arm.
    pub arm: Arm,
    /// Its violations.
    pub violations: Vec<Violation>,
}

/// The gate's verdict for one catalogue.
#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    /// Catalogue project id.
    pub project: String,
    /// Whether the gate passed.
    pub passed: bool,
    /// Why not, one line per reason.
    pub reasons: Vec<String>,
    /// Rounds with an M6 and M7 sample for every cycle.
    pub complete_rounds: usize,
    /// Per-round whole-loop totals (`None` for an incomplete round).
    pub m6_round_totals: Vec<Option<f64>>,
    /// As for M6.
    pub m7_round_totals: Vec<Option<f64>>,
    /// Median of complete M6 round totals.
    pub m6_median_ms: Option<f64>,
    /// Median of complete M7 round totals.
    pub m7_median_ms: Option<f64>,
    /// `m6_median_ms / m7_median_ms`.
    pub speedup: Option<f64>,
    /// Failing measured samples.
    pub failing: Vec<Failing>,
    /// `(round, cycle)` pairs whose full outcomes differ or are missing.
    pub differing: Vec<(usize, String)>,
}

fn sample<'a>(
    f: &'a RepairFile,
    project: &str,
    round: usize,
    cycle: &str,
    arm: Arm,
) -> Option<&'a Sample> {
    f.samples.iter().find(|s| {
        s.project == project && s.round == Some(round) && s.cycle == cycle && s.arm == Some(arm)
    })
}

/// Judges every catalogue project recorded in `f`.
#[must_use]
pub fn judge(f: &RepairFile) -> Vec<Verdict> {
    f.catalogue
        .iter()
        .map(|p| {
            let cycles: Vec<&str> = p.cycles.iter().map(String::as_str).collect();
            judge_project(
                f,
                &p.project,
                &cycles,
                f.minimum_rounds,
                f.minimum_median_speedup,
            )
        })
        .collect()
}

/// Per-round totals, failing samples and differing pairs of one catalogue.
struct Tally {
    totals: [Vec<Option<f64>>; 2],
    complete_rounds: usize,
    failing: Vec<Failing>,
    differing: Vec<(usize, String)>,
}

fn tally(f: &RepairFile, project: &str, cycles: &[&str]) -> Tally {
    let rounds = f
        .samples
        .iter()
        .filter(|s| s.project == project)
        .filter_map(|s| s.round)
        .max()
        .map_or(0, |r| r + 1);
    let mut failing = Vec::new();
    let mut differing = Vec::new();
    let mut totals = [Vec::new(), Vec::new()];
    let mut complete_rounds = 0;
    for round in 0..rounds {
        let mut round_total = [Some(0.0f64), Some(0.0f64)];
        for &cycle in cycles {
            let pair = [
                sample(f, project, round, cycle, Arm::M6),
                sample(f, project, round, cycle, Arm::M7),
            ];
            for (i, arm) in Arm::BOTH.into_iter().enumerate() {
                let (Some(slot), Some(s)) = (round_total.get_mut(i), pair.get(i)) else {
                    continue;
                };
                match s {
                    Some(s) => {
                        *slot = slot.map(|t| t + s.wall_ms);
                        let found = violations(s);
                        if !found.is_empty() {
                            failing.push(Failing {
                                round,
                                cycle: cycle.to_owned(),
                                arm,
                                violations: found,
                            });
                        }
                    }
                    None => *slot = None,
                }
            }
            if let [Some(a), Some(b)] = pair
                && (a.facts.outcome_sha256.is_none()
                    || a.facts.outcome_sha256 != b.facts.outcome_sha256)
            {
                differing.push((round, cycle.to_owned()));
            }
        }
        if round_total.iter().all(Option::is_some) {
            complete_rounds += 1;
        }
        for (i, t) in round_total.into_iter().enumerate() {
            if let Some(list) = totals.get_mut(i) {
                list.push(t);
            }
        }
    }
    Tally {
        totals,
        complete_rounds,
        failing,
        differing,
    }
}

/// Judges one catalogue given its cycle ids (in catalogue order).
#[must_use]
pub fn judge_project(
    f: &RepairFile,
    project: &str,
    cycles: &[&str],
    minimum_rounds: usize,
    minimum_speedup: f64,
) -> Verdict {
    let mut reasons = Vec::new();
    let Tally {
        totals: [m6_round_totals, m7_round_totals],
        complete_rounds,
        failing,
        differing,
    } = tally(f, project, cycles);
    let complete = |xs: &[Option<f64>], other: &[Option<f64>]| -> Vec<f64> {
        xs.iter()
            .zip(other)
            .filter_map(|(a, b)| a.and(*b).and(*a))
            .collect()
    };
    let m6_median_ms = median(&complete(&m6_round_totals, &m7_round_totals));
    let m7_median_ms = median(&complete(&m7_round_totals, &m6_round_totals));
    let speedup = m6_median_ms
        .zip(m7_median_ms)
        .filter(|(_, b)| *b > 0.0)
        .map(|(a, b)| a / b);
    let present: BTreeSet<&str> = f
        .samples
        .iter()
        .filter(|s| s.project == project)
        .map(|s| s.cycle.as_str())
        .collect();
    let missing: Vec<&str> = cycles
        .iter()
        .copied()
        .filter(|c| !present.contains(c))
        .collect();
    if complete_rounds < minimum_rounds {
        reasons.push(format!(
            "SAIL7108: {complete_rounds} complete round(s) of {} cycles, {minimum_rounds} required",
            cycles.len()
        ));
    }
    if !missing.is_empty() {
        reasons.push(format!(
            "SAIL7108: cycles never measured: {}",
            missing.join(", ")
        ));
    }
    if !failing.is_empty() {
        reasons.push(format!("{} failing measured sample(s)", failing.len()));
    }
    if !differing.is_empty() {
        reasons.push(format!(
            "SAIL7107: {} pair(s) with differing or missing M6/M7 full outcomes",
            differing.len()
        ));
    }
    match speedup {
        Some(x) if x >= minimum_speedup => {}
        Some(x) => reasons.push(format!(
            "SAIL7109: median whole-loop speedup {x:.2}× is below {minimum_speedup}×"
        )),
        None => reasons.push("SAIL7109: no complete round to compute a speedup".to_owned()),
    }
    Verdict {
        project: project.to_owned(),
        passed: reasons.is_empty(),
        reasons,
        complete_rounds,
        m6_round_totals,
        m7_round_totals,
        m6_median_ms,
        m7_median_ms,
        speedup,
        failing,
        differing,
    }
}

/// Nearest-rank 95th percentile.
#[must_use]
pub fn p95(xs: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = xs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let rank = (v.len() * 95).div_ceil(100);
    v.get(rank.saturating_sub(1)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repair::{Facts, Phase, Preparation};

    fn good(project: &str, round: usize, cycle: &str, arm: Arm, wall: f64) -> Sample {
        Sample {
            project: project.to_owned(),
            cycle: cycle.to_owned(),
            kind: "body".to_owned(),
            expected: "verified".to_owned(),
            arm: Some(arm),
            round: Some(round),
            phases: vec![Phase {
                name: "x".to_owned(),
                exit_code: Some(0),
                wall_ms: wall,
                ..Phase::default()
            }],
            wall_ms: wall,
            facts: Facts {
                full_currentness: Some("current".to_owned()),
                accepted: (arm == Arm::M7).then_some(true),
                batched: Some(true),
                engine_manifest: Some(true),
                focus_present: Some(true),
                compiler_invocations: Some(1),
                outcome_sha256: Some("a".to_owned()),
                verified: Some(true),
                ..Facts::default()
            },
            preparation: Some(Preparation {
                drivers: 1,
                enabled: true,
                within_budget: true,
                ..Preparation::default()
            }),
            ..Sample::default()
        }
    }

    fn file(rounds: usize, cycles: &[&str], m6: f64, m7: f64) -> RepairFile {
        let mut f = RepairFile::default();
        for r in 0..rounds {
            for c in cycles {
                f.samples.push(good("p", r, c, Arm::M6, m6));
                f.samples.push(good("p", r, c, Arm::M7, m7));
            }
        }
        f
    }

    #[test]
    fn complete_fast_run_passes() {
        let f = file(3, &["a", "b"], 3000.0, 1000.0);
        let v = judge_project(&f, "p", &["a", "b"], 3, 2.0);
        assert!(v.passed, "{:?}", v.reasons);
        assert_eq!(v.complete_rounds, 3);
        assert_eq!(v.m6_median_ms, Some(6000.0));
        assert_eq!(v.speedup, Some(3.0));
    }

    #[test]
    fn slow_or_short_runs_fail() {
        let f = file(3, &["a"], 1500.0, 1000.0);
        let v = judge_project(&f, "p", &["a"], 3, 2.0);
        assert!(!v.passed);
        assert!(v.reasons.iter().any(|r| r.starts_with("SAIL7109")));
        let f = file(1, &["a"], 3000.0, 1000.0);
        let v = judge_project(&f, "p", &["a"], 3, 2.0);
        assert!(!v.passed);
        assert!(v.reasons.iter().any(|r| r.starts_with("SAIL7108")));
        let v = judge_project(&RepairFile::default(), "p", &["a"], 3, 2.0);
        assert!(!v.passed && v.speedup.is_none());
    }

    #[test]
    fn a_missing_cycle_or_arm_never_passes() {
        let mut f = file(3, &["a", "b"], 3000.0, 1000.0);
        f.samples
            .retain(|s| !(s.cycle == "b" && s.round == Some(1) && s.arm == Some(Arm::M7)));
        let v = judge_project(&f, "p", &["a", "b"], 3, 2.0);
        assert!(!v.passed);
        assert_eq!(v.complete_rounds, 2);
        assert_eq!(v.m7_round_totals.get(1), Some(&None));
        let f = file(3, &["a"], 3000.0, 1000.0);
        let v = judge_project(&f, "p", &["a", "b"], 3, 2.0);
        assert!(!v.passed && v.reasons.iter().any(|r| r.contains("never measured: b")));
    }

    #[test]
    fn a_fast_violating_m7_sample_is_kept_and_fails() {
        let mut f = file(3, &["a"], 3000.0, 1000.0);
        if let Some(s) = f.samples.iter_mut().find(|s| s.arm == Some(Arm::M7)) {
            s.facts.compiler_invocations = Some(2);
            s.facts.full_currentness = Some("historical".to_owned());
        }
        let v = judge_project(&f, "p", &["a"], 3, 2.0);
        assert!(!v.passed);
        assert_eq!(v.failing.len(), 1);
        let codes: Vec<&str> = v
            .failing
            .iter()
            .flat_map(|x| &x.violations)
            .map(|x| x.code)
            .collect();
        assert_eq!(codes, ["SAIL7102", "SAIL7103"]);
        // Its time still counts: totals are over every recorded sample.
        assert_eq!(v.m7_median_ms, Some(1000.0));
    }

    #[test]
    fn differing_outcomes_fail() {
        let mut f = file(3, &["a"], 3000.0, 1000.0);
        if let Some(s) = f.samples.iter_mut().find(|s| s.arm == Some(Arm::M6)) {
            s.facts.outcome_sha256 = Some("b".to_owned());
        }
        let v = judge_project(&f, "p", &["a"], 3, 2.0);
        assert!(!v.passed && v.differing.len() == 1);
    }

    #[test]
    fn sample_violations() {
        let mut s = good("p", 0, "a", Arm::M7, 1.0);
        assert_eq!(violations(&s), Vec::<Violation>::new());
        s.preparation = None;
        s.facts.batched = Some(false);
        s.phases[0].timed_out = true;
        s.facts.accepted = Some(false);
        let codes: Vec<&str> = violations(&s).iter().map(|x| x.code).collect();
        assert_eq!(codes, ["SAIL7101", "SAIL7104", "SAIL7105", "SAIL7106"]);
        let mut m6 = good("p", 0, "a", Arm::M6, 1.0);
        m6.expected = "errors".to_owned();
        let codes: Vec<&str> = violations(&m6).iter().map(|x| x.code).collect();
        assert_eq!(codes, ["SAIL7106"]);
        m6.phases.clear();
        assert_eq!(violations(&m6).first().map(|x| x.code), Some("SAIL7101"));
    }

    #[test]
    fn nearest_rank_p95() {
        assert_eq!(p95(&[]), None);
        assert_eq!(p95(&[5.0]), Some(5.0));
        let xs: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(p95(&xs), Some(19.0));
        assert_eq!(p95(&[1.0, 2.0, 3.0]), Some(3.0));
    }
}
