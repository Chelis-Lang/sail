//! The `sail-bench` binary.
//!
//! ```text
//! sail-bench cold   [--tier T] [--project P]... [--filter S] [--rounds N] [--name NAME] [--shim PATH]
//! sail-bench warm   (cold's options) [--rounds WARM_ROUNDS] [--sail PATH]
//! sail-bench edit   [--edits corpus/edits.toml] [--filter S] [--rounds N] [--name NAME] [--shim PATH] [--sail PATH]
//! sail-bench fragility [--tier T] [--project P]... [--filter S] [--seeds N] [--shuffles K]
//!                      [--jobs J] [--verus-arg A]... [--name NAME] [--shim PATH]
//! sail-bench repair prepare|run ...   (M7 repair acceptance; see sail_bench::repair)
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
use sail_bench::{daemon, fragility, warm};
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
  sail-bench fragility [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                    [--seeds N] [--shuffles K] [--jobs J] [--near-limit F] [--top K]
                    [--verus-arg ARG]... [--name NAME] [--shim PATH]
  sail-bench repair prepare --scratch DIR
  sail-bench repair run --m6 SAIL --m6-commit SHA --m7 SAIL --m7-commit SHA --engine DIR
                    --scratch DIR --out DIR --environment LABEL [--name NAME] [--rounds N]
                    [--project ID]... [--filter CYCLE_SUBSTR] [--timeout-secs S] [--resume]
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
                .fetch_verus_source(&p.cache, &p.installed.pins.verus.commit)
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

/// Options of the `fragility` census.
struct FragilityArgs {
    selection: Options,
    seeds: u64,
    shuffles: u64,
    jobs: usize,
    near_limit: f64,
    top: usize,
    verus_args: Vec<String>,
}

fn parse_fragility(args: &[String]) -> Result<FragilityArgs, String> {
    let mut rest = Vec::new();
    let mut f = FragilityArgs {
        selection: parse_options(&[])?,
        seeds: 8,
        shuffles: 0,
        jobs: 4,
        near_limit: 0.5,
        top: 50,
        verus_args: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{arg} needs a value\n{USAGE}"))
        };
        let number = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--seeds" => f.seeds = number(value()?)?,
            "--shuffles" => f.shuffles = number(value()?)?,
            "--jobs" => f.jobs = usize::try_from(number(value()?)?).unwrap_or(1),
            "--top" => f.top = usize::try_from(number(value()?)?).unwrap_or(0),
            "--near-limit" => {
                f.near_limit = value()?
                    .parse()
                    .map_err(|_| "--near-limit needs a fraction".to_owned())?;
            }
            "--verus-arg" => f.verus_args.push(value()?),
            _ => {
                rest.push(arg.clone());
                if matches!(
                    arg.as_str(),
                    "--tier" | "--project" | "--filter" | "--name" | "--shim"
                ) {
                    rest.push(value()?);
                }
            }
        }
    }
    f.selection = parse_options(&rest)?;
    if f.seeds == 0 || f.jobs == 0 {
        return Err("--seeds and --jobs must be at least 1".to_owned());
    }
    Ok(f)
}

fn fragility_cmd(args: &[String]) -> Result<(), String> {
    let f = parse_fragility(args)?;
    let o = &f.selection;
    let p = prepare(o.shim.clone())?;
    let selection = Selection {
        tier: o.tier,
        projects: &o.projects,
        filter: o.filter.as_deref(),
    };
    let units = select_units(&selection, &p.corpus, &p.installed, &p.cache, &p.scratch)?;
    eprintln!(
        "fragility census: {} units, {} seeds, {} shuffles, {} jobs",
        units.len(),
        f.seeds,
        f.shuffles,
        f.jobs
    );
    let options = fragility::CensusOptions {
        portfolio: sail_portfolio::fragility::Options {
            seeds: f.seeds,
            shuffles: f.shuffles,
            jobs: f.jobs,
            near_limit: f.near_limit,
        },
        extra_verus_args: f.verus_args.clone(),
        top: f.top,
    };
    let census = fragility::census(
        &p.installed,
        &units,
        &p.shim,
        &p.scratch,
        &options,
        |line| eprintln!("{line}"),
    )?;
    eprint!("{}", sail_portfolio::fragility::render(&census.report, 20));
    let tier = tier_name(o.tier);
    let name = o.name.clone().unwrap_or_else(|| {
        format!(
            "fragility-census-{tier}-{}-{}",
            sail_verus::toolchain::host(),
            p.started
        )
    });
    let file = fragility::file(
        &census,
        &options,
        fragility::Meta {
            tier,
            verus_release: p.installed.pins.verus.release.clone(),
            z3_version: p.installed.pins.z3.version.clone(),
            sail_commit: sail_commit(),
            started_unix: p.started,
            machine: machine_info(),
        },
    );
    let path = Path::new("bench/results").join(format!("{name}.json"));
    std::fs::create_dir_all("bench/results").map_err(|e| e.to_string())?;
    let mut json = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?;
    json.push(b'\n');
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
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
    let mut censuses = Vec::new();
    let mut repairs = Vec::new();
    for path in &inputs {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let name = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let workload = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("workload")?.as_str().map(str::to_owned));
        if workload.as_deref() == Some("fragility") {
            let census = serde_json::from_str::<fragility::CensusFile>(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            censuses.push((name, census));
            continue;
        }
        if workload.as_deref() == Some(sail_bench::repair::WORKLOAD) {
            let repair = serde_json::from_str::<sail_bench::repair::RepairFile>(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            repairs.push((name, repair));
            continue;
        }
        let Ok(file) = serde_json::from_str::<BenchFile>(&text) else {
            // Other result kinds (profiles, seed runs) are reported elsewhere.
            continue;
        };
        files.push((name, file));
    }
    std::fs::write(
        &out,
        sail_bench::report::render(&files, &censuses, &repairs),
    )
    .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!(
        "wrote {} from {} result file(s)",
        out.display(),
        files.len() + censuses.len() + repairs.len()
    );
    Ok(())
}

/// Options of `sail-bench repair`.
#[derive(Default)]
struct RepairArgs {
    values: std::collections::BTreeMap<String, String>,
    projects: Vec<String>,
    resume: bool,
}

fn parse_repair(args: &[String]) -> Result<RepairArgs, String> {
    const VALUED: &[&str] = &[
        "--scratch",
        "--m6",
        "--m6-commit",
        "--m7",
        "--m7-commit",
        "--engine",
        "--out",
        "--environment",
        "--name",
        "--rounds",
        "--filter",
        "--timeout-secs",
    ];
    let mut a = RepairArgs::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--resume" => a.resume = true,
            "--project" => a
                .projects
                .push(it.next().cloned().ok_or("--project needs a value")?),
            flag if VALUED.contains(&flag) => {
                let value = it.next().cloned().ok_or(format!("{flag} needs a value"))?;
                a.values.insert(flag.to_owned(), value);
            }
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    Ok(a)
}

impl RepairArgs {
    fn get(&self, flag: &str) -> Result<String, String> {
        self.values
            .get(flag)
            .cloned()
            .ok_or(format!("sail-bench repair needs {flag}\n{USAGE}"))
    }

    fn number(&self, flag: &str, default: u64) -> Result<u64, String> {
        self.values.get(flag).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("{flag} needs a number"))
        })
    }
}

#[cfg(unix)]
fn repair_cmd(args: &[String]) -> Result<(), String> {
    use sail_bench::repair::{Catalogue, run};
    let (sub, rest) = args.split_first().ok_or(USAGE.to_owned())?;
    let a = parse_repair(rest)?;
    let catalogue = Catalogue::load(Path::new("corpus/repair-edits.json"))?;
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    match sub.as_str() {
        "prepare" => {
            let corpus =
                Corpus::load(Path::new("corpus/corpus.toml")).map_err(|e| e.to_string())?;
            let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
            let verus = corpus
                .fetch_verus_source(&cache, &catalogue.verus_commit)
                .map_err(|e| e.to_string())?;
            let fetch = |p: &sail_bench::repair::Project| -> Result<PathBuf, String> {
                corpus
                    .projects
                    .iter()
                    .find(|c| c.name == p.id)
                    .ok_or(format!("{} is not a corpus project", p.id))?
                    .fetch(&cache)
                    .map_err(|e| e.to_string())
            };
            let scratch = PathBuf::from(a.get("--scratch")?);
            let prepared = run::prepare(&run::PrepareOptions {
                catalogue: &catalogue,
                corpus_dir: &fetch,
                verus_source: &verus,
                scratch: &scratch,
                rust_toolchain: &pins.verus.rust_toolchain,
            })?;
            for p in prepared {
                println!("prepared {}", p.display());
            }
            Ok(())
        }
        "run" => {
            if std::env::var("CI").is_ok_and(|v| v == "true") {
                return Err("the repair workload is local-only (ADR 0013)".to_owned());
            }
            let started_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let name = a.values.get("--name").cloned().unwrap_or_else(|| {
                format!("repair-{}-{started_unix}", sail_verus::toolchain::host())
            });
            let options = run::RunOptions {
                m6: PathBuf::from(a.get("--m6")?),
                m7: PathBuf::from(a.get("--m7")?),
                commits: [a.get("--m6-commit")?, a.get("--m7-commit")?],
                engine: PathBuf::from(a.get("--engine")?),
                scratch: PathBuf::from(a.get("--scratch")?),
                out: PathBuf::from(a.get("--out")?),
                result: Path::new("bench/results").join(format!("{name}.json")),
                rounds: usize::try_from(a.number("--rounds", 3)?).unwrap_or(3),
                timeout: std::time::Duration::from_secs(a.number("--timeout-secs", 900)?),
                environment: a.get("--environment")?,
                projects: a.projects.clone(),
                filter: a.values.get("--filter").cloned(),
                resume: a.resume,
                rust_toolchain: pins.verus.rust_toolchain.clone(),
                verus_release: pins.verus.release.clone(),
                z3_version: pins.z3.version.clone(),
                started_unix,
                machine: machine_info(),
            };
            if options.rounds == 0 {
                return Err("--rounds must be at least 1".to_owned());
            }
            let file = run::run(&options, &catalogue)?;
            for v in sail_bench::repair::judge::judge(&file) {
                println!(
                    "{}: {} (speedup {}, {} complete rounds, {} failing samples)",
                    v.project,
                    if v.passed { "pass" } else { "fail" },
                    v.speedup
                        .map_or_else(|| "n/a".to_owned(), |x| format!("{x:.2}x")),
                    v.complete_rounds,
                    v.failing.len()
                );
                for r in &v.reasons {
                    println!("  {r}");
                }
            }
            println!("wrote {}", options.result.display());
            Ok(())
        }
        _ => Err(USAGE.to_owned()),
    }
}

#[cfg(not(unix))]
fn repair_cmd(_args: &[String]) -> Result<(), String> {
    Err("the repair workload needs Unix sockets".to_owned())
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
        Some("--help" | "-h" | "help")
    ) {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
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
        Some("fragility") => fragility_cmd(args.get(1..).unwrap_or_default()),
        Some("report") => report_cmd(args.get(1..).unwrap_or_default()),
        Some("repair") => repair_cmd(args.get(1..).unwrap_or_default()),
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
