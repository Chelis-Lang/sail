//! The fragility census (charter workload): capture every corpus unit's
//! solver sessions through the pass-through shim, re-run each session under
//! the seed portfolio ([`sail_portfolio::fragility`]), and report the
//! distribution of fragility per check, per unit and, where attribution is
//! unambiguous, per function.
//!
//! The capture runs plain Verus with `sail-z3` in pure pass-through (no
//! daemon, no cache), so every session is exactly what plain Verus sent the
//! pinned Z3. Verus runs one unit at a time; the replays run `jobs` solver
//! processes at a time. The census is a local workload (it takes minutes
//! to hours), never a CI job.
//!
//! **Function attribution.** The solver's input carries no function names.
//! A check is attributed to a function only when Verus's `--time` breakdown
//! for its unit lists exactly one function that used solver effort (for example a run restricted
//! with `--verify-function`). Otherwise results are per (session, check)
//! and per unit; ADR 0014 records the limitation, which the ledger's
//! function attribution will lift.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use sail_portfolio::fragility::{
    self, CheckFragility, GroupFragility, Options, Report, Stability, Summary, group,
};
use sail_portfolio::sessions::discover;
use sail_verus::corpus::Unit;
use sail_verus::run::{Solver as VerusSolver, run_unit};
use sail_verus::toolchain::Installed;

use crate::solver::{Identity, Solver};

/// One unit of the census.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CensusUnit {
    /// The corpus unit id.
    pub id: String,
    /// Verus's exit code on the capture run (`None` for an infrastructure
    /// failure).
    pub exit_code: Option<i32>,
    /// Functions in Verus's `--time` breakdown.
    pub functions: usize,
    /// Functions Verus reported as failing.
    pub failed_functions: Vec<String>,
    /// Captured solver sessions.
    pub sessions: usize,
    /// Checks measured.
    pub checks: usize,
    /// The worst class among its checks (`None` without checks).
    pub stability: Option<String>,
    /// Checks per class.
    pub by_class: BTreeMap<String, usize>,
    /// The largest check score.
    pub score: f64,
    /// The largest share of a limit any check used under any configuration.
    pub max_used_fraction: Option<f64>,
    /// The largest per-check effort CV.
    pub max_cv: Option<f64>,
    /// The capture's infrastructure error, if any.
    pub error: Option<String>,
}

/// A census result file (`bench/results/fragility-census-*.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CensusFile {
    /// Format version.
    pub schema_version: u32,
    /// Always `fragility`.
    pub workload: String,
    /// Corpus tier, or a label for a selection.
    pub tier: String,
    /// Pinned Verus release.
    pub verus_release: String,
    /// Pinned Z3 version.
    pub z3_version: String,
    /// The sail commit that produced it.
    pub sail_commit: String,
    /// Unix time the run started.
    pub started_unix: u64,
    /// Machine details.
    pub machine: BTreeMap<String, String>,
    /// The solver every configuration ran on.
    pub solver: Identity,
    /// Configuration labels, in per-check vector order.
    pub configs: Vec<String>,
    /// The near-limit reporting threshold.
    pub near_limit: f64,
    /// Extra Verus arguments added to every unit.
    #[serde(default)]
    pub extra_verus_args: Vec<String>,
    /// Per-unit results.
    pub units: Vec<CensusUnit>,
    /// Corpus-wide summary.
    pub summary: Summary,
    /// Per-function groups (attributed checks only).
    pub functions: Vec<GroupFragility>,
    /// Checks reported individually: every check that is not
    /// stable-proved or stable-unproved, every check at or above the
    /// near-limit threshold, every check where some configuration answered
    /// `unknown`, then the highest scores.
    pub notable: Vec<CheckFragility>,
    /// Incomplete runs (infrastructure, never solver answers).
    pub infra: Vec<String>,
    /// Runs with unpaired batches or solver errors.
    pub unpaired_runs: Vec<String>,
    /// Wall-clock time of the Verus captures, in milliseconds.
    pub capture_ms: u64,
    /// Total solver time of the replays, in milliseconds.
    pub replay_ms: u64,
}

/// Census options.
#[derive(Debug, Clone)]
pub struct CensusOptions {
    /// Portfolio options.
    pub portfolio: Options,
    /// Extra Verus arguments for every unit (for example
    /// `--verify-function allocate`).
    pub extra_verus_args: Vec<String>,
    /// How many top-scoring checks to keep beyond the always-kept ones.
    pub top: usize,
}

/// A trace subdirectory name for unit `index`.
fn unit_dir(index: usize) -> String {
    format!("u{index:04}")
}

/// What capturing one unit produced.
struct Capture {
    exit_code: Option<i32>,
    functions: Vec<String>,
    /// Functions that used solver effort (`rlimit > 0`); under
    /// `--verify-function` the others are listed but not verified.
    solved: Vec<String>,
    failed: Vec<String>,
    error: Option<String>,
}

/// The measured census: the portfolio report plus per-unit capture data.
pub struct Census {
    /// The portfolio report (checks attributed where unambiguous).
    pub report: Report,
    /// Per-unit results.
    pub units: Vec<CensusUnit>,
    /// Wall-clock time of the captures, in milliseconds.
    pub capture_ms: u64,
}

/// Runs Verus on each unit through the shim, tracing into
/// `<scratch>/traces/<unit dir>`.
fn capture_units(
    installed: &Installed,
    units: &[Unit],
    shim: &Path,
    scratch: &Path,
    options: &CensusOptions,
    progress: &impl Fn(&str),
) -> Vec<Capture> {
    let traces = scratch.join("traces");
    let mut captures = Vec::with_capacity(units.len());
    for (i, unit) in units.iter().enumerate() {
        let mut unit = unit.clone();
        unit.args.extend(options.extra_verus_args.iter().cloned());
        let dir = traces.join(unit_dir(i));
        let _ = std::fs::create_dir_all(&dir);
        let solver = VerusSolver::Shim {
            shim: shim.to_path_buf(),
            trace_dir: Some(dir),
            daemon_socket: None,
        };
        let capture = match run_unit(installed, &unit, &solver, &scratch.join("work")) {
            Ok(rec) => {
                let fns = rec.functions();
                Capture {
                    exit_code: rec.exit_code,
                    failed: fns
                        .iter()
                        .filter(|f| !f.success)
                        .map(|f| f.function.clone())
                        .collect(),
                    solved: fns
                        .iter()
                        .filter(|f| f.rlimit > 0)
                        .map(|f| f.function.clone())
                        .collect(),
                    functions: fns.into_iter().map(|f| f.function).collect(),
                    error: None,
                }
            }
            Err(e) => Capture {
                exit_code: None,
                functions: Vec::new(),
                solved: Vec::new(),
                failed: Vec::new(),
                error: Some(e.to_string()),
            },
        };
        progress(&format!(
            "captured {} ({} functions, exit {:?}){}",
            unit.id,
            capture.functions.len(),
            capture.exit_code,
            capture
                .error
                .as_deref()
                .map_or_else(String::new, |e| format!(": {e}"))
        ));
        captures.push(capture);
    }
    captures
}

/// Captures each unit's sessions into `traces/<unit dir>`, then measures
/// them all. `progress` gets a line per captured unit.
///
/// # Errors
///
/// A message if the trace directory cannot be read or the options are
/// invalid. Per-unit capture failures are recorded, not returned.
pub fn census(
    installed: &Installed,
    units: &[Unit],
    shim: &Path,
    scratch: &Path,
    options: &CensusOptions,
    progress: impl Fn(&str) + Sync,
) -> Result<Census, String> {
    let traces = scratch.join("traces");
    let started = std::time::Instant::now();
    let captures = capture_units(installed, units, shim, scratch, options, &progress);
    let capture_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = std::fs::create_dir_all(&traces);
    let sessions = discover(&traces).map_err(|e| e.to_string())?;
    let solver = Solver {
        path: installed.z3.clone(),
        timeout: std::time::Duration::from_secs(3600),
    };
    let mut report = fragility::measure(&solver, &sessions, &options.portfolio, |done, total| {
        if done.is_multiple_of(100) || done == total {
            progress(&format!("replayed {done}/{total}"));
        }
    })
    .map_err(|e| e.to_string())?;
    let unit_of = |session: &str| -> Option<usize> {
        let dir = session.split('/').next()?;
        dir.strip_prefix('u')?.parse().ok()
    };
    for c in &mut report.checks {
        if let Some(cap) = unit_of(&c.session).and_then(|i| captures.get(i))
            && let [only] = cap.solved.as_slice()
        {
            c.function = Some(only.clone());
        }
    }
    report.functions = group(&report.checks, |c| c.function.clone());
    let groups = group(&report.checks, |c| unit_of(&c.session).map(unit_dir));
    let by_dir: BTreeMap<&str, &GroupFragility> =
        groups.iter().map(|g| (g.name.as_str(), g)).collect();
    let session_counts = sessions.iter().fold(BTreeMap::new(), |mut m, s| {
        if let Some(i) = unit_of(&s.id) {
            *m.entry(i).or_insert(0usize) += 1;
        }
        m
    });
    let census_units = units
        .iter()
        .zip(&captures)
        .enumerate()
        .map(|(i, (unit, cap))| {
            let g = by_dir.get(unit_dir(i).as_str());
            CensusUnit {
                id: unit.id.clone(),
                exit_code: cap.exit_code,
                functions: cap.functions.len(),
                failed_functions: cap.failed.clone(),
                sessions: session_counts.get(&i).copied().unwrap_or(0),
                checks: g.map_or(0, |g| g.checks),
                stability: g.map(|g| g.stability.name().to_owned()),
                by_class: g.map(|g| g.by_class.clone()).unwrap_or_default(),
                score: g.map_or(0.0, |g| g.score),
                max_used_fraction: g.and_then(|g| g.max_used_fraction),
                max_cv: g.and_then(|g| g.max_cv),
                error: cap.error.clone(),
            }
        })
        .collect();
    // Report sessions by unit id rather than by scratch directory.
    for c in &mut report.checks {
        if let Some(i) = unit_of(&c.session)
            && let Some(unit) = units.get(i)
            && let Some((_, rest)) = c.session.split_once('/')
        {
            c.session = format!("{}/{rest}", unit.id);
        }
    }
    Ok(Census {
        report,
        units: census_units,
        capture_ms,
    })
}

/// The checks kept individually in a census file (see
/// [`CensusFile::notable`]).
#[must_use]
pub fn notable(report: &Report, top: usize) -> Vec<&CheckFragility> {
    let always = |c: &CheckFragility| {
        matches!(c.stability, Stability::Unstable | Stability::Incomplete)
            || c.max_used_fraction.is_some_and(|f| f >= report.near_limit)
            || c.answer_counts.contains_key("unknown")
    };
    let mut kept: Vec<&CheckFragility> = report.checks.iter().filter(|c| always(c)).collect();
    let mut rest: Vec<&CheckFragility> = report
        .checks
        .iter()
        .filter(|c| !always(c) && c.score > 0.0)
        .collect();
    rest.sort_by(|a, b| b.score.total_cmp(&a.score));
    kept.extend(rest.into_iter().take(top));
    kept
}

/// Assembles the result file. `meta` supplies the run's identity fields.
#[must_use]
pub fn file(census: &Census, options: &CensusOptions, meta: Meta) -> CensusFile {
    let r = &census.report;
    CensusFile {
        schema_version: 1,
        workload: "fragility".to_owned(),
        tier: meta.tier,
        verus_release: meta.verus_release,
        z3_version: meta.z3_version,
        sail_commit: meta.sail_commit,
        started_unix: meta.started_unix,
        machine: meta.machine,
        solver: r.solver.clone(),
        configs: r.config_labels.clone(),
        near_limit: r.near_limit,
        extra_verus_args: options.extra_verus_args.clone(),
        units: census.units.clone(),
        summary: r.summary.clone(),
        functions: r.functions.clone(),
        notable: notable(r, options.top).into_iter().cloned().collect(),
        infra: r.infra.clone(),
        unpaired_runs: r.unpaired_runs.clone(),
        capture_ms: census.capture_ms,
        replay_ms: r.replay_ms,
    }
}

/// Identity fields of a result file.
#[derive(Debug, Clone)]
pub struct Meta {
    /// Tier or selection label.
    pub tier: String,
    /// Pinned Verus release.
    pub verus_release: String,
    /// Pinned Z3 version.
    pub z3_version: String,
    /// The sail commit.
    pub sail_commit: String,
    /// Unix start time.
    pub started_unix: u64,
    /// Machine details.
    pub machine: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_portfolio::fragility::{BaselineCheck, analyse_check, summarise};
    use sail_portfolio::transcript::CheckObservation;

    fn obs(answer: &str, delta: u64) -> CheckObservation {
        CheckObservation {
            command_index: 0,
            answer: Some(answer.as_bytes().to_vec()),
            rlimit_before: Some(0),
            rlimit_delta: Some(delta),
            followups: vec![],
        }
    }

    #[test]
    fn notable_keeps_unstable_near_limit_and_top_scores() {
        let p = obs("unsat", 10);
        let big = obs("unsat", 800);
        let u = obs("unknown", 5);
        let checks = vec![
            analyse_check("a", 0, None, Some(1000), &[Some(&p), Some(&p)]),
            analyse_check("a", 1, None, Some(1000), &[Some(&p), Some(&u)]),
            analyse_check("a", 2, None, Some(1000), &[Some(&big), Some(&p)]),
            analyse_check(
                "a",
                3,
                None,
                Some(1000),
                &[Some(&p), Some(&obs("unsat", 20))],
            ),
        ];
        let report = Report {
            solver: Identity {
                path: String::new(),
                version: None,
                sha256: None,
            },
            configs: vec![],
            config_labels: vec![],
            near_limit: 0.5,
            sessions: 1,
            summary: summarise(&checks, 0.5, BaselineCheck::default()),
            checks,
            functions: vec![],
            infra: vec![],
            unpaired_runs: vec![],
            replay_ms: 0,
        };
        let kept: Vec<usize> = notable(&report, 1).iter().map(|c| c.seq).collect();
        assert_eq!(kept, [1, 2, 3]);
        let kept: Vec<usize> = notable(&report, 0).iter().map(|c| c.seq).collect();
        assert_eq!(kept, [1, 2]);
    }

    #[test]
    fn unit_dirs_sort_and_parse() {
        assert_eq!(unit_dir(7), "u0007");
        assert!(unit_dir(9) < unit_dir(10));
    }
}
