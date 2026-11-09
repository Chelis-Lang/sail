//! `sail-bench rename-check`: validation of the symbol-renaming mutation
//! ([`sail_portfolio::rename`]) over captured sessions. **Spike tooling**
//! (v0.3.0 Track B).
//!
//! For every session under the given trace directories it:
//! 1. plans the renaming with identity names, rewrites, and compares the
//!    result with the captured bytes (must be byte-identical);
//! 2. replays the session in a fresh pinned Z3 under the baseline, the
//!    requested reseeds, shuffles and renamings, recording every solver
//!    error reply, every answer and every `rlimit-count` delta.
//!
//! It writes all of that as one JSON file and prints a summary. It judges
//! nothing itself beyond the summary counts.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use sail_portfolio::config::{Config, configs};
use sail_portfolio::fragility::replay;
use sail_portfolio::rename::{Naming, Stats, plan};
use sail_portfolio::sessions::{SessionFiles, discover};
use sail_portfolio::transcript::split_commands;

use crate::solver::{Identity, Solver};

/// Options of `rename-check`.
#[derive(Debug, Clone)]
pub struct Args {
    /// Trace directories (each as [`discover`] accepts).
    pub traces: Vec<PathBuf>,
    /// The pinned Z3.
    pub z3: PathBuf,
    /// Reseeds including the baseline.
    pub seeds: u64,
    /// Assertion shuffles.
    pub shuffles: u64,
    /// Renamings.
    pub renames: u64,
    /// Solver processes at a time.
    pub jobs: usize,
    /// Where the JSON goes.
    pub out: PathBuf,
    /// A free-form machine label (instance type and region).
    pub label: String,
}

/// The static part of one session.
#[derive(Debug, Clone, Serialize)]
pub struct SessionRecord {
    /// Session id.
    pub id: String,
    /// Input size in bytes.
    pub bytes: usize,
    /// Top-level commands.
    pub commands: usize,
    /// Whether the identity renaming reproduced the input exactly (`None`
    /// if the session was refused or unreadable).
    pub identity_identical: Option<bool>,
    /// Tokens the identity renaming replaced with themselves.
    pub identity_tokens: usize,
    /// What the plan covers.
    pub plan: Option<Stats>,
    /// Why the session was refused or could not be read.
    pub refused: Option<String>,
}

/// One replay.
#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    /// Index into [`File::sessions`].
    pub session: usize,
    /// Index into [`File::configs`].
    pub config: usize,
    /// Infrastructure failure (never an answer).
    pub infra: Option<String>,
    /// Solver error replies.
    pub errors: Vec<String>,
    /// Batches that could not be paired.
    pub unpaired_batches: usize,
    /// Wall-clock milliseconds.
    pub elapsed_ms: u64,
    /// The answer of each check.
    pub answers: Vec<Option<String>>,
    /// The `rlimit-count` delta of each check.
    pub rlimit: Vec<Option<u64>>,
}

/// The result file.
#[derive(Debug, Clone, Serialize)]
pub struct File {
    /// Always `rename-check`.
    pub workload: &'static str,
    /// The machine label given.
    pub machine: String,
    /// The solver.
    pub solver: Identity,
    /// Configuration labels.
    pub configs: Vec<String>,
    /// Sessions.
    pub sessions: Vec<SessionRecord>,
    /// Replays.
    pub runs: Vec<RunRecord>,
}

fn inspect(files: &SessionFiles) -> (SessionRecord, Option<Vec<u8>>) {
    let mut record = SessionRecord {
        id: files.id.clone(),
        bytes: 0,
        commands: 0,
        identity_identical: None,
        identity_tokens: 0,
        plan: None,
        refused: None,
    };
    let input = match std::fs::read(&files.input) {
        Ok(input) => input,
        Err(e) => {
            record.refused = Some(format!("unreadable: {e}"));
            return (record, None);
        }
    };
    record.bytes = input.len();
    let commands = split_commands(&input);
    record.commands = commands.len();
    match plan(&commands, Naming::Identity).and_then(|p| {
        let (bytes, tokens) = p.rewrite(&input, &commands)?;
        Ok((p.stats, bytes, tokens))
    }) {
        Ok((stats, bytes, tokens)) => {
            record.plan = Some(stats);
            record.identity_tokens = tokens;
            record.identity_identical = Some(bytes == input);
        }
        Err(e) => record.refused = Some(e.to_string()),
    }
    (record, Some(input))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs the check and writes the result file.
///
/// # Errors
///
/// A message if a trace directory cannot be read or the file cannot be
/// written.
pub fn run(args: &Args) -> Result<(), String> {
    let mut sessions: Vec<SessionFiles> = Vec::new();
    for (i, dir) in args.traces.iter().enumerate() {
        for mut s in discover(dir).map_err(|e| e.to_string())? {
            s.id = format!("t{i}/{}", s.id);
            sessions.push(s);
        }
    }
    let solver = Solver {
        path: args.z3.clone(),
        timeout: std::time::Duration::from_secs(3600),
    };
    let configs: Vec<Config> = configs(args.seeds, args.shuffles, args.renames);
    eprintln!(
        "rename-check: {} sessions, {} configurations, {} jobs",
        sessions.len(),
        configs.len(),
        args.jobs
    );
    let inspected = sail_portfolio::parallel::map(&sessions, args.jobs, inspect, |_| {});
    let records: Vec<SessionRecord> = inspected.iter().map(|(r, _)| r.clone()).collect();
    let units: Vec<(usize, usize)> = (0..sessions.len())
        .flat_map(|s| (0..configs.len()).map(move |c| (s, c)))
        .collect();
    let total = units.len();
    let runs = sail_portfolio::parallel::map(
        &units,
        args.jobs,
        |&(s, c)| {
            let input = inspected.get(s).and_then(|(_, input)| input.as_deref());
            let config = configs.get(c).copied().unwrap_or(Config::BASELINE);
            let mut record = RunRecord {
                session: s,
                config: c,
                infra: None,
                errors: Vec::new(),
                unpaired_batches: 0,
                elapsed_ms: 0,
                answers: Vec::new(),
                rlimit: Vec::new(),
            };
            let Some(input) = input else {
                record.infra = Some("unreadable".to_owned());
                return record;
            };
            let run = replay(&solver, input, config);
            record.infra = run.infra;
            record.errors = run.errors;
            record.unpaired_batches = run.unpaired_batches;
            record.elapsed_ms = run.elapsed_ms;
            record.answers = run
                .checks
                .iter()
                .map(|c| c.answer.as_deref().map(text))
                .collect();
            record.rlimit = run.checks.iter().map(|c| c.rlimit_delta).collect();
            record
        },
        |done| {
            if done.is_multiple_of(200) || done == total {
                eprintln!("replayed {done}/{total}");
            }
        },
    );
    let file = File {
        workload: "rename-check",
        machine: args.label.clone(),
        solver: solver.identity(),
        configs: configs.iter().map(Config::label).collect(),
        sessions: records,
        runs,
    };
    print!("{}", summary(&file));
    let mut json = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
    json.push(b'\n');
    std::fs::write(&args.out, json)
        .map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
    println!("wrote {}", args.out.display());
    Ok(())
}

fn family(label: &str) -> &str {
    label.split('=').next().unwrap_or(label)
}

/// Per-family counts for [`summary`].
#[derive(Default)]
struct Tally {
    runs: usize,
    infra: usize,
    runs_with_errors: usize,
    errors: usize,
    unpaired: usize,
    compared: usize,
    flips: usize,
    missing: usize,
}

/// A plain-text summary: identity results, solver errors per family, and
/// answer flips against the baseline replay per family.
#[must_use]
pub fn summary(file: &File) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let identical = file
        .sessions
        .iter()
        .filter(|s| s.identity_identical == Some(true))
        .count();
    let different = file
        .sessions
        .iter()
        .filter(|s| s.identity_identical == Some(false))
        .count();
    let refused = file.sessions.iter().filter(|s| s.refused.is_some()).count();
    let tokens: usize = file.sessions.iter().map(|s| s.identity_tokens).sum();
    let bytes: usize = file.sessions.iter().map(|s| s.bytes).sum();
    let _ = writeln!(
        out,
        "sessions {}: identity byte-identical {identical}, different {different}, refused {refused} \
({bytes} bytes, {tokens} tokens rewritten by the identity plan)",
        file.sessions.len()
    );
    let baseline: BTreeMap<usize, &RunRecord> = file
        .runs
        .iter()
        .filter(|r| file.configs.get(r.config).is_some_and(|l| l == "baseline"))
        .map(|r| (r.session, r))
        .collect();
    let mut tallies: BTreeMap<&str, Tally> = BTreeMap::new();
    for r in &file.runs {
        let label = file.configs.get(r.config).map_or("?", String::as_str);
        let t = tallies.entry(family(label)).or_default();
        t.runs += 1;
        t.infra += usize::from(r.infra.is_some());
        t.runs_with_errors += usize::from(!r.errors.is_empty());
        t.errors += r.errors.len();
        t.unpaired += r.unpaired_batches;
        let Some(base) = baseline.get(&r.session) else {
            continue;
        };
        if r.infra.is_some() {
            continue;
        }
        for (i, b) in base.answers.iter().enumerate() {
            match (b, r.answers.get(i).and_then(Option::as_ref)) {
                (Some(b), Some(a)) => {
                    t.compared += 1;
                    t.flips += usize::from(a != b);
                }
                _ => t.missing += 1,
            }
        }
    }
    for (name, t) in &tallies {
        let _ = writeln!(
            out,
            "{name}: runs {}, infra {}, runs with solver errors {} ({} error replies), \
unpaired batches {}, answers compared {}, flips vs baseline {}, missing {}",
            t.runs,
            t.infra,
            t.runs_with_errors,
            t.errors,
            t.unpaired,
            t.compared,
            t.flips,
            t.missing
        );
    }
    out
}
