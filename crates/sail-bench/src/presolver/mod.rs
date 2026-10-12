//! Pre-solver coverage spike (experimental, rescue lane L6).
//!
//! Measures how many of Verus's satisfiability checks a fast in-process
//! decision procedure for an "easy fragment" could refute. For every check
//! in the shim's traces it rebuilds the assertion stack, extracts a sound
//! relaxation in each candidate [`Fragment`] ([`extract`]), and decides the
//! relaxation with the pinned Z3 as the oracle. It reports how often the
//! pinned Z3 said `unsat` *and* a relaxation is `unsat` (the pre-solver could
//! have answered), by count and by recorded solve time, overall and for the
//! checks a default-mode cache would have missed during edit sequences
//! ([`capture`]). A relaxation that is `unsat` while Z3's full answer is not
//! is reported separately: it is impossible for `sat`, and a real
//! divergence for `unknown`.
//!
//! Nothing here is used by the shim, cache or daemon.
//!
//! - [`capture`]: traced edit sequences (`sail-bench presolver-capture`);
//! - [`extract`]: fragment extraction and the oracle query;
//! - [`tight`]: the tight rational relaxation (Farkas-decidable);
//! - [`proto`]: a small in-process refutation prototype for timing.

pub mod capture;
pub mod extract;
pub mod proto;
pub mod term;
pub mod tight;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::BenchError;
use crate::solver::{self, Identity, Outcome, Solver};
use crate::stats::percentile;
use crate::trace::{self, CheckRecord, SessionFiles};
use crate::transcript::{Class, split_commands};
use extract::{Ctx, Fragment, Ir, Translated};

/// Options for [`presolver`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Parallel sessions.
    pub jobs: usize,
    /// Examples kept per kind.
    pub examples: usize,
    /// Where to save example and anomaly queries, if anywhere.
    pub save: Option<PathBuf>,
    /// Replay every Nth check fresh for the miss-cost model (edit misses
    /// are always replayed); 0 replays only edit misses.
    pub replay_every: u64,
    /// The `:rlimit` for oracle queries.
    pub rlimit: u64,
}

/// Count and recorded solve time.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Tally {
    /// Checks.
    pub checks: u64,
    /// Sum of the in-session solve times, microseconds.
    pub solve_us: u64,
}

impl Tally {
    fn add(&mut self, us: u64) {
        self.checks += 1;
        self.solve_us += us;
    }
}

/// Results for one fragment over one population.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FragmentStats {
    /// Z3 `unsat` and relaxation `unsat`: the pre-solver could answer.
    pub covered: Tally,
    /// Relaxation `unsat` while Z3's full answer was `sat`: impossible
    /// for a sound relaxation (a tool or solver bug).
    pub unsat_but_full_sat: Tally,
    /// Relaxation `unsat` while Z3's full answer was `unknown`.
    pub unsat_but_full_unknown: Tally,
    /// Relaxation `unknown` (resource limit).
    pub relaxation_unknown: u64,
    /// Oracle errors (an emission bug) or infrastructure failures.
    pub errors: u64,
}

/// One population's results.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Population {
    /// All checks.
    pub all: Tally,
    /// Checks Z3 answered `unsat`.
    pub z3_unsat: Tally,
    /// Checks Z3 answered `unknown`.
    pub z3_unknown: Tally,
    /// Checks Z3 answered `sat`.
    pub z3_sat: Tally,
    /// Per fragment, by name.
    pub fragments: BTreeMap<String, FragmentStats>,
    /// Covered by the tight rational relaxation of `lia+uc+bounds`.
    pub tight: FragmentStats,
    /// Covered by the in-process prototype on `lia+uc+bounds`.
    pub proto: FragmentStats,
    /// Proof obligations (location labels) in Z3-unsat checks.
    pub obligations: u64,
    /// Of those, discharged by the `lia+uc+bounds` relaxation alone.
    pub obligations_discharged: u64,
    /// Z3-unsat checks with every obligation but one discharged.
    pub all_but_one: u64,
}

/// Distribution summary.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Dist {
    /// Samples.
    pub n: usize,
    /// Median.
    pub p50: Option<u64>,
    /// 90th percentile.
    pub p90: Option<u64>,
    /// 99th percentile.
    pub p99: Option<u64>,
    /// Maximum.
    pub max: Option<u64>,
}

impl Dist {
    fn of(mut v: Vec<u64>) -> Self {
        v.sort_unstable();
        Self {
            n: v.len(),
            p50: percentile(&v, 50.0),
            p90: percentile(&v, 90.0),
            p99: percentile(&v, 99.0),
            max: v.last().copied(),
        }
    }
}

/// Latency model inputs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Latency {
    /// Fresh Z3 process that only exits, microseconds.
    pub z3_spawn_us: Dist,
    /// Fresh replay of the check's whole input up to (not including) the
    /// check: process start, parse, assert.
    pub setup_us: Dist,
    /// Fresh replay of the whole input including the check.
    pub fresh_total_us: Dist,
    /// In-session solve time of the replayed checks.
    pub solve_us: Dist,
    /// The same, for edit misses covered by `lia+uc+bounds`.
    pub covered_miss_fresh_total_us: Dist,
    /// In-process extraction per check (instances and emission; the
    /// per-assertion translation is amortized over the session).
    pub extract_us: Dist,
    /// Per-assertion translation time amortized per check.
    pub translate_amortized_us: Dist,
    /// Prototype decision time on covered checks.
    pub proto_us: Dist,
    /// Oracle wall time on covered checks (fresh Z3 on the relaxation).
    pub oracle_us: Dist,
}

/// Certificate-size inputs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Certificates {
    /// Conjuncts in the `lia+uc+bounds` relaxation of covered checks.
    pub conjuncts: Dist,
    /// Unsat-core size reported by the oracle on covered checks.
    pub core: Dist,
    /// Prelude instances added per check.
    pub instances: Dist,
}

/// An example check.
#[derive(Debug, Clone, Serialize)]
pub struct Example {
    /// What it exemplifies.
    pub kind: String,
    /// Session id.
    pub session: String,
    /// Check index.
    pub seq: usize,
    /// Z3's recorded answer.
    pub answer: Option<String>,
    /// Fragment.
    pub fragment: String,
    /// Saved query file, if saved.
    pub saved: Option<String>,
}

/// The report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    /// Solver identity.
    pub solver: Option<Identity>,
    /// Sessions read.
    pub sessions: usize,
    /// Sessions that could not be read.
    pub unreadable: Vec<String>,
    /// Checks skipped (inexact session model, untraced, unpaired).
    pub skipped: BTreeMap<String, u64>,
    /// Populations: `all`, `cold`, `edit`, `edit-miss-v2`, `edit-miss-v1`,
    /// `edit-hit-v2`.
    pub populations: BTreeMap<String, Population>,
    /// Monotonicity violations (a weaker fragment `unsat`, a stronger one
    /// not): a tool bug if non-zero.
    pub monotonicity_violations: u64,
    /// Latency model inputs.
    pub latency: Latency,
    /// Certificate-size inputs.
    pub certificates: Certificates,
    /// Checks whose stack mentions bit-vector sorts.
    pub bitvector_checks: u64,
    /// Per edit cycle: what a default-mode cache would have missed and
    /// what the pre-solver could have answered.
    pub cycles: Vec<CycleSummary>,
    /// Examples.
    pub examples: Vec<Example>,
}

/// One edit cycle.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CycleSummary {
    /// Sequence.
    pub sequence: String,
    /// Cycle.
    pub cycle: String,
    /// All checks in the cycle.
    pub checks: Tally,
    /// Key-v2 misses.
    pub misses: Tally,
    /// Misses Z3 answered `unsat`.
    pub misses_unsat: Tally,
    /// Misses Z3 did not answer `unsat` (the failures an agent needs).
    pub misses_not_unsat: Tally,
    /// Misses covered by `lia+uc+bounds`.
    pub misses_covered: Tally,
}

/// Where a session sits in a capture.
#[derive(Debug, Clone, Default)]
struct Placement {
    sequence: Option<String>,
    cycle: Option<String>,
}

impl Placement {
    fn is_edit(&self) -> bool {
        self.cycle.as_deref().is_some_and(|c| !c.starts_with("c00"))
    }
}

fn walk(
    dir: &Path,
    root: &Path,
    out: &mut Vec<(SessionFiles, Placement)>,
) -> Result<(), BenchError> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| BenchError::io(dir, &e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, root, out)?;
            continue;
        }
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        let rel = dir.strip_prefix(root).unwrap_or(dir);
        let cycle_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| {
                n.len() > 4
                    && n.starts_with('c')
                    && n.as_bytes()
                        .get(1..3)
                        .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
                    && n.as_bytes().get(3) == Some(&b'-')
            })
            .map(str::to_owned);
        let placement = Placement {
            sequence: cycle_name.as_ref().and_then(|_| {
                dir.parent()
                    .map(|s| format!("{}", root.join(s.strip_prefix(root).unwrap_or(s)).display()))
            }),
            cycle: cycle_name,
        };
        let id = format!("{}/{stem}", rel.display());
        out.push((
            SessionFiles {
                id,
                jsonl: p.clone(),
                input: dir.join(format!("{stem}.in.smt2")),
                output: dir.join(format!("{stem}.out")),
            },
            placement,
        ));
    }
    Ok(())
}

/// Cache-miss flags per session and check: `(v2 miss, v1 miss)`.
fn miss_flags(
    sessions: &[(SessionFiles, Placement)],
    traces: &[Option<trace::SessionTrace>],
) -> HashMap<(usize, usize), (bool, bool)> {
    let mut by_seq: BTreeMap<String, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
    for (i, (_, p)) in sessions.iter().enumerate() {
        if let (Some(s), Some(c)) = (&p.sequence, &p.cycle) {
            by_seq
                .entry(s.clone())
                .or_default()
                .entry(c.clone())
                .or_default()
                .push(i);
        }
    }
    let mut out = HashMap::new();
    for cycles in by_seq.values() {
        let mut seen_v2: HashSet<String> = HashSet::new();
        let mut seen_v1: HashSet<String> = HashSet::new();
        for members in cycles.values() {
            let mut add_v2 = Vec::new();
            let mut add_v1 = Vec::new();
            for &i in members {
                let Some(Some(t)) = traces.get(i) else {
                    continue;
                };
                for (k, c) in t.checks.iter().enumerate() {
                    let m2 = c.prefix_key.as_ref().is_none_or(|k| !seen_v2.contains(k));
                    let m1 = c.key.as_ref().is_none_or(|k| !seen_v1.contains(k));
                    out.insert((i, k), (m2, m1));
                    add_v2.extend(c.prefix_key.clone());
                    add_v1.extend(c.key.clone());
                }
            }
            seen_v2.extend(add_v2);
            seen_v1.extend(add_v1);
        }
    }
    out
}

#[derive(Debug, Clone)]
struct OracleResult {
    answer: Option<String>,
    core: Option<usize>,
    wall_us: u64,
    error: Option<String>,
}

type OracleCache = Mutex<HashMap<[u8; 32], OracleResult>>;

fn oracle(solver: &Solver, cache: &OracleCache, query: &str) -> OracleResult {
    let key = sail_key::sha256(query.as_bytes());
    if let Ok(c) = cache.lock()
        && let Some(r) = c.get(&key)
    {
        return r.clone();
    }
    let r = match solver::run(solver, query.as_bytes()) {
        Ok(run) => {
            let text = String::from_utf8_lossy(&run.stdout).into_owned();
            let mut lines = text.lines();
            let first = lines.next().map(str::trim).map(str::to_owned);
            let rest: String = lines.collect::<Vec<_>>().join(" ");
            let infra = run.outcome == Outcome::InfraTimeout;
            let answer = first
                .clone()
                .filter(|a| matches!(a.as_str(), "sat" | "unsat" | "unknown"));
            let core = (answer.as_deref() == Some("unsat")).then(|| {
                rest.trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .split_whitespace()
                    .count()
            });
            let error = if infra {
                Some("infra-timeout".to_owned())
            } else if answer.is_none() {
                // Any error (a sort mismatch in the emitted query) is printed
                // before the answer, so the first line is not an answer.
                Some(text.chars().take(400).collect())
            } else {
                None
            };
            OracleResult {
                answer,
                core,
                wall_us: u64::try_from(run.elapsed.as_micros()).unwrap_or(u64::MAX),
                error,
            }
        }
        Err(e) => OracleResult {
            answer: None,
            core: None,
            wall_us: 0,
            error: Some(e.to_string()),
        },
    };
    if let Ok(mut c) = cache.lock() {
        c.insert(key, r.clone());
    }
    r
}

/// What one check produced.
#[derive(Debug, Clone, Default)]
struct CheckRow {
    session: usize,
    seq: usize,
    answer: Option<String>,
    solve_us: u64,
    edit: bool,
    miss: Option<(bool, bool)>,
    bv: bool,
    frags: [Option<OracleRow>; 3],
    tight: Option<OracleRow>,
    proto: Option<ProtoRow>,
    /// `(labels, discharged)` for Z3-unsat checks.
    obligations: Option<(u64, u64)>,
    conjuncts: usize,
    instances: usize,
    extract_us: u64,
    translate_us: u64,
    fresh: Option<(u64, u64, Option<String>)>,
    saved: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct OracleRow {
    answer: Option<String>,
    core: Option<usize>,
    wall_us: u64,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct ProtoRow {
    unsat: bool,
    us: u64,
}

impl From<OracleResult> for OracleRow {
    fn from(r: OracleResult) -> Self {
        Self {
            answer: r.answer,
            core: r.core,
            wall_us: r.wall_us,
            error: r.error,
        }
    }
}

/// Oracle errors saved so far (at most 20 are kept).
static SAVED_ERRORS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Uncovered samples saved so far (at most 40 are kept).
static SAVED_UNCOVERED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct Level {
    commands: Vec<usize>,
    translated: Vec<Rc<Translated>>,
}

struct Shared<'a> {
    solver: &'a Solver,
    cache: &'a OracleCache,
    options: &'a Options,
}

fn fresh_input(
    cmds: &[crate::transcript::Cmd],
    options: &[usize],
    levels: &[Level],
    check: Option<usize>,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut line = |b: &[u8]| {
        out.extend_from_slice(b);
        out.push(b'\n');
    };
    for &i in options {
        if let Some(c) = cmds.get(i) {
            line(&c.bytes);
        }
    }
    for (n, level) in levels.iter().enumerate() {
        if n > 0 {
            line(b"(push)");
        }
        for &i in &level.commands {
            if let Some(c) = cmds.get(i) {
                line(&c.bytes);
            }
        }
    }
    match check.and_then(|i| cmds.get(i)) {
        Some(c) => line(&c.bytes),
        None => line(b"(get-info :version)"),
    }
    out
}

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

fn save(dir: Option<&Path>, name: &str, text: &str) -> Option<String> {
    let dir = dir?;
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(name);
    std::fs::write(&path, text).ok()?;
    Some(path.display().to_string())
}

/// Analyses one session.
#[allow(clippy::too_many_lines)] // One linear pass over the transcript.
fn analyse_session(
    sh: &Shared,
    index: usize,
    files: &SessionFiles,
    trace: &trace::SessionTrace,
    misses: &HashMap<(usize, usize), (bool, bool)>,
    edit: bool,
    skipped: &Mutex<BTreeMap<String, u64>>,
) -> Vec<CheckRow> {
    let skip = |why: &str, n: u64| {
        if let Ok(mut s) = skipped.lock() {
            *s.entry(why.to_owned()).or_default() += n;
        }
    };
    let Ok(input) = std::fs::read(&files.input) else {
        skip("unreadable transcript", trace.checks.len() as u64);
        return Vec::new();
    };
    let cmds = split_commands(&input);
    let mut ctx = Ctx::new();
    let mut levels = vec![Level {
        commands: Vec::new(),
        translated: Vec::new(),
    }];
    let mut options = Vec::new();
    let mut rows = Vec::new();
    let mut checks = 0usize;
    let mut translate_total = Duration::ZERO;
    let mut bv_seen = false;
    for (i, cmd) in cmds.iter().enumerate() {
        match &cmd.class {
            Class::SetOption => options.push(i),
            Class::Push | Class::Pop => {
                let n = term::parse(&cmd.bytes)
                    .and_then(|t| {
                        t.list()
                            .and_then(|l| l.get(1))
                            .and_then(|a| a.atom())
                            .and_then(|a| a.parse().ok())
                    })
                    .unwrap_or(1usize);
                if cmd.class == Class::Push {
                    ctx.push(n);
                    for _ in 0..n {
                        levels.push(Level {
                            commands: Vec::new(),
                            translated: Vec::new(),
                        });
                    }
                } else if !ctx.pop(n) || levels.len() <= n {
                    skip("pop underflow (session abandoned)", 1);
                    break;
                } else {
                    levels.truncate(levels.len() - n);
                }
            }
            Class::Stack => {
                let Some(level) = levels.last_mut() else {
                    break;
                };
                level.commands.push(i);
                let Some(t) = term::parse(&cmd.bytes) else {
                    skip("unparsed stack command (session abandoned)", 1);
                    break;
                };
                if !bv_seen && term::mentions(&t, &mut |a| a == "BitVec") {
                    bv_seen = true;
                }
                match t.head() {
                    Some("assert") => {
                        if let Some(body) = t.list().and_then(|l| l.get(1)) {
                            let started = Instant::now();
                            ctx.record_axioms(body);
                            let tr = ctx.translate(body);
                            translate_total += started.elapsed();
                            level.translated.push(Rc::new(tr));
                        }
                    }
                    _ => ctx.declare_command(&t),
                }
            }
            Class::Check => {
                let seq = checks;
                checks += 1;
                let Some(rec) = trace.checks.get(seq) else {
                    skip("check missing from the trace", 1);
                    continue;
                };
                if rec.inexact.is_some() || rec.answer.is_none() {
                    skip("inexact or unanswered in the trace", 1);
                    continue;
                }
                if cmd.bytes.as_slice() != b"(check-sat)" {
                    skip("check-sat variant", 1);
                    continue;
                }
                let row = analyse_check(
                    sh,
                    (index, seq),
                    &cmds,
                    &options,
                    &levels,
                    &mut ctx,
                    rec,
                    (edit, misses.get(&(index, seq)).copied()),
                    (i, bv_seen),
                    translate_total,
                    &files.id,
                );
                rows.push(row);
            }
            Class::Unknown => {
                skip("unmodelled command (session abandoned)", 1);
                break;
            }
            _ => {}
        }
    }
    rows
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Spike code.
fn analyse_check(
    sh: &Shared,
    (session, seq): (usize, usize),
    cmds: &[crate::transcript::Cmd],
    options: &[usize],
    levels: &[Level],
    ctx: &mut Ctx,
    rec: &CheckRecord,
    (edit, miss): (bool, Option<(bool, bool)>),
    (check_index, bv): (usize, bool),
    translate_total: Duration,
    id: &str,
) -> CheckRow {
    let started = Instant::now();
    let mut row = CheckRow {
        session,
        seq,
        answer: rec.answer.clone(),
        solve_us: rec.solve_us().unwrap_or(0),
        edit,
        miss,
        bv,
        ..CheckRow::default()
    };
    let mut ground = Vec::new();
    for l in levels {
        for t in &l.translated {
            ground.extend(t.ground.iter().cloned());
        }
    }
    let (inst, n_inst) = ctx.instances(&ground);
    row.instances = n_inst;
    let mut sets: [Vec<&Ir>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (k, set) in sets.iter_mut().enumerate() {
        for l in levels {
            for t in &l.translated {
                if let Some(cs) = t.conjuncts.get(k) {
                    set.extend(cs.iter());
                }
            }
        }
    }
    sets[2].extend(inst.iter());
    row.conjuncts = sets[2].len();
    let queries: Vec<String> = sets
        .iter()
        .map(|s| extract::emit(s, &ctx.vars, sh.options.rlimit))
        .collect();
    let tight_query = tight::emit(&sets[2], &ctx.vars, sh.options.rlimit);
    row.extract_us = micros(started.elapsed());
    let checks_so_far = u64::try_from(seq + 1).unwrap_or(1);
    row.translate_us = micros(translate_total) / checks_so_far;
    let proto_started = Instant::now();
    let p = proto::refute(&sets[2], &ctx.vars);
    row.proto = Some(ProtoRow {
        unsat: p.is_some(),
        us: micros(proto_started.elapsed()),
    });
    for (k, q) in queries.iter().enumerate() {
        let r = oracle(sh.solver, sh.cache, q);
        if let Some(e) = &r.error
            && SAVED_ERRORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 20
        {
            let file = format!("error-{}-{seq}-{k}.smt2", id.replace('/', "_"));
            if let Some(path) = save(sh.options.save.as_deref(), &file, &format!("{q}\n; {e}\n")) {
                row.saved.push(("oracle error".to_owned(), path));
            }
        }
        let unsat = r.answer.as_deref() == Some("unsat");
        let full = rec.answer.as_deref();
        if k == 2
            && !unsat
            && full == Some("unsat")
            && sail_key::sha256(format!("{id}/{seq}").as_bytes())[0] < 16
            && SAVED_UNCOVERED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 40
        {
            let file = format!("uncovered-{}-{seq}.smt2", id.replace('/', "_"));
            if let Some(path) = save(sh.options.save.as_deref(), &file, q) {
                row.saved.push(("uncovered sample".to_owned(), path));
            }
        }
        if unsat
            && full != Some("unsat")
            && let Some(name) = Fragment::ALL.get(k).map(|f| f.name())
        {
            let file = format!(
                "anomaly-{}-{}-{}.smt2",
                id.replace('/', "_"),
                seq,
                name.replace('+', "_")
            );
            if let Some(path) = save(sh.options.save.as_deref(), &file, q) {
                row.saved
                    .push(("relaxation unsat, full not unsat".to_owned(), path));
            }
        }
        if let Some(slot) = row.frags.get_mut(k) {
            *slot = Some(r.into());
        }
    }
    if let Some(q) = &tight_query {
        row.tight = Some(oracle(sh.solver, sh.cache, q).into());
    }
    if rec.answer.as_deref() == Some("unsat") {
        row.obligations = Some(obligations(
            sh,
            ctx,
            &sets[2],
            is_unsat(row.frags[2].as_ref()),
        ));
    }
    let replay = (edit && row.miss.is_some_and(|m| m.0))
        || (sh.options.replay_every > 0
            && u64::from(sail_key::sha256(format!("{id}/{seq}").as_bytes())[0])
                .is_multiple_of(sh.options.replay_every.min(256)));
    if replay {
        let setup = fresh_input(cmds, options, levels, None);
        let full = fresh_input(cmds, options, levels, Some(check_index));
        if let (Ok(a), Ok(b)) = (
            solver::run(sh.solver, &setup),
            solver::run(sh.solver, &full),
        ) {
            let answer = String::from_utf8_lossy(&b.stdout)
                .lines()
                .next()
                .map(str::to_owned);
            row.fresh = Some((micros(a.elapsed), micros(b.elapsed), answer));
        }
    }
    row
}

/// Labels per check examined at most; the rest count as not discharged.
const MAX_LABELS: usize = 64;

/// Per-obligation discharge: for each location label `l` the relaxation
/// mentions, the relaxation plus `l` and the negation of every other
/// location label. Verus guards each assertion's goal with its label, so
/// `unsat` means that obligation follows from the relaxed context.
fn obligations(sh: &Shared, ctx: &Ctx, set: &[&Ir], whole_unsat: bool) -> (u64, u64) {
    let mut used = std::collections::BTreeSet::new();
    for c in set {
        extract::used_vars(c, &mut used);
    }
    let labels: Vec<u32> = used
        .into_iter()
        .filter(|v| {
            ctx.vars
                .get(*v as usize)
                .is_some_and(|v| v.name.starts_with("%%location_label%%"))
        })
        .collect();
    let n = labels.len() as u64;
    if whole_unsat {
        return (n, n);
    }
    let mut discharged = 0;
    for &l in labels.iter().take(MAX_LABELS) {
        let extra: Vec<Ir> = labels
            .iter()
            .map(|&o| {
                if o == l {
                    Ir::Var(o)
                } else {
                    Ir::App(Rc::from("not"), vec![Ir::Var(o)])
                }
            })
            .collect();
        let mut all: Vec<&Ir> = set.to_vec();
        all.extend(extra.iter());
        let q = extract::emit(&all, &ctx.vars, sh.options.rlimit);
        if oracle(sh.solver, sh.cache, &q).answer.as_deref() == Some("unsat") {
            discharged += 1;
        }
    }
    (n, discharged)
}

fn tally_fragment(s: &mut FragmentStats, r: Option<&OracleRow>, full: Option<&str>, us: u64) {
    let Some(r) = r else { return };
    if r.error.is_some() {
        s.errors += 1;
        return;
    }
    match (r.answer.as_deref(), full) {
        (Some("unsat"), Some("unsat")) => s.covered.add(us),
        (Some("unsat"), Some("sat")) => s.unsat_but_full_sat.add(us),
        (Some("unsat"), _) => s.unsat_but_full_unknown.add(us),
        (Some("unknown") | None, _) => s.relaxation_unknown += 1,
        _ => {}
    }
}

fn tally(p: &mut Population, row: &CheckRow) {
    let us = row.solve_us;
    p.all.add(us);
    match row.answer.as_deref() {
        Some("unsat") => p.z3_unsat.add(us),
        Some("sat") => p.z3_sat.add(us),
        _ => p.z3_unknown.add(us),
    }
    for (k, f) in Fragment::ALL.iter().enumerate() {
        let s = p.fragments.entry(f.name().to_owned()).or_default();
        tally_fragment(
            s,
            row.frags.get(k).and_then(Option::as_ref),
            row.answer.as_deref(),
            us,
        );
    }
    tally_fragment(&mut p.tight, row.tight.as_ref(), row.answer.as_deref(), us);
    if let Some((n, d)) = row.obligations {
        p.obligations += n;
        p.obligations_discharged += d;
        if n > 0 && d + 1 == n {
            p.all_but_one += 1;
        }
    }
    if let Some(pr) = &row.proto {
        let r = OracleRow {
            answer: Some(if pr.unsat { "unsat" } else { "unknown" }.to_owned()),
            core: None,
            wall_us: pr.us,
            error: None,
        };
        tally_fragment(&mut p.proto, Some(&r), row.answer.as_deref(), us);
    }
}

fn is_unsat(r: Option<&OracleRow>) -> bool {
    r.is_some_and(|r| r.answer.as_deref() == Some("unsat") && r.error.is_none())
}

/// Runs the analysis over every session under `dirs`.
///
/// # Errors
///
/// I/O errors while discovering sessions.
#[allow(clippy::too_many_lines)] // Spike code: one pass plus aggregation.
pub fn presolver(solver: &Solver, dirs: &[PathBuf], o: &Options) -> Result<Report, BenchError> {
    let mut sessions = Vec::new();
    for d in dirs {
        walk(d, d, &mut sessions)?;
    }
    let mut report = Report {
        solver: Some(solver.identity()),
        sessions: sessions.len(),
        ..Report::default()
    };
    let traces: Vec<Option<trace::SessionTrace>> =
        crate::parallel::map(&sessions, o.jobs, |(f, _)| trace::load(f).ok(), |_| {});
    for ((f, _), t) in sessions.iter().zip(&traces) {
        if t.is_none() {
            report.unreadable.push(f.id.clone());
        }
    }
    let misses = miss_flags(&sessions, &traces);
    let cache: OracleCache = Mutex::new(HashMap::new());
    let skipped = Mutex::new(BTreeMap::new());
    let sh = Shared {
        solver,
        cache: &cache,
        options: o,
    };
    let indices: Vec<usize> = (0..sessions.len()).collect();
    let total = indices.len();
    let rows: Vec<Vec<CheckRow>> = crate::parallel::map(
        &indices,
        o.jobs,
        |&i| {
            let (Some((files, placement)), Some(Some(t))) = (sessions.get(i), traces.get(i)) else {
                return Vec::new();
            };
            analyse_session(&sh, i, files, t, &misses, placement.is_edit(), &skipped)
        },
        |done| crate::progress("presolver", done, total),
    );
    report.skipped = skipped.into_inner().unwrap_or_default();
    let mut lat_setup = Vec::new();
    let mut lat_total = Vec::new();
    let mut lat_solve = Vec::new();
    let mut lat_cov_miss = Vec::new();
    let mut extract_us = Vec::new();
    let mut translate_us = Vec::new();
    let mut proto_us = Vec::new();
    let mut oracle_us = Vec::new();
    let mut conj = Vec::new();
    let mut core = Vec::new();
    let mut inst = Vec::new();
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut cycles: BTreeMap<(String, String), CycleSummary> = BTreeMap::new();
    for row in rows.iter().flatten() {
        if let Some((
            _,
            Placement {
                sequence: Some(seqn),
                cycle: Some(cyc),
            },
        )) = sessions.get(row.session)
            && row.edit
        {
            let short = Path::new(seqn)
                .file_name()
                .map_or_else(|| seqn.clone(), |n| n.to_string_lossy().into_owned());
            let c = cycles
                .entry((short.clone(), cyc.clone()))
                .or_insert_with(|| CycleSummary {
                    sequence: short,
                    cycle: cyc.clone(),
                    ..CycleSummary::default()
                });
            c.checks.add(row.solve_us);
            if row.miss.is_some_and(|m| m.0) {
                c.misses.add(row.solve_us);
                if row.answer.as_deref() == Some("unsat") {
                    c.misses_unsat.add(row.solve_us);
                    if is_unsat(row.frags.get(2).and_then(Option::as_ref)) {
                        c.misses_covered.add(row.solve_us);
                    }
                } else {
                    c.misses_not_unsat.add(row.solve_us);
                }
            }
        }
        let mut pops = vec![
            "all",
            if row.bv {
                "all-bitvector-sessions"
            } else {
                "all-without-bitvectors"
            },
        ];
        if row.edit {
            pops.push("edit");
            match row.miss {
                Some((true, _)) => pops.push("edit-miss-v2"),
                Some((false, _)) => pops.push("edit-hit-v2"),
                None => {}
            }
            if row.miss.is_some_and(|m| m.1) {
                pops.push("edit-miss-v1");
            }
        } else {
            pops.push("cold");
        }
        for p in pops {
            tally(report.populations.entry(p.to_owned()).or_default(), row);
        }
        if row.bv {
            report.bitvector_checks += 1;
        }
        let u: Vec<bool> = row.frags.iter().map(|f| is_unsat(f.as_ref())).collect();
        if u.windows(2).any(|w| matches!(w, [true, false])) {
            report.monotonicity_violations += 1;
        }
        let full_unsat = row.answer.as_deref() == Some("unsat");
        let covered = full_unsat && u.get(2).copied().unwrap_or(false);
        extract_us.push(row.extract_us);
        translate_us.push(row.translate_us);
        inst.push(row.instances as u64);
        if covered {
            conj.push(row.conjuncts as u64);
            if let Some(c) = row.frags.get(2).and_then(Option::as_ref) {
                core.extend(c.core.map(|c| c as u64));
                oracle_us.push(c.wall_us);
            }
            if let Some(p) = &row.proto {
                proto_us.push(p.us);
            }
        }
        if let Some((setup, total, _)) = &row.fresh {
            lat_setup.push(*setup);
            lat_total.push(*total);
            lat_solve.push(row.solve_us);
            if covered && row.miss.is_some_and(|m| m.0) {
                lat_cov_miss.push(*total);
            }
        }
        let id = sessions
            .get(row.session)
            .map(|s| s.0.id.clone())
            .unwrap_or_default();
        for (kind, path) in &row.saved {
            report.examples.push(Example {
                kind: kind.clone(),
                session: id.clone(),
                seq: row.seq,
                answer: row.answer.clone(),
                fragment: String::new(),
                saved: Some(path.clone()),
            });
        }
        for (k, f) in Fragment::ALL.iter().enumerate() {
            let first_cover = full_unsat
                && u.get(k).copied().unwrap_or(false)
                && (k == 0 || !u.get(k - 1).copied().unwrap_or(false));
            if first_cover {
                let n = kept.entry(f.name().to_owned()).or_default();
                if *n < o.examples {
                    *n += 1;
                    report.examples.push(Example {
                        kind: format!("first covered by {}", f.name()),
                        session: id.clone(),
                        seq: row.seq,
                        answer: row.answer.clone(),
                        fragment: f.name().to_owned(),
                        saved: None,
                    });
                }
            }
        }
    }
    let spawn: Vec<u64> = (0..20)
        .filter_map(|_| {
            solver::run(solver, b"(exit)\n")
                .ok()
                .map(|r| micros(r.elapsed))
        })
        .collect();
    report.cycles = cycles.into_values().collect();
    report.latency = Latency {
        z3_spawn_us: Dist::of(spawn),
        setup_us: Dist::of(lat_setup),
        fresh_total_us: Dist::of(lat_total),
        solve_us: Dist::of(lat_solve),
        covered_miss_fresh_total_us: Dist::of(lat_cov_miss),
        extract_us: Dist::of(extract_us),
        translate_amortized_us: Dist::of(translate_us),
        proto_us: Dist::of(proto_us),
        oracle_us: Dist::of(oracle_us),
    };
    report.certificates = Certificates {
        conjuncts: Dist::of(conj),
        core: Dist::of(core),
        instances: Dist::of(inst),
    };
    Ok(report)
}

fn pct(a: u64, b: u64) -> String {
    if b == 0 {
        return "-".to_owned();
    }
    #[allow(clippy::cast_precision_loss)] // Display only.
    let p = a as f64 * 100.0 / b as f64;
    format!("{p:.1}%")
}

fn ms(us: u64) -> String {
    #[allow(clippy::cast_precision_loss)] // Display only.
    let v = us as f64 / 1000.0;
    format!("{v:.0}")
}

fn dist(d: &Dist) -> String {
    let f = |v: Option<u64>| v.map_or_else(|| "-".to_owned(), |v| v.to_string());
    format!(
        "n={} p50={} p90={} p99={} max={}",
        d.n,
        f(d.p50),
        f(d.p90),
        f(d.p99),
        f(d.max)
    )
}

/// Renders the report as Markdown tables.
#[allow(clippy::too_many_lines)] // One table after another.
#[must_use]
pub fn render(r: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "presolver coverage: {} sessions, solver {}",
        r.sessions,
        r.solver
            .as_ref()
            .and_then(|i| i.version.clone())
            .unwrap_or_default()
    );
    let _ = writeln!(
        s,
        "skipped: {:?}; unreadable sessions: {}",
        r.skipped,
        r.unreadable.len()
    );
    let _ = writeln!(
        s,
        "monotonicity violations: {}; checks with bit-vectors: {}\n",
        r.monotonicity_violations, r.bitvector_checks
    );
    let _ = writeln!(
        s,
        "| population | checks | solve ms | Z3 unsat | Z3 unknown | Z3 sat | fragment | covered (count) | covered (solve time) | unsat but full unsat-less (sat/unknown) | relax unknown | errors |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (name, p) in &r.populations {
        let mut rows: Vec<(String, &FragmentStats)> =
            p.fragments.iter().map(|(k, v)| (k.clone(), v)).collect();
        rows.push(("tight-rational(bounds)".to_owned(), &p.tight));
        rows.push(("prototype(bounds)".to_owned(), &p.proto));
        for (f, st) in rows {
            let _ = writeln!(
                s,
                "| {name} | {} | {} | {} | {} | {} | {f} | {} ({}) | {} | {}/{} | {} | {} |",
                p.all.checks,
                ms(p.all.solve_us),
                p.z3_unsat.checks,
                p.z3_unknown.checks,
                p.z3_sat.checks,
                st.covered.checks,
                pct(st.covered.checks, p.all.checks),
                pct(st.covered.solve_us, p.all.solve_us),
                st.unsat_but_full_sat.checks,
                st.unsat_but_full_unknown.checks,
                st.relaxation_unknown,
                st.errors
            );
        }
    }
    let l = &r.latency;
    let _ = writeln!(s, "\nlatency (microseconds):");
    for (k, d) in [
        ("z3 spawn+exit", &l.z3_spawn_us),
        ("fresh setup (no check)", &l.setup_us),
        ("fresh total (with check)", &l.fresh_total_us),
        ("in-session solve (replayed)", &l.solve_us),
        (
            "fresh total, covered edit misses",
            &l.covered_miss_fresh_total_us,
        ),
        ("extraction per check", &l.extract_us),
        ("translation amortized per check", &l.translate_amortized_us),
        ("prototype on covered", &l.proto_us),
        ("oracle wall on covered", &l.oracle_us),
    ] {
        let _ = writeln!(s, "  {k:<36} {}", dist(d));
    }
    if !r.cycles.is_empty() {
        let _ = writeln!(
            s,
            "\n| sequence | cycle | checks | misses | misses unsat | misses not unsat | misses covered | miss solve ms | covered solve ms |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|");
        for c in &r.cycles {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                c.sequence,
                c.cycle,
                c.checks.checks,
                c.misses.checks,
                c.misses_unsat.checks,
                c.misses_not_unsat.checks,
                c.misses_covered.checks,
                ms(c.misses.solve_us),
                ms(c.misses_covered.solve_us)
            );
        }
    }
    let _ = writeln!(
        s,
        "\nper-obligation discharge by lia+uc+bounds (Z3-unsat checks):"
    );
    for (name, p) in &r.populations {
        let _ = writeln!(
            s,
            "  {name:<24} obligations {} discharged {} ({}); checks with all but one discharged {}",
            p.obligations,
            p.obligations_discharged,
            pct(p.obligations_discharged, p.obligations),
            p.all_but_one
        );
    }
    let c = &r.certificates;
    let _ = writeln!(s, "\ncertificates (covered checks):");
    let _ = writeln!(s, "  conjuncts {}", dist(&c.conjuncts));
    let _ = writeln!(s, "  unsat core {}", dist(&c.core));
    let _ = writeln!(s, "  prelude instances (all checks) {}", dist(&c.instances));
    let anomalies = r
        .examples
        .iter()
        .filter(|e| e.kind.starts_with("relaxation unsat"))
        .count();
    let _ = writeln!(s, "\nanomaly queries saved: {anomalies}");
    s
}
