//! The *single-function body edit* and *specification edit* workloads
//! (charter "How to prove it's useful"; ADR 0012).
//!
//! Edits come from a reviewable catalogue (`corpus/edits.toml`): each is
//! one exact textual replacement in one file of a corpus unit, with the
//! outcome plain Verus gives on the edited file. For each edit, the unit's
//! project is copied to a scratch directory (the fetched corpus tree is
//! never modified). Each round then:
//! 1. starts a fresh `sail serve` with an empty cache;
//! 2. *fill*: runs plain Verus and sail with the cache on the original
//!    copy, and compares them;
//! 3. applies the edit to the copy;
//! 4. *edit*: runs plain Verus and sail with the cache on the edited copy,
//!    and compares them;
//! 5. restores the original text.
//!
//! Both sides of every comparison run on the same copy, so paths in
//! diagnostics match.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sail_verus::corpus::{Build, Unit};
use sail_verus::run::{RunRecord, Solver, copy_tree, run_unit};
use serde::{Deserialize, Serialize};

use crate::cold::{Judgement, Samples, judge};
use crate::daemon::{CacheRun, Daemon};
use crate::warm::Setup;

/// The edit catalogue file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// Format version (1).
    pub schema_version: u32,
    /// The edits.
    #[serde(rename = "edit", default)]
    pub edits: Vec<Edit>,
}

/// One exact textual edit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    /// Unique id.
    pub id: String,
    /// The corpus unit it belongs to.
    pub unit: String,
    /// `body` (a function body or proof only) or `spec` (a signature's
    /// `requires`/`ensures`).
    pub kind: String,
    /// The edited file, relative to the project's source directory.
    pub file: String,
    /// What the edit does, for the report.
    pub description: String,
    /// Plain Verus's outcome on the edited file: `verified` or `errors`
    /// (`pending` in a generated mutation catalogue before its oracle run).
    pub expected: String,
    /// Text that occurs exactly once in the file.
    pub find: String,
    /// Its replacement.
    pub replace: String,
}

impl Catalogue {
    /// Loads and checks a catalogue.
    ///
    /// # Errors
    ///
    /// A message for unreadable or malformed files, duplicate ids, unknown
    /// kinds or outcomes, and no-op edits.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let c: Self = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        c.check().map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(c)
    }

    fn check(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported schema_version {}",
                self.schema_version
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for e in &self.edits {
            if !seen.insert(e.id.as_str()) {
                return Err(format!("duplicate edit id `{}`", e.id));
            }
            if !matches!(e.kind.as_str(), "body" | "spec") {
                return Err(format!("{}: kind must be `body` or `spec`", e.id));
            }
            // `pending`: a generated mutant not yet settled by the oracle
            // (ADR 0063); the workloads refuse to run it.
            if !matches!(e.expected.as_str(), "verified" | "errors" | "pending") {
                return Err(format!(
                    "{}: expected must be `verified`, `errors` or `pending`",
                    e.id
                ));
            }
            if e.find.is_empty() || e.find == e.replace {
                return Err(format!("{}: the edit changes nothing", e.id));
            }
            if Path::new(&e.file).is_absolute() || e.file.contains("..") {
                return Err(format!(
                    "{}: file must be a relative path inside the project",
                    e.id
                ));
            }
        }
        Ok(())
    }
}

/// Applies `edit` to `original`.
///
/// # Errors
///
/// A message unless `edit.find` occurs exactly once.
pub fn apply(edit: &Edit, original: &str) -> Result<String, String> {
    match original.matches(&edit.find).count() {
        1 => Ok(original.replacen(&edit.find, &edit.replace, 1)),
        n => Err(format!(
            "{}: `find` occurs {n} times in {} (it must occur exactly once)",
            edit.id, edit.file
        )),
    }
}

/// `verified` if Verus exited cleanly with no verification errors, else
/// `errors`.
#[must_use]
pub fn outcome(r: &RunRecord) -> &'static str {
    if r.exit_code == Some(0) && r.total("errors").unwrap_or(0) == 0 {
        "verified"
    } else {
        "errors"
    }
}

/// One edit's results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditResult {
    /// The edit id.
    pub id: String,
    /// The corpus unit.
    pub unit: String,
    /// `body` or `spec`.
    pub kind: String,
    /// What the edit does.
    pub description: String,
    /// The outcome the catalogue expects from plain Verus on the edit.
    pub expected: String,
    /// What plain Verus gave on the edited copy (last round).
    pub outcome: Option<String>,
    /// Functions Verus reported on the edited copy.
    pub functions: usize,
    /// Plain Verus on the original copy.
    pub plain_original: Samples,
    /// sail with an empty cache on the original copy.
    pub fill: Samples,
    /// Plain Verus on the edited copy.
    pub plain: Samples,
    /// sail with the filled cache on the edited copy.
    pub sail: Samples,
    /// The daemon's counters per sail run (`fill`, `edit`).
    pub cache: Vec<CacheRun>,
    /// Comparisons (fill and edit) that were the same.
    pub same: usize,
    /// Comparisons that differed: each one is a bug.
    pub different: usize,
    /// Comparisons that matched another plain run while plain runs
    /// disagreed (ADR 0009).
    pub unstable: usize,
    /// Rounds the harness could not run.
    pub infra_errors: usize,
}

/// Finds each edit's unit in `units` (matched by id).
///
/// # Errors
///
/// A message for an edit whose unit is not among `units` or is not a
/// `verus` unit (Cargo units are not supported yet).
pub fn resolve<'a>(
    catalogue: &'a Catalogue,
    units: &'a [Unit],
) -> Result<Vec<(&'a Edit, &'a Unit)>, String> {
    let by_id: BTreeMap<&str, &Unit> = units.iter().map(|u| (u.id.as_str(), u)).collect();
    catalogue
        .edits
        .iter()
        .map(|e| {
            let unit = by_id.get(e.unit.as_str()).ok_or_else(|| {
                format!("{}: unit `{}` is not in the selected corpus", e.id, e.unit)
            })?;
            if unit.build != Build::Verus {
                return Err(format!(
                    "{}: only `verus` units can be edited for now",
                    e.id
                ));
            }
            Ok((e, *unit))
        })
        .collect()
}

struct Copy {
    unit: Unit,
    file: PathBuf,
    original: String,
    edited: String,
}

/// Copies the unit's project to `<scratch>/edits/<id>/src` and prepares the
/// edited text.
fn copy_for(setup: &Setup, edit: &Edit, unit: &Unit) -> Result<Copy, String> {
    let src = setup.scratch.join("edits").join(&edit.id).join("src");
    let src = std::path::absolute(&src).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&src);
    copy_tree(&unit.source_dir, &src)
        .map_err(|e| format!("cannot copy {}: {e}", unit.source_dir.display()))?;
    let file = src.join(&edit.file);
    let original = std::fs::read_to_string(&file)
        .map_err(|e| format!("{}: cannot read {}: {e}", edit.id, file.display()))?;
    let edited = apply(edit, &original)?;
    let mut copy = unit.clone();
    copy.source_dir = src;
    Ok(Copy {
        unit: copy,
        file,
        original,
        edited,
    })
}

/// Runs plain and sail (alternating order by `round`), reads the cache
/// counters around them and records the comparison.
fn pair(
    setup: &Setup,
    r: &mut EditResult,
    (unit, round, phase): (&Unit, usize, &str),
    daemon: &Daemon,
    plains: &mut Vec<RunRecord>,
) -> Option<(RunRecord, RunRecord)> {
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
        r.cache.push(CacheRun::between(phase, &b, &a));
    }
    let (Ok(p), Ok(s)) = (p, s) else {
        r.infra_errors += 1;
        return None;
    };
    plains.push(p.clone());
    let (j, functions, differences) = judge(plains, &s);
    if phase == "edit" {
        r.functions = functions;
    }
    match j {
        Judgement::Same => r.same += 1,
        Judgement::Unstable => r.unstable += 1,
        Judgement::Different => {
            r.different += 1;
            eprintln!(
                "{} ({phase} round {round}) DIFFERENT: {}",
                r.id,
                differences.join("; ")
            );
        }
    }
    Some((p, s))
}

/// Runs every edit for `rounds` rounds.
pub fn run(
    setup: &Setup,
    edits: &[(&Edit, &Unit)],
    rounds: usize,
    mut progress: impl FnMut(&EditResult),
) -> Vec<EditResult> {
    let mut results = Vec::new();
    for (i, (edit, unit)) in edits.iter().enumerate() {
        let r = run_one(setup, i, edit, unit, rounds);
        progress(&r);
        results.push(r);
    }
    results
}

fn run_one(setup: &Setup, index: usize, edit: &Edit, unit: &Unit, rounds: usize) -> EditResult {
    let mut r = EditResult {
        id: edit.id.clone(),
        unit: edit.unit.clone(),
        kind: edit.kind.clone(),
        description: edit.description.clone(),
        expected: edit.expected.clone(),
        ..EditResult::default()
    };
    let copy = match copy_for(setup, edit, unit) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: infra error: {e}", edit.id);
            r.infra_errors = rounds;
            return r;
        }
    };
    let (mut plain_original, mut plain_edited) = (Vec::new(), Vec::new());
    for round in 0..rounds {
        let cache_root = setup
            .scratch
            .join("cache")
            .join(format!("e{index}-{round}"));
        let restored = std::fs::write(&copy.file, &copy.original);
        let daemon = match restored
            .map_err(|e| e.to_string())
            .and_then(|()| Daemon::start(setup.sail, &setup.socket(), &cache_root))
        {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{}: infra error: {e}", edit.id);
                r.infra_errors += 1;
                continue;
            }
        };
        if let Some((p, s)) = pair(
            setup,
            &mut r,
            (&copy.unit, round, "fill"),
            &daemon,
            &mut plain_original,
        ) {
            r.plain_original.push(&p);
            r.fill.push(&s);
        } else {
            continue;
        }
        if let Err(e) = std::fs::write(&copy.file, &copy.edited) {
            eprintln!("{}: infra error: cannot apply the edit: {e}", edit.id);
            r.infra_errors += 1;
            continue;
        }
        if let Some((p, s)) = pair(
            setup,
            &mut r,
            (&copy.unit, round, "edit"),
            &daemon,
            &mut plain_edited,
        ) {
            r.outcome = Some(outcome(&p).to_owned());
            r.plain.push(&p);
            r.sail.push(&s);
        }
        drop(daemon);
        let _ = std::fs::remove_dir_all(&cache_root);
    }
    let _ = std::fs::write(&copy.file, &copy.original);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(find: &str, replace: &str) -> Edit {
        Edit {
            id: "e".to_owned(),
            unit: "p/u.rs".to_owned(),
            kind: "body".to_owned(),
            file: "u.rs".to_owned(),
            description: String::new(),
            expected: "verified".to_owned(),
            find: find.to_owned(),
            replace: replace.to_owned(),
        }
    }

    #[test]
    fn applies_exactly_once() {
        assert_eq!(apply(&edit("b", "x"), "abc").ok().as_deref(), Some("axc"));
        assert!(apply(&edit("b", "x"), "abcb").is_err());
        assert!(apply(&edit("z", "x"), "abc").is_err());
    }

    #[test]
    fn checks_catalogues() {
        let ok = Catalogue {
            schema_version: 1,
            edits: vec![edit("a", "b")],
        };
        assert!(ok.check().is_ok());
        let mut bad = ok.clone();
        bad.edits.push(edit("a", "b"));
        assert!(bad.check().is_err(), "duplicate id");
        let mut bad = ok.clone();
        bad.edits[0].kind = "refactor".to_owned();
        assert!(bad.check().is_err(), "kind");
        let mut bad = ok.clone();
        bad.edits[0].replace = "a".to_owned();
        assert!(bad.check().is_err(), "no-op");
        let mut bad = ok;
        bad.edits[0].file = "../x.rs".to_owned();
        assert!(bad.check().is_err(), "escapes the project");
    }

    #[test]
    fn the_shipped_catalogue_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/edits.toml");
        let c = Catalogue::load(&path);
        assert!(c.as_ref().is_ok_and(|c| !c.edits.is_empty()), "{c:?}");
    }
}
