//! The agent-loop workload (charter: "Agent loop simulation").
//!
//! A loop is a sequence of small edits, applied cumulatively to one scratch
//! copy of a project (never the fetched tree), with a verification after
//! each: the edit-and-check cycle of a coding agent. Per round, every loop
//! gets a fresh `sail serve` with an empty cache. Cycle 0 verifies the
//! original, which fills the cache, and each later cycle runs after the next
//! step, so it is warm from all the cycles before it. Every cycle runs plain
//! Verus and Verus through `sail-z3`, and every sail run is judged against
//! plain Verus.
//! The measure is time to verdict per cycle.

use std::path::{Path, PathBuf};

use sail_verus::corpus::Unit;
use sail_verus::run::{RunRecord, Solver, copy_tree, run_unit};
use serde::{Deserialize, Serialize};

use crate::cold::{Judgement, Samples, judge};
use crate::daemon::{CacheRun, Daemon};
use crate::edit::outcome;
use crate::warm::Setup;

/// The loop catalogue (`corpus/agent-loops.toml`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// Format version.
    pub schema_version: u32,
    /// The loops.
    #[serde(rename = "loop", default)]
    pub loops: Vec<Loop>,
}

/// One agent loop.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    /// Unique id.
    pub id: String,
    /// The corpus unit it edits.
    pub unit: String,
    /// What the loop models.
    pub description: String,
    /// The edits, applied in order and cumulatively.
    #[serde(rename = "step", default)]
    pub steps: Vec<Step>,
}

/// One edit in a loop.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Id, unique within the loop.
    pub id: String,
    /// File to edit, relative to the project's source directory.
    pub file: String,
    /// What the step does.
    pub description: String,
    /// Plain Verus's outcome after the step: `verified` or `errors`.
    pub expected: String,
    /// Exact text to replace; must occur once at that point in the loop.
    pub find: String,
    /// Replacement text.
    pub replace: String,
}

impl Catalogue {
    /// Reads and checks a catalogue.
    ///
    /// # Errors
    ///
    /// A message if it cannot be read or parsed, or fails [`Self::check`].
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let c: Self =
            toml::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
        c.check()?;
        Ok(c)
    }

    /// Checks ids, expectations and paths.
    ///
    /// # Errors
    ///
    /// A message for the first problem found.
    pub fn check(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported schema_version {}",
                self.schema_version
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for l in &self.loops {
            if !ids.insert(&l.id) {
                return Err(format!("duplicate loop id {}", l.id));
            }
            if l.steps.is_empty() {
                return Err(format!("{}: a loop needs at least one step", l.id));
            }
            let mut step_ids = std::collections::BTreeSet::new();
            for s in &l.steps {
                if !step_ids.insert(&s.id) {
                    return Err(format!("{}: duplicate step id {}", l.id, s.id));
                }
                if !matches!(s.expected.as_str(), "verified" | "errors") {
                    return Err(format!(
                        "{}/{}: expected must be verified or errors",
                        l.id, s.id
                    ));
                }
                if s.find == s.replace {
                    return Err(format!("{}/{}: the step changes nothing", l.id, s.id));
                }
                if Path::new(&s.file).is_absolute() || s.file.contains("..") {
                    return Err(format!(
                        "{}/{}: file must be a relative path inside the project",
                        l.id, s.id
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Applies `step` to `text`.
///
/// # Errors
///
/// A message unless `step.find` occurs exactly once.
pub fn apply(step: &Step, text: &str) -> Result<String, String> {
    match text.matches(&step.find).count() {
        1 => Ok(text.replacen(&step.find, &step.replace, 1)),
        n => Err(format!(
            "step {}: `find` occurs {n} times in {} (it must occur exactly once)",
            step.id, step.file
        )),
    }
}

/// One cycle's results, over all rounds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CycleResult {
    /// `initial`, or the step id.
    pub step: String,
    /// What the step did.
    pub description: String,
    /// Plain Verus's expected outcome (`initial` has none).
    pub expected: Option<String>,
    /// Plain Verus's observed outcome (last round).
    pub outcome: Option<String>,
    /// Plain Verus, per round.
    pub plain: Samples,
    /// sail with the loop's warm cache, per round.
    pub sail: Samples,
    /// Cache counters per round.
    pub cache: Vec<CacheRun>,
    /// Rounds that were the same as plain Verus.
    pub same: usize,
    /// Rounds that differed (each one a bug).
    pub different: usize,
    /// Rounds where plain Verus varied and sail matched one run.
    pub unstable: usize,
    /// Rounds the harness could not run.
    pub infra_errors: usize,
}

/// One loop's results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoopResult {
    /// The loop id.
    pub id: String,
    /// The unit it edits.
    pub unit: String,
    /// What it models.
    pub description: String,
    /// Cycle 0 (`initial`) and one cycle per step.
    pub cycles: Vec<CycleResult>,
}

/// Matches catalogue loops to selected units.
///
/// # Errors
///
/// A message naming a loop whose unit is not selected.
pub fn resolve<'a>(
    c: &'a Catalogue,
    units: &'a [Unit],
) -> Result<Vec<(&'a Loop, &'a Unit)>, String> {
    c.loops
        .iter()
        .map(|l| {
            let unit = units
                .iter()
                .find(|u| u.id == l.unit)
                .ok_or_else(|| format!("{}: unit {} is not in the corpus", l.id, l.unit))?;
            Ok((l, unit))
        })
        .collect()
}

/// Runs plain and sail on `unit` (alternating order by `round`), records the
/// cache counters and judges the pair.
fn cycle(
    setup: &Setup,
    c: &mut CycleResult,
    (unit, round): (&Unit, usize),
    daemon: &Daemon,
    plains: &mut Vec<RunRecord>,
) {
    let sail_solver = Solver::Shim {
        shim: setup.shim.to_path_buf(),
        trace_dir: None,
        daemon_socket: Some(daemon.socket().to_path_buf()),
        observation_logs: false,
    };
    let work = setup.scratch.join("work");
    let go =
        |solver: &Solver, side: &str| run_unit(setup.installed, unit, solver, &work.join(side));
    let before = daemon.stats();
    let (p, s) = if round % 2 == 0 {
        let p = go(&Solver::Plain, "plain");
        (p, go(&sail_solver, "sail"))
    } else {
        let s = go(&sail_solver, "sail");
        (go(&Solver::Plain, "plain"), s)
    };
    if let (Some(b), Some(a)) = (before, daemon.stats()) {
        c.cache.push(CacheRun::between("cycle", &b, &a));
    }
    let (Ok(p), Ok(s)) = (p, s) else {
        c.infra_errors += 1;
        return;
    };
    c.outcome = Some(outcome(&p).to_owned());
    c.plain.push(&p);
    c.sail.push(&s);
    plains.push(p);
    let (j, _, differences) = judge(plains, &s);
    match j {
        Judgement::Same => c.same += 1,
        Judgement::Unstable => c.unstable += 1,
        Judgement::Different => {
            c.different += 1;
            eprintln!(
                "{} (round {round}) DIFFERENT: {}",
                c.step,
                differences.join("; ")
            );
        }
    }
}

/// Runs every loop for `rounds` rounds.
pub fn run(
    setup: &Setup,
    loops: &[(&Loop, &Unit)],
    rounds: usize,
    mut progress: impl FnMut(&LoopResult),
) -> Vec<LoopResult> {
    let mut results = Vec::new();
    for (i, (l, unit)) in loops.iter().enumerate() {
        let r = run_one(setup, i, l, unit, rounds);
        progress(&r);
        results.push(r);
    }
    results
}

fn run_one(setup: &Setup, index: usize, l: &Loop, unit: &Unit, rounds: usize) -> LoopResult {
    let mut r = LoopResult {
        id: l.id.clone(),
        unit: l.unit.clone(),
        description: l.description.clone(),
        cycles: std::iter::once(CycleResult {
            step: "initial".to_owned(),
            description: "verify the original (fills the cache)".to_owned(),
            ..CycleResult::default()
        })
        .chain(l.steps.iter().map(|s| CycleResult {
            step: s.id.clone(),
            description: s.description.clone(),
            expected: Some(s.expected.clone()),
            ..CycleResult::default()
        }))
        .collect(),
    };
    let mut plains: Vec<Vec<RunRecord>> = vec![Vec::new(); r.cycles.len()];
    for round in 0..rounds {
        let src: PathBuf = setup.scratch.join("loops").join(&l.id).join("src");
        let Ok(src) = std::path::absolute(&src) else {
            continue;
        };
        let _ = std::fs::remove_dir_all(&src);
        let cache_root = setup
            .scratch
            .join("cache")
            .join(format!("l{index}-{round}"));
        let fail = |r: &mut LoopResult, e: String| {
            eprintln!("{}: infra error: {e}", l.id);
            for c in &mut r.cycles {
                c.infra_errors += 1;
            }
        };
        if let Err(e) = copy_tree(&unit.source_dir, &src) {
            fail(
                &mut r,
                format!("cannot copy {}: {e}", unit.source_dir.display()),
            );
            continue;
        }
        let daemon = match Daemon::start(setup.sail, &setup.socket(), &cache_root) {
            Ok(d) => d,
            Err(e) => {
                fail(&mut r, e);
                continue;
            }
        };
        let mut copy = unit.clone();
        copy.source_dir.clone_from(&src);
        for (k, (c, ps)) in r.cycles.iter_mut().zip(plains.iter_mut()).enumerate() {
            if let Some(step) = k.checked_sub(1).and_then(|i| l.steps.get(i)) {
                let file = src.join(&step.file);
                let applied = std::fs::read_to_string(&file)
                    .map_err(|e| format!("cannot read {}: {e}", file.display()))
                    .and_then(|text| apply(step, &text))
                    .and_then(|text| std::fs::write(&file, text).map_err(|e| e.to_string()));
                if let Err(e) = applied {
                    eprintln!("{}: infra error: {e}", l.id);
                    c.infra_errors += 1;
                    break;
                }
            }
            cycle(setup, c, (&copy, round), &daemon, ps);
        }
        drop(daemon);
        let _ = std::fs::remove_dir_all(&cache_root);
        let _ = std::fs::remove_dir_all(&src);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(find: &str, replace: &str) -> Step {
        Step {
            id: "s".to_owned(),
            file: "a.rs".to_owned(),
            description: String::new(),
            expected: "verified".to_owned(),
            find: find.to_owned(),
            replace: replace.to_owned(),
        }
    }

    #[test]
    fn steps_apply_exactly_once_and_cumulatively() {
        let t = apply(&step("b", "x"), "abc");
        assert_eq!(t.as_deref(), Ok("axc"));
        let t2 = t.and_then(|t| apply(&step("x", "y"), &t));
        assert_eq!(t2.as_deref(), Ok("ayc"));
        assert!(apply(&step("z", "x"), "abc").is_err());
        assert!(apply(&step("a", "x"), "aa").is_err());
    }

    #[test]
    fn the_shipped_catalogue_parses_and_checks() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/agent-loops.toml");
        let c = Catalogue::load(&path);
        assert!(c.as_ref().is_ok_and(|c| !c.loops.is_empty()), "{c:?}");
    }

    #[test]
    fn checks_reject_bad_catalogues() {
        let good = Catalogue {
            schema_version: 1,
            loops: vec![Loop {
                id: "l".to_owned(),
                unit: "p/u.rs".to_owned(),
                description: String::new(),
                steps: vec![step("a", "b")],
            }],
        };
        assert!(good.check().is_ok());
        let mut bad = good.clone();
        bad.loops[0].steps.clear();
        assert!(bad.check().is_err());
        let mut bad = good.clone();
        bad.loops[0].steps[0].expected = "maybe".to_owned();
        assert!(bad.check().is_err());
        let mut bad = good;
        bad.loops[0].steps[0].file = "../x.rs".to_owned();
        assert!(bad.check().is_err());
    }
}
