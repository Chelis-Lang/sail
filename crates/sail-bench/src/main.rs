//! The `sail-bench` binary.
//!
//! ```text
//! sail-bench cold   [--tier T] [--project P]... [--filter S] [--rounds N] [--name NAME] [--shim PATH]
//! sail-bench report [--out docs/benchmarks.md] FILE...
//! sail-bench profile <trace-dir>... [--json out.json]
//! sail-bench seeds   <trace-dir>... --z3 <path> [--seeds N] [--jobs J] [--json out.json]
//! sail-bench history <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
//! ```
//!
//! `profile`, `seeds` and `history` analyse shim traces; see
//! [`sail_bench::cli`].
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use sail_bench::cold::{self, BenchFile};
use sail_verus::corpus::{Corpus, Tier, Unit};
use sail_verus::run::{machine_info, prepare_unit};
use sail_verus::toolchain::{Installed, Pins};

const USAGE: &str = "usage:
  sail-bench cold   [--tier fast|standard|deep] [--project NAME]... [--filter SUBSTR]
                    [--rounds N] [--name NAME] [--shim PATH]
  sail-bench report [--out PATH] FILE...
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
                .fetch_verus_source(cache)
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

fn cold_cmd(args: &[String]) -> Result<(), String> {
    let (mut tier, mut projects, mut filter, mut rounds, mut name, mut shim) = (
        Tier::Fast,
        Vec::new(),
        None::<String>,
        3usize,
        None::<String>,
        None::<PathBuf>,
    );
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--tier" => {
                tier = value()?
                    .parse()
                    .map_err(|e: sail_verus::Error| e.to_string())?;
            }
            "--project" => projects.push(value()?),
            "--filter" => filter = Some(value()?),
            "--rounds" => {
                rounds = value()?
                    .parse()
                    .map_err(|_| "--rounds needs a number".to_owned())?;
            }
            "--name" => name = Some(value()?),
            "--shim" => shim = Some(value()?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
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
    let selection = Selection {
        tier,
        projects: &projects,
        filter: filter.as_deref(),
    };
    let units = select_units(&selection, &corpus, &installed, &cache, &scratch)?;
    eprintln!("cold workload: {} units x {rounds} rounds", units.len());
    let results = cold::run(&installed, &units, &shim, rounds, &scratch, |r| {
        eprintln!(
            "{:<60} plain {:>8.0} ms  sail {:>8.0} ms  same {}/{}",
            r.id,
            cold::median(&r.plain.wall_ms).unwrap_or(0.0),
            cold::median(&r.sail.wall_ms).unwrap_or(0.0),
            r.same,
            r.same + r.different + r.infra_errors
        );
    });
    let tier_name = format!("{tier:?}").to_lowercase();
    let file = BenchFile {
        schema_version: 1,
        workload: "cold".to_owned(),
        tier: tier_name.clone(),
        rounds,
        verus_release: installed.pins.verus.release.clone(),
        z3_version: installed.pins.z3.version.clone(),
        sail_commit: sail_commit(),
        started_unix: started,
        machine: machine_info(),
        units: results,
    };
    let name = name.unwrap_or_else(|| {
        format!(
            "cold-{tier_name}-{}-{started}",
            sail_verus::toolchain::host()
        )
    });
    let path = Path::new("bench/results").join(format!("{name}.json"));
    std::fs::create_dir_all("bench/results").map_err(|e| e.to_string())?;
    let json = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?;
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
        Some("report") => report_cmd(args.get(1..).unwrap_or_default()),
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
