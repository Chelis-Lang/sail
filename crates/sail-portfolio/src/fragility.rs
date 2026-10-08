//! Per-check fragility: re-running each captured session under several
//! configurations of the same pinned Z3 and summarising how each check's
//! answer and effort move.
//!
//! Every configuration ([`crate::config`]) replays the **whole captured
//! session** into a fresh solver process. Z3 reads commands in order, so the
//! replies to check *k* depend only on the bytes up to it: every check is
//! measured after its exact session prefix (under that configuration),
//! and one process per configuration covers every check of the session.
//!
//! **What the measurement is, and is not.** The commands after a check are
//! the captured ones. If a configuration changes an earlier check's answer,
//! Verus itself might have sent something different after it (for example
//! no `get-model` after a proof). The later checks are still measured on
//! their exact captured prefix, which is the prefix plain Verus produced.
//!
//! Per check this reports:
//! - the answer under each configuration and their distribution;
//! - the fraction of configurations that prove it (`unsat`);
//! - the `rlimit-count` delta under each configuration: min, median, max,
//!   coefficient of variation;
//! - the `:rlimit` in force, the largest fraction of it used, and the
//!   margin `1 - max_used / limit`;
//! - a [`Stability`] class and a [`CheckFragility::score`].
//!
//! Nothing here changes a verdict: it reports a property of a proof.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use sail_smtlib::command::{CommandKind, classify};
use sail_solver::{Identity, Outcome, Solver};

use crate::config::{Config, configs};
use crate::error::PortfolioError;
use crate::sessions::SessionFiles;
use crate::stats::{FloatDistribution, as_f64, coefficient_of_variation, count_f64, percentile};
use crate::transcript::{CheckObservation, Class, Cmd, Transcript, split_commands, split_replies};

/// Measurement options.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Reseeds, including the baseline (seed 0). At least 1.
    pub seeds: u64,
    /// Assertion-shuffle configurations (Mariposa's shuffling), under the
    /// default seed. 0 for reseeding only.
    pub shuffles: u64,
    /// Solver processes run at the same time.
    pub jobs: usize,
    /// The used/limit fraction from which a check is reported as near its
    /// limit (a reporting threshold only; it decides nothing).
    pub near_limit: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            seeds: 8,
            shuffles: 0,
            jobs: 4,
            near_limit: 0.5,
        }
    }
}

/// One replay of one session under one configuration.
#[derive(Debug, Clone)]
pub struct ConfigRun {
    /// The configuration.
    pub config: Config,
    /// Per-check observations, in order.
    pub checks: Vec<CheckObservation>,
    /// Batches that could not be paired.
    pub unpaired_batches: usize,
    /// Solver error replies.
    pub errors: Vec<String>,
    /// `None` if the solver exited by itself; otherwise what went wrong
    /// (`infra-timeout`, or a start failure). Never a solver answer.
    pub infra: Option<String>,
    /// Wall-clock milliseconds.
    pub elapsed_ms: u64,
}

/// Replays a session's exact `input` under `config` in a fresh solver.
#[must_use]
pub fn replay(solver: &Solver, input: &[u8], config: Config) -> ConfigRun {
    let mut run = ConfigRun {
        config,
        checks: Vec::new(),
        unpaired_batches: 0,
        errors: Vec::new(),
        infra: None,
        elapsed_ms: 0,
    };
    let (input, commands) = config.apply(input, split_commands(input));
    match sail_solver::run(solver, &input) {
        Ok(r) => {
            run.elapsed_ms = u64::try_from(r.elapsed.as_millis()).unwrap_or(u64::MAX);
            if r.outcome == Outcome::InfraTimeout {
                run.infra = Some("infra-timeout".to_owned());
            }
            let t = Transcript::from_parts(commands, &split_replies(&r.stdout));
            run.checks = t.checks();
            run.unpaired_batches = t.unpaired_batches;
            run.errors = t.errors;
        }
        Err(e) => run.infra = Some(e.to_string()),
    }
    run
}

/// How a check's verdict behaves across configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stability {
    /// Every configuration proved it (`unsat`).
    StableProved,
    /// No configuration proved it (`sat` or `unknown` everywhere).
    StableUnproved,
    /// Some configurations proved it and some did not.
    Unstable,
    /// Some configuration gave no paired answer (infrastructure failure or
    /// unpaired batch); the others agree.
    Incomplete,
}

impl Stability {
    /// The kebab-case name used in reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::StableProved => "stable-proved",
            Self::StableUnproved => "stable-unproved",
            Self::Unstable => "unstable",
            Self::Incomplete => "incomplete",
        }
    }

    /// Severity for aggregation: the worst check decides a group's class.
    fn severity(self) -> u8 {
        match self {
            Self::StableProved => 0,
            Self::StableUnproved => 1,
            Self::Incomplete => 2,
            Self::Unstable => 3,
        }
    }
}

/// The spread of a check's `rlimit-count` delta across configurations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effort {
    /// Smallest delta.
    pub min: u64,
    /// Median delta (nearest rank).
    pub median: u64,
    /// Largest delta.
    pub max: u64,
    /// Population coefficient of variation; `None` with fewer than two
    /// values or a zero mean.
    pub cv: Option<f64>,
}

impl Effort {
    /// Summarizes `values`; `None` if empty.
    #[must_use]
    pub fn of(values: &[u64]) -> Option<Self> {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        Some(Self {
            min: *sorted.first()?,
            median: percentile(&sorted, 50.0)?,
            max: *sorted.last()?,
            cv: coefficient_of_variation(&sorted),
        })
    }
}

/// One check across all configurations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckFragility {
    /// Session id.
    pub session: String,
    /// Check index in the session.
    pub seq: usize,
    /// The Verus function the check belongs to, when attributed
    /// ([`attribute_single_function`]); `None` otherwise.
    pub function: Option<String>,
    /// The captured answer, when the session's output was available.
    pub recorded: Option<String>,
    /// The `:rlimit` in force at the check (0 or absent means unlimited).
    pub limit: Option<u64>,
    /// The answer under each configuration, in configuration order.
    pub answers: Vec<Option<String>>,
    /// The `:reason-unknown` reply under each configuration, if any.
    pub reasons: Vec<Option<String>>,
    /// The `rlimit-count` delta under each configuration.
    pub rlimit: Vec<Option<u64>>,
    /// Answer counts (`(none)` for a missing answer).
    pub answer_counts: BTreeMap<String, usize>,
    /// Configurations that answered `unsat`.
    pub proved: usize,
    /// `proved / configurations that answered`.
    pub proved_fraction: Option<f64>,
    /// Effort across the configurations that report a delta.
    pub effort: Option<Effort>,
    /// `effort.max / limit`, when there is a positive limit.
    pub max_used_fraction: Option<f64>,
    /// `1 - max_used_fraction` (negative if a configuration overran).
    pub margin: Option<f64>,
    /// The stability class.
    pub stability: Stability,
    /// A score in `[0, 1]`: the larger of answer disagreement
    /// (`2 · min(p, 1 − p)` for proved fraction `p`) and effort pressure
    /// (`min(1, max_used_fraction)`). 0 is a check that every configuration
    /// answers alike, using a negligible part of its limit.
    pub score: f64,
}

/// A group of checks (a function, a session, a corpus unit) summarised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupFragility {
    /// The group's name.
    pub name: String,
    /// Checks in the group.
    pub checks: usize,
    /// The worst class among them (unstable > incomplete > stable-unproved
    /// > stable-proved).
    pub stability: Stability,
    /// Checks per class.
    pub by_class: BTreeMap<String, usize>,
    /// The smallest proved fraction among checks that some configuration
    /// proved.
    pub min_proved_fraction: Option<f64>,
    /// The largest used/limit fraction of any check.
    pub max_used_fraction: Option<f64>,
    /// The largest effort CV of any check.
    pub max_cv: Option<f64>,
    /// The largest check score.
    pub score: f64,
}

/// Groups `checks` by `key` and summarises each group. Checks whose key is
/// `None` are left out.
#[must_use]
pub fn group<'a>(
    checks: impl IntoIterator<Item = &'a CheckFragility>,
    key: impl Fn(&CheckFragility) -> Option<String>,
) -> Vec<GroupFragility> {
    let mut groups: BTreeMap<String, Vec<&CheckFragility>> = BTreeMap::new();
    for c in checks {
        if let Some(k) = key(c) {
            groups.entry(k).or_default().push(c);
        }
    }
    groups
        .into_iter()
        .map(|(name, cs)| summarise_group(name, &cs))
        .collect()
}

fn max_f(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
}

fn summarise_group(name: String, checks: &[&CheckFragility]) -> GroupFragility {
    let mut by_class = BTreeMap::new();
    for c in checks {
        *by_class.entry(c.stability.name().to_owned()).or_insert(0) += 1;
    }
    GroupFragility {
        name,
        checks: checks.len(),
        stability: checks
            .iter()
            .map(|c| c.stability)
            .max_by_key(|s| s.severity())
            .unwrap_or(Stability::StableProved),
        by_class,
        min_proved_fraction: checks
            .iter()
            .filter(|c| c.proved > 0)
            .filter_map(|c| c.proved_fraction)
            .fold(None, |m, v| Some(m.map_or(v, |m: f64| m.min(v)))),
        max_used_fraction: max_f(checks.iter().filter_map(|c| c.max_used_fraction)),
        max_cv: max_f(checks.iter().filter_map(|c| c.effort.as_ref()?.cv)),
        score: max_f(checks.iter().map(|c| c.score)).unwrap_or(0.0),
    }
}

/// The `:rlimit` value in force at each check of `commands`, in check
/// order.
#[must_use]
pub fn limits_at_checks(commands: &[Cmd]) -> Vec<Option<u64>> {
    let mut current = None;
    let mut out = Vec::new();
    for c in commands {
        match c.class {
            Class::SetOption => {
                if let Ok(cmd) = classify(&c.bytes)
                    && let CommandKind::SetOption { keyword, value } = cmd.kind
                    && keyword == b":rlimit"
                {
                    current = std::str::from_utf8(value)
                        .ok()
                        .and_then(|v| v.trim().parse::<u64>().ok());
                }
            }
            Class::Check => out.push(current),
            _ => {}
        }
    }
    out
}

fn text(b: Option<&[u8]>) -> Option<String> {
    b.map(|b| String::from_utf8_lossy(b).into_owned())
}

/// Analyses one check from its per-configuration observations.
#[must_use]
pub fn analyse_check(
    session: &str,
    seq: usize,
    recorded: Option<&CheckObservation>,
    limit: Option<u64>,
    observed: &[Option<&CheckObservation>],
) -> CheckFragility {
    let answers: Vec<Option<String>> = observed
        .iter()
        .map(|o| o.and_then(|o| text(o.answer.as_deref())))
        .collect();
    let reasons: Vec<Option<String>> = observed
        .iter()
        .map(|o| o.and_then(|o| text(o.reason_unknown())))
        .collect();
    let rlimit: Vec<Option<u64>> = observed
        .iter()
        .map(|o| o.and_then(|o| o.rlimit_delta))
        .collect();
    let mut answer_counts = BTreeMap::new();
    for a in &answers {
        *answer_counts
            .entry(a.clone().unwrap_or_else(|| "(none)".to_owned()))
            .or_insert(0) += 1;
    }
    let answered = answers.iter().flatten().count();
    let proved = answers.iter().flatten().filter(|a| *a == "unsat").count();
    let proved_fraction = (answered > 0).then(|| count_f64(proved) / count_f64(answered));
    let stability = if proved > 0 && proved < answered {
        Stability::Unstable
    } else if answered < answers.len() {
        Stability::Incomplete
    } else if proved == answered && answered > 0 {
        Stability::StableProved
    } else {
        Stability::StableUnproved
    };
    let deltas: Vec<u64> = rlimit.iter().flatten().copied().collect();
    let effort = Effort::of(&deltas);
    let max_used_fraction = match (&effort, limit) {
        (Some(e), Some(l)) if l > 0 => Some(as_f64(e.max) / as_f64(l)),
        _ => None,
    };
    let disagreement = proved_fraction.map_or(0.0, |p| 2.0 * p.min(1.0 - p));
    let pressure = max_used_fraction.map_or(0.0, |f| f.clamp(0.0, 1.0));
    CheckFragility {
        session: session.to_owned(),
        seq,
        function: None,
        recorded: recorded.and_then(|r| text(r.answer.as_deref())),
        limit,
        answers,
        reasons,
        rlimit,
        answer_counts,
        proved,
        proved_fraction,
        effort,
        max_used_fraction,
        margin: max_used_fraction.map(|f| 1.0 - f),
        stability,
        score: disagreement.max(pressure),
    }
}

/// A bucket of the used/limit histogram: `lo <= fraction < hi`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    /// Lower bound (inclusive).
    pub lo: f64,
    /// Upper bound (exclusive); `None` for no bound.
    pub hi: Option<f64>,
    /// Checks in the bucket.
    pub checks: usize,
    /// Of those, checks where some configuration answered `unknown`.
    pub with_unknown: usize,
    /// Of those, unstable checks.
    pub unstable: usize,
}

/// The bucket bounds of [`Summary::used_fraction_histogram`].
pub const HISTOGRAM_BOUNDS: &[f64] = &[0.0, 1e-5, 1e-4, 1e-3, 1e-2, 0.1, 0.25, 0.5, 0.75, 1.0];

/// What the `unknown` answers looked like (ADR 0009's calibration data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnknownSummary {
    /// Checks where at least one configuration answered `unknown`.
    pub checks: usize,
    /// Of those, checks that some other configuration proved.
    pub proved_elsewhere: usize,
    /// `:reason-unknown` replies, counted over (check, configuration).
    pub reasons: BTreeMap<String, usize>,
    /// The used/limit fraction of each `unknown` answer (per check and
    /// configuration), where a limit and delta are known.
    pub used_fraction: FloatDistribution,
    /// The largest used/limit fraction of any `unknown` whose reason does
    /// not indicate resource exhaustion.
    pub max_used_fraction_not_exhausted: Option<f64>,
}

/// Seed-0 replay versus the captured session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineCheck {
    /// Checks where both had an answer.
    pub answers_compared: usize,
    /// Of those, how many differ.
    pub answer_mismatches: usize,
    /// Checks where both had a delta.
    pub rlimit_compared: usize,
    /// Of those, how many differ.
    pub rlimit_mismatches: usize,
}

/// Corpus-wide summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Checks measured.
    pub checks: usize,
    /// Checks per [`Stability`] class.
    pub by_class: BTreeMap<String, usize>,
    /// Checks whose answer text differs between configurations (any
    /// change, including `sat` versus `unknown`), among checks with an
    /// answer under every configuration.
    pub answer_unstable: usize,
    /// Answers over all (check, configuration) pairs.
    pub answers: BTreeMap<String, usize>,
    /// Distribution of per-check effort CV.
    pub effort_cv: FloatDistribution,
    /// Distribution of per-check max/min effort ratio (min > 0).
    pub effort_ratio: FloatDistribution,
    /// Distribution of per-check `max_used_fraction`.
    pub used_fraction: FloatDistribution,
    /// Checks by `max_used_fraction`, in [`HISTOGRAM_BOUNDS`] buckets.
    pub used_fraction_histogram: Vec<Bucket>,
    /// Checks with `max_used_fraction >= near_limit`.
    pub near_limit: usize,
    /// Distribution of check scores.
    pub score: FloatDistribution,
    /// The `unknown` answers.
    pub unknown: UnknownSummary,
    /// Baseline replay versus the captured session.
    pub baseline_vs_recorded: BaselineCheck,
}

/// Whether a `:reason-unknown` reply indicates resource exhaustion or
/// cancellation: the same test the shim's serving policy applies (ADR 0009).
#[must_use]
pub fn reason_is_exhaustion(reason: &str) -> bool {
    let r = reason.to_ascii_lowercase();
    [
        "canceled",
        "cancelled",
        "resource",
        "rlimit",
        "timeout",
        "memout",
        "memory",
    ]
    .iter()
    .any(|w| r.contains(w))
}

/// Builds the corpus-wide summary of `checks`.
#[must_use]
pub fn summarise(checks: &[CheckFragility], near_limit: f64, baseline: BaselineCheck) -> Summary {
    let mut by_class = BTreeMap::new();
    let mut answers = BTreeMap::new();
    let mut answer_unstable = 0;
    let mut unknown_checks = 0;
    let mut proved_elsewhere = 0;
    let mut reasons = BTreeMap::new();
    let mut unknown_fracs = Vec::new();
    let mut max_not_exhausted: Option<f64> = None;
    for c in checks {
        *by_class.entry(c.stability.name().to_owned()).or_insert(0) += 1;
        for (k, n) in &c.answer_counts {
            *answers.entry(k.clone()).or_insert(0) += n;
        }
        if c.answers.iter().all(Option::is_some) && c.answer_counts.len() > 1 {
            answer_unstable += 1;
        }
        let mut any_unknown = false;
        for ((a, r), d) in c.answers.iter().zip(&c.reasons).zip(&c.rlimit) {
            if a.as_deref() != Some("unknown") {
                continue;
            }
            any_unknown = true;
            if let Some(r) = r {
                *reasons.entry(r.clone()).or_insert(0) += 1;
            }
            if let (Some(d), Some(l)) = (d, c.limit.filter(|l| *l > 0)) {
                let f = as_f64(*d) / as_f64(l);
                unknown_fracs.push(f);
                if !r.as_deref().is_some_and(reason_is_exhaustion) {
                    max_not_exhausted = Some(max_not_exhausted.map_or(f, |m| m.max(f)));
                }
            }
        }
        if any_unknown {
            unknown_checks += 1;
            if c.proved > 0 {
                proved_elsewhere += 1;
            }
        }
    }
    let ratios: Vec<f64> = checks
        .iter()
        .filter_map(|c| {
            let e = c.effort.as_ref()?;
            (e.min > 0).then(|| as_f64(e.max) / as_f64(e.min))
        })
        .collect();
    let used: Vec<f64> = checks.iter().filter_map(|c| c.max_used_fraction).collect();
    Summary {
        checks: checks.len(),
        by_class,
        answer_unstable,
        answers,
        effort_cv: FloatDistribution::of(
            &checks
                .iter()
                .filter_map(|c| c.effort.as_ref()?.cv)
                .collect::<Vec<_>>(),
        ),
        effort_ratio: FloatDistribution::of(&ratios),
        used_fraction: FloatDistribution::of(&used),
        used_fraction_histogram: histogram(checks),
        near_limit: used.iter().filter(|f| **f >= near_limit).count(),
        score: FloatDistribution::of(&checks.iter().map(|c| c.score).collect::<Vec<_>>()),
        unknown: UnknownSummary {
            checks: unknown_checks,
            proved_elsewhere,
            reasons,
            used_fraction: FloatDistribution::of(&unknown_fracs),
            max_used_fraction_not_exhausted: max_not_exhausted,
        },
        baseline_vs_recorded: baseline,
    }
}

fn histogram(checks: &[CheckFragility]) -> Vec<Bucket> {
    let mut buckets: Vec<Bucket> = HISTOGRAM_BOUNDS
        .iter()
        .enumerate()
        .map(|(i, &lo)| Bucket {
            lo,
            hi: HISTOGRAM_BOUNDS.get(i + 1).copied(),
            checks: 0,
            with_unknown: 0,
            unstable: 0,
        })
        .collect();
    for c in checks {
        let Some(f) = c.max_used_fraction else {
            continue;
        };
        let at = buckets.iter().rposition(|b| f >= b.lo).unwrap_or_default();
        if let Some(b) = buckets.get_mut(at) {
            b.checks += 1;
            if c.answer_counts.contains_key("unknown") {
                b.with_unknown += 1;
            }
            if c.stability == Stability::Unstable {
                b.unstable += 1;
            }
        }
    }
    buckets
}

/// The fragility report for a set of sessions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The solver every configuration ran on.
    pub solver: Identity,
    /// The configurations, in the order of every per-check vector.
    pub configs: Vec<Config>,
    /// Their labels.
    pub config_labels: Vec<String>,
    /// The near-limit reporting threshold used.
    pub near_limit: f64,
    /// Sessions measured.
    pub sessions: usize,
    /// The summary.
    pub summary: Summary,
    /// Every check.
    pub checks: Vec<CheckFragility>,
    /// Per-function groups, for attributed checks.
    pub functions: Vec<GroupFragility>,
    /// Runs that did not complete (`session@config: why`): infrastructure
    /// failures, never solver answers.
    pub infra: Vec<String>,
    /// Runs with unpaired batches or solver errors (`session@config`).
    pub unpaired_runs: Vec<String>,
    /// Total solver wall-clock time over all runs, in milliseconds.
    pub replay_ms: u64,
}

/// Measures every session in `sessions` under the configurations that
/// `options` names.
///
/// # Errors
///
/// [`PortfolioError::Options`] for zero seeds or jobs.
pub fn measure(
    solver: &Solver,
    sessions: &[SessionFiles],
    options: &Options,
    progress: impl Fn(usize, usize) + Sync,
) -> Result<Report, PortfolioError> {
    if options.seeds == 0 || options.jobs == 0 {
        return Err(PortfolioError::Options(
            "seeds and jobs must be at least 1".to_owned(),
        ));
    }
    let configs = configs(options.seeds, options.shuffles);
    let inputs: Vec<Option<Vec<u8>>> = sessions
        .iter()
        .map(|s| std::fs::read(&s.input).ok())
        .collect();
    let units: Vec<(usize, Config)> = (0..sessions.len())
        .flat_map(|s| configs.iter().map(move |&c| (s, c)))
        .collect();
    let total = units.len();
    let runs = crate::parallel::map(
        &units,
        options.jobs,
        |&(s, config)| {
            inputs
                .get(s)
                .and_then(Option::as_deref)
                .map(|input| replay(solver, input, config))
        },
        |done| progress(done, total),
    );
    let mut by_session: Vec<Vec<Option<ConfigRun>>> = vec![Vec::new(); sessions.len()];
    for ((s, _), run) in units.iter().zip(runs) {
        if let Some(slot) = by_session.get_mut(*s) {
            slot.push(run);
        }
    }
    let mut report = Report {
        solver: solver.identity(),
        config_labels: configs.iter().map(Config::label).collect(),
        configs,
        near_limit: options.near_limit,
        sessions: sessions.len(),
        summary: summarise(&[], options.near_limit, BaselineCheck::default()),
        checks: Vec::new(),
        functions: Vec::new(),
        infra: Vec::new(),
        unpaired_runs: Vec::new(),
        replay_ms: 0,
    };
    let mut baseline = BaselineCheck::default();
    for ((files, input), runs) in sessions.iter().zip(&inputs).zip(&by_session) {
        let Some(input) = input else {
            report.infra.push(format!(
                "{}: cannot read {}",
                files.id,
                files.input.display()
            ));
            continue;
        };
        let recorded = read_recorded(input, &files.output);
        let checks = analyse_session(&files.id, input, recorded.as_deref(), runs, &mut report);
        if let Some(rec) = &recorded {
            compare_baseline(rec, runs, &mut baseline);
        }
        report.checks.extend(checks);
    }
    report.summary = summarise(&report.checks, options.near_limit, baseline);
    Ok(report)
}

fn read_recorded(input: &[u8], output: &Path) -> Option<Vec<CheckObservation>> {
    let output = std::fs::read(output).ok()?;
    Some(Transcript::new(input, &output).checks())
}

fn compare_baseline(
    recorded: &[CheckObservation],
    runs: &[Option<ConfigRun>],
    tally: &mut BaselineCheck,
) {
    let Some(base) = runs
        .iter()
        .flatten()
        .find(|r| r.config.is_baseline() && r.infra.is_none())
    else {
        return;
    };
    for (rec, rep) in recorded.iter().zip(&base.checks) {
        if let (Some(a), Some(b)) = (&rec.answer, &rep.answer) {
            tally.answers_compared += 1;
            tally.answer_mismatches += usize::from(a != b);
        }
        if let (Some(a), Some(b)) = (rec.rlimit_delta, rep.rlimit_delta) {
            tally.rlimit_compared += 1;
            tally.rlimit_mismatches += usize::from(a != b);
        }
    }
}

fn analyse_session(
    id: &str,
    input: &[u8],
    recorded: Option<&[CheckObservation]>,
    runs: &[Option<ConfigRun>],
    report: &mut Report,
) -> Vec<CheckFragility> {
    for (i, run) in runs.iter().enumerate() {
        let label = report.config_labels.get(i).cloned().unwrap_or_default();
        let tag = format!("{id}@{label}");
        match run {
            None => report.infra.push(format!("{tag}: not run")),
            Some(r) => {
                report.replay_ms += r.elapsed_ms;
                if let Some(why) = &r.infra {
                    report.infra.push(format!("{tag}: {why}"));
                }
                if r.unpaired_batches > 0 || !r.errors.is_empty() {
                    report.unpaired_runs.push(tag);
                }
            }
        }
    }
    let limits = limits_at_checks(&split_commands(input));
    (0..limits.len())
        .map(|seq| {
            let observed: Vec<Option<&CheckObservation>> = runs
                .iter()
                .map(|r| {
                    r.as_ref()
                        .filter(|r| r.infra.is_none())
                        .and_then(|r| r.checks.get(seq))
                })
                .collect();
            analyse_check(
                id,
                seq,
                recorded.and_then(|r| r.get(seq)),
                limits.get(seq).copied().flatten(),
                &observed,
            )
        })
        .collect()
}

/// Attributes every check to `functions[0]` when Verus reported exactly one
/// function for the run (for example under `--verify-function`), and
/// rebuilds [`Report::functions`]. With more than one function, checks stay
/// unattributed: the solver's input carries no function names, and a wrong
/// attribution would be worse than none (ADR 0014). Returns whether
/// attribution happened.
pub fn attribute_single_function(report: &mut Report, functions: &[String]) -> bool {
    let [only] = functions else {
        return false;
    };
    for c in &mut report.checks {
        c.function = Some(only.clone());
    }
    report.functions = group(&report.checks, |c| c.function.clone());
    true
}

fn fmt_f(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |v| format!("{v:.3}"))
}

fn fmt_pct(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |v| format!("{:.4}%", v * 100.0))
}

/// A human-readable summary: the totals, then the most fragile checks.
#[must_use]
pub fn render(r: &Report, top: usize) -> String {
    let mut s = String::new();
    let sm = &r.summary;
    let _ = writeln!(
        s,
        "solver: {} ({})",
        r.solver.version.as_deref().unwrap_or("?"),
        r.solver.sha256.as_deref().unwrap_or("?")
    );
    let _ = writeln!(
        s,
        "configurations ({}): {}",
        r.configs.len(),
        r.config_labels.join(" ")
    );
    let _ = writeln!(s, "sessions: {}  checks: {}", r.sessions, sm.checks);
    let classes: Vec<String> = sm
        .by_class
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let _ = writeln!(s, "stability: {}", classes.join(", "));
    let _ = writeln!(
        s,
        "answer changed under some configuration: {}  answers: {:?}",
        sm.answer_unstable, sm.answers
    );
    let _ = writeln!(
        s,
        "effort CV: p50 {} p90 {} p99 {} max {}  max/min: p50 {} p99 {} max {}",
        fmt_f(sm.effort_cv.p50),
        fmt_f(sm.effort_cv.p90),
        fmt_f(sm.effort_cv.p99),
        fmt_f(sm.effort_cv.max),
        fmt_f(sm.effort_ratio.p50),
        fmt_f(sm.effort_ratio.p99),
        fmt_f(sm.effort_ratio.max),
    );
    let _ = writeln!(
        s,
        "largest share of the rlimit used: p50 {} p99 {} max {}  near the limit (>= {:.0}%): {}",
        fmt_pct(sm.used_fraction.p50),
        fmt_pct(sm.used_fraction.p99),
        fmt_pct(sm.used_fraction.max),
        r.near_limit * 100.0,
        sm.near_limit
    );
    let _ = writeln!(
        s,
        "unknown: {} checks ({} proved under another configuration), max share of limit used {}, reasons {:?}",
        sm.unknown.checks,
        sm.unknown.proved_elsewhere,
        fmt_pct(sm.unknown.used_fraction.max),
        sm.unknown.reasons
    );
    let b = &sm.baseline_vs_recorded;
    let _ = writeln!(
        s,
        "baseline replay vs capture: answers {}/{} differ, rlimit {}/{} differ",
        b.answer_mismatches, b.answers_compared, b.rlimit_mismatches, b.rlimit_compared
    );
    if !r.infra.is_empty() || !r.unpaired_runs.is_empty() {
        let _ = writeln!(
            s,
            "infrastructure: {} incomplete runs, {} runs with unpaired batches (not solver answers)",
            r.infra.len(),
            r.unpaired_runs.len()
        );
    }
    for g in &r.functions {
        let _ = writeln!(
            s,
            "function {}: {} checks, {}, score {:.3}, max used {}, max CV {}",
            g.name,
            g.checks,
            g.stability.name(),
            g.score,
            fmt_pct(g.max_used_fraction),
            fmt_f(g.max_cv)
        );
    }
    render_top_checks(&mut s, r, top);
    s
}

/// Appends the `top` highest-scoring checks.
fn render_top_checks(s: &mut String, r: &Report, top: usize) {
    let mut ranked: Vec<&CheckFragility> = r.checks.iter().collect();
    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    let shown: Vec<&&CheckFragility> = ranked.iter().take(top).filter(|c| c.score > 0.0).collect();
    if !shown.is_empty() {
        let _ = writeln!(s, "most fragile checks:");
    }
    for c in shown {
        let _ = writeln!(
            s,
            "  {} #{}{}: {} proved {}/{} effort {}..{} (CV {}) used {} answers {:?}",
            c.session,
            c.seq,
            c.function
                .as_deref()
                .map_or_else(String::new, |f| format!(" ({f})")),
            c.stability.name(),
            c.proved,
            c.answers.len(),
            c.effort.as_ref().map_or(0, |e| e.min),
            c.effort.as_ref().map_or(0, |e| e.max),
            fmt_f(c.effort.as_ref().and_then(|e| e.cv)),
            fmt_pct(c.max_used_fraction),
            c.answer_counts
        );
    }
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

    #[test]
    fn classifies_a_stable_proof() {
        let a = obs("unsat", 100, None);
        let b = obs("unsat", 300, None);
        let c = analyse_check("s", 0, Some(&a), Some(1000), &[Some(&a), Some(&b)]);
        assert_eq!(c.stability, Stability::StableProved);
        assert_eq!(c.proved, 2);
        assert_eq!(c.proved_fraction, Some(1.0));
        assert_eq!(
            c.effort,
            Some(Effort {
                min: 100,
                median: 100,
                max: 300,
                cv: Some(0.5)
            })
        );
        assert_eq!(c.max_used_fraction, Some(0.3));
        assert_eq!(c.margin, Some(0.7));
        assert!((c.score - 0.3).abs() < 1e-12);
        assert_eq!(c.recorded.as_deref(), Some("unsat"));
    }

    #[test]
    fn classifies_instability_and_incompleteness() {
        let p = obs("unsat", 900, None);
        let u = obs("unknown", 1000, Some("(:reason-unknown \"canceled\")"));
        let c = analyse_check(
            "s",
            1,
            None,
            Some(1000),
            &[Some(&p), Some(&u), Some(&p), None],
        );
        assert_eq!(c.stability, Stability::Unstable);
        assert_eq!(c.proved, 2);
        assert_eq!(c.answer_counts.get("(none)"), Some(&1));
        // 2 of 3 answers proved: disagreement 2/3, pressure 1.
        assert!((c.score - 1.0).abs() < 1e-12);
        let c = analyse_check("s", 2, None, None, &[Some(&p), None]);
        assert_eq!(c.stability, Stability::Incomplete);
        assert_eq!(c.max_used_fraction, None);
        let q = obs(
            "unknown",
            1,
            Some("(:reason-unknown \"(incomplete quantifiers)\")"),
        );
        let c = analyse_check("s", 3, None, Some(0), &[Some(&q), Some(&q)]);
        assert_eq!(c.stability, Stability::StableUnproved);
        assert_eq!(c.max_used_fraction, None, "a zero limit is unlimited");
        assert_eq!(c.score, 0.0);
    }

    #[test]
    fn reads_the_limit_in_force_at_each_check() {
        let cmds = split_commands(
            b"(set-option :rlimit 30000000)(check-sat)(set-option :rlimit 0)(check-sat)(set-option :other 1)(check-sat)",
        );
        assert_eq!(
            limits_at_checks(&cmds),
            [Some(30_000_000), Some(0), Some(0)]
        );
        assert_eq!(limits_at_checks(&split_commands(b"(check-sat)")), [None]);
    }

    #[test]
    fn summarises_unknowns_and_groups() {
        let p = obs("unsat", 10, None);
        let q = obs(
            "unknown",
            20,
            Some("(:reason-unknown \"(incomplete quantifiers)\")"),
        );
        let x = obs("unknown", 900, Some("(:reason-unknown \"canceled\")"));
        let mut checks = vec![
            analyse_check("a", 0, None, Some(1000), &[Some(&p), Some(&p)]),
            analyse_check("a", 1, None, Some(1000), &[Some(&q), Some(&q)]),
            analyse_check("b", 0, None, Some(1000), &[Some(&p), Some(&x)]),
        ];
        let s = summarise(&checks, 0.5, BaselineCheck::default());
        assert_eq!(s.checks, 3);
        assert_eq!(s.by_class.get("unstable"), Some(&1));
        assert_eq!(s.unknown.checks, 2);
        assert_eq!(s.unknown.proved_elsewhere, 1);
        assert_eq!(s.unknown.max_used_fraction_not_exhausted, Some(0.02));
        assert_eq!(s.near_limit, 1);
        assert_eq!(s.answer_unstable, 1);
        let total: usize = s.used_fraction_histogram.iter().map(|b| b.checks).sum();
        assert_eq!(total, 3);
        assert_eq!(s.used_fraction_histogram.last().map(|b| b.checks), Some(0));
        let groups = group(&checks, |c| Some(c.session.clone()));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].stability, Stability::StableUnproved);
        assert_eq!(groups[1].stability, Stability::Unstable);
        assert_eq!(groups[1].min_proved_fraction, Some(0.5));
        checks[0].function = Some("f".to_owned());
        assert_eq!(group(&checks, |c| c.function.clone()).len(), 1);
    }

    #[test]
    fn attribution_only_when_unambiguous() {
        let p = obs("unsat", 10, None);
        let mut report = Report {
            solver: Identity {
                path: String::new(),
                version: None,
                sha256: None,
            },
            configs: vec![Config::BASELINE],
            config_labels: vec!["baseline".to_owned()],
            near_limit: 0.5,
            sessions: 1,
            summary: summarise(&[], 0.5, BaselineCheck::default()),
            checks: vec![analyse_check("a", 0, None, Some(100), &[Some(&p)])],
            functions: vec![],
            infra: vec![],
            unpaired_runs: vec![],
            replay_ms: 0,
        };
        assert!(!attribute_single_function(
            &mut report,
            &["m::f".to_owned(), "m::g".to_owned()]
        ));
        assert_eq!(report.checks[0].function, None);
        assert!(attribute_single_function(&mut report, &["m::f".to_owned()]));
        assert_eq!(report.checks[0].function.as_deref(), Some("m::f"));
        assert_eq!(report.functions.len(), 1);
        let text = render(&report, 5);
        assert!(text.contains("function m::f: 1 checks, stable-proved"));
    }

    #[test]
    fn exhaustion_reasons() {
        assert!(reason_is_exhaustion("(:reason-unknown \"canceled\")"));
        assert!(reason_is_exhaustion("max. resource limit exceeded"));
        assert!(!reason_is_exhaustion(
            "(:reason-unknown \"(incomplete quantifiers)\")"
        ));
    }
}
