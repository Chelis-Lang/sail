//! `sail-bench mutants oracle`: settles each mutant's expected outcome from
//! repeated stock Verus runs (ADR 0063).
//!
//! Workers each keep one persistent working copy. A worker first verifies
//! the original (so Cargo's dependencies are built), then for each of its
//! mutants applies it and runs stock `cargo verus verify` `--runs` times,
//! cleaning the package's build products before every run so Cargo rebuilds
//! and re-verifies the crate, and restores the file. Stock Verus's outcome does not depend on wall time
//! (Z3 stops at an rlimit, not a deadline), so workers may share the
//! machine; timings from this step are never reported as performance.
//!
//! A mutant is dropped, and listed with the reason, when any run did not
//! finish (a run that does not finish is never an outcome), when any run
//! never reached verification (it does not compile, Verus rejects it before
//! verifying, or Cargo did not rebuild it), or when the runs disagree on the
//! exit code, the outcome or the set of failing functions.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

use super::workspace::{Workspace, execute, write_fresh};
use super::{
    Plain, Setup, class_of, failing, header_of, load_meta, plain_outcome, render, save_meta,
    sidecar, value,
};
use crate::edit::{Catalogue, Edit, apply};

struct Options {
    catalogue: PathBuf,
    runs: usize,
    jobs: usize,
    timeout: Duration,
    scratch: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        catalogue: PathBuf::new(),
        runs: 3,
        jobs: 4,
        timeout: Duration::from_mins(30),
        scratch: None,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let number = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--catalogue" => o.catalogue = value(&mut it, arg)?.into(),
            "--runs" => o.runs = usize::try_from(number(value(&mut it, arg)?)?).unwrap_or(3),
            "--jobs" => o.jobs = usize::try_from(number(value(&mut it, arg)?)?).unwrap_or(1),
            "--timeout-secs" => o.timeout = Duration::from_secs(number(value(&mut it, arg)?)?),
            "--scratch" => o.scratch = Some(value(&mut it, arg)?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{}", super::USAGE)),
        }
    }
    if o.catalogue.as_os_str().is_empty() || o.runs == 0 {
        return Err(super::USAGE.to_owned());
    }
    Ok(o)
}

/// One mutant's stock runs.
#[derive(Debug, Clone, Default)]
struct Runs {
    outcomes: Vec<String>,
    exits: Vec<Option<i32>>,
    failing: Vec<Vec<String>>,
    wall_s: Vec<f64>,
    /// Why it is dropped, if it is.
    dropped: Option<String>,
}

impl Runs {
    fn json(&self, original: &[String]) -> Value {
        let new: Vec<&String> = self
            .failing
            .first()
            .map(|f| f.iter().filter(|x| !original.contains(x)).collect())
            .unwrap_or_default();
        json!({
            "outcomes": self.outcomes,
            "exits": self.exits,
            "failing": self.failing.first(),
            "new_failures": new,
            "wall_s": self.wall_s.iter().map(|w| (w * 10.0).round() / 10.0).collect::<Vec<_>>(),
            "dropped": self.dropped,
        })
    }
}

fn run_mutant(setup: &Setup, ws: &Workspace, edit: &Edit, runs: usize, timeout: Duration) -> Runs {
    let mut r = Runs::default();
    let file = ws.src.join(&edit.file);
    let original = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) => {
            r.dropped = Some(format!("cannot read {}: {e}", edit.file));
            return r;
        }
    };
    let mutated = match apply(edit, &original) {
        Ok(t) => t,
        Err(e) => {
            r.dropped = Some(e);
            return r;
        }
    };
    let logs = setup.scratch.join("oracle-logs").join(&edit.id);
    for k in 0..runs {
        if let Err(e) = write_fresh(&file, &mutated) {
            r.dropped = Some(format!("cannot apply: {e}"));
            break;
        }
        // Every run must rebuild and re-verify the crate.
        if let Err(e) = ws.clean_stock(&setup.installed) {
            r.dropped = Some(format!("infrastructure: {e}"));
            break;
        }
        match execute(
            &mut ws.stock(&setup.installed),
            timeout,
            Some(&logs.join(format!("run{k}"))),
        ) {
            Ok(rec) => {
                let p = plain_outcome(&rec);
                if let Plain::Stillborn(why) = &p {
                    r.dropped.get_or_insert_with(|| why.clone());
                }
                r.outcomes.push(p.name().to_owned());
                r.exits.push(rec.exit_code);
                r.failing.push(failing(&rec).into_iter().collect());
                r.wall_s.push(rec.wall_ms / 1000.0);
            }
            Err(e) => {
                r.dropped = Some(format!("a stock run {e}; never an outcome"));
                break;
            }
        }
        if r.dropped.is_some() {
            break;
        }
    }
    let _ = write_fresh(&file, &original);
    if r.dropped.is_none() {
        let varies = r.outcomes.windows(2).any(|w| w.first() != w.last())
            || r.exits.windows(2).any(|w| w.first() != w.last())
            || r.failing.windows(2).any(|w| w.first() != w.last());
        if varies {
            r.dropped = Some(format!(
                "stock outcome varies across runs: {}",
                r.outcomes.join(", ")
            ));
        }
    }
    r
}

/// Entry point for `sail-bench mutants oracle`.
///
/// # Errors
///
/// A message for bad usage, a missing tool or an unwritable catalogue.
#[allow(
    clippy::too_many_lines,
    reason = "the workers, their results and the catalogue rewrite are one step"
)]
pub fn main(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    let text = std::fs::read_to_string(&o.catalogue)
        .map_err(|e| format!("cannot read {}: {e}", o.catalogue.display()))?;
    let catalogue = Catalogue::load(&o.catalogue)?;
    let meta_path = sidecar(&o.catalogue);
    let mut meta = load_meta(&meta_path)?;
    let setup = Setup::new(&catalogue, o.scratch.clone())?;
    let edits = catalogue.edits.clone();
    let jobs = o.jobs.clamp(1, edits.len().max(1));
    eprintln!(
        "oracle: {} mutants x {} stock runs, {jobs} workers, scratch {}",
        edits.len(),
        o.runs,
        setup.scratch.display()
    );
    let results: Mutex<BTreeMap<String, Runs>> = Mutex::new(BTreeMap::new());
    let original: Mutex<Option<(String, Vec<String>, f64)>> = Mutex::new(None);
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for w in 0..jobs {
            let (setup, edits, results, original, errors) =
                (&setup, &edits, &results, &original, &errors);
            let (runs, timeout) = (o.runs, o.timeout);
            s.spawn(move || {
                let mine: Vec<&Edit> = edits.iter().skip(w).step_by(jobs).collect();
                let ws = Workspace::prepare(
                    &setup.installed,
                    &setup.unit,
                    &setup.verus_source,
                    &setup.scratch.join(format!("oracle-{w}")),
                );
                let ws = match ws {
                    Ok(ws) => ws,
                    Err(e) => {
                        if let Ok(mut v) = errors.lock() {
                            v.push(format!("worker {w}: {e}"));
                        }
                        return;
                    }
                };
                let cold = execute(
                    &mut ws.stock(&setup.installed),
                    timeout,
                    Some(&setup.scratch.join(format!("oracle-logs/original-{w}"))),
                );
                match cold {
                    Ok(rec) => {
                        let o = (
                            plain_outcome(&rec).name().to_owned(),
                            failing(&rec).into_iter().collect::<Vec<String>>(),
                            rec.wall_ms / 1000.0,
                        );
                        eprintln!(
                            "worker {w}: original {} ({} failing) in {:.0} s",
                            o.0,
                            o.1.len(),
                            o.2
                        );
                        if let Ok(mut g) = original.lock() {
                            g.get_or_insert(o);
                        }
                    }
                    Err(e) => {
                        if let Ok(mut v) = errors.lock() {
                            v.push(format!("worker {w}: the original {e}"));
                        }
                        return;
                    }
                }
                for edit in mine {
                    let r = run_mutant(setup, &ws, edit, runs, timeout);
                    eprintln!(
                        "{:<32} {:<26} {}",
                        edit.id,
                        r.outcomes.join(","),
                        r.dropped
                            .as_deref()
                            .map_or_else(String::new, |d| format!("DROPPED: {d}"))
                    );
                    if let Ok(mut m) = results.lock() {
                        m.insert(edit.id.clone(), r);
                    }
                }
                let _ = std::fs::remove_dir_all(ws.root.join("target-stock"));
            });
        }
    });
    let errors = errors.into_inner().unwrap_or_default();
    if !errors.is_empty() {
        return Err(format!(
            "oracle infrastructure failures:\n{}",
            errors.join("\n")
        ));
    }
    let results = results.into_inner().unwrap_or_default();
    let (orig_outcome, orig_failing, orig_wall) = original
        .into_inner()
        .ok()
        .flatten()
        .ok_or("the original was never verified")?;
    let mut kept = Vec::new();
    let mut dropped = serde_json::Map::new();
    let mut per_class: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut detail = serde_json::Map::new();
    for e in &edits {
        let class = class_of(&meta, &e.id);
        let slot = per_class.entry(class).or_default();
        let Some(r) = results.get(&e.id) else {
            dropped.insert(e.id.clone(), json!("not run"));
            slot.1 += 1;
            continue;
        };
        detail.insert(e.id.clone(), r.json(&orig_failing));
        if let Some(why) = &r.dropped {
            dropped.insert(e.id.clone(), json!(why));
            slot.1 += 1;
        } else {
            let mut k = e.clone();
            k.expected = r.outcomes.first().cloned().unwrap_or_default();
            kept.push(k);
            slot.0 += 1;
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let oracle = json!({
        "platform": sail_verus::toolchain::host(),
        "machine": sail_verus::run::machine_info(),
        "stock_verus": setup.installed.pins.verus.release,
        "runs": o.runs,
        "timeout_secs": o.timeout.as_secs(),
        "finished_unix": now,
        "original": {"outcome": orig_outcome, "failing": orig_failing, "wall_s": (orig_wall * 10.0).round() / 10.0},
        "kept": kept.len(),
        "dropped": dropped,
        "results": detail,
    });
    if let Some(m) = meta.as_object_mut() {
        m.insert("oracle".to_owned(), oracle);
    }
    let mut header = header_of(&text);
    let _ = write!(
        header,
        "# Oracle: {} stock runs per mutant on {} (Verus {}); {} kept, {} dropped\n# (reasons in the sidecar).\n",
        o.runs,
        sail_verus::toolchain::host(),
        setup.installed.pins.verus.release,
        kept.len(),
        dropped.len()
    );
    let mut kept_header = String::new();
    for l in header.lines().filter(|l| {
        !l.contains("is \"pending\" until")
            && !l.contains("three stock Verus runs per mutant on the reference")
    }) {
        kept_header.push_str(l);
        kept_header.push('\n');
    }
    let header = kept_header;
    std::fs::write(&o.catalogue, render(&header, &kept))
        .map_err(|e| format!("cannot write {}: {e}", o.catalogue.display()))?;
    save_meta(&meta_path, &meta)?;
    println!("class            kept  dropped");
    for (c, (k, d)) in &per_class {
        println!("{c:<16} {k:>4}  {d:>7}");
    }
    println!(
        "original: {orig_outcome}; kept {} of {}; wrote {} and {}",
        kept.len(),
        edits.len(),
        o.catalogue.display(),
        meta_path.display()
    );
    Ok(())
}
