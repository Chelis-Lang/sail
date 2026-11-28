//! Drives the direct and Sail arms over the Kani corpus (ADR 0183).
//!
//! Each harness runs direct, then Sail, then direct again, in one job slot.
//! Every slot has its own copy of the prepared Cargo home, so concurrent
//! Cargo processes never print lock-wait lines into one another's output.
//! Each run gets a fresh private directory holding its target, home, tmp and
//! export. The direct arm runs the original driver from Sail's staged tool
//! bundle (the pinned stock bytes), so tool and library paths in check
//! locations are the same on both sides; only the run directory differs.
//!
//! The Sail side is a [`SailSide`] adapter. [`Cli`] runs one
//! `sail verify --backend kani` per harness; a batch executor needs only a
//! new adapter that returns one record per harness.
use std::collections::{BTreeMap, VecDeque};
use std::os::unix::fs::DirBuilderExt as _;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use super::Error;
use super::compare::{HarnessResult, KaniVerdict, classify, witness};
use super::corpus::{Corpus, Project, Tier};
use super::record::{Arm, Claim, KaniRunRecord, NoClaim};

/// One harness to check.
#[derive(Debug, Clone, Serialize)]
pub struct Harness {
    /// Corpus project.
    pub project: String,
    /// Canonical package directory.
    pub package: PathBuf,
    /// Exact harness name.
    pub name: String,
    /// Unwind bound both arms pass.
    pub unwind: u32,
    /// Harness attributes from `kani-metadata.json`.
    pub attributes: Value,
}

/// A job slot: its own Cargo home.
#[derive(Debug, Clone)]
pub struct Slot {
    /// Canonical private Cargo home for this slot.
    pub cargo_home: PathBuf,
}

/// The Sail side of the comparison.
pub trait SailSide: Sync {
    /// A short name for the report.
    fn name(&self) -> &'static str;
    /// Runs one harness and returns its record. `Err` only when the harness
    /// itself could not run Sail (not a Sail outcome).
    ///
    /// # Errors
    /// `SAIL6322` when Sail could not be started or its capture is missing.
    fn run(&self, harness: &Harness, slot: &Slot) -> Result<KaniRunRecord, Error>;
}

/// The direct oracle: Sail's staged original tools, run without Sail.
#[derive(Debug, Clone)]
pub struct Direct {
    /// Canonical stage root (`bin/kani-driver`, `toolchain/bin`).
    pub stage: PathBuf,
    /// Deadline after which the run is killed and the harness is an
    /// infrastructure error.
    pub deadline: Duration,
}

impl Direct {
    /// The environment Sail's executor gives the driver (ADR 0154).
    #[must_use]
    pub fn environment(&self, capture: &Path, cargo_home: &Path) -> BTreeMap<String, String> {
        let text = |p: &Path| p.to_string_lossy().into_owned();
        BTreeMap::from([
            ("LC_ALL".into(), "C".into()),
            ("HOME".into(), text(&capture.join("home"))),
            ("TMPDIR".into(), text(&capture.join("tmp"))),
            ("CARGO_HOME".into(), text(cargo_home)),
            ("CARGO_NET_OFFLINE".into(), "true".into()),
            (
                "PATH".into(),
                format!(
                    "{}:{}:/usr/bin:/bin",
                    text(&self.stage.join("toolchain/bin")),
                    text(&self.stage.join("bin"))
                ),
            ),
        ])
    }

    /// Sail's per-harness argv, built here independently of Sail.
    #[must_use]
    pub fn argv(harness: &Harness, capture: &Path) -> Vec<String> {
        let text = |p: &Path| p.to_string_lossy().into_owned();
        vec![
            "kani".into(),
            "-Z".into(),
            "unstable-options".into(),
            "--export-json".into(),
            text(&capture.join("export.json")),
            "--harness".into(),
            harness.name.clone(),
            "--exact".into(),
            "--unwind".into(),
            harness.unwind.to_string(),
            "--manifest-path".into(),
            text(&harness.package.join("Cargo.toml")),
            "--target-dir".into(),
            text(&capture.join("target")),
        ]
    }

    /// Runs the driver with `argv` in a new private `capture` directory.
    ///
    /// # Errors
    /// `SAIL6322` when the directory cannot be made or the driver not started.
    pub fn run_argv(
        &self,
        harness: &Harness,
        slot: &Slot,
        capture: &Path,
        argv: Vec<String>,
        driver: &Path,
    ) -> Result<KaniRunRecord, Error> {
        for dir in ["", "home", "tmp", "target"] {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(capture.join(dir))
                .map_err(|e| Error::arm(format!("{}: {e}", capture.display())))?;
        }
        let environment = self.environment(capture, &slot.cargo_home);
        let started = Instant::now();
        let (exit, signal, stdout, stderr) = spawn(
            Command::new(driver)
                .args(&argv)
                .current_dir(&harness.package)
                .env_clear()
                .envs(&environment),
            self.deadline,
        )?;
        let export = read_export(&capture.join("export.json"));
        let mut record = KaniRunRecord {
            arm: Arm::Direct,
            project: harness.project.clone(),
            harness: harness.name.clone(),
            unwind: harness.unwind,
            argv,
            environment,
            capture: capture.to_path_buf(),
            exit,
            signal,
            stdout,
            stderr,
            export,
            claim: Claim::NoClaim(NoClaim::Refused),
            refusal: None,
            sail_exit: None,
            wall_ms: millis(started),
        };
        record.claim = Claim::of_direct(&record);
        Ok(record)
    }

    /// One direct run of `harness` into `capture`.
    ///
    /// # Errors
    /// As [`Direct::run_argv`].
    pub fn run(
        &self,
        harness: &Harness,
        slot: &Slot,
        capture: &Path,
    ) -> Result<KaniRunRecord, Error> {
        let argv = Self::argv(harness, capture);
        self.run_argv(
            harness,
            slot,
            capture,
            argv,
            &self.stage.join("bin/kani-driver"),
        )
    }
}

/// Today's Sail CLI: one fresh `sail verify --backend kani` per harness.
#[derive(Debug, Clone)]
pub struct Cli {
    /// The `sail` binary.
    pub sail: PathBuf,
    /// Sail's Kani state directory (`<cache>/kani`), where its attempts live.
    pub kani_root: PathBuf,
    /// Extra environment for Sail itself (for example `SAIL_CACHE_DIR`).
    pub environment: BTreeMap<String, String>,
    /// `--timeout-ms`.
    pub timeout_ms: u64,
}

/// Maps Sail's refusal text to the outcome taxonomy.
#[must_use]
pub fn refusal_reason(stderr: &str, driver_stderr: &str) -> NoClaim {
    if stderr.contains("timed out") {
        NoClaim::Timeout
    } else if stderr.contains("cancelled") {
        NoClaim::Cancelled
    } else if stderr.contains("terminated by a signal") {
        NoClaim::Crash
    } else if stderr.contains("SAIL6303") || stderr.contains("changed") {
        NoClaim::Integrity
    } else if driver_stderr.contains("could not compile") {
        NoClaim::BuildFailure
    } else if stderr.contains("only ordinary hand-written proof harnesses")
        || stderr.contains("unsupported")
    {
        NoClaim::Unsupported
    } else {
        NoClaim::Refused
    }
}

impl Cli {
    /// The `sail` arguments for one harness.
    #[must_use]
    pub fn arguments(&self, harness: &Harness, slot: &Slot) -> Vec<String> {
        let text = |p: &Path| p.to_string_lossy().into_owned();
        vec![
            "verify".into(),
            "--backend".into(),
            "kani".into(),
            "--project".into(),
            text(&harness.package),
            "--cargo-home".into(),
            text(&slot.cargo_home),
            "--harness".into(),
            harness.name.clone(),
            "--unwind".into(),
            harness.unwind.to_string(),
            "--timeout-ms".into(),
            self.timeout_ms.to_string(),
            "--output".into(),
            "json".into(),
        ]
    }
}

/// Sail's claim from its exit status, JSON result and messages.
#[must_use]
pub fn sail_claim(exit: Option<i32>, stdout: &str, stderr: &str, driver_stderr: &str) -> Claim {
    if exit == Some(2) || exit.is_none() {
        return Claim::NoClaim(refusal_reason(stderr, driver_stderr));
    }
    let result: Value = serde_json::from_str(stdout).unwrap_or(Value::Null);
    match result["observation"]["outcome"].as_str() {
        Some("bounded_success") => Claim::BoundedSuccess,
        Some("counterexample") => Claim::Counterexample,
        Some("bound_insufficient") => Claim::BoundInsufficient,
        _ => Claim::NoClaim(NoClaim::Refused),
    }
}

impl SailSide for Cli {
    fn name(&self) -> &'static str {
        "sail-cli-per-harness"
    }

    fn run(&self, harness: &Harness, slot: &Slot) -> Result<KaniRunRecord, Error> {
        let mut command = Command::new(&self.sail);
        command
            .args(self.arguments(harness, slot))
            .env_clear()
            .envs(&self.environment);
        let started = Instant::now();
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::arm(format!("sail: {e}")))?;
        let pid = child.id();
        let output = child
            .wait_with_output()
            .map_err(|e| Error::arm(format!("sail: {e}")))?;
        let wall_ms = millis(started);
        let out = String::from_utf8_lossy(&output.stdout).into_owned();
        let err = String::from_utf8_lossy(&output.stderr).into_owned();
        let capture = attempt(&self.kani_root, pid);
        let read = |name: &str| {
            capture
                .as_ref()
                .and_then(|c| std::fs::read(c.join(name)).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let request: Value = read("request.json")
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let exit = read("exit").and_then(|t| t.trim().parse().ok());
        let stdout = read("stdout").unwrap_or_default();
        let stderr = read("stderr").unwrap_or_default();
        let claim = sail_claim(output.status.code(), &out, &err, &stderr);
        Ok(KaniRunRecord {
            arm: Arm::Sail,
            project: harness.project.clone(),
            harness: harness.name.clone(),
            unwind: harness.unwind,
            argv: request["argv"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            environment: request["environment"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default(),
            capture: capture.clone().unwrap_or_default(),
            exit,
            signal: None,
            stdout,
            stderr,
            export: capture
                .as_ref()
                .and_then(|c| read_export(&c.join("export.json"))),
            claim,
            refusal: (!claim.is_claim()).then(|| err.trim().to_owned()),
            sail_exit: output.status.code(),
            wall_ms,
        })
    }
}

/// Sail's attempt capture for the process `pid`, if it made one.
#[must_use]
pub fn attempt(kani_root: &Path, pid: u32) -> Option<PathBuf> {
    let prefix = format!("attempt-{pid}-");
    std::fs::read_dir(kani_root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| e.path().join("capture"))
        .max()
}

fn read_export(path: &Path) -> Option<Value> {
    std::fs::read(path)
        .ok()
        .and_then(|b| super::strict_json::parse(&b).ok())
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs `command` in its own process group with piped streams; kills the
/// group at exit or at `deadline`. Returns exit code, signal and streams.
///
/// # Errors
/// `SAIL6322` when the process cannot be started.
pub fn spawn(
    command: &mut Command,
    deadline: Duration,
) -> Result<(Option<i32>, Option<i32>, String, String), Error> {
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::arm(format!("spawn: {e}")))?;
    let pid = child.id();
    let mut pipes = (child.stdout.take(), child.stderr.take());
    let readers = (
        pipes.0.take().map(|mut p| {
            std::thread::spawn(move || {
                let mut s = Vec::new();
                let _ = std::io::Read::read_to_end(&mut p, &mut s);
                s
            })
        }),
        pipes.1.take().map(|mut p| {
            std::thread::spawn(move || {
                let mut s = Vec::new();
                let _ = std::io::Read::read_to_end(&mut p, &mut s);
                s
            })
        }),
    );
    let end = Instant::now() + deadline;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| Error::arm(e.to_string()))? {
            break Some(status);
        }
        if Instant::now() >= end {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Leave nothing of the group behind (a timed-out run leaves solvers).
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
    if status.is_none() {
        // Timed out: reap it, and report no exit status.
        let _ = child.wait();
    }
    let text = |h: Option<std::thread::JoinHandle<Vec<u8>>>| {
        h.and_then(|h| h.join().ok())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    };
    let (stdout, stderr) = (text(readers.0), text(readers.1));
    Ok((
        status.and_then(|s| s.code()),
        status.and_then(|s| s.signal()),
        stdout,
        stderr,
    ))
}

/// Options for one conformance run.
#[derive(Debug, Clone)]
pub struct Options {
    /// The corpus file.
    pub corpus: PathBuf,
    /// The repository root (for lock files).
    pub repo_root: PathBuf,
    /// Tier.
    pub tier: Tier,
    /// Named projects (empty: the whole tier).
    pub projects: Vec<String>,
    /// Keep harnesses whose name contains this.
    pub filter: Option<String>,
    /// Rounds; every round reruns every arm.
    pub rounds: usize,
    /// Direct runs per harness per round (at least two).
    pub direct_runs: usize,
    /// Parallel job slots.
    pub jobs: usize,
    /// Checkouts and the prepared Cargo home.
    pub cache: PathBuf,
    /// Output directory.
    pub out: PathBuf,
    /// Keep each run's target directory (default: removed after reading).
    pub keep_targets: bool,
}

/// Per-project totals.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Totals {
    /// Harnesses per round.
    pub harnesses: usize,
    /// Verdict counts over every round.
    pub verdicts: BTreeMap<KaniVerdict, usize>,
    /// Sail claims over every round.
    pub sail_claims: BTreeMap<String, usize>,
    /// Checks compared over every round (Sail against direct).
    pub checks: usize,
    /// Named fields seen to vary between direct runs, with counts.
    pub direct_variances: BTreeMap<String, usize>,
    /// The project's CI arguments neither arm passed.
    pub ci_args_not_passed: Vec<String>,
    /// Why no harness could be compared, when the shared argv cannot build
    /// the project (with the same argv both arms fail alike).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
}

/// The report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Schema.
    pub kind: &'static str,
    /// The Sail adapter.
    pub sail_side: &'static str,
    /// Tier.
    pub tier: Tier,
    /// Rounds.
    pub rounds: usize,
    /// Direct runs per harness per round.
    pub direct_runs: usize,
    /// Totals by project.
    pub projects: BTreeMap<String, Totals>,
    /// Named fields that differed only against Sail.
    pub unwitnessed: Vec<String>,
    /// Every harness result, by round.
    pub results: Vec<(usize, HarnessResult)>,
}

impl Report {
    /// True when no harness is `different` or an infrastructure error.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.results
            .iter()
            .all(|(_, r)| !matches!(r.verdict, KaniVerdict::Different | KaniVerdict::InfraError))
    }
}

/// Prepares the shared Cargo home for `package` with the staged cargo.
///
/// # Errors
/// `SAIL6321` when `cargo fetch --locked` fails or the workspace has no lock.
pub fn prepare_cargo_home(direct: &Direct, package: &Path, cargo_home: &Path) -> Result<(), Error> {
    let root = super::corpus::workspace_root(package);
    if !root.join("Cargo.lock").is_file() {
        return Err(Error::prepare(format!(
            "{}: no Cargo.lock; record one in the corpus",
            root.display()
        )));
    }
    std::fs::create_dir_all(cargo_home).map_err(|e| Error::prepare(e.to_string()))?;
    let mut env = direct.environment(Path::new("/nonexistent"), cargo_home);
    env.remove("CARGO_NET_OFFLINE");
    env.insert("HOME".into(), cargo_home.to_string_lossy().into_owned());
    env.remove("TMPDIR");
    let (exit, _, _, stderr) = spawn(
        Command::new(direct.stage.join("toolchain/bin/cargo"))
            .args(["fetch", "--locked", "--manifest-path"])
            .arg(package.join("Cargo.toml"))
            .env_clear()
            .envs(&env),
        Duration::from_mins(30),
    )?;
    if exit == Some(0) {
        Ok(())
    } else {
        Err(Error::prepare(format!("cargo fetch: {stderr}")))
    }
}

/// Lists `project`'s harnesses from the `kani-metadata.json` files an
/// `--only-codegen` run writes (`cargo kani list` misses real harnesses, S5).
///
/// # Errors
/// `SAIL6321` when codegen fails or finds no harness.
pub fn discover(
    direct: &Direct,
    project: &Project,
    package: &Path,
    slot: &Slot,
    capture: &Path,
) -> Result<Vec<Harness>, Error> {
    let probe = Harness {
        project: project.name.clone(),
        package: package.to_path_buf(),
        name: String::new(),
        unwind: project.default_unwind,
        attributes: Value::Null,
    };
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let argv = vec![
        "kani".into(),
        "-Z".into(),
        "unstable-options".into(),
        "--only-codegen".into(),
        "--manifest-path".into(),
        text(&package.join("Cargo.toml")),
        "--target-dir".into(),
        text(&capture.join("target")),
    ];
    let record = direct.run_argv(
        &probe,
        slot,
        capture,
        argv,
        &direct.stage.join("bin/kani-driver"),
    )?;
    if record.exit != Some(0) {
        return Err(Error::prepare(format!(
            "{}: codegen failed: {}",
            project.name,
            record
                .stderr
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n")
        )));
    }
    let mut files = Vec::new();
    collect(&capture.join("target/kani"), &mut files);
    let mut harnesses: BTreeMap<String, Harness> = BTreeMap::new();
    for file in files {
        let Some(doc) = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        else {
            continue;
        };
        for h in doc["proof_harnesses"].as_array().into_iter().flatten() {
            let Some(name) = h["pretty_name"].as_str() else {
                continue;
            };
            let unwind = h["attributes"]["unwind_value"]
                .as_u64()
                .and_then(|u| u32::try_from(u).ok())
                .unwrap_or(project.default_unwind);
            let harness = Harness {
                project: project.name.clone(),
                package: package.to_path_buf(),
                name: name.to_owned(),
                unwind,
                attributes: h["attributes"].clone(),
            };
            if harnesses.insert(name.to_owned(), harness).is_some() {
                return Err(Error::prepare(format!(
                    "{}: harness `{name}` twice",
                    project.name
                )));
            }
        }
    }
    if harnesses.is_empty() {
        return Err(Error::prepare(format!(
            "{}: no harness found",
            project.name
        )));
    }
    Ok(harnesses.into_values().collect())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.to_string_lossy().ends_with(".kani-metadata.json") {
            out.push(path);
        }
    }
}

fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '.'
            }
        })
        .collect()
}

/// Copies `from` to `to` with `cp -a` (a slot's private Cargo home).
///
/// # Errors
/// `SAIL6321` when the copy fails.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    if to.exists() {
        return Ok(());
    }
    let status = Command::new("cp")
        .arg("-a")
        .arg(from)
        .arg(to)
        .status()
        .map_err(|e| Error::prepare(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::prepare(format!("cp -a {} failed", from.display())))
    }
}

fn make_slots(out: &Path, seed: &Path, jobs: usize) -> Result<Vec<Slot>, Error> {
    (0..jobs.max(1))
        .map(|i| {
            let home = out.join(format!("slots/{i}/cargo-home"));
            std::fs::create_dir_all(out.join(format!("slots/{i}")))
                .map_err(|e| Error::prepare(e.to_string()))?;
            copy_tree(seed, &home)?;
            Ok(Slot {
                cargo_home: home
                    .canonicalize()
                    .map_err(|e| Error::prepare(e.to_string()))?,
            })
        })
        .collect()
}

/// Runs the selected tier and writes `report.json`, `summary.md` and every
/// record under `options.out`.
///
/// # Errors
/// `SAIL6320`/`SAIL6321`/`SAIL6322` for corpus, preparation or arm failures.
pub fn run(options: &Options, direct: &Direct, sail: &dyn SailSide) -> Result<Report, Error> {
    let corpus = Corpus::load(&options.corpus)?;
    let projects = corpus.select(options.tier, &options.projects)?;
    std::fs::create_dir_all(&options.out).map_err(|e| Error::prepare(e.to_string()))?;
    let out = options
        .out
        .canonicalize()
        .map_err(|e| Error::prepare(e.to_string()))?;
    let seed = options.cache.join("cargo-home");
    let mut packages = Vec::new();
    for project in &projects {
        let package = project
            .fetch(&options.cache, &options.repo_root)?
            .canonicalize()
            .map_err(|e| Error::prepare(e.to_string()))?;
        prepare_cargo_home(direct, &package, &seed)?;
        packages.push((*project, package));
    }
    let slots = make_slots(&out, &seed, options.jobs)?;
    let mut harnesses = Vec::new();
    let mut totals = BTreeMap::new();
    for (project, package) in &packages {
        // A project the shared argv cannot build is reported as blocked, not
        // compared: with the same argv both arms fail the same way.
        let found = match discover(
            direct,
            project,
            package,
            &slots[0],
            &out.join(format!("discover/{}", project.name)),
        ) {
            Ok(found) => found,
            Err(error) => {
                eprintln!("kani conformance: {} blocked: {error}", project.name);
                totals.insert(
                    project.name.clone(),
                    Totals {
                        blocked: Some(
                            error
                                .to_string()
                                .lines()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join(" "),
                        ),
                        ci_args_not_passed: project.ci_args.clone(),
                        ..Totals::default()
                    },
                );
                continue;
            }
        };
        let found: Vec<Harness> = found
            .into_iter()
            .filter(|h| {
                options
                    .filter
                    .as_ref()
                    .is_none_or(|f| h.name.contains(f.as_str()))
            })
            .collect();
        totals.insert(
            project.name.clone(),
            Totals {
                harnesses: found.len(),
                ci_args_not_passed: project.ci_args.clone(),
                ..Totals::default()
            },
        );
        harnesses.extend(found);
    }
    std::fs::write(
        out.join("harnesses.json"),
        serde_json::to_vec_pretty(&harnesses).map_err(|e| Error::arm(e.to_string()))?,
    )
    .map_err(|e| Error::arm(e.to_string()))?;
    let results = execute(options, &out, direct, sail, &slots, &harnesses)?;
    let report = report(options, sail.name(), totals, results);
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| Error::arm(e.to_string()))?,
    )
    .map_err(|e| Error::arm(e.to_string()))?;
    std::fs::write(out.join("summary.md"), summary(&report))
        .map_err(|e| Error::arm(e.to_string()))?;
    Ok(report)
}

/// Runs every (round, harness) over the slots in parallel.
fn execute(
    options: &Options,
    out: &Path,
    direct: &Direct,
    sail: &dyn SailSide,
    slots: &[Slot],
    harnesses: &[Harness],
) -> Result<Vec<(usize, HarnessResult)>, Error> {
    let queue: Mutex<VecDeque<(usize, &Harness)>> = Mutex::new(
        (1..=options.rounds)
            .flat_map(|round| harnesses.iter().map(move |h| (round, h)))
            .collect(),
    );
    let results: Mutex<Vec<(usize, HarnessResult)>> = Mutex::new(Vec::new());
    let failure: Mutex<Option<Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for slot in slots {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some((round, harness)) = next else { break };
                    match one(options, out, direct, sail, slot, round, harness) {
                        Ok(result) => {
                            eprintln!(
                                "kani conformance: round {round} {} {}: {:?}",
                                harness.project, harness.name, result.verdict
                            );
                            if let Ok(mut r) = results.lock() {
                                r.push((round, result));
                            }
                        }
                        Err(error) => {
                            if let Ok(mut f) = failure.lock() {
                                f.get_or_insert(error);
                            }
                        }
                    }
                }
            });
        }
    });
    if let Some(error) = failure.into_inner().ok().flatten() {
        return Err(error);
    }
    results
        .into_inner()
        .map_err(|_| Error::arm("poisoned results"))
}

fn report(
    options: &Options,
    sail_side: &'static str,
    mut totals: BTreeMap<String, Totals>,
    mut results: Vec<(usize, HarnessResult)>,
) -> Report {
    results
        .sort_by(|a, b| (a.0, &a.1.project, &a.1.harness).cmp(&(b.0, &b.1.project, &b.1.harness)));
    let mut flat: Vec<HarnessResult> = results.iter().map(|(_, r)| r.clone()).collect();
    let unwitnessed = witness(&mut flat);
    for ((_, slot), updated) in results.iter_mut().zip(flat) {
        *slot = updated;
    }
    for (_, r) in &results {
        if let Some(t) = totals.get_mut(&r.project) {
            *t.verdicts.entry(r.verdict).or_insert(0) += 1;
            let claim = serde_json::to_value(r.sail).map_or_else(
                |_| "?".into(),
                |v| {
                    v["reason"].as_str().map_or_else(
                        || v["kind"].as_str().unwrap_or("?").to_owned(),
                        |reason| format!("no-claim:{reason}"),
                    )
                },
            );
            *t.sail_claims.entry(claim).or_insert(0) += 1;
            t.checks += r.sail_against.last().map_or(0, |c| c.checks);
            for (name, n) in &r.direct_pair.variances {
                *t.direct_variances.entry(name.clone()).or_insert(0) += n;
            }
        }
    }
    Report {
        kind: "sail-kani-conformance",
        sail_side,
        tier: options.tier,
        rounds: options.rounds,
        direct_runs: options.direct_runs,
        projects: totals,
        unwitnessed: unwitnessed.into_iter().collect(),
        results,
    }
}

fn one(
    options: &Options,
    out: &Path,
    direct: &Direct,
    sail: &dyn SailSide,
    slot: &Slot,
    round: usize,
    harness: &Harness,
) -> Result<HarnessResult, Error> {
    let dir = out.join(format!(
        "runs/{round}/{}/{}",
        harness.project,
        safe(&harness.name)
    ));
    let mut directs = Vec::new();
    let first = direct.run(harness, slot, &dir.join("direct-1"))?;
    directs.push(first);
    let sail_record = sail.run(harness, slot)?;
    for n in 2..=options.direct_runs.max(2) {
        directs.push(direct.run(harness, slot, &dir.join(format!("direct-{n}")))?);
    }
    let result = classify(&directs, &sail_record);
    let records: Vec<&KaniRunRecord> = directs.iter().chain([&sail_record]).collect();
    std::fs::write(
        dir.join("records.json"),
        serde_json::to_vec(&records).map_err(|e| Error::arm(e.to_string()))?,
    )
    .map_err(|e| Error::arm(e.to_string()))?;
    if !options.keep_targets {
        for r in &records {
            let _ = std::fs::remove_dir_all(r.capture.join("target"));
        }
    }
    Ok(result)
}

/// The Markdown summary of a report.
#[must_use]
pub fn summary(report: &Report) -> String {
    use std::fmt::Write as _;
    let mut s = format!(
        "# Kani conformance ({:?} tier, {} rounds, {} direct runs per harness, Sail side `{}`)\n\n| Project | Harnesses | Verdicts (all rounds) | Sail claims | Checks compared | CI args not passed |\n|---|---|---|---|---|---|\n",
        report.tier, report.rounds, report.direct_runs, report.sail_side
    );
    for (name, t) in &report.projects {
        let verdicts: Vec<String> = t
            .verdicts
            .iter()
            .map(|(v, n)| format!("{v:?} {n}"))
            .collect();
        let claims: Vec<String> = t
            .sail_claims
            .iter()
            .map(|(c, n)| format!("{c} {n}"))
            .collect();
        let verdicts = t.blocked.as_ref().map_or_else(
            || verdicts.join(", "),
            |b| format!("blocked: {}", b.replace('|', "/")),
        );
        let _ = writeln!(
            s,
            "| {name} | {} | {} | {} | {} | {} |",
            t.harnesses,
            verdicts,
            claims.join(", "),
            t.checks,
            if t.ci_args_not_passed.is_empty() {
                "none".into()
            } else {
                format!("`{}`", t.ci_args_not_passed.join(" "))
            }
        );
    }
    let _ = writeln!(s, "\nUnwitnessed named fields: {:?}\n", report.unwitnessed);
    for (round, r) in &report.results {
        if r.verdict != KaniVerdict::Same {
            let _ = writeln!(
                s,
                "- round {round} {} `{}`: {:?}; {}",
                r.project,
                r.harness,
                r.verdict,
                r.notes.join("; ")
            );
            for c in r.sail_against.iter().chain([&r.direct_pair]) {
                for d in c.differences.iter().take(5) {
                    let _ = writeln!(s, "  - {d}");
                }
            }
        }
    }
    s
}
