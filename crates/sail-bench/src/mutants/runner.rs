//! `sail-bench mutants run`: the edit loop over a catalogue, one Sail
//! daemon across every mutant (ADR 0063).
//!
//! Each round:
//! 1. prepares a fresh working copy and an empty Sail cache (so Sail starts
//!    one daemon, which serves the whole round);
//! 2. `cold`: stock `cargo verus verify` and `sail verify` on the original;
//! 3. `mutant`, for each mutant in a rotated order: applies it, runs both,
//!    and restores the file without verifying (the next mutant starts from
//!    the original text, as an agent's next attempt does);
//! 4. `revert`: both on the restored original. Sail's daemon now holds every
//!    mutant's answers; the original must get its own (equal to stock's,
//!    and to Sail's own cold result for the package's crate);
//! 5. `revert-forced`: the same with `SAIL_MANAGED_FORCE=1`, so Verus runs
//!    and every check goes through the solver cache instead of a restored
//!    proof.
//!
//! The arms alternate which runs first, by round and position. Every Sail
//! run is judged against the same step's stock run (and earlier rounds'
//! stock runs of the same step, for ADR 0009's `unstable`), and false
//! verified is counted on its own. Every sample is appended to
//! `samples.jsonl` as it finishes; nothing is dropped.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sail_verus::run::RunRecord;
use serde_json::{Value, json};

use super::workspace::{
    DaemonState, Workspace, daemon_state, execute, retained_receipts, stop_daemon,
    verus_build_from_env, write_fresh,
};
use super::{Setup, class_of, false_verified, load_meta, plain_outcome, sidecar, value};
use crate::cold::{Judgement, judge};
use crate::daemon::CacheRun;
use crate::edit::{Catalogue, Edit, apply};

struct Options {
    catalogue: PathBuf,
    rounds: usize,
    per_class: Option<usize>,
    filter: Option<String>,
    out: Option<PathBuf>,
    sail: Option<PathBuf>,
    timeout: Duration,
    scratch: Option<PathBuf>,
    machine: String,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        catalogue: PathBuf::new(),
        rounds: 3,
        per_class: None,
        filter: None,
        out: None,
        sail: None,
        timeout: Duration::from_mins(30),
        scratch: None,
        machine: String::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let number = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--catalogue" => o.catalogue = value(&mut it, arg)?.into(),
            "--rounds" => o.rounds = usize::try_from(number(value(&mut it, arg)?)?).unwrap_or(3),
            "--per-class" => {
                o.per_class = Some(usize::try_from(number(value(&mut it, arg)?)?).unwrap_or(0));
            }
            "--filter" => o.filter = Some(value(&mut it, arg)?),
            "--out" => o.out = Some(value(&mut it, arg)?.into()),
            "--sail" => o.sail = Some(value(&mut it, arg)?.into()),
            "--timeout-secs" => o.timeout = Duration::from_secs(number(value(&mut it, arg)?)?),
            "--scratch" => o.scratch = Some(value(&mut it, arg)?.into()),
            "--machine" => o.machine = value(&mut it, arg)?,
            _ => return Err(format!("unknown argument `{arg}`\n{}", super::USAGE)),
        }
    }
    if o.catalogue.as_os_str().is_empty() || o.rounds == 0 {
        return Err(super::USAGE.to_owned());
    }
    Ok(o)
}

/// The mutants to run: the first `per_class` of each class in catalogue
/// order, those matching `filter`.
fn select<'a>(
    edits: &'a [Edit],
    meta: &Value,
    per_class: Option<usize>,
    filter: Option<&str>,
) -> Vec<&'a Edit> {
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    edits
        .iter()
        .filter(|e| filter.is_none_or(|f| e.id.contains(f)))
        .filter(|e| {
            let n = taken.entry(class_of(meta, &e.id)).or_default();
            *n += 1;
            per_class.is_none_or(|k| *n <= k)
        })
        .collect()
}

struct Round<'a> {
    setup: &'a Setup,
    sail: &'a Path,
    verus_build: Option<&'a Path>,
    cache: PathBuf,
    timeout: Duration,
    index: usize,
    rounds: usize,
    out: &'a Path,
    samples: std::fs::File,
    /// Stock runs of each step in earlier rounds (and this one), by step.
    plains: &'a mut BTreeMap<String, Vec<RunRecord>>,
    failures: usize,
    daemon_pid: Option<u64>,
}

/// One step's two runs.
struct Pair {
    stock: Result<RunRecord, String>,
    sail: Result<RunRecord, String>,
    cache: CacheRun,
    pid: Option<u64>,
    stock_first: bool,
    retained: Result<usize, String>,
}

impl Round<'_> {
    fn run_pair(&self, ws: &Workspace, step: &str, position: usize, force: bool) -> Pair {
        let logs = self
            .setup
            .scratch
            .join("logs")
            .join(format!("r{}", self.index))
            .join(step);
        let stock_first = (self.index + position).is_multiple_of(2);
        let before = daemon_state(self.sail, &self.cache);
        let receipts_before = retained_receipts(&self.cache);
        let go_stock = || {
            execute(
                &mut ws.stock(&self.setup.installed),
                self.timeout,
                &logs.join("stock"),
            )
        };
        let go_sail = || {
            let mut cmd = ws.sail(self.sail, &self.cache, self.verus_build);
            if force {
                cmd.env("SAIL_MANAGED_FORCE", "1");
            }
            execute(&mut cmd, self.timeout, &logs.join("sail"))
        };
        let (stock, sail) = if stock_first {
            let s = go_stock();
            (s, go_sail())
        } else {
            let t = go_sail();
            (go_stock(), t)
        };
        let after: DaemonState = daemon_state(self.sail, &self.cache);
        let cache = CacheRun::between(
            step,
            &before.stats.unwrap_or_default(),
            &after.stats.unwrap_or_default(),
        );
        let retained = receipts_before.and_then(|before| {
            retained_receipts(&self.cache).map(|after| after.difference(&before).count())
        });
        Pair {
            retained,
            stock,
            sail,
            cache,
            pid: after.pid,
            stock_first,
        }
    }

    /// Judges and records one step; returns the Sail run for later checks.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one sample's identity, kept explicit at the call sites, recorded field by field"
    )]
    fn record(
        &mut self,
        pair: Pair,
        phase: &str,
        position: usize,
        edit: Option<&Edit>,
        class: &str,
        own: Option<&RunRecord>,
    ) -> Option<RunRecord> {
        let id = edit.map_or("original", |e| e.id.as_str());
        let step = format!("{phase}:{id}");
        let served = if pair.retained.as_ref().is_ok_and(|count| *count > 0) {
            "retained"
        } else if pair.cache.sessions == 0 {
            "no-solver"
        } else if pair.cache.hits > 0 && pair.cache.puts == 0 {
            "cache"
        } else if pair.cache.hits > 0 {
            "mixed"
        } else {
            "fresh"
        };
        let mut sample = json!({
            "round": self.index,
            "position": position,
            "phase": phase,
            "id": id,
            "class": class,
            "expected": edit.map(|e| e.expected.clone()),
            "stock_first": pair.stock_first,
            "cache": {
                "sessions": pair.cache.sessions,
                "gets": pair.cache.gets,
                "hits": pair.cache.hits,
                "puts": pair.cache.puts,
                "memory_hits": pair.cache.memory_hits,
            },
            "served": served,
            "daemon_pid": pair.pid,
        });
        match &pair.retained {
            Ok(count) => sample["retained_restores"] = json!(count),
            Err(error) => sample["provenance_error"] = json!(error),
        }
        if !daemon_continuity(&mut self.daemon_pid, pair.pid) {
            sample["daemon_error"] = json!("daemon missing or changed within the round");
        }
        let (stock, sail) = match (pair.stock, pair.sail) {
            (Ok(p), Ok(s)) => (p, s),
            (p, s) => {
                let msg = [p.err(), s.err()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ");
                sample["judgement"] = json!("infra");
                sample["error"] = json!(msg);
                eprintln!("r{} {phase:<13} {id:<32} INFRA: {msg}", self.index);
                self.failures += 1;
                self.keep_logs(&step);
                self.write(&sample);
                return None;
            }
        };
        let plains = self.plains.entry(step.clone()).or_default();
        plains.push(stock.clone());
        let (j, functions, differences) = judge(plains, &sail);
        let fv = false_verified(&stock, &sail);
        let outcome = plain_outcome(&stock);
        let mismatch = edit.is_some_and(|e| e.expected != outcome.name());
        let judgement = match j {
            Judgement::Same => "same",
            Judgement::Unstable => "unstable",
            Judgement::Different => "different",
        };
        sample["judgement"] = json!(judgement);
        sample["functions"] = json!(functions);
        sample["stock_s"] = json!(stock.wall_ms / 1000.0);
        sample["sail_s"] = json!(sail.wall_ms / 1000.0);
        sample["stock_exit"] = json!(stock.exit_code);
        sample["sail_exit"] = json!(sail.exit_code);
        sample["stock_outcome"] = json!(outcome.name());
        sample["expected_mismatch"] = json!(mismatch);
        sample["false_verified"] = json!(fv);
        sample["differences"] = json!(differences.iter().take(5).collect::<Vec<_>>());
        let accepted = sail_conformance::compare(&stock, &sail).accepted;
        if !accepted.is_empty() {
            sample["accepted"] = json!(accepted);
        }
        if let Some(cold) = own {
            let d = own_differences(cold, &sail);
            sample["own_answers"] = json!(d.is_empty());
            if !d.is_empty() {
                sample["own_differences"] = json!(d);
            }
        }
        let bad = sample.get("provenance_error").is_some()
            || sample.get("daemon_error").is_some()
            || judgement == "different"
            || !fv.is_empty()
            || sample.get("own_answers") == Some(&Value::Bool(false));
        if bad {
            self.failures += 1;
            self.keep_logs(&step);
        }
        eprintln!(
            "r{} {phase:<13} {id:<32} stock {:>7.1} s  sail {:>7.1} s  {judgement:<9} {served:<8} hits {}/{}{}{}",
            self.index,
            stock.wall_ms / 1000.0,
            sail.wall_ms / 1000.0,
            pair.cache.hits,
            pair.cache.hits + pair.cache.puts,
            if fv.is_empty() {
                String::new()
            } else {
                format!("  FALSE VERIFIED {fv:?}")
            },
            if mismatch {
                "  (stock outcome differs from the catalogue)"
            } else {
                ""
            },
        );
        self.write(&sample);
        Some(sail)
    }

    fn write(&mut self, sample: &Value) {
        if let Err(error) = writeln!(self.samples, "{sample}") {
            self.failures += 1;
            eprintln!("cannot retain mutation sample: {error}");
        }
    }

    /// Copies a failing step's logs into `<out>/failing/`.
    fn keep_logs(&self, step: &str) {
        let from = self
            .setup
            .scratch
            .join("logs")
            .join(format!("r{}", self.index))
            .join(step);
        let to =
            self.out
                .join("failing")
                .join(format!("r{}-{}", self.index, step.replace(':', "-")));
        let _ = sail_verus::run::copy_tree(&from, &to);
    }
}

fn daemon_continuity(expected: &mut Option<u64>, actual: Option<u64>) -> bool {
    let Some(actual) = actual else {
        return false;
    };
    if let Some(previous) = *expected {
        previous == actual
    } else {
        *expected = Some(actual);
        true
    }
}

/// How a revert's Sail result differs from Sail's own cold result on the
/// same source. The cold run also verified the dependencies (one more
/// `--output-json` document each), so the revert's documents are compared
/// with the cold run's last ones: Verus's results and per-function details,
/// and the exit code.
fn own_differences(cold: &RunRecord, revert: &RunRecord) -> Vec<String> {
    let mut d = Vec::new();
    if cold.exit_code != revert.exit_code {
        d.push(format!(
            "exit code: cold {:?} / revert {:?}",
            cold.exit_code, revert.exit_code
        ));
    }
    let skip = cold.outputs.len().saturating_sub(revert.outputs.len());
    if revert.outputs.is_empty() || cold.outputs.len() < revert.outputs.len() {
        d.push(format!(
            "--output-json documents: cold {} / revert {}",
            cold.outputs.len(),
            revert.outputs.len()
        ));
    }
    for (i, (c, r)) in cold
        .outputs
        .iter()
        .skip(skip)
        .zip(&revert.outputs)
        .enumerate()
    {
        for key in ["verification-results", "func-details"] {
            if c.get(key) != r.get(key) {
                d.push(format!("{key} differs (document {i})"));
            }
        }
    }
    d
}

#[allow(
    clippy::too_many_lines,
    reason = "the round's steps read best in order"
)]
fn run_round(r: &mut Round, edits: &[&Edit], meta: &Value) -> Result<(), String> {
    let ws = Workspace::prepare(
        &r.setup.installed,
        &r.setup.unit,
        &r.setup.verus_source,
        &r.setup.scratch.join(format!("r{}", r.index)),
    )?;
    let _ = std::fs::remove_dir_all(&r.cache);
    std::fs::create_dir_all(&r.cache).map_err(|e| e.to_string())?;
    let pair = r.run_pair(&ws, "cold:original", 0, false);
    let cold = r.record(pair, "cold", 0, None, "", None);
    let mut originals: BTreeMap<String, String> = BTreeMap::new();
    for e in edits {
        if !originals.contains_key(&e.file) {
            let text = std::fs::read_to_string(ws.src.join(&e.file))
                .map_err(|err| format!("{}: {err}", e.file))?;
            originals.insert(e.file.clone(), text);
        }
    }
    let n = edits.len();
    // Rotate the order so each mutant sits at a different point of the
    // loop in each round.
    let offset = r.index * n / r.rounds.max(1);
    for k in 0..n {
        let Some(edit) = edits.get((k + offset) % n.max(1)).copied() else {
            continue;
        };
        let file = ws.src.join(&edit.file);
        let original = originals.get(&edit.file).cloned().unwrap_or_default();
        let mutated = apply(edit, &original)?;
        write_fresh(&file, &mutated).map_err(|e| e.to_string())?;
        let pair = r.run_pair(&ws, &format!("mutant:{}", edit.id), k + 1, false);
        r.record(
            pair,
            "mutant",
            k + 1,
            Some(edit),
            &class_of(meta, &edit.id),
            None,
        );
        write_fresh(&file, &original).map_err(|e| e.to_string())?;
    }
    let pair = r.run_pair(&ws, "revert:original", n + 1, false);
    r.record(pair, "revert", n + 1, None, "", cold.as_ref());
    // The stock side must rebuild for the forced step too (Sail's side is
    // forced by `SAIL_MANAGED_FORCE`).
    ws.clean_stock(&r.setup.installed)?;
    let pair = r.run_pair(&ws, "revert-forced:original", n + 2, true);
    r.record(pair, "revert-forced", n + 2, None, "", cold.as_ref());
    stop_daemon(r.sail, &r.cache);
    let _ = std::fs::remove_dir_all(&ws.root);
    let _ = std::fs::remove_dir_all(&r.cache);
    Ok(())
}

fn version_of(cmd: &Path, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or_default()
                .to_owned()
        })
        .unwrap_or_default()
}

/// Entry point for `sail-bench mutants run`.
///
/// # Errors
///
/// A message for bad usage or setup; after the run, when any comparison
/// differed, any function was falsely verified, a revert check failed or a
/// step could not run.
pub fn main(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    let catalogue = Catalogue::load(&o.catalogue)?;
    if let Some(e) = catalogue.edits.iter().find(|e| e.expected == "pending") {
        return Err(format!(
            "{}: `expected` is pending; run `sail-bench mutants oracle` first",
            e.id
        ));
    }
    let meta = load_meta(&sidecar(&o.catalogue)).unwrap_or(Value::Null);
    let edits = select(&catalogue.edits, &meta, o.per_class, o.filter.as_deref());
    if edits.is_empty() {
        return Err("no mutants selected".to_owned());
    }
    let setup = Setup::new(&catalogue, o.scratch.clone())?;
    let sail = crate::daemon::locate_sail(o.sail.clone())?;
    let verus_build = verus_build_from_env();
    let name = o.catalogue.file_stem().map_or_else(
        || "mutants".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let out = o
        .out
        .clone()
        .unwrap_or_else(|| setup.scratch.join("out").join(&name));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let catalogue_sha = fingerprint(&std::fs::read(&o.catalogue).unwrap_or_default());
    let header = json!({
        "catalogue": o.catalogue.display().to_string(),
        "catalogue_fingerprint": catalogue_sha,
        "unit": setup.unit.id,
        "mutants": edits.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        "rounds": o.rounds,
        "machine": o.machine,
        "machine_info": sail_verus::run::machine_info(),
        "sail": sail.display().to_string(),
        "sail_version": version_of(&sail, &["--version"]),
        "sail_verus_build": verus_build.as_ref().map(|p| p.display().to_string()),
        "stock_verus": setup.installed.pins.verus.release,
        "stock_cargo_verus": setup.installed.cargo_verus.display().to_string(),
    });
    std::fs::write(
        out.join("header.json"),
        serde_json::to_string_pretty(&header).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    let samples_path = out.join("samples.jsonl");
    let _ = std::fs::remove_file(&samples_path);
    eprintln!(
        "mutants: {} ({} mutants x {} rounds), sail {}, scratch {}, out {}",
        setup.unit.id,
        edits.len(),
        o.rounds,
        sail.display(),
        setup.scratch.display(),
        out.display()
    );
    let mut plains = BTreeMap::new();
    let mut failures = 0;
    for round in 0..o.rounds {
        let samples = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&samples_path)
            .map_err(|e| e.to_string())?;
        let mut r = Round {
            setup: &setup,
            sail: &sail,
            verus_build: verus_build.as_deref(),
            cache: setup.scratch.join(format!("c{round}")),
            timeout: o.timeout,
            index: round,
            rounds: o.rounds,
            out: &out,
            samples,
            plains: &mut plains,
            failures: 0,
            daemon_pid: None,
        };
        let result = run_round(&mut r, &edits, &meta);
        failures += r.failures;
        if let Err(e) = result {
            stop_daemon(&sail, &setup.scratch.join(format!("c{round}")));
            return Err(format!("round {round}: {e}"));
        }
    }
    let report = super::stats::report_dir(&out)?;
    std::fs::write(out.join("report.md"), &report).map_err(|e| e.to_string())?;
    print!("{report}");
    if failures > 0 {
        return Err(format!(
            "{failures} steps differed, falsely verified, failed a revert check or could not run; logs under {}",
            out.join("failing").display()
        ));
    }
    Ok(())
}

/// A content fingerprint for the run header (FNV-1a, 64-bit): it
/// identifies which catalogue bytes a run used; it is not a security
/// property, and it avoids a new dependency.
fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(id: &str) -> Edit {
        Edit {
            id: id.to_owned(),
            unit: "p/u".to_owned(),
            kind: "body".to_owned(),
            file: "a.rs".to_owned(),
            description: String::new(),
            expected: "errors".to_owned(),
            find: "a".to_owned(),
            replace: "b".to_owned(),
        }
    }

    #[test]
    fn selects_per_class_in_order() {
        let edits = vec![
            edit("x-arith-001"),
            edit("x-arith-002"),
            edit("x-compare-001"),
            edit("x-arith-003"),
        ];
        let ids = |v: Vec<&Edit>| v.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(select(&edits, &Value::Null, Some(1), None)),
            ["x-arith-001", "x-compare-001"]
        );
        assert_eq!(
            ids(select(&edits, &Value::Null, None, Some("compare"))),
            ["x-compare-001"]
        );
        assert_eq!(select(&edits, &Value::Null, None, None).len(), 4);
    }
}

#[cfg(test)]
mod continuity_tests {
    use super::daemon_continuity;

    #[test]
    fn one_round_requires_the_same_observed_daemon() {
        let mut expected = None;
        assert!(!daemon_continuity(&mut expected, None));
        assert!(daemon_continuity(&mut expected, Some(42)));
        assert!(daemon_continuity(&mut expected, Some(42)));
        assert!(!daemon_continuity(&mut expected, Some(43)));
        assert!(!daemon_continuity(&mut expected, None));
    }
}
