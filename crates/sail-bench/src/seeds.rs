//! `sail-bench seeds`: answer and effort stability across random seeds.
//!
//! Every captured session is replayed whole into a fresh solver once per
//! seed. Seed 0 is the unmodified session (the baseline, and Z3's default
//! seed). For seed `s > 0`, `(set-option :smt.random_seed s)` and
//! `(set-option :sat.random_seed s)` are inserted immediately before the
//! first command that is not a `set-option`, that is just after Verus's
//! option prelude. Replies are paired with commands as in
//! [`crate::transcript`], and each check's answer and `rlimit-count` delta
//! are compared across seeds.
//!
//! The seed-0 replay is also compared with the recorded session: any
//! difference in answer, follow-up reply bytes or `rlimit-count` delta is
//! evidence of nondeterminism in the solver or in the replay.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use crate::solver::{self, Identity, Outcome, Solver};
use crate::stats::{FloatDistribution, as_f64, coefficient_of_variation};
use crate::trace::SessionFiles;
use crate::transcript::{CheckObservation, Transcript, split_commands};

pub use sail_portfolio::config::inject_seed;

/// One replay of one session under one seed.
#[derive(Debug, Clone)]
pub struct SeedRun {
    /// The seed.
    pub seed: u64,
    /// Per-check observations, in order.
    pub checks: Vec<CheckObservation>,
    /// Batches that could not be paired.
    pub unpaired_batches: usize,
    /// Solver error replies.
    pub errors: Vec<String>,
    /// `None` if the solver exited by itself; otherwise what went wrong
    /// (`infra-timeout`, or a start failure).
    pub infra: Option<String>,
    /// Wall-clock milliseconds.
    pub elapsed_ms: u64,
}

/// Replays `files` under `seed`.
#[must_use]
pub fn replay(solver: &Solver, files: &SessionFiles, seed: u64) -> SeedRun {
    let mut run = SeedRun {
        seed,
        checks: Vec::new(),
        unpaired_batches: 0,
        errors: Vec::new(),
        infra: None,
        elapsed_ms: 0,
    };
    let input = match std::fs::read(&files.input) {
        Ok(b) => b,
        Err(e) => {
            run.infra = Some(format!("read {}: {e}", files.input.display()));
            return run;
        }
    };
    let commands = split_commands(&input);
    let input = inject_seed(&input, &commands, seed);
    let commands = if seed == 0 {
        commands
    } else {
        split_commands(&input)
    };
    match solver::run(solver, &input) {
        Ok(r) => {
            run.elapsed_ms = u64::try_from(r.elapsed.as_millis()).unwrap_or(u64::MAX);
            if r.outcome == Outcome::InfraTimeout {
                run.infra = Some("infra-timeout".to_owned());
            }
            let t = Transcript::from_parts(commands, &crate::transcript::split_replies(&r.stdout));
            run.checks = t.checks();
            run.unpaired_batches = t.unpaired_batches;
            run.errors = t.errors;
        }
        Err(e) => run.infra = Some(e.to_string()),
    }
    run
}

/// Text of an optional reply.
fn text(b: Option<&[u8]>) -> Option<String> {
    b.map(|b| String::from_utf8_lossy(b).into_owned())
}

/// One check across all seeds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckSeeds {
    /// Session id.
    pub id: String,
    /// Check index in the session.
    pub seq: usize,
    /// The recorded (in-session) answer.
    pub recorded: Option<String>,
    /// The answer under each seed, in seed order.
    pub answers: Vec<Option<String>>,
    /// The `:reason-unknown` reply under each seed, where there was one.
    pub reasons: Vec<Option<String>>,
    /// `rlimit-count` delta under each seed.
    pub rlimit: Vec<Option<u64>>,
    /// Whether every seed gave an answer and they were all equal. `None`
    /// if some seed has no answer.
    pub stable: Option<bool>,
    /// Whether "unsat" versus "not unsat" (the verdict) was the same for
    /// every seed. `None` if some seed has no answer.
    pub verdict_stable: Option<bool>,
    /// Coefficient of variation of the rlimit deltas, when every seed has
    /// one.
    pub rlimit_cv: Option<f64>,
    /// Largest delta over smallest, when every seed has one.
    pub rlimit_ratio: Option<f64>,
}

/// A difference between the seed-0 replay and the recorded session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seed0Difference {
    /// Session id.
    pub id: String,
    /// Check index.
    pub seq: usize,
    /// What differs: `answer`, `followup` or `rlimit`.
    pub what: String,
    /// The recorded value (truncated).
    pub recorded: Option<String>,
    /// The replayed value (truncated).
    pub replayed: Option<String>,
}

/// Seed-0 replay versus the recorded session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Seed0Report {
    /// Checks where both had an answer.
    pub answers_compared: usize,
    /// Of those, how many differ.
    pub answer_mismatches: usize,
    /// Follow-up replies compared (both paired).
    pub followups_compared: usize,
    /// Of those, how many differ in any byte.
    pub followup_mismatches: usize,
    /// Checks where both had an rlimit delta.
    pub rlimit_compared: usize,
    /// Of those, how many differ.
    pub rlimit_mismatches: usize,
    /// Sessions whose replay had a different number of checks.
    pub check_count_mismatches: usize,
    /// Every difference (up to the example limit).
    pub differences: Vec<Seed0Difference>,
}

/// The `seeds` report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeedsReport {
    /// The solver replayed against.
    pub solver: Identity,
    /// Seeds used; 0 is the unmodified session.
    pub seeds: Vec<u64>,
    /// Sessions replayed.
    pub sessions: usize,
    /// Checks seen in the recorded sessions.
    pub checks: usize,
    /// Checks with an answer under every seed.
    pub complete_checks: usize,
    /// Of those, checks whose answer differs between seeds.
    pub unstable: usize,
    /// `unstable / complete_checks`.
    pub unstable_fraction: Option<f64>,
    /// Checks whose verdict (unsat versus not) differs between seeds.
    pub verdict_unstable: usize,
    /// Answer counts per seed.
    pub answers_by_seed: BTreeMap<u64, BTreeMap<String, usize>>,
    /// `:reason-unknown` replies seen, with counts.
    pub reasons_unknown: BTreeMap<String, usize>,
    /// Distribution of per-check rlimit coefficient of variation.
    pub rlimit_cv: FloatDistribution,
    /// Distribution of per-check max/min rlimit ratio.
    pub rlimit_ratio: FloatDistribution,
    /// Seed-0 replay versus the recording.
    pub seed0: Seed0Report,
    /// Runs killed by the safety-net timeout, as `id@seed` (not solver
    /// answers).
    pub infra_timeouts: Vec<String>,
    /// Runs that failed to start or read, as `id@seed: why`.
    pub infra_errors: Vec<String>,
    /// Runs with unpaired batches or solver errors, as `id@seed`.
    pub unpaired_runs: Vec<String>,
    /// Total replay wall-clock time, in milliseconds.
    pub replay_ms: u64,
    /// Checks reported individually: every unstable check, then the
    /// highest rlimit CV, or every check with `--all-checks`.
    pub notable: Vec<CheckSeeds>,
}

fn truncate(s: &[u8], limit: usize) -> String {
    let t = String::from_utf8_lossy(s.get(..limit.min(s.len())).unwrap_or_default()).into_owned();
    if s.len() > limit {
        format!("{t}…")
    } else {
        t
    }
}

fn is_unsat(a: Option<&String>) -> Option<bool> {
    a.map(|a| a == "unsat")
}

/// Compares the seed-0 replay with the recorded session.
fn compare_seed0(
    id: &str,
    recorded: &[CheckObservation],
    runs: &[SeedRun],
    s0: &mut Seed0Report,
    limit: usize,
) {
    // Seed 0 versus the recording.
    if let Some(base) = runs.iter().find(|r| r.seed == 0 && r.infra.is_none()) {
        if base.checks.len() != recorded.len() {
            s0.check_count_mismatches += 1;
        }
        let mut diff = |seq: usize, what: &str, rec: Option<String>, rep: Option<String>| {
            if s0.differences.len() < limit {
                s0.differences.push(Seed0Difference {
                    id: id.to_owned(),
                    seq,
                    what: what.to_owned(),
                    recorded: rec,
                    replayed: rep,
                });
            }
        };
        for (seq, (rec, rep)) in recorded.iter().zip(&base.checks).enumerate() {
            if let (Some(a), Some(b)) = (&rec.answer, &rep.answer) {
                s0.answers_compared += 1;
                if a != b {
                    s0.answer_mismatches += 1;
                    diff(seq, "answer", Some(truncate(a, 40)), Some(truncate(b, 40)));
                }
            }
            for (fa, fb) in rec.followups.iter().zip(&rep.followups) {
                if let (Some(a), Some(b)) = (&fa.reply, &fb.reply) {
                    s0.followups_compared += 1;
                    if a != b {
                        s0.followup_mismatches += 1;
                        diff(
                            seq,
                            "followup",
                            Some(truncate(a, 300)),
                            Some(truncate(b, 300)),
                        );
                    }
                }
            }
            if let (Some(a), Some(b)) = (rec.rlimit_delta, rep.rlimit_delta) {
                s0.rlimit_compared += 1;
                if a != b {
                    s0.rlimit_mismatches += 1;
                    diff(seq, "rlimit", Some(a.to_string()), Some(b.to_string()));
                }
            }
        }
    }
}

/// Analyzes one session: its recording and its runs (in seed order).
fn analyze_session(
    id: &str,
    recorded: &[CheckObservation],
    runs: &[SeedRun],
    report: &mut SeedsReport,
    limit: usize,
) -> Vec<CheckSeeds> {
    let mut out = Vec::new();
    compare_seed0(id, recorded, runs, &mut report.seed0, limit);
    for (seq, rec) in recorded.iter().enumerate() {
        let obs: Vec<Option<&CheckObservation>> = runs
            .iter()
            .map(|r| r.checks.get(seq).filter(|_| r.infra.is_none()))
            .collect();
        let answers: Vec<Option<String>> = obs
            .iter()
            .map(|o| o.and_then(|o| text(o.answer.as_deref())))
            .collect();
        let reasons: Vec<Option<String>> = obs
            .iter()
            .map(|o| o.and_then(|o| text(o.reason_unknown())))
            .collect();
        let rlimit: Vec<Option<u64>> = obs.iter().map(|o| o.and_then(|o| o.rlimit_delta)).collect();
        let complete = answers.iter().all(Option::is_some);
        let stable = complete.then(|| answers.windows(2).all(|w| w[0] == w[1]));
        let verdicts: Vec<Option<bool>> = answers.iter().map(|a| is_unsat(a.as_ref())).collect();
        let verdict_stable = complete.then(|| verdicts.windows(2).all(|w| w[0] == w[1]));
        let all_rlimit: Option<Vec<u64>> = rlimit.iter().copied().collect();
        let rlimit_cv = all_rlimit.as_deref().and_then(coefficient_of_variation);
        let rlimit_ratio = all_rlimit.as_deref().and_then(|v| {
            let min = *v.iter().min()?;
            let max = *v.iter().max()?;
            (min > 0).then(|| as_f64(max) / as_f64(min))
        });
        for (run, answer) in runs.iter().zip(&answers) {
            let key = answer.clone().unwrap_or_else(|| "(none)".to_owned());
            *report
                .answers_by_seed
                .entry(run.seed)
                .or_default()
                .entry(key)
                .or_insert(0) += 1;
        }
        for r in reasons.iter().flatten() {
            *report.reasons_unknown.entry(r.clone()).or_insert(0) += 1;
        }
        report.checks += 1;
        if complete {
            report.complete_checks += 1;
        }
        if stable == Some(false) {
            report.unstable += 1;
        }
        if verdict_stable == Some(false) {
            report.verdict_unstable += 1;
        }
        out.push(CheckSeeds {
            id: id.to_owned(),
            seq,
            recorded: text(rec.answer.as_deref()),
            answers,
            reasons,
            rlimit,
            stable,
            verdict_stable,
            rlimit_cv,
            rlimit_ratio,
        });
    }
    out
}

/// Options for [`seeds`].
#[derive(Debug, Clone)]
pub struct SeedsOptions {
    /// Number of seeds, including the baseline seed 0.
    pub seeds: u64,
    /// Worker threads.
    pub jobs: usize,
    /// How many examples and notable checks to keep.
    pub examples: usize,
    /// Report every check, not only notable ones.
    pub all_checks: bool,
}

/// Runs the seed-stability experiment over `sessions`.
#[must_use]
pub fn seeds(solver: &Solver, sessions: &[SessionFiles], options: &SeedsOptions) -> SeedsReport {
    let seeds: Vec<u64> = (0..options.seeds.max(1)).collect();
    let units: Vec<(usize, u64)> = (0..sessions.len())
        .flat_map(|s| seeds.iter().map(move |&seed| (s, seed)))
        .collect();
    let total = units.len();
    let runs = crate::parallel::map(
        &units,
        options.jobs,
        |&(s, seed)| sessions.get(s).map(|files| replay(solver, files, seed)),
        |done| crate::progress("seeds", done, total),
    );
    let mut report = SeedsReport {
        solver: solver.identity(),
        seeds: seeds.clone(),
        sessions: sessions.len(),
        checks: 0,
        complete_checks: 0,
        unstable: 0,
        unstable_fraction: None,
        verdict_unstable: 0,
        answers_by_seed: BTreeMap::new(),
        reasons_unknown: BTreeMap::new(),
        rlimit_cv: FloatDistribution::of(&[]),
        rlimit_ratio: FloatDistribution::of(&[]),
        seed0: Seed0Report::default(),
        infra_timeouts: Vec::new(),
        infra_errors: Vec::new(),
        unpaired_runs: Vec::new(),
        replay_ms: 0,
        notable: Vec::new(),
    };
    let mut all = Vec::new();
    let mut by_session: Vec<Vec<SeedRun>> = vec![Vec::new(); sessions.len()];
    for ((s, _), run) in units.iter().zip(runs) {
        if let (Some(run), Some(slot)) = (run, by_session.get_mut(*s)) {
            slot.push(run);
        }
    }
    for (files, runs) in sessions.iter().zip(&by_session) {
        for r in runs {
            report.replay_ms += r.elapsed_ms;
            let tag = format!("{}@{}", files.id, r.seed);
            match r.infra.as_deref() {
                Some("infra-timeout") => report.infra_timeouts.push(tag.clone()),
                Some(why) => report.infra_errors.push(format!("{tag}: {why}")),
                None => {}
            }
            if r.unpaired_batches > 0 || !r.errors.is_empty() {
                report.unpaired_runs.push(tag);
            }
        }
        let (Ok(input), Ok(output)) = (std::fs::read(&files.input), std::fs::read(&files.output))
        else {
            report
                .infra_errors
                .push(format!("{}: cannot read the recorded transcript", files.id));
            continue;
        };
        let recorded = Transcript::new(&input, &output).checks();
        all.extend(analyze_session(
            &files.id,
            &recorded,
            runs,
            &mut report,
            options.examples,
        ));
    }
    report.unstable_fraction = (report.complete_checks > 0)
        .then(|| as_f64(report.unstable as u64) / as_f64(report.complete_checks as u64));
    let cvs: Vec<f64> = all.iter().filter_map(|c| c.rlimit_cv).collect();
    report.rlimit_cv = FloatDistribution::of(&cvs);
    let ratios: Vec<f64> = all.iter().filter_map(|c| c.rlimit_ratio).collect();
    report.rlimit_ratio = FloatDistribution::of(&ratios);
    if options.all_checks {
        report.notable = all;
    } else {
        let mut notable: Vec<CheckSeeds> = all
            .iter()
            .filter(|c| c.stable == Some(false))
            .cloned()
            .collect();
        let mut by_cv: Vec<&CheckSeeds> = all
            .iter()
            .filter(|c| c.stable != Some(false) && c.rlimit_cv.is_some())
            .collect();
        by_cv.sort_by(|a, b| {
            b.rlimit_cv
                .unwrap_or_default()
                .total_cmp(&a.rlimit_cv.unwrap_or_default())
        });
        notable.extend(by_cv.into_iter().take(options.examples).cloned());
        report.notable = notable;
    }
    report
}

fn fmt_f(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |v| format!("{v:.3}"))
}

/// A human-readable summary.
#[must_use]
pub fn render(r: &SeedsReport) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "solver: {} ({})",
        r.solver.version.as_deref().unwrap_or("?"),
        r.solver.sha256.as_deref().unwrap_or("?")
    );
    let _ = writeln!(
        s,
        "seeds: {:?} (0 = unmodified session)  sessions: {}  checks: {}  complete under every seed: {}",
        r.seeds, r.sessions, r.checks, r.complete_checks
    );
    let s0 = &r.seed0;
    let flag = if s0.answer_mismatches
        + s0.followup_mismatches
        + s0.rlimit_mismatches
        + s0.check_count_mismatches
        > 0
    {
        "  <-- NONDETERMINISM"
    } else {
        ""
    };
    let _ = writeln!(
        s,
        "seed 0 vs recorded: answers {}/{} differ, follow-up replies {}/{} differ, rlimit deltas {}/{} differ, check counts differ in {} sessions{flag}",
        s0.answer_mismatches,
        s0.answers_compared,
        s0.followup_mismatches,
        s0.followups_compared,
        s0.rlimit_mismatches,
        s0.rlimit_compared,
        s0.check_count_mismatches
    );
    for d in s0.differences.iter().take(10) {
        let _ = writeln!(
            s,
            "  {} #{} {}: recorded {:?} replayed {:?}",
            d.id, d.seq, d.what, d.recorded, d.replayed
        );
    }
    let _ = writeln!(
        s,
        "unstable answers: {} of {} ({}), verdict-unstable (unsat vs not): {}",
        r.unstable,
        r.complete_checks,
        r.unstable_fraction
            .map_or_else(|| "-".to_owned(), |f| format!("{:.2}%", f * 100.0)),
        r.verdict_unstable
    );
    let _ = writeln!(
        s,
        "rlimit CV across seeds: mean {} p50 {} p90 {} p99 {} max {} (zero CV: {} of {})",
        fmt_f(r.rlimit_cv.mean),
        fmt_f(r.rlimit_cv.p50),
        fmt_f(r.rlimit_cv.p90),
        fmt_f(r.rlimit_cv.p99),
        fmt_f(r.rlimit_cv.max),
        r.rlimit_cv.zeros,
        r.rlimit_cv.count
    );
    let _ = writeln!(
        s,
        "rlimit max/min across seeds: p50 {} p90 {} p99 {} max {}",
        fmt_f(r.rlimit_ratio.p50),
        fmt_f(r.rlimit_ratio.p90),
        fmt_f(r.rlimit_ratio.p99),
        fmt_f(r.rlimit_ratio.max)
    );
    let _ = writeln!(s, "reason-unknown replies: {:?}", r.reasons_unknown);
    let _ = writeln!(
        s,
        "infra: {} timeouts, {} errors, {} runs with unpaired batches or solver errors",
        r.infra_timeouts.len(),
        r.infra_errors.len(),
        r.unpaired_runs.len()
    );
    for c in r
        .notable
        .iter()
        .filter(|c| c.stable == Some(false))
        .take(20)
    {
        let _ = writeln!(s, "  unstable {} #{}: {:?}", c.id, c.seq, c.answers);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Followup;

    fn obs(answer: &str, delta: u64, reason: Option<&str>) -> CheckObservation {
        CheckObservation {
            command_index: 0,
            answer: Some(answer.as_bytes().to_vec()),
            rlimit_before: Some(0),
            rlimit_delta: Some(delta),
            followups: reason
                .map(|r| Followup {
                    command: b"(get-info :reason-unknown)".to_vec(),
                    reply: Some(r.as_bytes().to_vec()),
                })
                .into_iter()
                .collect(),
        }
    }

    fn run(seed: u64, checks: Vec<CheckObservation>) -> SeedRun {
        SeedRun {
            seed,
            checks,
            unpaired_batches: 0,
            errors: vec![],
            infra: None,
            elapsed_ms: 1,
        }
    }

    fn empty_report() -> SeedsReport {
        SeedsReport {
            solver: Identity {
                path: String::new(),
                version: None,
                sha256: None,
            },
            seeds: vec![0, 1],
            sessions: 1,
            checks: 0,
            complete_checks: 0,
            unstable: 0,
            unstable_fraction: None,
            verdict_unstable: 0,
            answers_by_seed: BTreeMap::new(),
            reasons_unknown: BTreeMap::new(),
            rlimit_cv: FloatDistribution::of(&[]),
            rlimit_ratio: FloatDistribution::of(&[]),
            seed0: Seed0Report::default(),
            infra_timeouts: vec![],
            infra_errors: vec![],
            unpaired_runs: vec![],
            replay_ms: 0,
            notable: vec![],
        }
    }

    #[test]
    fn classifies_stability_and_seed0_differences() {
        let recorded = vec![obs("unsat", 100, None), obs("unsat", 50, None)];
        let runs = vec![
            run(0, vec![obs("unsat", 100, None), obs("unsat", 60, None)]),
            run(
                1,
                vec![
                    obs("unsat", 300, None),
                    obs("unknown", 60, Some("(:reason-unknown \"canceled\")")),
                ],
            ),
        ];
        let mut report = empty_report();
        let checks = analyze_session("s", &recorded, &runs, &mut report, 10);
        assert_eq!(checks[0].stable, Some(true));
        assert_eq!(checks[1].stable, Some(false));
        assert_eq!(checks[1].verdict_stable, Some(false));
        assert_eq!(checks[0].rlimit_ratio, Some(3.0));
        assert_eq!(checks[0].rlimit_cv, Some(0.5));
        assert_eq!(report.unstable, 1);
        assert_eq!(report.complete_checks, 2);
        assert_eq!(report.seed0.answer_mismatches, 0);
        assert_eq!(report.seed0.rlimit_mismatches, 1);
        assert_eq!(report.seed0.differences[0].what, "rlimit");
        assert_eq!(
            report.reasons_unknown.get("(:reason-unknown \"canceled\")"),
            Some(&1)
        );
    }

    #[test]
    fn infra_failures_make_checks_incomplete() {
        let recorded = vec![obs("unsat", 100, None)];
        let mut bad = run(1, vec![]);
        bad.infra = Some("infra-timeout".to_owned());
        let runs = vec![run(0, vec![obs("unsat", 100, None)]), bad];
        let mut report = empty_report();
        let checks = analyze_session("s", &recorded, &runs, &mut report, 10);
        assert_eq!(checks[0].stable, None);
        assert_eq!(report.complete_checks, 0);
    }
}
