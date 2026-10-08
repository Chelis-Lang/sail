//! The `sail-bench` binary.
//!
//! ```text
//! sail-bench cold   [--tier T] [--project P]... [--filter S] [--rounds N] [--name NAME] [--shim PATH]
//! sail-bench warm   (cold's options) [--rounds WARM_ROUNDS] [--sail PATH]
//! sail-bench edit   [--edits corpus/edits.toml] [--filter S] [--rounds N] [--name NAME] [--shim PATH] [--sail PATH]
//! sail-bench report [--out docs/benchmarks.md] FILE...
//! sail-bench profile <trace-dir>... [--json out.json]
//! sail-bench seeds   <trace-dir>... --z3 <path> [--seeds N] [--jobs J] [--json out.json]
//! sail-bench history <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
//! ```
//!
//! `warm` and `edit` start `sail serve` (found next to this binary, like
//! `sail-z3`) with a fresh cache per unit or edit round; see ADR 0012.
//! `profile`, `seeds` and `history` analyse shim traces; see
//! [`sail_bench::cli`].
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use sail_bench::cold::{self, BenchFile, UnitResult};
use sail_bench::edit::{self, EditResult};
use sail_bench::{daemon, warm};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{machine_info, prepare_unit};
use sail_verus::toolchain::{Installed, Pins};

const USAGE: &str = "usage:
  sail-bench cold   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                    [--rounds N] [--name NAME] [--shim PATH]
  sail-bench warm   (cold's options) [--rounds WARM_ROUNDS] [--sail PATH]
  sail-bench edit   [--edits corpus/edits.toml] [--filter SUBSTR] [--rounds N]
                    [--name NAME] [--shim PATH] [--sail PATH]
  sail-bench agent-loop [--loops corpus/agent-loops.toml] [--filter SUBSTR] [--rounds N]
                    [--name NAME] [--shim PATH] [--sail PATH]
  sail-bench history-replay --project NAME [--commits N] [--filter SUBSTR]
                    [--name NAME] [--shim PATH] [--sail PATH]
  sail-bench report [--out PATH] FILE...
  sail-bench hits   --base TRACE_DIR --then TRACE_DIR [--json PATH]
  sail-bench profile|seeds|history ...   (trace analyses; run one for its usage)";

/// Which corpus units to run.
struct Selection<'a> {
    tier: Tier,
    projects: &'a [String],
    filter: Option<&'a str>,
}

/// Fetches and prepares the selected units. Projects whose requirements are
/// unavailable are skipped with a note on stderr; `cargo-verus` units get
/// their scratch copies under `<scratch>/prepared` (ADR 0006).
fn select_units(
    selection: &Selection,
    corpus: &Corpus,
    installed: &Installed,
    cache: &Path,
    scratch: &Path,
) -> Result<Vec<Unit>, String> {
    let mut units = Vec::new();
    for project in corpus.projects_in(selection.tier, selection.projects) {
        let missing = project.missing_requirements(&installed.pins.verus.rust_toolchain);
        if !missing.is_empty() {
            eprintln!(
                "skipping {}: requires {} (not available)",
                project.name,
                missing.join(", ")
            );
            continue;
        }
        let dir = project.fetch(cache).map_err(|e| e.to_string())?;
        units.extend(project.units(&dir).map_err(|e| e.to_string())?);
    }
    if let Some(f) = selection.filter {
        units.retain(|u| u.id.contains(f));
    }
    let verus_source = if units.iter().any(|u| u.patch_verus_deps) {
        Some(
            corpus
                .fetch_verus_source(cache, &installed.pins.verus.commit)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let dir = scratch.join("prepared").join(i.to_string());
            prepare_unit(installed, u, verus_source.as_deref(), &dir).map_err(|e| e.to_string())
        })
        .collect()
}

fn sail_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        )
}

/// Options shared by the `cold`, `warm` and `edit` workloads.
struct Options {
    tier: Tier,
    projects: Vec<String>,
    filter: Option<String>,
    rounds: Option<usize>,
    name: Option<String>,
    shim: Option<PathBuf>,
    sail: Option<PathBuf>,
    edits: PathBuf,
    loops: PathBuf,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        tier: Tier::Fast,
        projects: Vec::new(),
        filter: None,
        rounds: None,
        name: None,
        shim: None,
        sail: None,
        edits: PathBuf::from("corpus/edits.toml"),
        loops: PathBuf::from("corpus/agent-loops.toml"),
    };
    let mut it = args.iter();
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
            "--rounds" => {
                o.rounds = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--rounds needs a number".to_owned())?,
                );
            }
            "--name" => o.name = Some(value()?),
            "--shim" => o.shim = Some(value()?.into()),
            "--sail" => o.sail = Some(value()?.into()),
            "--edits" => o.edits = value()?.into(),
            "--loops" => o.loops = value()?.into(),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    Ok(o)
}

/// Everything a workload run needs, located and prepared.
struct Prepared {
    installed: Installed,
    corpus: Corpus,
    cache: PathBuf,
    shim: PathBuf,
    started: u64,
    scratch: PathBuf,
}

fn prepare(shim: Option<PathBuf>) -> Result<Prepared, String> {
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    let corpus = Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
    corpus.check_pins(&pins).map_err(|e| e.to_string())?;
    let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let shim = match shim {
        Some(s) => s,
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("sail-z3"),
    };
    if !shim.is_file() {
        return Err(format!(
            "shim not found at {} (pass --shim)",
            shim.display()
        ));
    }
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let scratch = cache.join("bench").join(started.to_string());
    Ok(Prepared {
        installed,
        corpus,
        cache,
        shim,
        started,
        scratch,
    })
}

/// What a workload produced, ready to be written as a result file.
struct Output {
    workload: &'static str,
    tier: String,
    rounds: usize,
    units: Vec<UnitResult>,
    edits: Vec<EditResult>,
    loops: Vec<sail_bench::agent_loop::LoopResult>,
    commits: Vec<sail_bench::replay::CommitResult>,
}

fn write_result(out: Output, name: Option<String>, p: &Prepared) -> Result<(), String> {
    let name = name.unwrap_or_else(|| {
        format!(
            "{}-{}-{}-{}",
            out.workload,
            out.tier,
            sail_verus::toolchain::host(),
            p.started
        )
    });
    let file = BenchFile {
        schema_version: 1,
        workload: out.workload.to_owned(),
        tier: out.tier,
        rounds: out.rounds,
        verus_release: p.installed.pins.verus.release.clone(),
        z3_version: p.installed.pins.z3.version.clone(),
        sail_commit: sail_commit(),
        started_unix: p.started,
        machine: machine_info(),
        units: out.units,
        edits: out.edits,
        loops: out.loops,
        commits: out.commits,
    };
    let path = Path::new("bench/results").join(format!("{name}.json"));
    std::fs::create_dir_all("bench/results").map_err(|e| e.to_string())?;
    let json = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn tier_name(tier: Tier) -> String {
    format!("{tier:?}").to_lowercase()
}

fn cold_cmd(args: &[String]) -> Result<(), String> {
    let o = parse_options(args)?;
    let rounds = o.rounds.unwrap_or(3);
    let p = prepare(o.shim.clone())?;
    let selection = Selection {
        tier: o.tier,
        projects: &o.projects,
        filter: o.filter.as_deref(),
    };
    let units = select_units(&selection, &p.corpus, &p.installed, &p.cache, &p.scratch)?;
    eprintln!("cold workload: {} units x {rounds} rounds", units.len());
    let results = cold::run(&p.installed, &units, &p.shim, rounds, &p.scratch, |r| {
        eprintln!(
            "{:<60} plain {:>8.0} ms  sail {:>8.0} ms  same {}/{}",
            r.id,
            cold::median(&r.plain.wall_ms).unwrap_or(0.0),
            cold::median(&r.sail.wall_ms).unwrap_or(0.0),
            r.same,
            r.same + r.different + r.infra_errors
        );
    });
    let out = Output {
        workload: "cold",
        tier: tier_name(o.tier),
        rounds,
        units: results,
        edits: Vec::new(),
        loops: Vec::new(),
        commits: Vec::new(),
    };
    write_result(out, o.name, &p)
}

/// Sums `f` over a result's cache counters for `phase`.
fn cache_sum(cache: &[daemon::CacheRun], phase: &str, f: fn(&daemon::CacheRun) -> u64) -> u64 {
    cache.iter().filter(|c| c.phase == phase).map(f).sum()
}

fn warm_cmd(args: &[String]) -> Result<(), String> {
    let o = parse_options(args)?;
    let rounds = o.rounds.unwrap_or(2);
    if rounds == 0 {
        return Err("--rounds must be at least 1 for the warm workload".to_owned());
    }
    let p = prepare(o.shim.clone())?;
    let sail = daemon::locate_sail(o.sail.clone())?;
    let selection = Selection {
        tier: o.tier,
        projects: &o.projects,
        filter: o.filter.as_deref(),
    };
    let units = select_units(&selection, &p.corpus, &p.installed, &p.cache, &p.scratch)?;
    eprintln!(
        "warm workload: {} units x (1 fill + {rounds} warm rounds)",
        units.len()
    );
    let setup = warm::Setup {
        installed: &p.installed,
        shim: &p.shim,
        sail: &sail,
        scratch: &p.scratch,
    };
    let results = warm::run(&setup, &units, rounds, |r| {
        eprintln!(
            "{:<60} plain {:>8.0} ms  fill {:>8.0} ms  warm {:>8.0} ms  served {} stored {}  same {}/{}",
            r.id,
            cold::median(&r.plain.wall_ms).unwrap_or(0.0),
            r.fill
                .as_ref()
                .and_then(|f| cold::median(&f.wall_ms))
                .unwrap_or(0.0),
            cold::median(&r.sail.wall_ms).unwrap_or(0.0),
            cache_sum(&r.cache, "warm", |c| c.hits),
            cache_sum(&r.cache, "warm", |c| c.puts),
            r.same,
            r.same + r.different + r.unstable + r.infra_errors
        );
    });
    let out = Output {
        workload: "warm",
        tier: tier_name(o.tier),
        rounds,
        units: results,
        edits: Vec::new(),
        loops: Vec::new(),
        commits: Vec::new(),
    };
    write_result(out, o.name, &p)
}

fn edit_cmd(args: &[String]) -> Result<(), String> {
    let o = parse_options(args)?;
    let rounds = o.rounds.unwrap_or(2);
    if rounds == 0 {
        return Err("--rounds must be at least 1".to_owned());
    }
    let catalogue = edit::Catalogue::load(&o.edits)?;
    let p = prepare(o.shim.clone())?;
    let sail = daemon::locate_sail(o.sail.clone())?;
    // Edits name their units: select from every tier, limited to their
    // projects.
    let mut projects: Vec<String> = catalogue
        .edits
        .iter()
        .filter_map(|e| e.unit.split('/').next().map(str::to_owned))
        .collect();
    projects.sort();
    projects.dedup();
    let selection = Selection {
        tier: Tier::Deep,
        projects: &projects,
        filter: None,
    };
    let units = select_units(&selection, &p.corpus, &p.installed, &p.cache, &p.scratch)?;
    let mut edits = edit::resolve(&catalogue, &units)?;
    if let Some(f) = o.filter.as_deref() {
        edits.retain(|(e, _)| e.id.contains(f) || e.unit.contains(f));
    }
    eprintln!("edit workload: {} edits x {rounds} rounds", edits.len());
    let setup = warm::Setup {
        installed: &p.installed,
        shim: &p.shim,
        sail: &sail,
        scratch: &p.scratch,
    };
    let results = edit::run(&setup, &edits, rounds, |r| {
        eprintln!(
            "{:<28} original: plain {:>6.0} ms fill {:>6.0} ms | edited: plain {:>6.0} ms sail {:>6.0} ms | served {} stored {} | outcome {} (expected {}) | same {}/{}",
            r.id,
            cold::median(&r.plain_original.wall_ms).unwrap_or(0.0),
            cold::median(&r.fill.wall_ms).unwrap_or(0.0),
            cold::median(&r.plain.wall_ms).unwrap_or(0.0),
            cold::median(&r.sail.wall_ms).unwrap_or(0.0),
            cache_sum(&r.cache, "edit", |c| c.hits),
            cache_sum(&r.cache, "edit", |c| c.puts),
            r.outcome.as_deref().unwrap_or("n/a"),
            r.expected,
            r.same,
            r.same + r.different + r.unstable + r.infra_errors
        );
    });
    let out = Output {
        workload: "edit",
        tier: "catalogue".to_owned(),
        rounds,
        units: Vec::new(),
        edits: results,
        loops: Vec::new(),
        commits: Vec::new(),
    };
    write_result(out, o.name, &p)
}

fn agent_loop_cmd(args: &[String]) -> Result<(), String> {
    use sail_bench::agent_loop;
    let o = parse_options(args)?;
    let rounds = o.rounds.unwrap_or(2);
    if rounds == 0 {
        return Err("--rounds must be at least 1".to_owned());
    }
    let catalogue = agent_loop::Catalogue::load(&o.loops)?;
    let p = prepare(o.shim.clone())?;
    let sail = daemon::locate_sail(o.sail.clone())?;
    let mut projects: Vec<String> = catalogue
        .loops
        .iter()
        .filter_map(|l| l.unit.split('/').next().map(str::to_owned))
        .collect();
    projects.sort();
    projects.dedup();
    let selection = Selection {
        tier: Tier::Deep,
        projects: &projects,
        filter: None,
    };
    let units = select_units(&selection, &p.corpus, &p.installed, &p.cache, &p.scratch)?;
    let mut loops = agent_loop::resolve(&catalogue, &units)?;
    if let Some(f) = o.filter.as_deref() {
        loops.retain(|(l, _)| l.id.contains(f) || l.unit.contains(f));
    }
    eprintln!(
        "agent-loop workload: {} loops x {rounds} rounds",
        loops.len()
    );
    let setup = warm::Setup {
        installed: &p.installed,
        shim: &p.shim,
        sail: &sail,
        scratch: &p.scratch,
    };
    let results = agent_loop::run(&setup, &loops, rounds, |r| {
        for c in &r.cycles {
            eprintln!(
                "{:<18} {:<18} plain {:>6.0} ms  sail {:>6.0} ms  outcome {} (expected {})  same {}/{}",
                r.id,
                c.step,
                cold::median(&c.plain.wall_ms).unwrap_or(0.0),
                cold::median(&c.sail.wall_ms).unwrap_or(0.0),
                c.outcome.as_deref().unwrap_or("n/a"),
                c.expected.as_deref().unwrap_or("-"),
                c.same,
                c.same + c.different + c.unstable + c.infra_errors
            );
        }
    });
    let out = Output {
        workload: "agent-loop",
        tier: "catalogue".to_owned(),
        rounds,
        units: Vec::new(),
        edits: Vec::new(),
        loops: results,
        commits: Vec::new(),
    };
    write_result(out, o.name, &p)
}

fn history_replay_cmd(args: &[String]) -> Result<(), String> {
    use sail_bench::replay;
    let (mut project, mut count, mut rest) = (None::<String>, 10usize, Vec::new());
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--project" => project = it.next().cloned(),
            "--commits" => {
                count = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--commits needs a number")?;
            }
            _ => rest.push(arg.clone()),
        }
    }
    let name = project.ok_or(format!("history-replay needs --project\n{USAGE}"))?;
    if count == 0 {
        return Err("--commits must be at least 1".to_owned());
    }
    let o = parse_options(&rest)?;
    let p = prepare(o.shim.clone())?;
    let sail = daemon::locate_sail(o.sail.clone())?;
    let proj = p
        .corpus
        .projects
        .iter()
        .find(|x| x.name == name)
        .ok_or(format!("no corpus project named {name}"))?;
    let missing = proj.missing_requirements(&p.installed.pins.verus.rust_toolchain);
    if !missing.is_empty() {
        return Err(format!(
            "{name} requires {} (not available)",
            missing.join(", ")
        ));
    }
    let verus_source = if proj.patch_verus_deps {
        Some(
            p.corpus
                .fetch_verus_source(&p.cache)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let repo = p.scratch.join("history").join(&name);
    let commits = replay::history(proj, &repo, count)?;
    eprintln!(
        "history replay: {name}, {} commits (oldest first)",
        commits.len()
    );
    let setup = warm::Setup {
        installed: &p.installed,
        shim: &p.shim,
        sail: &sail,
        scratch: &p.scratch,
    };
    let cache_root = p.scratch.join("cache").join("replay");
    let results = replay::run(
        &setup,
        proj,
        &repo,
        &commits,
        verus_source.as_deref(),
        o.filter.as_deref(),
        &cache_root,
        |r| {
            eprintln!(
                "{} {:<40} plain {:>7.0} ms  sail {:>7.0} ms  outcome {}  same {}/{}{}",
                r.sha.get(..10).unwrap_or(&r.sha),
                r.subject.chars().take(40).collect::<String>(),
                cold::median(&r.plain.wall_ms).unwrap_or(0.0),
                cold::median(&r.sail.wall_ms).unwrap_or(0.0),
                r.outcome.as_deref().unwrap_or("n/a"),
                r.same,
                r.same + r.different + r.unstable + r.infra_errors,
                r.note
                    .as_deref()
                    .map(|n| format!("  ({n})"))
                    .unwrap_or_default()
            );
        },
    )?;
    let out = Output {
        workload: "history-replay",
        tier: name,
        rounds: 1,
        units: Vec::new(),
        edits: Vec::new(),
        loops: Vec::new(),
        commits: results,
    };
    write_result(out, o.name, &p)
}

fn report_cmd(args: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("docs/benchmarks.md");
    let mut inputs = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--out" {
            out = it.next().ok_or("--out needs a value")?.into();
        } else {
            inputs.push(PathBuf::from(arg));
        }
    }
    inputs.sort();
    let mut files = Vec::new();
    for path in &inputs {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let Ok(file) = serde_json::from_str::<BenchFile>(&text) else {
            // Other result kinds (profiles, seed runs) are reported elsewhere.
            continue;
        };
        let name = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        files.push((name, file));
    }
    std::fs::write(&out, sail_bench::report::render(&files))
        .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!(
        "wrote {} from {} result file(s)",
        out.display(),
        files.len()
    );
    Ok(())
}

fn hits_cmd(args: &[String]) -> Result<(), String> {
    let (mut base, mut then, mut json) = (None::<PathBuf>, None::<PathBuf>, None::<PathBuf>);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let value = it
            .next()
            .map(PathBuf::from)
            .ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--base" => base = Some(value?),
            "--then" => then = Some(value?),
            "--json" => json = Some(value?),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    let (Some(base), Some(then)) = (base, then) else {
        return Err(format!("hits needs --base and --then\n{USAGE}"));
    };
    let report = sail_bench::hits::run(&base, &then).map_err(|e| e.to_string())?;
    println!("{}", sail_bench::hits::summary(&report));
    if let Some(path) = json {
        let text = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        args.first().map(String::as_str),
        Some("profile" | "seeds" | "history")
    ) {
        return sail_bench::cli::run(args);
    }
    let result = match args.first().map(String::as_str) {
        Some("cold") => cold_cmd(args.get(1..).unwrap_or_default()),
        Some("warm") => warm_cmd(args.get(1..).unwrap_or_default()),
        Some("edit") => edit_cmd(args.get(1..).unwrap_or_default()),
        Some("agent-loop") => agent_loop_cmd(args.get(1..).unwrap_or_default()),
        Some("history-replay") => history_replay_cmd(args.get(1..).unwrap_or_default()),
        Some("report") => report_cmd(args.get(1..).unwrap_or_default()),
        Some("hits") => hits_cmd(args.get(1..).unwrap_or_default()),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
