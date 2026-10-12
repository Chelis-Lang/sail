//! The `sail-conformance` binary: differential conformance on the corpus.
//!
//! ```text
//! sail-conformance fetch [--tier T] [--project P]...
//! sail-conformance run   [--tier T] [--project P]... [--filter S] [--jobs N]
//!                        [--trace] [--out DIR] [--shim PATH]
//! sail-conformance engine [--tier T] [--project P]... [--filter S] [--jobs N]
//!                        [--out DIR] [--engine-build PATH] [--no-focus]
//!                        [--no-oracle] [--oracle-fault KIND]
//!                        [--oracle-fresh-arms N]
//! sail-conformance rust-verify-test [--filter S]... [--every N] [--jobs N] ...
//! ```
//!
//! `run` exits 0 when every unit is the same under sail as under plain
//! Verus, 1 if any unit differs, and 2 on usage or infrastructure errors.
//! `engine` is the source-engine arm (ADR 0027): the optional interactive
//! engine's full phase against plain Verus, with the same comparison and
//! instability rules, plus completion-seal and same-input byte-oracle checks.
//! With `--oracle-fault`, it is a negative control instead: it exits 0 only
//! if every unit with a restored preparation hit reports the injected
//! mismatch, at least one such unit exists, and at least one injected
//! mismatch is confirmed hard (a second fresh lowering reproduces the first).
//!
//! A failed oracle check is confirmed by the oracle's second, independent
//! fresh lowering in the same callback (ADR 0038, amendment of 2026-10-12):
//! if the two fresh lowerings differ, the unit is `unstable` (VI-0015),
//! provided an engine run without the oracle matches plain Verus; if they
//! agree, the mismatch is a hard failure.
#![forbid(unsafe_code)]

mod rust_verify_test;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sail_conformance::engine::{
    Confirmation, EngineStats, ORACLE_MISMATCH, OracleRecord, confirm, oracle_records,
};
use sail_conformance::{Comparison, Verdict, classify};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{EngineRun, RunRecord, Solver, machine_info, prepare_unit, run_unit};
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

const USAGE: &str = "usage:
  sail-conformance fetch [--tier fast|standard|deep] [--project NAME]...
  sail-conformance run   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--trace] [--out DIR] [--shim PATH]
                         [--plain-runs N] [--confirm-runs N]

  sail-conformance engine [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--out DIR] [--plain-runs N] [--confirm-runs N]
                         [--engine-build PATH] [--no-focus] [--no-oracle]
                         [--oracle-fault order|effects|body] [--oracle-fresh-arms N]

  sail-conformance rust-verify-test ...   (see `rust-verify-test --help`)

  --engine-build PATH  the validated interactive engine build (default
                    $SAIL_ENGINE_BUILD)
  --no-focus        run only the engine's full phase (no focused selector)
  --no-oracle       skip the same-input cached-versus-fresh byte oracle
  --oracle-fault K  negative control: inject oracle fault K and require detection
  --oracle-fresh-arms N  confirming fresh lowerings after an oracle mismatch
                    (default 1, at most 8; ADR 0038)

  --plain-runs N    plain Verus runs per unit up front (default 1)
  --confirm-runs N  extra plain runs when sail differs, to tell plain-Verus
                    instability from sail differences (default 2; ADR 0009)";

#[derive(Debug)]
struct Options {
    command: String,
    tier: Tier,
    projects: Vec<String>,
    filter: Option<String>,
    jobs: usize,
    trace: bool,
    out: Option<PathBuf>,
    shim: Option<PathBuf>,
    plain_runs: usize,
    confirm_runs: usize,
    engine_build: Option<PathBuf>,
    focus: bool,
    oracle: bool,
    oracle_fault: Option<String>,
    oracle_fresh_arms: usize,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut it = args.iter();
    let command = it.next().cloned().ok_or(USAGE)?;
    let mut o = Options {
        command,
        tier: Tier::Fast,
        projects: Vec::new(),
        filter: None,
        jobs: 1,
        trace: false,
        out: None,
        shim: None,
        plain_runs: 1,
        confirm_runs: 2,
        engine_build: std::env::var_os("SAIL_ENGINE_BUILD")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        focus: true,
        oracle: true,
        oracle_fault: None,
        oracle_fresh_arms: 1,
    };
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--tier" => {
                o.tier = value()?
                    .parse()
                    .map_err(|e: sail_verus::Error| e.to_string())?;
            }
            "--project" => o.projects.push(value()?),
            "--filter" => o.filter = Some(value()?),
            "--jobs" => {
                o.jobs = value()?
                    .parse()
                    .map_err(|_| "--jobs needs a number".to_owned())?;
            }
            "--trace" => o.trace = true,
            "--plain-runs" => {
                o.plain_runs = value()?
                    .parse()
                    .map_err(|_| "--plain-runs needs a number".to_owned())?;
            }
            "--confirm-runs" => {
                o.confirm_runs = value()?
                    .parse()
                    .map_err(|_| "--confirm-runs needs a number".to_owned())?;
            }
            "--out" => o.out = Some(value()?.into()),
            "--engine-build" => o.engine_build = Some(value()?.into()),
            "--no-focus" => o.focus = false,
            "--no-oracle" => o.oracle = false,
            "--oracle-fault" => {
                let fault = value()?;
                if !["order", "effects", "body"].contains(&fault.as_str()) {
                    return Err(format!(
                        "--oracle-fault must be order, effects or body\n{USAGE}"
                    ));
                }
                o.oracle_fault = Some(fault);
            }
            "--oracle-fresh-arms" => {
                o.oracle_fresh_arms = value()?
                    .parse()
                    .ok()
                    .filter(|n| (1..=8).contains(n))
                    .ok_or_else(|| "--oracle-fresh-arms needs a number from 1 to 8".to_owned())?;
            }
            "--shim" => o.shim = Some(value()?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    o.jobs = o.jobs.max(1);
    o.plain_runs = o.plain_runs.max(1);
    Ok(o)
}

#[derive(Serialize)]
struct Side {
    exit_code: Option<i32>,
    wall_ms: f64,
    smt_ms: Option<u64>,
    max_rss_bytes: Option<u64>,
    verified: Option<u64>,
    errors: Option<u64>,
}

impl From<&RunRecord> for Side {
    fn from(r: &RunRecord) -> Self {
        Self {
            exit_code: r.exit_code,
            wall_ms: r.wall_ms,
            smt_ms: r.smt_ms(),
            max_rss_bytes: r.max_rss_bytes,
            verified: r.total("verified"),
            errors: r.total("errors"),
        }
    }
}

#[derive(Serialize)]
struct UnitReport {
    id: String,
    expected: Option<String>,
    verdict: Verdict,
    #[serde(flatten)]
    comparison: Comparison,
    plain_runs: usize,
    unstable: Vec<String>,
    infra_error: Option<String>,
    plain: Option<Side>,
    sail: Option<Side>,
    /// Engine arm only: what the engine reported beyond Verus's output.
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<EngineStats>,
}

#[derive(Serialize)]
struct Skipped {
    project: String,
    missing: Vec<String>,
}

#[derive(Serialize, Default)]
struct Summary {
    projects_skipped: usize,
    units: usize,
    same: usize,
    unstable: usize,
    different: usize,
    infra_error: usize,
    functions: usize,
    units_with_rlimit_drift: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<EngineSummary>,
}

/// Engine-arm totals.
#[derive(Serialize, Default)]
struct EngineSummary {
    manifest_sha256: String,
    oracle: bool,
    oracle_fault: Option<String>,
    focused_units: usize,
    units_with_hits: usize,
    hits: usize,
    oracle_checks: usize,
    oracle_failures: usize,
    oracle_hard_failures: usize,
    oracle_unstable: usize,
    oracle_unstable_units: usize,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    tier: String,
    verus_release: String,
    z3_version: String,
    machine: std::collections::BTreeMap<String, String>,
    started_unix: u64,
    flags: Vec<String>,
    summary: Summary,
    skipped: Vec<Skipped>,
    units: Vec<UnitReport>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn slug(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The units to run, the projects skipped for missing requirements, and the
/// pinned Verus source if any unit patches its Verus dependencies.
struct Selection {
    units: Vec<Unit>,
    skipped: Vec<Skipped>,
    verus_source: Option<PathBuf>,
}

fn load() -> Result<(Corpus, Installed, PathBuf), String> {
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    let corpus = Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
    corpus.check_pins(&pins).map_err(|e| e.to_string())?;
    let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    Ok((corpus, installed, cache))
}

fn select(
    o: &Options,
    corpus: &Corpus,
    cache: &Path,
    verus: &sail_verus::toolchain::VerusPin,
) -> Result<Selection, String> {
    let toolchain = verus.rust_toolchain.as_str();
    let mut all = Vec::new();
    let mut skipped = Vec::new();
    for project in corpus.projects_in(o.tier, &o.projects) {
        let missing = project.missing_requirements(toolchain);
        if !missing.is_empty() {
            eprintln!(
                "skipping {}: requires {} (not available)",
                project.name,
                missing.join(", ")
            );
            skipped.push(Skipped {
                project: project.name.clone(),
                missing,
            });
            continue;
        }
        eprintln!("fetching {} @ {}", project.name, project.commit);
        let dir = project.fetch(cache).map_err(|e| e.to_string())?;
        all.extend(project.units(&dir).map_err(|e| e.to_string())?);
    }
    if let Some(f) = &o.filter {
        all.retain(|u| u.id.contains(f.as_str()));
    }
    let verus_source = if all.iter().any(|u| u.patch_verus_deps) {
        eprintln!("fetching verus @ {}", verus.commit);
        Some(
            corpus
                .fetch_verus_source(cache, &verus.commit)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    Ok(Selection {
        units: all,
        skipped,
        verus_source,
    })
}

struct Context<'a> {
    installed: &'a Installed,
    verus_source: Option<&'a Path>,
    shim: &'a Path,
    dir: &'a Path,
    /// Where `cargo-verus` units get their scratch copies: under the cache,
    /// because `--out` may be inside a Cargo workspace (such as this one).
    work: &'a Path,
    trace: bool,
    plain_runs: usize,
    confirm_runs: usize,
    engine: Option<EngineArm>,
}

/// The source-engine arm's configuration.
struct EngineArm {
    verus: PathBuf,
    cargo_verus: PathBuf,
    focus: bool,
    oracle: bool,
    oracle_fault: Option<String>,
    oracle_fresh_arms: usize,
}

impl EngineArm {
    fn run(&self, function: Option<&String>, oracle: bool) -> Solver {
        Solver::Engine(EngineRun {
            verus: self.verus.clone(),
            cargo_verus: self.cargo_verus.clone(),
            function: function.cloned(),
            oracle,
            oracle_fault: self.oracle_fault.clone().filter(|_| oracle),
            oracle_fresh_arms: Some(self.oracle_fresh_arms),
        })
    }
}

fn infra_error(unit: &Unit, message: String) -> UnitReport {
    UnitReport {
        id: unit.id.clone(),
        expected: unit.expected.clone(),
        verdict: Verdict::InfraError,
        comparison: Comparison::default(),
        plain_runs: 0,
        unstable: Vec::new(),
        infra_error: Some(message),
        plain: None,
        sail: None,
        engine: None,
    }
}

fn run_one(cx: &Context, unit: &Unit) -> UnitReport {
    let (installed, dir) = (cx.installed, cx.dir);
    let base = dir.join("units").join(slug(&unit.id));
    let scratch = cx.work.join(slug(&unit.id));
    // Both sides run on the same prepared tree.
    let prepared = prepare_unit(installed, unit, cx.verus_source, &scratch);
    let report = match prepared {
        Ok(u) => compare_sides(cx, &u, &base),
        Err(e) => infra_error(unit, e.to_string()),
    };
    let _ = std::fs::remove_dir_all(&scratch);
    report
}

fn compare_sides(cx: &Context, unit: &Unit, base: &Path) -> UnitReport {
    let (installed, dir) = (cx.installed, cx.dir);
    let run_plain = |i: usize| {
        let dir = if i == 0 {
            base.join("plain")
        } else {
            base.join(format!("plain-{}", i + 1))
        };
        run_unit(installed, unit, &Solver::Plain, &dir)
    };
    let mut plains = Vec::new();
    for i in 0..cx.plain_runs {
        match run_plain(i) {
            Ok(p) => plains.push(p),
            Err(e) => return infra_error(unit, e.to_string()),
        }
    }
    if let Some(engine) = &cx.engine {
        return compare_engine(cx, engine, unit, base, plains);
    }
    let solver = Solver::Shim {
        shim: cx.shim.to_path_buf(),
        trace_dir: cx.trace.then(|| dir.join("traces").join(slug(&unit.id))),
        // Conformance with the cache on (ADR 0011): the sail side uses the
        // daemon named in the harness's own environment, if any.
        daemon_socket: std::env::var_os("SAIL_DAEMON_SOCKET")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
    };
    let sail = match run_unit(installed, unit, &solver, &base.join("sail")) {
        Ok(s) => s,
        Err(e) => return infra_error(unit, e.to_string()),
    };
    let mut c = classify(&plains, &sail);
    // ADR 0009: before calling a difference a sail error, check whether
    // plain Verus agrees with itself.
    for _ in 0..cx.confirm_runs {
        if c.verdict != Verdict::Different {
            break;
        }
        match run_plain(plains.len()) {
            Ok(p) => plains.push(p),
            Err(e) => return infra_error(unit, e.to_string()),
        }
        c = classify(&plains, &sail);
    }
    UnitReport {
        id: unit.id.clone(),
        expected: unit.expected.clone(),
        verdict: c.verdict,
        comparison: c.comparison,
        plain_runs: c.plain_runs,
        unstable: c.unstable,
        infra_error: None,
        plain: plains.first().map(Side::from),
        sail: Some(Side::from(&sail)),
        engine: None,
    }
}

/// The engine with the same focus and no oracle, and its seal issues.
fn engine_without_oracle(
    cx: &Context,
    engine: &EngineArm,
    unit: &Unit,
    base: &Path,
    function: Option<&String>,
    single_crate: bool,
) -> Result<(RunRecord, Vec<String>), String> {
    let dir = base.join("engine-no-oracle");
    let run = run_unit(cx.installed, unit, &engine.run(function, false), &dir)
        .map_err(|e| e.to_string())?;
    let events = std::fs::read_to_string(dir.join(sail_verus::run::engine_files::EVENTS))
        .unwrap_or_default();
    let (issues, _) = sail_conformance::engine::check(&run, &events, None, single_crate);
    Ok((
        run,
        issues
            .into_iter()
            .map(|i| format!("without oracle: {i}"))
            .collect(),
    ))
}

/// The engine arm for one unit: the engine's full phase against plain Verus
/// with ADR 0009 confirmation, plus the engine-only seal and oracle checks.
fn compare_engine(
    cx: &Context,
    engine: &EngineArm,
    unit: &Unit,
    base: &Path,
    mut plains: Vec<RunRecord>,
) -> UnitReport {
    let function = if engine.focus {
        plains
            .first()
            .and_then(sail_conformance::engine::focus_function)
    } else {
        None
    };
    let single_crate = unit.build == sail_verus::corpus::Build::Verus;
    let work = base.join("engine");
    let run = match run_unit(
        cx.installed,
        unit,
        &engine.run(function.as_ref(), engine.oracle),
        &work,
    ) {
        Ok(run) => run,
        Err(e) => return infra_error(unit, e.to_string()),
    };
    let events = std::fs::read_to_string(work.join(sail_verus::run::engine_files::EVENTS))
        .unwrap_or_default();
    let records = engine
        .oracle
        .then(|| oracle_records(&work.join(sail_verus::run::engine_files::ORACLE)));
    let summaries: Option<Vec<serde_json::Value>> = records
        .as_ref()
        .map(|r| r.iter().map(|x| x.summary.clone()).collect());
    let (mut issues, mut stats) =
        sail_conformance::engine::check(&run, &events, summaries.as_deref(), single_crate);
    stats.focus_function.clone_from(&function);
    // ADR 0038 amendment: a mismatch whose two independent fresh lowerings
    // differ is stock nondeterminism (VI-0015), not a failure.
    let failed: Vec<&OracleRecord> = records.iter().flatten().filter(|r| r.failed()).collect();
    let mut verdict_run = None;
    for c in failed.iter().map(|f| confirm(f)) {
        match c {
            Confirmation::Unstable(why) => {
                stats.oracle_unstable += 1;
                stats.oracle_confirmations.push(format!("unstable: {why}"));
            }
            Confirmation::Hard(why) => {
                stats.oracle_hard_failures += 1;
                stats.oracle_confirmations.push(format!("hard: {why}"));
                issues.push(format!("{ORACLE_MISMATCH}hard: {why}"));
            }
        }
    }
    if stats.oracle_unstable > 0 && stats.oracle_hard_failures == 0 {
        // The oracle stops the run at its first mismatch, so verdicts and
        // diagnostics come from an engine run without the oracle, with the
        // same focus; it must still match plain Verus.
        issues.retain(|i| !i.starts_with(ORACLE_MISMATCH));
        match engine_without_oracle(cx, engine, unit, base, function.as_ref(), single_crate) {
            Ok((no_oracle, more)) => {
                issues.extend(more);
                verdict_run = Some(no_oracle);
            }
            Err(e) => return infra_error(unit, e),
        }
    }
    let compared = verdict_run.as_ref().unwrap_or(&run);
    let mut c = classify(&plains, compared);
    for _ in 0..cx.confirm_runs {
        if c.verdict != Verdict::Different {
            break;
        }
        match run_unit(
            cx.installed,
            unit,
            &Solver::Plain,
            &base.join(format!("plain-{}", plains.len() + 1)),
        ) {
            Ok(p) => plains.push(p),
            Err(e) => return infra_error(unit, e.to_string()),
        }
        c = classify(&plains, compared);
    }
    if !issues.is_empty() {
        // Engine completion and hard oracle failures are never instability.
        c.verdict = Verdict::Different;
        c.comparison.differences.extend(issues);
    } else if stats.oracle_unstable > 0 && c.verdict == Verdict::Same {
        c.verdict = Verdict::Unstable;
    }
    if stats.oracle_unstable > 0 {
        c.unstable.push(format!(
            "same-input oracle: {} restored hit(s) whose two independent fresh lowerings differ (VI-0015)",
            stats.oracle_unstable
        ));
    }
    UnitReport {
        id: unit.id.clone(),
        expected: unit.expected.clone(),
        verdict: c.verdict,
        comparison: c.comparison,
        plain_runs: c.plain_runs,
        unstable: c.unstable,
        infra_error: None,
        plain: plains.first().map(Side::from),
        sail: Some(Side::from(compared)),
        engine: Some(stats),
    }
}

fn run_all(units: Vec<Unit>, jobs: usize, cx: &Context) -> Vec<UnitReport> {
    let queue = Mutex::new(units.into_iter().collect::<VecDeque<_>>());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some(unit) = next else { break };
                    let report = run_one(cx, &unit);
                    let (p, s) = (
                        report.plain.as_ref().map_or(0.0, |x| x.wall_ms),
                        report.sail.as_ref().map_or(0.0, |x| x.wall_ms),
                    );
                    eprintln!(
                        "{:<11} {}  (plain {p:.0} ms, sail {s:.0} ms){}",
                        format!("{:?}", report.verdict).to_lowercase(),
                        report.id,
                        report
                            .comparison
                            .differences
                            .first()
                            .or(report.infra_error.as_ref())
                            .map(|d| format!("\n            {d}"))
                            .unwrap_or_default()
                    );
                    if let Ok(mut r) = results.lock() {
                        r.push(report);
                    }
                }
            });
        }
    });
    results.into_inner().unwrap_or_default()
}

fn summary_line(report: &Report) -> String {
    let s = &report.summary;
    let skipped = if report.skipped.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = report
            .skipped
            .iter()
            .map(|k| format!("{}: needs {}", k.project, k.missing.join(", ")))
            .collect();
        format!(" ({})", list.join("; "))
    };
    let engine = s.engine.as_ref().map_or_else(String::new, |e| {
        format!(
            "; engine {}: {} focused units, {} units with {} restored preparation hits, {} oracle checks, {} oracle failures ({} hard, {} unstable in {} units){}",
            e.manifest_sha256.get(..12).unwrap_or_default(),
            e.focused_units,
            e.units_with_hits,
            e.hits,
            e.oracle_checks,
            e.oracle_failures,
            e.oracle_hard_failures,
            e.oracle_unstable,
            e.oracle_unstable_units,
            if e.oracle { "" } else { " (oracle off)" }
        )
    });
    format!(
        "{}conformance ({} tier): {} units, {} same, {} unstable, {} different, {} infra errors; {} functions compared; {} units with rlimit drift; {} projects skipped{skipped}{engine}",
        if s.engine.is_some() { "engine " } else { "" },
        report.tier,
        s.units,
        s.same,
        s.unstable,
        s.different,
        s.infra_error,
        s.functions,
        s.units_with_rlimit_drift,
        s.projects_skipped,
    )
}

/// Locates and validates the engine build for the `engine` command.
fn engine_arm(o: &Options, installed: &Installed) -> Result<Option<(EngineArm, String)>, String> {
    if o.command != "engine" {
        return Ok(None);
    }
    let build = o.engine_build.as_ref().ok_or_else(|| {
        format!("error[SAIL3120]: engine needs --engine-build or SAIL_ENGINE_BUILD\n{USAGE}")
    })?;
    let engine = sail_verus::engine::Installed::at(build, &installed.pins)
        .map_err(|e| format!("error[{}]: {e}", e.code))?;
    Ok(Some((
        EngineArm {
            verus: engine.verus,
            cargo_verus: engine.cargo_verus,
            focus: o.focus,
            oracle: o.oracle || o.oracle_fault.is_some(),
            oracle_fault: o.oracle_fault.clone(),
            oracle_fresh_arms: o.oracle_fresh_arms,
        },
        engine.manifest_sha256,
    )))
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear run: select, execute, summarize and report"
)]
fn run(o: &Options) -> Result<ExitCode, String> {
    let (corpus, installed, cache) = load()?;
    let engine = engine_arm(o, &installed)?;
    let shim = match &o.shim {
        Some(p) => p.clone(),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("sail-z3"),
    };
    if engine.is_none() && !shim.is_file() {
        return Err(format!(
            "error[SAIL3030]: shim not found at {} (build it with `cargo build --release -p sail-shim`, or pass --shim)",
            shim.display()
        ));
    }
    let Selection {
        units,
        skipped,
        verus_source,
    } = select(o, &corpus, &cache, &installed.pins.verus)?;
    let started = now_unix();
    let dir = o
        .out
        .clone()
        .unwrap_or_else(|| cache.join("conformance").join(started.to_string()));
    eprintln!(
        "running {} units with {} jobs; output in {}",
        units.len(),
        o.jobs,
        dir.display()
    );

    let cx = Context {
        installed: &installed,
        verus_source: verus_source.as_deref(),
        shim: &shim,
        dir: &dir,
        work: &cache.join("work").join(started.to_string()),
        trace: o.trace,
        plain_runs: o.plain_runs,
        confirm_runs: o.confirm_runs,
        engine: engine.as_ref().map(|(arm, _)| EngineArm {
            verus: arm.verus.clone(),
            cargo_verus: arm.cargo_verus.clone(),
            focus: arm.focus,
            oracle: arm.oracle,
            oracle_fault: arm.oracle_fault.clone(),
            oracle_fresh_arms: arm.oracle_fresh_arms,
        }),
    };
    let mut units = run_all(units, o.jobs, &cx);
    units.sort_by(|a, b| a.id.cmp(&b.id));
    let mut summary = Summary {
        projects_skipped: skipped.len(),
        units: units.len(),
        ..Summary::default()
    };
    for u in &units {
        match u.verdict {
            Verdict::Same => summary.same += 1,
            Verdict::Unstable => summary.unstable += 1,
            Verdict::Different => summary.different += 1,
            Verdict::InfraError => summary.infra_error += 1,
        }
        summary.functions += u.comparison.functions;
        summary.units_with_rlimit_drift += usize::from(!u.comparison.rlimit_drift.is_empty());
    }
    if let Some((arm, manifest_sha256)) = &engine {
        let mut totals = EngineSummary {
            manifest_sha256: manifest_sha256.clone(),
            oracle: arm.oracle,
            oracle_fault: arm.oracle_fault.clone(),
            ..EngineSummary::default()
        };
        for stats in units.iter().filter_map(|u| u.engine.as_ref()) {
            totals.focused_units += usize::from(stats.focus_established);
            totals.units_with_hits += usize::from(stats.hits() > 0);
            totals.hits += stats.hits();
            totals.oracle_checks += stats.oracle_checks;
            totals.oracle_failures += stats.oracle_failures;
            totals.oracle_hard_failures += stats.oracle_hard_failures;
            totals.oracle_unstable += stats.oracle_unstable;
            totals.oracle_unstable_units += usize::from(stats.oracle_unstable > 0);
        }
        summary.engine = Some(totals);
    }
    let report = Report {
        schema_version: 1,
        tier: format!("{:?}", o.tier).to_lowercase(),
        verus_release: installed.pins.verus.release.clone(),
        z3_version: installed.pins.z3.version.clone(),
        machine: machine_info(),
        started_unix: started,
        flags: sail_verus::run::COMMON_FLAGS
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        summary,
        skipped,
        units,
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("report.json");
    let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    println!("{}\nreport: {}", summary_line(&report), path.display());
    let s = &report.summary;
    if let Some(engine) = s.engine.as_ref().filter(|e| e.oracle_fault.is_some()) {
        // Negative control: every unit with a restored hit must report the
        // injected oracle mismatch, and there must be at least one such unit.
        let undetected = report
            .units
            .iter()
            .filter(|u| {
                u.engine
                    .as_ref()
                    .is_some_and(|e| e.hits() > 0 && e.oracle_failures == 0)
            })
            .count();
        // At least one injected mismatch must survive confirmation as a
        // hard failure: stable fresh lowering never excuses a real mismatch.
        let detected =
            engine.units_with_hits > 0 && undetected == 0 && engine.oracle_hard_failures > 0;
        println!(
            "oracle negative control ({}): {} units with restored hits, {undetected} without a detected mismatch, {} confirmed hard, {} unstable: {}",
            engine.oracle_fault.as_deref().unwrap_or_default(),
            engine.units_with_hits,
            engine.oracle_hard_failures,
            engine.oracle_unstable,
            if detected { "detected" } else { "NOT DETECTED" }
        );
        return Ok(if detected {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        });
    }
    Ok(if s.different > 0 {
        ExitCode::from(1)
    } else if s.infra_error > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("rust-verify-test") {
        let rest = args.get(1..).unwrap_or_default();
        let result = if rest.iter().any(|a| a == "--help" || a == "-h") {
            println!("{}", rust_verify_test::USAGE);
            Ok(ExitCode::SUCCESS)
        } else {
            rust_verify_test::main(rest)
        };
        return result.unwrap_or_else(|message| {
            eprintln!("{message}");
            ExitCode::from(2)
        });
    }
    let result = parse(&args).and_then(|o| match o.command.as_str() {
        "run" | "engine" => run(&o),
        "fetch" => {
            let (corpus, installed, cache) = load()?;
            let n = select(&o, &corpus, &cache, &installed.pins.verus)?
                .units
                .len();
            println!("{n} units ready");
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(USAGE.to_owned()),
    });
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
