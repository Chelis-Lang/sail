//! The `sail-conformance` binary: differential conformance on the corpus.
//!
//! ```text
//! sail-conformance fetch [--tier T] [--project P]...
//! sail-conformance run   [--tier T] [--project P]... [--filter S] [--jobs N]
//!                        [--trace] [--out DIR] [--shim PATH]
//! ```
//!
//! `run` exits 0 when every unit is the same under sail as under plain
//! Verus, 1 if any unit differs, and 2 on usage or infrastructure errors.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sail_conformance::{Comparison, Verdict, classify};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{RunRecord, Solver, machine_info, prepare_unit, run_unit};
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

const USAGE: &str = "usage:
  sail-conformance fetch [--tier fast|standard|deep] [--project NAME]...
  sail-conformance run   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--trace] [--out DIR] [--shim PATH]
                         [--plain-runs N] [--confirm-runs N]

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
    toolchain: &str,
) -> Result<Selection, String> {
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
        eprintln!("fetching verus @ {}", corpus.verus_commit);
        Some(
            corpus
                .fetch_verus_source(cache)
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
    trace: bool,
    plain_runs: usize,
    confirm_runs: usize,
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
    }
}

fn run_one(cx: &Context, unit: &Unit) -> UnitReport {
    let (installed, dir) = (cx.installed, cx.dir);
    let base = dir.join("units").join(slug(&unit.id));
    // Both sides run on the same prepared tree.
    let unit = match prepare_unit(installed, unit, cx.verus_source, &base) {
        Ok(u) => u,
        Err(e) => return infra_error(unit, e.to_string()),
    };
    let unit = &unit;
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
    format!(
        "conformance ({} tier): {} units, {} same, {} unstable, {} different, {} infra errors; {} functions compared; {} units with rlimit drift; {} projects skipped{skipped}",
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

fn run(o: &Options) -> Result<ExitCode, String> {
    let (corpus, installed, cache) = load()?;
    let shim = match &o.shim {
        Some(p) => p.clone(),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("sail-z3"),
    };
    if !shim.is_file() {
        return Err(format!(
            "error[SAIL3030]: shim not found at {} (build it with `cargo build --release -p sail-shim`, or pass --shim)",
            shim.display()
        ));
    }
    let Selection {
        units,
        skipped,
        verus_source,
    } = select(o, &corpus, &cache, &installed.pins.verus.rust_toolchain)?;
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
        trace: o.trace,
        plain_runs: o.plain_runs,
        confirm_runs: o.confirm_runs,
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
    let result = parse(&args).and_then(|o| match o.command.as_str() {
        "run" => run(&o),
        "fetch" => {
            let (corpus, installed, cache) = load()?;
            let toolchain = &installed.pins.verus.rust_toolchain;
            let n = select(&o, &corpus, &cache, toolchain)?.units.len();
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
