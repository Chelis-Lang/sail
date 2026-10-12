//! `sail-bench warm-capture`: cached edit sequences with per-session timing
//! for rescue lane L11 (warm exact-prefix solver sessions; experimental).
//!
//! Sequences are built as in the L6 capture (agent loops cumulative,
//! catalogue edits from the original, repair cycles from the original or a
//! named earlier cycle). Unlike L6, every round of a sequence runs against
//! one fresh `sail serve` daemon, so cycle N is served from the cache the
//! earlier cycles filled, exactly as in an agent's edit loop. Each cycle
//! runs Verus through `sail-z3` with the daemon socket and `SAIL_TRACE_DIR`
//! set, so the shim's L11 timing writes one record and the input per
//! session under `<out>/r<round>/<sequence>/<cycle>/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sail_verus::corpus::Unit;
use sail_verus::run::{Solver, copy_tree, run_unit};
use sail_verus::toolchain::Installed;
use serde::{Deserialize, Serialize};

use crate::daemon::Daemon;
use crate::{agent_loop, edit};

/// One exact textual edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// File relative to the unit's source directory.
    pub file: String,
    /// Text that must occur exactly once.
    pub find: String,
    /// Its replacement.
    pub replace: String,
}

/// One cycle of a sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle {
    /// Directory name, `cNN-<id>`.
    pub name: String,
    /// The edits applied to a fresh copy of the original, in order.
    pub chain: Vec<Change>,
    /// The catalogue's expected plain-Verus outcome, if it records one.
    pub expected: Option<String>,
}

/// A unit and its cycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    /// Directory name.
    pub id: String,
    /// The corpus unit id.
    pub unit: String,
    /// Cycle 0 is always the unmodified unit.
    pub cycles: Vec<Cycle>,
}

fn initial() -> Cycle {
    Cycle {
        name: "c00-initial".to_owned(),
        chain: Vec::new(),
        expected: None,
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Agent loops: one cumulative sequence per loop.
#[must_use]
pub fn from_loops(c: &agent_loop::Catalogue) -> Vec<Sequence> {
    c.loops
        .iter()
        .map(|l| {
            let mut cycles = vec![initial()];
            let mut chain = Vec::new();
            for (i, s) in l.steps.iter().enumerate() {
                chain.push(Change {
                    file: s.file.clone(),
                    find: s.find.clone(),
                    replace: s.replace.clone(),
                });
                cycles.push(Cycle {
                    name: format!("c{:02}-{}", i + 1, slug(&s.id)),
                    chain: chain.clone(),
                    expected: Some(s.expected.clone()),
                });
            }
            Sequence {
                id: format!("loop-{}", slug(&l.id)),
                unit: l.unit.clone(),
                cycles,
            }
        })
        .collect()
}

/// Catalogue edits: one sequence per unit, each edit from the original.
#[must_use]
pub fn from_edits(c: &edit::Catalogue) -> Vec<Sequence> {
    let mut by_unit: BTreeMap<&str, Vec<&edit::Edit>> = BTreeMap::new();
    for e in &c.edits {
        by_unit.entry(e.unit.as_str()).or_default().push(e);
    }
    by_unit
        .into_iter()
        .map(|(unit, edits)| {
            let mut cycles = vec![initial()];
            for (i, e) in edits.iter().enumerate() {
                cycles.push(Cycle {
                    name: format!("c{:02}-{}", i + 1, slug(&e.id)),
                    chain: vec![Change {
                        file: e.file.clone(),
                        find: e.find.clone(),
                        replace: e.replace.clone(),
                    }],
                    expected: Some(e.expected.clone()),
                });
            }
            Sequence {
                id: format!("edits-{}", slug(unit)),
                unit: unit.to_owned(),
                cycles,
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct RepairFile {
    projects: Vec<RepairProject>,
}

#[derive(Debug, Deserialize)]
struct RepairProject {
    id: String,
    root: String,
    cycles: Vec<RepairCycle>,
}

#[derive(Debug, Deserialize)]
struct RepairCycle {
    id: String,
    file: String,
    expected: String,
    from: String,
    find: String,
    replace: String,
}

/// Repair cycles: one sequence per project. `units` maps the project to its
/// corpus unit id (`<project>/<root>`).
///
/// # Errors
///
/// A message if the file cannot be read or parsed, or a cycle starts from a
/// cycle that does not precede it.
pub fn from_repair(path: &Path) -> Result<Vec<Sequence>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let f: RepairFile =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for p in f.projects {
        let mut chains: BTreeMap<String, Vec<Change>> = BTreeMap::new();
        let mut cycles = vec![initial()];
        for (i, c) in p.cycles.iter().enumerate() {
            let mut chain = if c.from == "original" {
                Vec::new()
            } else {
                chains
                    .get(&c.from)
                    .cloned()
                    .ok_or_else(|| format!("{}/{}: unknown base {}", p.id, c.id, c.from))?
            };
            chain.push(Change {
                file: c.file.clone(),
                find: c.find.clone(),
                replace: c.replace.clone(),
            });
            chains.insert(c.id.clone(), chain.clone());
            cycles.push(Cycle {
                name: format!("c{:02}-{}", i + 1, slug(&c.id)),
                chain,
                expected: Some(c.expected.clone()),
            });
        }
        out.push(Sequence {
            id: format!("repair-{}", slug(&p.id)),
            unit: format!("{}/{}", p.id, p.root),
            cycles,
        });
    }
    Ok(out)
}

/// What one cycle's capture produced.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CycleRecord {
    /// Sequence directory name.
    pub sequence: String,
    /// Cycle directory name.
    pub cycle: String,
    /// The unit.
    pub unit: String,
    /// `verified` or `errors`, if Verus ran.
    pub outcome: Option<String>,
    /// The catalogue's expectation.
    pub expected: Option<String>,
    /// Verus wall-clock milliseconds.
    pub wall_ms: Option<f64>,
    /// Infrastructure error, if the cycle could not run.
    pub error: Option<String>,
    /// Round number.
    pub round: usize,
    /// Daemon solver starts during the cached arm, if known.
    pub solver_starts: Option<u64>,
    /// Every arm's run.
    pub arms: Vec<ArmRecord>,
}

fn apply_chain(src: &Path, chain: &[Change]) -> Result<(), String> {
    for c in chain {
        let file = src.join(&c.file);
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        let n = text.matches(&c.find).count();
        if n != 1 {
            return Err(format!(
                "`find` occurs {n} times in {} (it must occur exactly once)",
                c.file
            ));
        }
        std::fs::write(&file, text.replacen(&c.find, &c.replace, 1))
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    }
    Ok(())
}

/// One arm's run of a cycle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArmRecord {
    /// `sail` (cached), `plain` or `fresh` (pass-through with the tap).
    pub arm: String,
    /// `verified` or `errors`, if Verus ran.
    pub outcome: Option<String>,
    /// Verus wall-clock milliseconds.
    pub wall_ms: Option<f64>,
    /// Unix nanoseconds when Verus was started.
    pub started_ns: String,
    /// Unix nanoseconds when Verus returned.
    pub ended_ns: String,
    /// Infrastructure error.
    pub error: Option<String>,
}

/// Runs every cycle of `seq` on `unit`: the cached arm against one fresh
/// daemon per round, plain Verus, and (round 0 only) a traced pass-through
/// arm. Traces go to `<out>/r<round>/<sequence>/<cycle>/<arm>/`.
#[must_use]
#[allow(clippy::too_many_lines, reason = "L11 experimental capture")]
pub fn run_sequence(
    installed: &Installed,
    (shim, sail): (&Path, &Path),
    scratch: &Path,
    out: &Path,
    (seq, round): (&Sequence, usize),
    unit: &Unit,
) -> Vec<CycleRecord> {
    let mut records = Vec::new();
    let cold = seq.id.starts_with("cold-");
    let socket = scratch.join("l11.sock");
    let daemon = if cold {
        None
    } else {
        match Daemon::start(sail, &socket, &scratch.join("l11-cache")) {
            Ok(d) => Some(d),
            Err(e) => {
                records.push(CycleRecord {
                    sequence: seq.id.clone(),
                    round,
                    error: Some(e),
                    ..CycleRecord::default()
                });
                return records;
            }
        }
    };
    for c in &seq.cycles {
        let mut r = CycleRecord {
            sequence: seq.id.clone(),
            cycle: c.name.clone(),
            unit: seq.unit.clone(),
            expected: c.expected.clone(),
            round,
            ..CycleRecord::default()
        };
        let prepared = (|| -> Result<Unit, String> {
            let src: PathBuf = std::path::absolute(scratch.join("l11").join(&seq.id))
                .map_err(|e| e.to_string())?
                .join("src");
            let _ = std::fs::remove_dir_all(&src);
            copy_tree(&unit.source_dir, &src)
                .map_err(|e| format!("cannot copy {}: {e}", unit.source_dir.display()))?;
            apply_chain(&src, &c.chain)?;
            let mut copy = unit.clone();
            copy.source_dir.clone_from(&src);
            Ok(copy)
        })();
        let copy = match prepared {
            Ok(u) => u,
            Err(e) => {
                r.error = Some(e);
                records.push(r);
                continue;
            }
        };
        let mut arms: Vec<&str> = if cold {
            vec![]
        } else if round % 2 == 0 {
            vec!["sail", "plain"]
        } else {
            vec!["plain", "sail"]
        };
        if round == 0 {
            arms.push("fresh");
        }
        for arm in arms {
            let mut a = ArmRecord {
                arm: arm.to_owned(),
                ..ArmRecord::default()
            };
            let trace = std::path::absolute(
                out.join(format!("r{round}"))
                    .join(&seq.id)
                    .join(&c.name)
                    .join(arm),
            )
            .unwrap_or_default();
            let _ = std::fs::remove_dir_all(&trace);
            let _ = std::fs::create_dir_all(&trace);
            let solver = match arm {
                "sail" => Solver::Shim {
                    shim: shim.to_path_buf(),
                    trace_dir: Some(trace),
                    daemon_socket: daemon.as_ref().map(|d| d.socket().to_path_buf()),
                },
                "fresh" => Solver::Shim {
                    shim: shim.to_path_buf(),
                    trace_dir: Some(trace),
                    daemon_socket: None,
                },
                _ => Solver::Plain,
            };
            let before = daemon.as_ref().and_then(Daemon::stats);
            a.started_ns = now_ns();
            let run = run_unit(installed, &copy, &solver, &scratch.join("work"));
            a.ended_ns = now_ns();
            if arm == "sail"
                && let (Some(b), Some(after)) = (before, daemon.as_ref().and_then(Daemon::stats))
            {
                r.solver_starts =
                    Some(crate::daemon::CacheRun::between("cycle", &b, &after).solver_starts());
            }
            match run {
                Ok(run) => {
                    a.wall_ms = Some(run.wall_ms);
                    a.outcome = Some(edit::outcome(&run).to_owned());
                }
                Err(e) => a.error = Some(e.to_string()),
            }
            if arm == "sail" || (cold && arm == "fresh") {
                r.wall_ms = a.wall_ms;
                r.outcome.clone_from(&a.outcome);
                r.error.clone_from(&a.error);
            }
            r.arms.push(a);
        }
        records.push(r);
    }
    records
}

fn now_ns() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_chains_follow_their_base() {
        let dir = std::env::temp_dir().join(format!("sail-warmcap-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("r.json");
        let json = r#"{"projects":[{"id":"p","root":"lib.rs","cycles":[
            {"id":"a","file":"f.rs","expected":"errors","from":"original","find":"x","replace":"y"},
            {"id":"b","file":"f.rs","expected":"verified","from":"a","find":"y","replace":"x"}]}]}"#;
        assert!(std::fs::write(&path, json).is_ok());
        let seqs = from_repair(&path);
        let Ok(seqs) = seqs else {
            unreachable!("{seqs:?}")
        };
        assert_eq!(seqs.len(), 1);
        assert_eq!(seqs[0].unit, "p/lib.rs");
        assert_eq!(seqs[0].cycles.len(), 3);
        assert_eq!(seqs[0].cycles[2].chain.len(), 2);
        assert_eq!(seqs[0].cycles[2].name, "c02-b");
        let src = dir.join("src");
        let _ = std::fs::create_dir_all(&src);
        assert!(std::fs::write(src.join("f.rs"), "x").is_ok());
        assert!(apply_chain(&src, &seqs[0].cycles[2].chain).is_ok());
        assert_eq!(
            std::fs::read_to_string(src.join("f.rs")).ok().as_deref(),
            Some("x")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
