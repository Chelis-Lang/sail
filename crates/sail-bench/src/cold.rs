//! The *cold* workload and the plain-Verus baseline.
//!
//! For each unit, every round runs plain Verus and Verus through `sail-z3`
//! (pass-through, no tracing), one at a time, alternating which goes first.
//! It records wall time, CPU time, peak RSS and Verus's reported solver time.
//! Every round also compares the two runs, as the charter requires of every
//! benchmark ("a speedup that coincides with any verdict difference is a
//! bug, not a result").

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sail_conformance::{Verdict, classify, compare};
use sail_verus::corpus::Unit;
use sail_verus::run::{RunRecord, Solver, run_unit};
use sail_verus::toolchain::Installed;
use serde::{Deserialize, Serialize};

use crate::daemon::CacheRun;
use crate::edit::EditResult;

/// Timings for one side of one unit, one sample per round.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Samples {
    /// Wall-clock milliseconds.
    pub wall_ms: Vec<f64>,
    /// User plus system CPU seconds of the process tree.
    pub cpu_s: Vec<f64>,
    /// Solver time Verus reports, in milliseconds.
    pub smt_ms: Vec<u64>,
    /// Peak RSS of the largest process, in bytes.
    pub max_rss_bytes: Vec<u64>,
}

impl Samples {
    /// Adds one run's figures.
    pub(crate) fn push(&mut self, r: &RunRecord) {
        self.wall_ms.push(r.wall_ms);
        if let (Some(u), Some(s)) = (r.user_s, r.sys_s) {
            self.cpu_s.push(u + s);
        }
        if let Some(t) = r.smt_ms() {
            self.smt_ms.push(t);
        }
        if let Some(m) = r.max_rss_bytes {
            self.max_rss_bytes.push(m);
        }
    }
}

/// The median of `xs`, or `None` if empty.
#[must_use]
pub fn median(xs: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = xs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[mid]
    } else {
        f64::midpoint(v[mid - 1], v[mid])
    })
}

/// One unit's results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResult {
    /// The unit id.
    pub id: String,
    /// Functions Verus reported.
    pub functions: usize,
    /// Plain Verus.
    pub plain: Samples,
    /// Verus through `sail-z3`, pass-through.
    pub sail: Samples,
    /// Rounds in which the two runs were the same under rule 1.
    pub same: usize,
    /// Rounds that differed (each one is a bug to investigate).
    pub different: usize,
    /// Rounds the harness could not run.
    pub infra_errors: usize,
    /// Rounds where sail differed from that round's plain run but matched
    /// another plain run of the unit while plain runs disagreed (ADR 0009).
    /// Not counted in `different`. Cached workloads only.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unstable: usize,
    /// Warm workload: the fill run (sail with an empty cache). In a `warm`
    /// file, [`Self::sail`] holds the warm runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Samples>,
    /// Cached workloads: the daemon's counters for each sail run, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache: Vec<CacheRun>,
    /// Cached workloads: bytes the cache held after the fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_bytes: Option<u64>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// How one sail run compares with plain Verus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgement {
    /// The same as this round's plain run.
    Same,
    /// Plain runs disagree among themselves and sail matches one (ADR 0009).
    Unstable,
    /// A difference: a bug, never a result.
    Different,
}

/// Judges `sail` against `plains`, whose last element is this round's plain
/// run (the others are earlier plain runs of the same unit). Returns the
/// judgement, the number of functions compared and the differences found
/// against this round's plain run.
#[must_use]
pub fn judge(plains: &[RunRecord], sail: &RunRecord) -> (Judgement, usize, Vec<String>) {
    let Some(this) = plains.last() else {
        return (Judgement::Different, 0, vec!["no plain run".to_owned()]);
    };
    let c = compare(this, sail);
    if c.verdict() == Verdict::Same {
        return (Judgement::Same, c.functions, Vec::new());
    }
    // `classify` compares with its first plain run: put this round's first.
    let mut ordered = vec![this.clone()];
    ordered.extend(plains.iter().take(plains.len() - 1).cloned());
    let j = match classify(&ordered, sail).verdict {
        Verdict::Unstable => Judgement::Unstable,
        _ => Judgement::Different,
    };
    (j, c.functions, c.differences)
}

/// Runs the cold workload over `units` for `rounds` rounds.
pub fn run(
    installed: &Installed,
    units: &[Unit],
    shim: &Path,
    rounds: usize,
    scratch: &Path,
    mut progress: impl FnMut(&UnitResult),
) -> Vec<UnitResult> {
    let mut results = Vec::new();
    for unit in units {
        let mut r = UnitResult {
            id: unit.id.clone(),
            functions: 0,
            plain: Samples::default(),
            sail: Samples::default(),
            same: 0,
            different: 0,
            infra_errors: 0,
            unstable: 0,
            fill: None,
            cache: Vec::new(),
            cache_bytes: None,
        };
        let sail_solver = Solver::Shim {
            shim: shim.to_path_buf(),
            trace_dir: None,
            daemon_socket: None,
            observation_logs: false,
        };
        let work: PathBuf = scratch.join("work");
        for round in 0..rounds {
            let plain_first = round % 2 == 0;
            let go =
                |solver: &Solver, side: &str| run_unit(installed, unit, solver, &work.join(side));
            let (p, s) = if plain_first {
                let p = go(&Solver::Plain, "plain");
                (p, go(&sail_solver, "sail"))
            } else {
                let s = go(&sail_solver, "sail");
                (go(&Solver::Plain, "plain"), s)
            };
            match (p, s) {
                (Ok(p), Ok(s)) => {
                    let c = compare(&p, &s);
                    r.functions = c.functions;
                    match c.verdict() {
                        Verdict::Same => r.same += 1,
                        // `compare` is pairwise and never yields `Unstable`;
                        // any non-same round is counted as a difference.
                        Verdict::Unstable
                        | Verdict::Different
                        | Verdict::InfraError
                        | Verdict::EngineLacksFeature => {
                            r.different += 1;
                        }
                    }
                    r.plain.push(&p);
                    r.sail.push(&s);
                }
                _ => r.infra_errors += 1,
            }
        }
        progress(&r);
        results.push(r);
    }
    results
}

/// Machine-readable benchmark results (`bench/results/*.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchFile {
    /// Format version.
    pub schema_version: u32,
    /// Workload name: `cold`, `warm`, `edit`, `agent-loop` or `history-replay`.
    pub workload: String,
    /// Corpus tier.
    pub tier: String,
    /// Rounds per unit (`warm`: warm rounds after the fill; `edit`: rounds
    /// per edit).
    pub rounds: usize,
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
    /// Per-unit results (`cold`, `warm`).
    #[serde(default)]
    pub units: Vec<UnitResult>,
    /// Per-edit results (`edit`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<EditResult>,
    /// Per-loop results (`agent-loop`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loops: Vec<crate::agent_loop::LoopResult>,
    /// Per-commit results (`history-replay`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<crate::replay::CommitResult>,
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn medians() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[f64::NAN, 1.0]), Some(1.0));
    }
}
