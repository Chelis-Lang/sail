//! Kani benchmark arms (ADR 0183): cold, warm, edit, history replay and CI
//! rerun, on the Kani corpus (`corpus/kani-corpus.toml`).
//!
//! The baseline is always one original Kani driver over every harness with
//! `-j N` (ADR 0180), started from Sail's staged pinned tools with the
//! executor's environment, on a target kept between samples where the arm
//! says so. The Sail side is measured where a Sail layer for that arm exists.
//! Today only the per-harness CLI exists, so `cold` measures it and the other
//! arms record a `not-built` Sail row naming the layer that will fill it:
//! warm unchanged runs need closure receipts (ADR 0189), edits and history
//! replay need the CBMC-boundary store (ADR 0188), and a CI rerun needs the
//! signed tier (ADR 0190). Every measured Sail row must agree with the direct
//! row on per-harness outcomes; a speedup with any disagreement is a bug.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sail_conformance::kani::corpus::{Corpus, Project, workspace_root};
use sail_conformance::kani::record::Claim;
use sail_conformance::kani::runner::{
    Cli, Direct, SailSide, Slot, copy_tree, discover, prepare_cargo_home, spawn,
};
use serde::Serialize;

/// The arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    /// Removed target before every sample.
    Cold,
    /// Unchanged project, target kept.
    Warm,
    /// One edit to the package's sources, target kept.
    Edit,
    /// The last N commits in order, target kept.
    HistoryReplay,
    /// A fresh checkout and target, unchanged commit (a CI rerun).
    CiRerun,
}

impl std::str::FromStr for Arm {
    type Err = String;
    fn from_str(text: &str) -> Result<Self, String> {
        Ok(match text {
            "cold" => Self::Cold,
            "warm" => Self::Warm,
            "edit" => Self::Edit,
            "history-replay" => Self::HistoryReplay,
            "ci-rerun" => Self::CiRerun,
            _ => return Err(format!("unknown Kani bench arm `{text}`")),
        })
    }
}

/// One report row.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// Arm.
    pub arm: Arm,
    /// Project.
    pub project: String,
    /// `direct-kani-jN` or the Sail adapter's name.
    pub side: String,
    /// `measured`, `failed` (a sample's driver did not complete; every
    /// sample is still recorded) or `not-built`.
    pub status: &'static str,
    /// Samples whose driver printed no `Complete -` summary, or whose
    /// commit's dependencies could not be fetched.
    pub failed_samples: usize,
    /// Wall seconds of every sample, in order.
    pub samples_s: Vec<f64>,
    /// Median of `samples_s`.
    pub median_s: Option<f64>,
    /// Per-sample label (commit for history replay).
    pub labels: Vec<String>,
    /// Harness outcomes of the last sample, by harness (Sail rows), or the
    /// driver's summary line (direct rows).
    pub outcomes: BTreeMap<String, String>,
    /// Why a row is not measured, or what it measures.
    pub note: String,
}

/// The report file.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Schema.
    pub kind: &'static str,
    /// Host description supplied by the caller.
    pub host: String,
    /// Rows.
    pub rows: Vec<Row>,
}

/// Inputs to one arm.
pub struct Setup<'a> {
    /// The corpus project.
    pub project: &'a Project,
    /// Canonical package directory.
    pub package: &'a Path,
    /// The direct driver.
    pub direct: &'a Direct,
    /// The Sail CLI.
    pub sail: &'a Cli,
    /// A prepared Cargo home.
    pub slot: &'a Slot,
    /// Scratch directory for targets and copies.
    pub scratch: &'a Path,
    /// Samples.
    pub rounds: usize,
    /// Kani's `-j`.
    pub jobs: usize,
    /// Commits for history replay.
    pub commits: usize,
}

fn median(samples: &[f64]) -> Option<f64> {
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => s.get(n / 2).copied(),
        _ => Some(f64::midpoint(s[n / 2 - 1], s[n / 2])),
    }
}

/// One whole-project direct run; returns wall seconds, the summary line and
/// whether the driver completed (printed its `Complete -` summary).
///
/// # Errors
/// When the driver cannot be started.
pub fn direct_once(
    direct: &Direct,
    package: &Path,
    slot: &Slot,
    capture: &Path,
    jobs: usize,
) -> Result<(f64, String, bool), String> {
    for dir in ["home", "tmp", "target"] {
        std::fs::create_dir_all(capture.join(dir)).map_err(|e| e.to_string())?;
    }
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let started = Instant::now();
    let (exit, signal, stdout, _) = spawn(
        Command::new(direct.stage.join("bin/kani-driver"))
            .args(["kani", "-Z", "unstable-options", "-j", &jobs.to_string()])
            .args(["--output-format", "terse", "--manifest-path"])
            .arg(text(&package.join("Cargo.toml")))
            .arg("--target-dir")
            .arg(text(&capture.join("target")))
            .current_dir(package)
            .env_clear()
            .envs(direct.environment(capture, &slot.cargo_home)),
        Duration::from_hours(4),
    )
    .map_err(|e| e.to_string())?;
    let wall = started.elapsed().as_secs_f64();
    let summary = stdout.lines().rev().find(|l| l.starts_with("Complete - "));
    Ok((
        wall,
        format!(
            "exit {exit:?} signal {signal:?}: {}",
            summary.unwrap_or("no summary")
        ),
        summary.is_some(),
    ))
}

fn sample(d: &mut Row, (wall, summary, completed): (f64, String, bool), label: Option<&str>) {
    d.samples_s.push(wall);
    if !completed {
        d.failed_samples += 1;
    }
    match label {
        Some(l) => {
            d.labels.push(l.to_owned());
            d.outcomes.insert(l.to_owned(), summary);
        }
        None => {
            d.outcomes.insert("summary".into(), summary);
        }
    }
}

fn row(arm: Arm, project: &str, side: String, status: &'static str, note: &str) -> Row {
    Row {
        arm,
        project: project.to_owned(),
        side,
        status,
        samples_s: Vec::new(),
        failed_samples: 0,
        median_s: None,
        labels: Vec::new(),
        outcomes: BTreeMap::new(),
        note: note.to_owned(),
    }
}

fn not_built(arm: Arm, project: &str, layer: &str) -> Row {
    row(
        arm,
        project,
        "sail".into(),
        "not-built",
        &format!("no Sail layer for this arm yet ({layer}); the direct row is the baseline only"),
    )
}

/// Runs `arm` and returns its rows.
///
/// # Errors
/// When a run cannot be started or a checkout cannot be made.
pub fn run(arm: Arm, s: &Setup<'_>) -> Result<Vec<Row>, String> {
    let name = &s.project.name;
    let mut d = row(
        arm,
        name,
        format!("direct-kani-j{}", s.jobs),
        "measured",
        "",
    );
    let target = s.scratch.join(format!("{name}-kept"));
    match arm {
        Arm::Cold | Arm::Warm => {
            if arm == Arm::Warm {
                let _ = direct_once(s.direct, s.package, s.slot, &target, s.jobs)?;
                d.note = "after one unmeasured warming run".into();
            }
            for round in 0..s.rounds {
                let capture = if arm == Arm::Cold {
                    let fresh = s.scratch.join(format!("{name}-cold-{round}"));
                    let _ = std::fs::remove_dir_all(&fresh);
                    fresh
                } else {
                    target.clone()
                };
                sample(
                    &mut d,
                    direct_once(s.direct, s.package, s.slot, &capture, s.jobs)?,
                    None,
                );
                if arm == Arm::Cold {
                    let _ = std::fs::remove_dir_all(&capture);
                }
            }
        }
        Arm::Edit => {
            let _ = direct_once(s.direct, s.package, s.slot, &target, s.jobs)?;
            let file = s.package.join("src/lib.rs");
            let original = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            for round in 0..s.rounds {
                let mut edited = original.clone();
                edited.extend_from_slice(format!("\n// sail-bench kani edit {round}\n").as_bytes());
                std::fs::write(&file, &edited).map_err(|e| e.to_string())?;
                let result = direct_once(s.direct, s.package, s.slot, &target, s.jobs);
                std::fs::write(&file, &original).map_err(|e| e.to_string())?;
                sample(&mut d, result?, None);
            }
            d.note =
                "a comment appended to src/lib.rs (no line shift above it), target kept".into();
        }
        Arm::HistoryReplay => replay(s, &mut d)?,
        Arm::CiRerun => {
            for round in 0..s.rounds {
                let checkout = s.scratch.join(format!("{name}-ci-{round}"));
                let _ = std::fs::remove_dir_all(&checkout);
                let root = workspace_root(s.package);
                copy_tree(&root, &checkout).map_err(|e| e.to_string())?;
                let package = checkout.join(s.package.strip_prefix(&root).unwrap_or(Path::new("")));
                sample(
                    &mut d,
                    direct_once(s.direct, &package, s.slot, &checkout.join(".ci"), s.jobs)?,
                    None,
                );
                let _ = std::fs::remove_dir_all(&checkout);
            }
            d.note = "a fresh copy of the checkout and a new target; registry prepared".into();
        }
    }
    d.median_s = median(&d.samples_s);
    if d.failed_samples > 0 {
        d.status = "failed";
    }
    let sail = match arm {
        Arm::Cold => sail_cold(s)?,
        Arm::Warm => not_built(arm, name, "closure receipts, ADR 0189"),
        Arm::Edit | Arm::HistoryReplay => not_built(arm, name, "CBMC-boundary store, ADR 0188"),
        Arm::CiRerun => not_built(arm, name, "signed CI tier, ADR 0190"),
    };
    Ok(vec![d, sail])
}

fn sail_cold(s: &Setup<'_>) -> Result<Row, String> {
    let harnesses = discover(
        s.direct,
        s.project,
        s.package,
        s.slot,
        &s.scratch.join(format!("{}-discover", s.project.name)),
    )
    .map_err(|e| e.to_string())?;
    let mut r = row(
        Arm::Cold,
        &s.project.name,
        s.sail.name().into(),
        "measured",
        "one fresh `sail verify --backend kani` per harness, in sequence (the current CLI)",
    );
    for _ in 0..s.rounds {
        let started = Instant::now();
        let mut complete = true;
        for h in &harnesses {
            let record = s.sail.run(h, s.slot).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_dir_all(record.capture.join("target"));
            let claim = match record.claim {
                Claim::NoClaim(reason) => {
                    complete = false;
                    format!("no-claim {reason:?}")
                }
                other => format!("{other:?}"),
            };
            r.outcomes.insert(h.name.clone(), claim);
        }
        r.samples_s.push(started.elapsed().as_secs_f64());
        if !complete {
            r.failed_samples += 1;
        }
    }
    r.median_s = median(&r.samples_s);
    if r.failed_samples > 0 {
        r.status = "failed";
    }
    Ok(r)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn replay(s: &Setup<'_>, d: &mut Row) -> Result<(), String> {
    let root = workspace_root(s.package);
    let checkout = s.scratch.join(format!("{}-history", s.project.name));
    if !checkout.exists() {
        copy_tree(&root, &checkout).map_err(|e| e.to_string())?;
    }
    let depth = (s.commits + 1).to_string();
    git(
        &checkout,
        &[
            "fetch",
            "-q",
            "--depth",
            &depth,
            "origin",
            &s.project.commit,
        ],
    )?;
    let list = git(
        &checkout,
        &[
            "rev-list",
            "--first-parent",
            "-n",
            &s.commits.to_string(),
            &s.project.commit,
        ],
    )?;
    let commits: Vec<&str> = list.lines().rev().collect();
    let package = checkout.join(s.package.strip_prefix(&root).unwrap_or(Path::new("")));
    let target = s.scratch.join(format!("{}-history-target", s.project.name));
    for commit in commits {
        git(
            &checkout,
            &["checkout", "-q", "--force", "--detach", commit],
        )?;
        // Each commit has its own lock file: fetch its dependencies first.
        if let Err(e) = prepare_cargo_home(s.direct, &package, &s.slot.cargo_home) {
            sample(
                d,
                (0.0, format!("dependencies not fetched: {e}"), false),
                Some(commit),
            );
            continue;
        }
        sample(
            d,
            direct_once(s.direct, &package, s.slot, &target, s.jobs)?,
            Some(commit),
        );
    }
    d.note = format!(
        "{} first-parent commits ending at the pin, oldest first, one kept target",
        d.labels.len()
    );
    Ok(())
}

/// The staged direct driver and the Sail CLI from Sail's cache.
fn tools(sail: Option<PathBuf>) -> Result<(Direct, Cli, PathBuf), String> {
    let sail_cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let kani_root = sail_cache
        .join("kani")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let direct = Direct {
        stage: kani_root
            .join("stage-0.68.0-linux-x86_64")
            .canonicalize()
            .map_err(|e| e.to_string())?,
        deadline: Duration::from_hours(4),
    };
    let mut environment = BTreeMap::new();
    for key in ["HOME", "PATH", "SAIL_CACHE_DIR", "XDG_CACHE_HOME"] {
        if let Ok(v) = std::env::var(key) {
            environment.insert(key.to_owned(), v);
        }
    }
    let cli = Cli {
        sail: match sail {
            Some(p) => p,
            None => std::env::current_exe()
                .map_err(|e| e.to_string())?
                .with_file_name("sail"),
        },
        kani_root,
        environment,
        timeout_ms: 600_000,
    };
    Ok((direct, cli, sail_cache))
}

/// `sail-bench kani ARM ...`.
///
/// # Errors
/// On usage, setup or run errors.
pub fn main(args: &[String]) -> Result<(), String> {
    let usage = "usage: sail-bench kani cold|warm|edit|history-replay|ci-rerun --project NAME
       [--rounds N] [--jobs N] [--commits N] [--out FILE] [--corpus PATH] [--cache DIR]
       [--sail PATH] [--host TEXT]";
    let mut it = args.iter();
    let arm: Arm = it.next().ok_or(usage)?.parse()?;
    let (mut project, mut rounds, mut jobs, mut commits) = (None, 3, 8, 10);
    let mut out = PathBuf::from("kani-bench.json");
    let mut corpus = PathBuf::from("corpus/kani-corpus.toml");
    let (mut cache, mut sail, mut host) = (None, None, String::from("unspecified"));
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{flag} needs a value\n{usage}"))
        };
        let number = |v: String| v.parse::<usize>().map_err(|e| format!("{flag}: {e}"));
        match flag.as_str() {
            "--project" => project = Some(value()?),
            "--rounds" => rounds = number(value()?)?.max(1),
            "--jobs" => jobs = number(value()?)?.max(1),
            "--commits" => commits = number(value()?)?.max(1),
            "--out" => out = value()?.into(),
            "--corpus" => corpus = value()?.into(),
            "--cache" => cache = Some(PathBuf::from(value()?)),
            "--sail" => sail = Some(PathBuf::from(value()?)),
            "--host" => host = value()?,
            _ => return Err(format!("unknown option {flag}\n{usage}")),
        }
    }
    let name = project.ok_or(usage)?;
    let corpus_file = Corpus::load(&corpus).map_err(|e| e.to_string())?;
    let project = corpus_file
        .projects
        .iter()
        .find(|p| p.name == name)
        .ok_or(format!("unknown Kani project {name}"))?;
    let (direct, cli, sail_cache) = tools(sail)?;
    let cache = cache.unwrap_or_else(|| sail_cache.join("kani-corpus"));
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let repo_root = corpus
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().and_then(Path::parent).map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let package = project
        .fetch(&cache, &repo_root)
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let seed = cache.join("cargo-home");
    prepare_cargo_home(&direct, &package, &seed).map_err(|e| e.to_string())?;
    let scratch = cache.join(format!("bench-{name}"));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let slot = Slot {
        cargo_home: seed.canonicalize().map_err(|e| e.to_string())?,
    };
    let rows = run(
        arm,
        &Setup {
            project,
            package: &package,
            direct: &direct,
            sail: &cli,
            slot: &slot,
            scratch: &scratch,
            rounds,
            jobs,
            commits,
        },
    )?;
    for r in &rows {
        println!(
            "{:?} {} {}: {} median {:?} s over {} samples ({} failed). {}",
            r.arm,
            r.project,
            r.side,
            r.status,
            r.median_s,
            r.samples_s.len(),
            r.failed_samples,
            r.note
        );
    }
    let report = Report {
        kind: "sail-kani-bench",
        host,
        rows,
    };
    std::fs::write(
        &out,
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Arm, median};

    #[test]
    fn arms_parse_and_medians_are_middle_values() {
        for (text, arm) in [
            ("cold", Arm::Cold),
            ("warm", Arm::Warm),
            ("edit", Arm::Edit),
            ("history-replay", Arm::HistoryReplay),
            ("ci-rerun", Arm::CiRerun),
        ] {
            assert_eq!(text.parse::<Arm>(), Ok(arm));
        }
        assert!("hot".parse::<Arm>().is_err());
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }
}
