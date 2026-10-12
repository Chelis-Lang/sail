//! `sail-bench presolver-capture`: traced edit sequences for the pre-solver
//! coverage spike (experimental, rescue lane L6).
//!
//! A *sequence* is one corpus unit and an ordered list of edit cycles. Cycle
//! `c00-initial` verifies the unmodified unit; each later cycle verifies a
//! fresh copy of the unit with a chain of exact textual edits applied:
//! - agent loops (`corpus/agent-loops.toml`) are cumulative;
//! - catalogue edits (`corpus/edits.toml`) each start from the original;
//! - repair cycles (`corpus/repair-edits.json`) start from the original or
//!   from the state a named earlier cycle left.
//!
//! Every cycle runs Verus through `sail-z3` in pure pass-through with the
//! observation tap on (`SAIL_TRACE_DIR`), so each solver session leaves a
//! transcript under `<out>/<sequence>/<cycle>/`. The cache is never used:
//! the analysis (`sail-bench presolver`) reconstructs which checks a
//! default-mode cache would have missed from the recorded key-v2 values, by
//! accumulating keys over the earlier cycles of the same sequence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sail_verus::corpus::Unit;
use sail_verus::run::{Solver, copy_tree, run_unit};
use sail_verus::toolchain::Installed;
use serde::{Deserialize, Serialize};

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

/// Runs every cycle of `seq` on `unit`, tracing into
/// `<out>/<sequence>/<cycle>/`.
#[must_use]
pub fn run_sequence(
    installed: &Installed,
    shim: &Path,
    scratch: &Path,
    out: &Path,
    seq: &Sequence,
    unit: &Unit,
) -> Vec<CycleRecord> {
    let mut records = Vec::new();
    for c in &seq.cycles {
        let mut r = CycleRecord {
            sequence: seq.id.clone(),
            cycle: c.name.clone(),
            unit: seq.unit.clone(),
            expected: c.expected.clone(),
            ..CycleRecord::default()
        };
        let result = (|| -> Result<(f64, &'static str), String> {
            let src: PathBuf = std::path::absolute(scratch.join("presolver").join(&seq.id))
                .map_err(|e| e.to_string())?
                .join("src");
            let _ = std::fs::remove_dir_all(&src);
            copy_tree(&unit.source_dir, &src)
                .map_err(|e| format!("cannot copy {}: {e}", unit.source_dir.display()))?;
            apply_chain(&src, &c.chain)?;
            let mut copy = unit.clone();
            copy.source_dir.clone_from(&src);
            let trace =
                std::path::absolute(out.join(&seq.id).join(&c.name)).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_dir_all(&trace);
            std::fs::create_dir_all(&trace).map_err(|e| e.to_string())?;
            let solver = Solver::Shim {
                shim: shim.to_path_buf(),
                trace_dir: Some(trace),
                daemon_socket: None,
            };
            let run = run_unit(installed, &copy, &solver, &scratch.join("work"))
                .map_err(|e| e.to_string())?;
            Ok((run.wall_ms, edit::outcome(&run)))
        })();
        match result {
            Ok((ms, outcome)) => {
                r.wall_ms = Some(ms);
                r.outcome = Some(outcome.to_owned());
            }
            Err(e) => r.error = Some(e),
        }
        records.push(r);
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_chains_follow_their_base() {
        let dir = std::env::temp_dir().join(format!("sail-presolver-cap-{}", std::process::id()));
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
