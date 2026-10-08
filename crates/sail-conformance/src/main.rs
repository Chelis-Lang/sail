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

use sail_conformance::{Comparison, Verdict, compare};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{RunRecord, Solver, machine_info, run_unit};
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

const USAGE: &str = "usage:
  sail-conformance fetch [--tier fast|standard|deep] [--project NAME]...
  sail-conformance run   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                         [--jobs N] [--trace] [--out DIR] [--shim PATH]";

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
            "--out" => o.out = Some(value()?.into()),
            "--shim" => o.shim = Some(value()?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    o.jobs = o.jobs.max(1);
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
        let field = |k: &str| {
            r.verification_results()
                .and_then(|v| v.get(k))
                .and_then(serde_json::Value::as_u64)
        };
        Self {
            exit_code: r.exit_code,
            wall_ms: r.wall_ms,
            smt_ms: r.smt_ms(),
            max_rss_bytes: r.max_rss_bytes,
            verified: field("verified"),
            errors: field("errors"),
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
    infra_error: Option<String>,
    plain: Option<Side>,
    sail: Option<Side>,
}

#[derive(Serialize, Default)]
struct Summary {
    units: usize,
    same: usize,
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

fn load() -> Result<(Corpus, Installed, PathBuf), String> {
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    let corpus = Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
    corpus.check_pins(&pins).map_err(|e| e.to_string())?;
    let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    Ok((corpus, installed, cache))
}

fn units(o: &Options, corpus: &Corpus, cache: &Path) -> Result<Vec<Unit>, String> {
    let mut all = Vec::new();
    for project in corpus.projects_in(o.tier, &o.projects) {
        eprintln!("fetching {} @ {}", project.name, project.commit);
        let dir = project.fetch(cache).map_err(|e| e.to_string())?;
        all.extend(project.units(&dir).map_err(|e| e.to_string())?);
    }
    if let Some(f) = &o.filter {
        all.retain(|u| u.id.contains(f.as_str()));
    }
    Ok(all)
}

fn run_one(installed: &Installed, unit: &Unit, shim: &Path, dir: &Path, trace: bool) -> UnitReport {
    let base = dir.join("units").join(slug(&unit.id));
    let plain = run_unit(installed, unit, &Solver::Plain, &base.join("plain"));
    let solver = Solver::Shim {
        shim: shim.to_path_buf(),
        trace_dir: trace.then(|| dir.join("traces").join(slug(&unit.id))),
    };
    let sail = run_unit(installed, unit, &solver, &base.join("sail"));
    match (plain, sail) {
        (Ok(p), Ok(s)) => {
            let comparison = compare(&p, &s);
            UnitReport {
                id: unit.id.clone(),
                expected: unit.expected.clone(),
                verdict: comparison.verdict(),
                comparison,
                infra_error: None,
                plain: Some(Side::from(&p)),
                sail: Some(Side::from(&s)),
            }
        }
        (p, s) => UnitReport {
            id: unit.id.clone(),
            expected: unit.expected.clone(),
            verdict: Verdict::InfraError,
            comparison: Comparison::default(),
            infra_error: Some(
                [p.err(), s.err()]
                    .into_iter()
                    .flatten()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            plain: None,
            sail: None,
        },
    }
}

fn run_all(
    units: Vec<Unit>,
    jobs: usize,
    installed: &Installed,
    shim: &Path,
    dir: &Path,
    trace: bool,
) -> Vec<UnitReport> {
    let queue = Mutex::new(units.into_iter().collect::<VecDeque<_>>());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some(unit) = next else { break };
                    let report = run_one(installed, &unit, shim, dir, trace);
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
    let units = units(o, &corpus, &cache)?;
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

    let mut units = run_all(units, o.jobs, &installed, &shim, &dir, o.trace);
    units.sort_by(|a, b| a.id.cmp(&b.id));
    let mut summary = Summary {
        units: units.len(),
        ..Summary::default()
    };
    for u in &units {
        match u.verdict {
            Verdict::Same => summary.same += 1,
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
        units,
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("report.json");
    let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let s = &report.summary;
    println!(
        "conformance ({} tier): {} units, {} same, {} different, {} infra errors; {} functions compared; {} units with rlimit drift\nreport: {}",
        report.tier,
        s.units,
        s.same,
        s.different,
        s.infra_error,
        s.functions,
        s.units_with_rlimit_drift,
        path.display()
    );
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
            let (corpus, _, cache) = load()?;
            let n = units(&o, &corpus, &cache)?.len();
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
