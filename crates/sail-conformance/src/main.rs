//! The `sail-conformance` binary: differential conformance on the corpus.
//!
//! ```text
//! sail-conformance fetch [--tier T] [--project P]...
//! sail-conformance run   [--tier T] [--project P]... [--filter S] [--jobs N]
//!                        [--trace] [--out DIR] [--shim PATH] [--sail-verus fork|stock]
//!                        [--sail-arm managed|shim]
//! sail-conformance engine [--tier T] [--project P]... [--filter S] [--jobs N]
//!                        [--out DIR] [--engine-build PATH] [--no-focus]
//! sail-conformance rust-verify-test [--filter S]... [--every N] [--jobs N] ...
//! ```
//!
//! `run` compares Sail as users run it against plain stock Verus with the
//! same Z3 (charter rule 1). The Sail side is what `sail verify` runs for
//! each verified crate (ADR 0046): Sail's Verus (built from the fork,
//! ADR 0043) with `sail-z3` in front of the pinned Z3 and the logging flags
//! the managed driver adds. Two diagnostic switches isolate a difference:
//! `--sail-verus stock` runs the Sail side on stock Verus (a fork
//! difference), and `--sail-arm shim` drops the managed driver's flags,
//! leaving the bare shim (a difference those flags cause). It exits 0 when every unit is the same under sail as under
//! plain Verus, 1 if any unit differs, and 2 on usage or infrastructure
//! errors.
//! `engine` is the source-engine arm (ADR 0027): the optional interactive
//! engine's full phase against plain Verus, with the same comparison and
//! instability rules on verdicts and diagnostics, plus a completion-seal check.
#![forbid(unsafe_code)]

mod rust_verify_test;

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sail_conformance::engine::EngineStats;
use sail_conformance::{Comparison, Verdict, classify};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{EngineRun, RunRecord, Solver, machine_info, prepare_unit, run_unit};
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

const USAGE: &str = "usage:
  sail-conformance fetch [--tier fast|standard|deep] [--project NAME]...
  sail-conformance run   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--trace] [--out DIR] [--shim PATH]
                         [--plain-runs N] [--confirm-runs N] [--sail-verus fork|stock]
                         [--sail-arm managed|shim]

  sail-conformance engine [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--out DIR] [--plain-runs N] [--confirm-runs N]
                         [--engine-build PATH] [--no-focus]

  sail-conformance rust-verify-test ...   (see `rust-verify-test --help`)

  --engine-build PATH  the validated build of Sail's Verus: the fork runtime,
                    which contains the interactive engine (default
                    $SAIL_ENGINE_BUILD, else the one `sail` would use)
  --sail-verus V    the Verus on the Sail side of `run`: `fork` (default;
                    Sail's Verus, as users run it) or `stock` (the pinned
                    release, to isolate the shim)
  --sail-arm A      what the Sail side of `run` adds: `managed` (default;
                    the flags `sail verify`'s managed driver adds to each
                    verified crate) or `shim` (`sail-z3` alone, to isolate
                    a difference those flags cause)
  --no-focus        run only the engine's full phase (no focused selector)

  --plain-runs N    plain Verus runs per unit up front (default 1)
  --confirm-runs N  extra plain runs when sail differs, to tell plain-Verus
                    instability from sail differences (default 2; ADR 0009)";

#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one bool per independent command-line switch"
)]
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
    /// `run`: whether the Sail side runs Sail's Verus (the fork) rather
    /// than stock.
    sail_on_fork: bool,
    /// `run`: whether the Sail side adds the managed driver's logging flags
    /// (`--sail-arm managed`, the default) or runs the bare shim.
    managed_arm: bool,
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
        sail_on_fork: true,
        managed_arm: true,
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
            "--shim" => o.shim = Some(value()?.into()),
            "--sail-verus" => {
                o.sail_on_fork = match value()?.as_str() {
                    "fork" => true,
                    "stock" => false,
                    other => {
                        return Err(format!("--sail-verus needs fork or stock, not `{other}`"));
                    }
                };
            }
            "--sail-arm" => {
                o.managed_arm = match value()?.as_str() {
                    "managed" => true,
                    "shim" => false,
                    other => {
                        return Err(format!("--sail-arm needs managed or shim, not `{other}`"));
                    }
                };
            }
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
    #[serde(skip_serializing_if = "is_zero")]
    engine_lacks_feature: usize,
    functions: usize,
    units_with_rlimit_drift: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<EngineSummary>,
}

/// Engine-arm totals.
#[derive(Serialize, Default)]
struct EngineSummary {
    manifest_sha256: String,
    /// The Verus Cargo features the engine manifest records.
    verus_features: Vec<String>,
    focused_units: usize,
    /// Units whose project needs a Verus feature the engine lacks, by
    /// feature (verdict `engine-lacks-feature`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    lacks_feature: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    tier: String,
    verus_release: String,
    /// `run`: the Verus on the Sail side: `fork:<manifest sha256>` (Sail's
    /// Verus) or `stock`. The plain side is always stock `verus_release`.
    #[serde(skip_serializing_if = "Option::is_none")]
    sail_verus: Option<String>,
    /// `run`: `managed` (the managed driver's flags, as `sail verify` runs
    /// Verus) or `shim` (the bare shim).
    #[serde(skip_serializing_if = "Option::is_none")]
    sail_arm: Option<String>,
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
    /// Engine arm: projects with the Verus features they need and the engine
    /// lacks.
    engine_lacks: BTreeMap<String, Vec<String>>,
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn load() -> Result<(Corpus, Installed, PathBuf), String> {
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    let corpus = Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
    corpus.check_pins(&pins).map_err(|e| e.to_string())?;
    let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    Ok((corpus, installed, cache))
}

/// `engine_features` is the engine arm's recorded Verus Cargo features, and
/// `None` for the other arms.
fn select(
    o: &Options,
    corpus: &Corpus,
    cache: &Path,
    verus: &sail_verus::toolchain::VerusPin,
    engine_features: Option<&[String]>,
) -> Result<Selection, String> {
    let toolchain = verus.rust_toolchain.as_str();
    let mut all = Vec::new();
    let mut skipped = Vec::new();
    let mut engine_lacks = BTreeMap::new();
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
        if let Some(available) = engine_features {
            let lacks = sail_verus::engine::missing_features(&project.requires, available);
            if !lacks.is_empty() {
                eprintln!(
                    "{}: the engine lacks Verus feature {}; its units are counted, not run on the engine",
                    project.name,
                    lacks.join(", ")
                );
                engine_lacks.insert(project.name.clone(), lacks);
            }
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
        engine_lacks,
    })
}

struct Context<'a> {
    installed: &'a Installed,
    /// The Verus the Sail side of `run` uses: Sail's Verus (the fork) by
    /// default, or `installed` with `--sail-verus stock`.
    sail_verus: &'a Installed,
    verus_source: Option<&'a Path>,
    shim: &'a Path,
    dir: &'a Path,
    /// Where `cargo-verus` units get their scratch copies: under the cache,
    /// because `--out` may be inside a Cargo workspace (such as this one).
    work: &'a Path,
    trace: bool,
    /// [`Options::managed_arm`].
    managed_arm: bool,
    plain_runs: usize,
    confirm_runs: usize,
    engine: Option<EngineArm>,
    /// [`Selection::engine_lacks`].
    engine_lacks: &'a BTreeMap<String, Vec<String>>,
}

/// The source-engine arm's configuration.
struct EngineArm {
    verus: PathBuf,
    cargo_verus: PathBuf,
    focus: bool,
}

impl EngineArm {
    fn run(&self, function: Option<&String>) -> Solver {
        Solver::Engine(EngineRun {
            verus: self.verus.clone(),
            cargo_verus: self.cargo_verus.clone(),
            function: function.cloned(),
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

/// A unit the engine arm does not run because its project needs a Verus
/// feature the engine lacks: the product falls back to stock for it
/// (SAIL3124), so there is no engine verdict to compare.
fn lacks_feature(unit: &Unit, features: &[String]) -> UnitReport {
    UnitReport {
        id: unit.id.clone(),
        expected: unit.expected.clone(),
        verdict: Verdict::EngineLacksFeature,
        comparison: Comparison::default(),
        plain_runs: 0,
        unstable: Vec::new(),
        infra_error: None,
        plain: None,
        sail: None,
        engine: Some(EngineStats {
            lacks_features: features.to_vec(),
            ..EngineStats::default()
        }),
    }
}

fn run_one(cx: &Context, unit: &Unit) -> UnitReport {
    if cx.engine.is_some()
        && let Some(features) = cx.engine_lacks.get(&unit.project)
    {
        return lacks_feature(unit, features);
    }
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
        observation_logs: cx.managed_arm,
    };
    let sail = match run_unit(cx.sail_verus, unit, &solver, &base.join("sail")) {
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

/// The engine arm for one unit: the engine's full phase against plain Verus
/// with ADR 0009 confirmation, plus the engine-only seal check.
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
    let run = match run_unit(cx.installed, unit, &engine.run(function.as_ref()), &work) {
        Ok(run) => run,
        Err(e) => return infra_error(unit, e.to_string()),
    };
    let events = std::fs::read_to_string(work.join(sail_verus::run::engine_files::EVENTS))
        .unwrap_or_default();
    let (issues, mut stats) = sail_conformance::engine::check(&run, &events, single_crate);
    stats.focus_function.clone_from(&function);
    let compared = &run;
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
        // Engine completion failures are never instability.
        c.verdict = Verdict::Different;
        c.comparison.differences.extend(issues);
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
                    let lacks = report
                        .engine
                        .as_ref()
                        .filter(|e| !e.lacks_features.is_empty())
                        .map(|e| {
                            format!("engine lacks Verus feature {}", e.lacks_features.join(", "))
                        });
                    eprintln!(
                        "{:<11} {}  (plain {p:.0} ms, sail {s:.0} ms){}",
                        format!("{:?}", report.verdict).to_lowercase(),
                        report.id,
                        report
                            .comparison
                            .differences
                            .first()
                            .or(report.infra_error.as_ref())
                            .or(lacks.as_ref())
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
        let lacks = e
            .lacks_feature
            .iter()
            .fold(String::new(), |mut text, (feature, units)| {
                let _ = write!(text, ", {units} units engine lacks feature {feature}");
                text
            });
        format!(
            "; engine {}: {} focused units{lacks}",
            e.manifest_sha256.get(..12).unwrap_or_default(),
            e.focused_units,
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

/// The engine arm, the engine manifest's digest and the Verus Cargo features
/// the manifest records.
type EngineBuild = (EngineArm, String, Vec<String>);

/// The Sail side's Verus for `run` (ADR 0043): Sail's Verus, the fork
/// runtime, validated like `sail` validates it, with the plain side's pinned
/// Z3 bytes (the runtime's own Z3 has the same pinned digest). `None` when
/// `--sail-verus stock` asks for the stock release.
fn sail_verus(o: &Options, installed: &Installed) -> Result<Option<(Installed, String)>, String> {
    if o.command != "run" || !o.sail_on_fork {
        return Ok(None);
    }
    if installed.pins.is_secondary {
        // Sail's Verus is built on the primary pin only; a secondary pin
        // checks the shim on that stock release (docs/compatibility.md).
        eprintln!(
            "conformance: secondary Verus {}: the Sail side runs that stock release (Sail's Verus is built on the primary pin)",
            installed.pins.verus.release
        );
        return Ok(None);
    }
    let fork = match &o.engine_build {
        Some(build) => sail_verus::engine::Installed::at(build, &installed.pins),
        None => sail_verus::cache_dir()
            .and_then(|cache| sail_verus::engine::Installed::locate(&cache, &installed.pins)),
    }
    .map_err(|e| {
        format!(
            "error[{}]: the Sail side runs Sail's Verus (ADR 0043): {e}\n\
             (pass --sail-verus stock to run the Sail side on stock Verus)",
            e.code
        )
    })?;
    Ok(Some((
        Installed {
            verus: fork.verus,
            cargo_verus: fork.cargo_verus,
            ..installed.clone()
        },
        fork.manifest_sha256,
    )))
}

/// Locates and validates the engine build for the `engine` command.
fn engine_arm(o: &Options, installed: &Installed) -> Result<Option<EngineBuild>, String> {
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
        },
        engine.manifest_sha256,
        engine.cargo_features,
    )))
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear run: select, execute, summarize and report"
)]
fn run(o: &Options) -> Result<ExitCode, String> {
    let (corpus, installed, cache) = load()?;
    let engine = engine_arm(o, &installed)?;
    let fork = sail_verus(o, &installed)?;
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
        engine_lacks,
    } = select(
        o,
        &corpus,
        &cache,
        &installed.pins.verus,
        engine.as_ref().map(|(_, _, features)| features.as_slice()),
    )?;
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
        sail_verus: fork.as_ref().map_or(&installed, |(fork, _)| fork),
        verus_source: verus_source.as_deref(),
        shim: &shim,
        dir: &dir,
        work: &cache.join("work").join(started.to_string()),
        trace: o.trace,
        managed_arm: o.managed_arm,
        plain_runs: o.plain_runs,
        confirm_runs: o.confirm_runs,
        engine: engine.as_ref().map(|(arm, _, _)| EngineArm {
            verus: arm.verus.clone(),
            cargo_verus: arm.cargo_verus.clone(),
            focus: arm.focus,
        }),
        engine_lacks: &engine_lacks,
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
            Verdict::EngineLacksFeature => summary.engine_lacks_feature += 1,
        }
        summary.functions += u.comparison.functions;
        summary.units_with_rlimit_drift += usize::from(!u.comparison.rlimit_drift.is_empty());
    }
    if let Some((_, manifest_sha256, features)) = &engine {
        let mut totals = EngineSummary {
            manifest_sha256: manifest_sha256.clone(),
            verus_features: features.clone(),
            ..EngineSummary::default()
        };
        for stats in units.iter().filter_map(|u| u.engine.as_ref()) {
            totals.focused_units += usize::from(stats.focus_established);
            for feature in &stats.lacks_features {
                *totals.lacks_feature.entry(feature.clone()).or_default() += 1;
            }
        }
        summary.engine = Some(totals);
    }
    let report = Report {
        schema_version: 1,
        tier: format!("{:?}", o.tier).to_lowercase(),
        verus_release: installed.pins.verus.release.clone(),
        sail_verus: (o.command == "run").then(|| {
            fork.as_ref()
                .map_or_else(|| "stock".to_owned(), |(_, sha)| format!("fork:{sha}"))
        }),
        sail_arm: (o.command == "run")
            .then(|| if o.managed_arm { "managed" } else { "shim" }.to_owned()),
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
            let n = select(&o, &corpus, &cache, &installed.pins.verus, None)?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_the_engine_cannot_run_are_a_visible_count_not_same_or_different() {
        let unit = Unit {
            id: "verus-integer-ring/integer_ring.rs".to_owned(),
            project: "verus-integer-ring".to_owned(),
            source_dir: PathBuf::new(),
            root: "integer_ring.rs".to_owned(),
            build: sail_verus::corpus::Build::Verus,
            args: Vec::new(),
            cargo_args: Vec::new(),
            patch_verus_deps: false,
            target_dir: None,
            env: BTreeMap::new(),
            expected: None,
        };
        let unit_report = lacks_feature(&unit, &["singular".to_owned()]);
        assert_eq!(unit_report.verdict, Verdict::EngineLacksFeature);
        let mut report = Report {
            schema_version: 1,
            tier: "deep".to_owned(),
            verus_release: String::new(),
            sail_verus: None,
            sail_arm: None,
            z3_version: String::new(),
            machine: BTreeMap::new(),
            started_unix: 0,
            flags: Vec::new(),
            summary: Summary {
                units: 1,
                engine_lacks_feature: 1,
                engine: Some(EngineSummary {
                    manifest_sha256: "0123456789abcdef".to_owned(),
                    lacks_feature: BTreeMap::from([("singular".to_owned(), 1)]),
                    ..EngineSummary::default()
                }),
                ..Summary::default()
            },
            skipped: Vec::new(),
            units: vec![unit_report],
        };
        let line = summary_line(&report);
        assert!(
            line.contains("0 same") && line.contains("0 different"),
            "{line}"
        );
        assert!(
            line.ends_with("1 units engine lacks feature singular"),
            "{line}"
        );
        let json = serde_json::to_value(&report).unwrap_or_default();
        assert_eq!(json["units"][0]["verdict"], "engine-lacks-feature");
        assert_eq!(json["units"][0]["engine"]["lacks_features"][0], "singular");
        assert_eq!(json["summary"]["engine_lacks_feature"], 1);
        report.summary.engine_lacks_feature = 0;
        let json = serde_json::to_value(&report).unwrap_or_default();
        assert!(json["summary"].get("engine_lacks_feature").is_none());
    }
}
