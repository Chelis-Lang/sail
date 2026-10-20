//! The seeded mutation suite (ADR 0063): a soundness probe and the release
//! benchmark's edit-loop rows.
//!
//! Catalogues come from `cargo xtask gen-mutants` in the `corpus/edits.toml`
//! format, with a JSON sidecar (`<name>.meta.json`). Two steps run them:
//!
//! - [`oracle`]: stock Verus verifies every mutant three times (by default)
//!   in a persistent working copy. A mutant whose outcome varies, that does
//!   not compile, or whose run does not finish is dropped and listed; the
//!   others get `expected` from those runs. Stock Verus is the only oracle.
//! - [`runner`]: per round, one working copy and one Sail cache (so one
//!   daemon) serve every mutant in turn. Each mutant is verified by stock
//!   `cargo verus verify` and by `sail verify` and the two are compared
//!   (verdicts, diagnostics, exit). After the last mutant the original is
//!   restored and verified again, ordinarily and with fresh verification
//!   forced, and must get its own answers.

pub mod oracle;
pub mod runner;
pub mod stats;
pub mod workspace;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::RunRecord;
use sail_verus::toolchain::{Installed, Pins};
use serde_json::Value;

use crate::edit::{Catalogue, Edit};

/// The sidecar beside a catalogue: `<name>.meta.json` for `<name>.toml`.
#[must_use]
pub fn sidecar(catalogue: &Path) -> PathBuf {
    catalogue.with_extension("meta.json")
}

/// Reads a sidecar.
///
/// # Errors
///
/// A message if it cannot be read or parsed.
pub fn load_meta(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes a sidecar.
///
/// # Errors
///
/// A message if it cannot be written.
pub fn save_meta(path: &Path, meta: &Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The nine mutation classes, in catalogue order.
pub const CLASSES: &[&str] = &[
    "arith",
    "compare",
    "constant",
    "logic",
    "statement",
    "invariant",
    "spec-weaken",
    "spec-strengthen",
    "rare-region",
];

/// A mutant's class, from the sidecar (or its id, `<name>-<class>-<n>`).
#[must_use]
pub fn class_of(meta: &Value, id: &str) -> String {
    meta.pointer(&format!("/mutants/{id}/class"))
        .and_then(Value::as_str)
        .or_else(|| {
            CLASSES
                .iter()
                .filter(|c| id.contains(&format!("-{c}-")))
                .max_by_key(|c| c.len())
                .copied()
        })
        .unwrap_or("unknown")
        .to_owned()
}

/// A TOML string: a multi-line literal string where possible.
fn toml_string(s: &str) -> String {
    if s.contains('\n') && !s.contains("'''") && !s.contains('\r') {
        format!("'''\n{s}'''")
    } else {
        serde_json::to_string(s).unwrap_or_default()
    }
}

/// Renders a catalogue in the generator's format, with `header` (comment
/// lines) first.
#[must_use]
pub fn render(header: &str, edits: &[Edit]) -> String {
    let mut t = String::new();
    t.push_str(header);
    t.push_str("schema_version = 1\n");
    for e in edits {
        let _ = write!(
            t,
            "\n[[edit]]\nid = {}\nunit = {}\nkind = {}\nfile = {}\ndescription = {}\nexpected = {}\nfind = {}\nreplace = {}\n",
            toml_string(&e.id),
            toml_string(&e.unit),
            toml_string(&e.kind),
            toml_string(&e.file),
            toml_string(&e.description),
            toml_string(&e.expected),
            toml_string(&e.find),
            toml_string(&e.replace),
        );
    }
    t
}

/// The comment lines at the top of a catalogue.
#[must_use]
pub fn header_of(text: &str) -> String {
    let mut out = String::new();
    for l in text.lines().take_while(|l| l.starts_with('#')) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// How stock Verus ended on one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plain {
    /// Exit 0, no errors.
    Verified,
    /// Verification errors.
    Errors,
    /// Never reached verification: it does not compile, Verus rejects it
    /// before verifying, or Cargo did not rebuild it. The reason says which.
    Stillborn(String),
}

impl Plain {
    /// The catalogue outcome name (`verified`, `errors`) or `stillborn`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Verified => "verified",
            Self::Errors => "errors",
            Self::Stillborn(_) => "stillborn",
        }
    }
}

/// Classifies a stock run of a mutant.
#[must_use]
pub fn plain_outcome(r: &RunRecord) -> Plain {
    if r.outputs.is_empty() {
        return Plain::Stillborn(if r.exit_code == Some(0) {
            "Cargo did not rebuild the crate (the file is not compiled)".to_owned()
        } else {
            "does not compile".to_owned()
        });
    }
    let vir = r.outputs.iter().any(|o| {
        ["encountered-vir-error", "encountered_vir_error"]
            .iter()
            .any(|k| o.pointer(&format!("/verification-results/{k}")) == Some(&Value::Bool(true)))
    });
    if vir {
        return Plain::Stillborn("Verus rejects it before verification".to_owned());
    }
    if r.stderr.lines().any(|l| l.starts_with("error[E")) {
        return Plain::Stillborn("does not compile".to_owned());
    }
    match crate::edit::outcome(r) {
        "verified" => Plain::Verified,
        _ => Plain::Errors,
    }
}

/// Functions a run reports as failed, as `module::function` (by output).
#[must_use]
pub fn failing(r: &RunRecord) -> BTreeSet<String> {
    r.functions()
        .into_iter()
        .filter(|f| !f.success)
        .map(|f| format!("{}#{}::{}", f.output, f.module, f.function))
        .collect()
}

/// Failed entries per function key (`output#module::function`). Several
/// entries can share a key (for example one generic impl's methods under
/// different type arguments), so these are counts, not a set.
fn failures_by_key(r: &RunRecord) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for f in r.functions().into_iter().filter(|f| !f.success) {
        *m.entry(format!("{}#{}::{}", f.output, f.module, f.function))
            .or_default() += 1;
    }
    m
}

/// False verified: what Sail reports verified that stock rejected. Per
/// function key, Sail reporting fewer failed entries than stock while still
/// reporting the function; plus `exit` when stock failed and Sail exited 0
/// with nothing failed.
#[must_use]
pub fn false_verified(stock: &RunRecord, sail: &RunRecord) -> Vec<String> {
    let reported: BTreeSet<String> = sail
        .functions()
        .into_iter()
        .map(|f| format!("{}#{}::{}", f.output, f.module, f.function))
        .collect();
    let sail_failed = failures_by_key(sail);
    let mut out: Vec<String> = failures_by_key(stock)
        .into_iter()
        .filter(|(k, n)| reported.contains(k) && sail_failed.get(k).copied().unwrap_or(0) < *n)
        .map(|(k, _)| k)
        .collect();
    if stock.exit_code != Some(0) && sail.exit_code == Some(0) && sail_failed.is_empty() {
        out.push("exit".to_owned());
    }
    out
}

/// Rechecks the steps a run kept under `<dir>/failing/<step>/{stock,sail}`:
/// false verified, and the full comparison (with each side's exit code from
/// `samples.jsonl`). One line per step.
///
/// # Errors
///
/// A message if the directory cannot be listed.
pub fn recheck(dir: &Path) -> Result<String, String> {
    let failing = dir.join("failing");
    let mut steps: Vec<PathBuf> = std::fs::read_dir(&failing)
        .map_err(|e| format!("cannot list {}: {e}", failing.display()))?
        .flatten()
        .map(|e| e.path())
        .collect();
    steps.sort();
    // Exit codes by step name (`r<round>-<phase>-<id>`).
    let mut exits: BTreeMap<String, (Option<i32>, Option<i32>)> = BTreeMap::new();
    for line in std::fs::read_to_string(dir.join("samples.jsonl"))
        .unwrap_or_default()
        .lines()
    {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let get = |k: &str| {
            v.get(k)
                .and_then(Value::as_i64)
                .and_then(|x| i32::try_from(x).ok())
        };
        let name = format!(
            "r{}-{}-{}",
            v.get("round").and_then(Value::as_u64).unwrap_or(0),
            v.get("phase").and_then(Value::as_str).unwrap_or_default(),
            v.get("id").and_then(Value::as_str).unwrap_or_default()
        );
        exits.insert(name, (get("stock_exit"), get("sail_exit")));
    }
    let load = |p: &Path, exit: Option<i32>| -> Option<RunRecord> {
        let stdout = std::fs::read(p.join("stdout.txt")).ok()?;
        let stderr = std::fs::read_to_string(p.join("stderr.txt")).unwrap_or_default();
        let mut stream = serde_json::Deserializer::from_slice(&stdout).into_iter::<Value>();
        let mut outputs = Vec::new();
        let mut end = 0;
        while let Some(Ok(doc)) = stream.next() {
            outputs.push(doc);
            end = stream.byte_offset();
        }
        Some(RunRecord {
            exit_code: exit,
            stderr: workspace::drop_progress_notes(&stderr),
            outputs,
            stdout_rest: String::from_utf8_lossy(stdout.get(end..).unwrap_or_default())
                .trim()
                .to_owned(),
            wall_ms: 0.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        })
    };
    let mut t = String::new();
    for step in steps {
        let name = step
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (pe, se) = exits.get(&name).copied().unwrap_or_default();
        match (load(&step.join("stock"), pe), load(&step.join("sail"), se)) {
            (Some(p), Some(s)) => {
                let fv = false_verified(&p, &s);
                let c = sail_conformance::compare(&p, &s);
                let _ = writeln!(
                    t,
                    "{name}: false verified {fv:?}; {} ({} accepted divergences){}",
                    if c.differences.is_empty() {
                        "same"
                    } else {
                        "different"
                    },
                    c.accepted.len(),
                    c.differences
                        .first()
                        .map(|d| format!("; first difference: {d}"))
                        .unwrap_or_default()
                );
            }
            _ => {
                let _ = writeln!(t, "{name}: outputs not kept");
            }
        }
    }
    Ok(t)
}

/// What every subcommand needs: the pins, corpus, stock Verus and the unit.
pub struct Setup {
    /// Stock Verus (the oracle).
    pub installed: Installed,
    /// The pinned Verus source (for patching and wrappers).
    pub verus_source: PathBuf,
    /// The catalogue's unit, fetched.
    pub unit: Unit,
    /// Scratch root.
    pub scratch: PathBuf,
}

impl Setup {
    /// Locates stock Verus and fetches the unit `catalogue` names.
    ///
    /// # Errors
    ///
    /// A message if the pins, corpus or Verus cannot be found, or the
    /// catalogue names several units or one outside the corpus.
    pub fn new(catalogue: &Catalogue, scratch: Option<PathBuf>) -> Result<Self, String> {
        let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
        let corpus = Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
        corpus.check_pins(&pins).map_err(|e| e.to_string())?;
        let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
        let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
        let units: BTreeSet<&str> = catalogue.edits.iter().map(|e| e.unit.as_str()).collect();
        let [unit_id] = units.into_iter().collect::<Vec<_>>()[..] else {
            return Err("a mutation catalogue must name exactly one unit".to_owned());
        };
        let project_name = unit_id.split('/').next().unwrap_or_default();
        let mut found = None;
        for project in corpus.projects_in(Tier::Deep, &[project_name.to_owned()]) {
            let dir = project.fetch(&cache).map_err(|e| e.to_string())?;
            found = project
                .units(&dir)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|u| u.id == unit_id);
        }
        let unit = found.ok_or_else(|| format!("unit `{unit_id}` is not in the corpus"))?;
        let verus_source = corpus
            .fetch_verus_source(&cache, &installed.pins.verus.commit)
            .map_err(|e| e.to_string())?;
        let scratch = scratch.unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            cache.join("mut").join(now.to_string())
        });
        Ok(Self {
            installed,
            verus_source,
            unit,
            scratch,
        })
    }
}

/// Usage of `sail-bench mutants`.
pub const USAGE: &str = "usage:
  sail-bench mutants oracle --catalogue corpus/mutants/NAME.toml [--runs 3] [--jobs 4]
                            [--timeout-secs 1800] [--scratch DIR]
  sail-bench mutants run --catalogue corpus/mutants/NAME.toml [--rounds 3] [--per-class N]
                         [--filter SUBSTR] [--out DIR] [--sail PATH] [--timeout-secs 1800]
                         [--scratch DIR] [--machine TEXT]
  sail-bench mutants report DIR...
  sail-bench mutants recheck DIR...   (false verified again, from kept outputs)";

/// Entry point for `sail-bench mutants`.
///
/// # Errors
///
/// A message for bad usage or a failed step; a run with any difference,
/// false verified, failed revert check or infrastructure error also fails.
pub fn main(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("oracle") => oracle::main(args.get(1..).unwrap_or_default()),
        Some("run") => runner::main(args.get(1..).unwrap_or_default()),
        Some("recheck") => {
            for d in args.iter().skip(1) {
                print!("{}", recheck(Path::new(d))?);
            }
            Ok(())
        }
        Some("report") => {
            let dirs: Vec<PathBuf> = args.iter().skip(1).map(PathBuf::from).collect();
            if dirs.is_empty() {
                return Err(USAGE.to_owned());
            }
            let mut text = String::new();
            for d in &dirs {
                text.push_str(&stats::report_dir(d)?);
            }
            print!("{text}");
            Ok(())
        }
        _ => Err(USAGE.to_owned()),
    }
}

/// A value after a flag, or a usage error.
pub(crate) fn value(it: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<String, String> {
    it.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(exit: i32, fns: &[(&str, bool)]) -> RunRecord {
        let breakdown: Vec<Value> = fns
            .iter()
            .map(|(f, ok)| json!({"function": f, "success": ok, "rlimit": 1, "time-micros": 1}))
            .collect();
        RunRecord {
            exit_code: Some(exit),
            stderr: String::new(),
            outputs: vec![json!({
                "verification-results": {"verified": 1, "errors": i32::from(exit != 0), "success": exit == 0},
                "times-ms": {"smt": {"smt-run-module-times": [{"module": "m", "function-breakdown": breakdown}]}}
            })],
            stdout_rest: String::new(),
            wall_ms: 1.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        }
    }

    #[test]
    fn counts_entries_that_share_a_name() {
        // Two entries named `f` (say, one generic impl twice): one fails on
        // both sides. Nothing was falsely verified.
        let stock = record(1, &[("f", false), ("f", true)]);
        let sail = record(1, &[("f", true), ("f", false)]);
        assert_eq!(false_verified(&stock, &sail), Vec::<String>::new());
        let both = record(1, &[("f", false), ("f", false)]);
        assert_eq!(false_verified(&both, &sail), ["0#m::f"]);
    }

    #[test]
    fn finds_false_verified() {
        let stock = record(1, &[("f", false), ("g", true)]);
        assert_eq!(false_verified(&stock, &stock), Vec::<String>::new());
        let sail = record(0, &[("f", true), ("g", true)]);
        assert_eq!(false_verified(&stock, &sail), ["0#m::f", "exit"]);
    }

    #[test]
    fn classifies_stock_runs() {
        assert_eq!(plain_outcome(&record(0, &[("f", true)])), Plain::Verified);
        assert_eq!(plain_outcome(&record(1, &[("f", false)])), Plain::Errors);
        let mut none = record(1, &[]);
        none.outputs.clear();
        assert!(matches!(plain_outcome(&none), Plain::Stillborn(_)));
    }

    #[test]
    fn renders_parseable_catalogues() {
        let e = Edit {
            id: "x-arith-001".to_owned(),
            unit: "p/u".to_owned(),
            kind: "body".to_owned(),
            file: "a.rs".to_owned(),
            description: "arith: `+` to `-` in `f`".to_owned(),
            expected: "errors".to_owned(),
            find: "    a + b\n".to_owned(),
            replace: "    a - b\n".to_owned(),
        };
        let text = render("# h\n", std::slice::from_ref(&e));
        let parsed: Result<Catalogue, _> = toml::from_str(&text);
        assert!(parsed.is_ok_and(|c| c.edits.first().is_some_and(|f| f.find == e.find)));
        assert_eq!(header_of(&text), "# h\n");
        assert_eq!(
            class_of(&Value::Null, "human-eval-spec-weaken-003"),
            "spec-weaken"
        );
    }
}
