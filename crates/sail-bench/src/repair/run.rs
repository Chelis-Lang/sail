//! Preparing and running the repair workload. Unix only: the arms talk to
//! owned `sail serve` daemons over Unix sockets.
//!
//! Every invocation's raw stdout, stderr, GNU `time` report and proof-journal
//! pages are kept under the run's output directory. The result file is
//! rewritten atomically after every sample, so an interrupted run loses at
//! most the sample in progress, and `--resume` continues it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::process::{Call, Env, OwnedDaemon, write_json};
use super::{
    Arm, ArmBinary, Catalogue, Cycle, FROZEN_SHA256, Phase, Project, RAW_WORKLOAD, RawFile,
    RawRecord, RepairFile, Sample, WORKLOAD, m6_facts, m7_facts, sha256_hex,
};
use crate::daemon::CacheRun;

/// Name of the per-project workload description written by [`prepare`].
pub const WORKLOAD_FILE: &str = ".sail-repair-workload.json";

/// File name to text.
pub type Texts = BTreeMap<String, String>;

/// What [`prepare`] freezes for one project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workload {
    /// Catalogue digest.
    pub catalogue_sha256: String,
    /// Project id.
    pub project: String,
    /// Pinned commit.
    pub commit: String,
    /// Original text of every edited file.
    pub originals: Texts,
    /// Full edited text per cycle id.
    pub versions: Texts,
}

fn checked(cmd: &mut Command) -> Result<String, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd:?} failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The original files and each cycle's full edited text, checking that
/// every edit matches exactly once in the text it starts from.
///
/// # Errors
///
/// A message naming the first cycle whose edit does not apply.
pub fn versions(
    cycles: &[Cycle],
    read: impl Fn(&str) -> Result<String, String>,
) -> Result<(Texts, Texts), String> {
    let mut originals = Texts::new();
    let mut versions = Texts::new();
    for c in cycles {
        if !originals.contains_key(&c.file) {
            originals.insert(c.file.clone(), read(&c.file)?);
        }
        let before = if c.from == "original" {
            originals.get(&c.file)
        } else {
            versions.get(&c.from)
        }
        .ok_or_else(|| format!("cycle {} starts from unknown `{}`", c.id, c.from))?;
        if before.matches(c.find.as_str()).count() != 1 {
            return Err(format!(
                "edit of cycle {} does not match exactly once",
                c.id
            ));
        }
        let after = before.replacen(c.find.as_str(), &c.replace, 1);
        versions.insert(c.id.clone(), after);
    }
    Ok((originals, versions))
}

/// Pins the single `vstd = { version = "=…"` requirement of a manifest to
/// `version` and patches it to the local `vstd` source.
///
/// # Errors
///
/// A message unless exactly one such requirement exists and no
/// `[patch.crates-io]` section does.
pub fn patch_manifest(text: &str, version: &str, vstd: &Path) -> Result<String, String> {
    const NEEDLE: &str = "vstd = { version = \"=";
    const BAD: &str = "unexpected Vest dependency manifest";
    if text.matches(NEEDLE).count() != 1 || text.contains("[patch.crates-io]") {
        return Err(BAD.to_owned());
    }
    let (head, rest) = text.split_once(NEEDLE).ok_or(BAD)?;
    let (_, tail) = rest.split_once('"').ok_or(BAD)?;
    let quoted = serde_json::to_string(&vstd.display().to_string()).map_err(|e| e.to_string())?;
    Ok(format!(
        "{head}{NEEDLE}{version}\"{tail}\n[patch.crates-io]\nvstd = {{ path = {quoted} }}\n"
    ))
}

/// The generated library package wrapping human-eval-verus.
fn wrapper_manifest(package: &str, root: &str, vstd: &Path) -> Result<String, String> {
    let quoted = serde_json::to_string(&vstd.display().to_string()).map_err(|e| e.to_string())?;
    Ok(format!(
        "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[lib]\npath = \"{root}\"\n[package.metadata.verus]\nverify = true\n[workspace]\n[dependencies]\nvstd = {{ path = {quoted} }}\n"
    ))
}

/// M6 selects the package (its managed `--target-dir` cannot follow
/// `--lib`); this holds the scope equal to M7's `--lib` selection.
fn check_single_library(metadata: &str, package: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(metadata).map_err(|e| e.to_string())?;
    let packages: Vec<&Value> = v
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| p.get("name").and_then(Value::as_str) == Some(package))
        .collect();
    let [p] = packages.as_slice() else {
        return Err(format!("{} packages named {package}", packages.len()));
    };
    // Cargo's default package selection is the library and the binaries;
    // test, bench and example targets are not selected either way.
    let count = |kind: &str| {
        p.get("targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|t| {
                t.get("kind")
                    .and_then(Value::as_array)
                    .is_some_and(|k| k.iter().any(|x| x.as_str() == Some(kind)))
            })
            .count()
    };
    if count("lib") == 1 && count("bin") == 0 {
        Ok(())
    } else {
        Err(format!(
            "package {package} has {} library and {} binary targets; one library and no binary is required",
            count("lib"),
            count("bin")
        ))
    }
}

/// Inputs to [`prepare`].
pub struct PrepareOptions<'a> {
    /// The frozen catalogue.
    pub catalogue: &'a Catalogue,
    /// Fetches a project's pinned corpus checkout.
    pub corpus_dir: &'a dyn Fn(&Project) -> Result<PathBuf, String>,
    /// The pinned Verus source checkout.
    pub verus_source: &'a Path,
    /// Destination; one fresh directory per project.
    pub scratch: &'a Path,
    /// Verus's Rust toolchain.
    pub rust_toolchain: &'a str,
}

fn git_head(dir: &Path) -> Result<String, String> {
    checked(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"]),
    )
    .map(|s| s.trim().to_owned())
}

/// Creates fresh scratch copies of the catalogue projects from committed
/// objects, with their Cargo setup, and freezes each cycle's text.
///
/// # Errors
///
/// A message on the first failure; nothing is measured.
pub fn prepare(o: &PrepareOptions) -> Result<Vec<PathBuf>, String> {
    if git_head(o.verus_source)? != o.catalogue.verus_commit {
        return Err("Verus source has the wrong commit".to_owned());
    }
    let vstd = o.verus_source.join("source/vstd");
    let manifest: toml::Value = std::fs::read_to_string(vstd.join("Cargo.toml"))
        .map_err(|e| e.to_string())
        .and_then(|t| toml::from_str(&t).map_err(|e| e.to_string()))?;
    let version = manifest
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or("vstd has no package version")?
        .to_owned();
    let mut prepared = Vec::new();
    for project in &o.catalogue.projects {
        let fetched = (o.corpus_dir)(project)?;
        if git_head(&fetched)? != project.commit {
            return Err(format!("{} checkout has the wrong commit", project.id));
        }
        let dest = o.scratch.join(&project.id);
        if dest.exists() {
            return Err(format!("{} already exists", dest.display()));
        }
        std::fs::create_dir_all(o.scratch).map_err(|e| e.to_string())?;
        // Committed objects only: never a prior experiment's worktree edits.
        checked(
            Command::new("git")
                .args(["clone", "--local", "--no-hardlinks", "--quiet"])
                .arg(&fetched)
                .arg(&dest),
        )?;
        checked(Command::new("git").arg("-C").arg(&dest).args([
            "checkout",
            "--quiet",
            "--detach",
            &project.commit,
        ]))?;
        let cargo = dest.join("Cargo.toml");
        let text = if project.cargo_wrapper {
            wrapper_manifest(&project.package, &project.root, &vstd)?
        } else {
            let original = std::fs::read_to_string(&cargo).map_err(|e| e.to_string())?;
            patch_manifest(&original, &version, &vstd)?
        };
        std::fs::write(&cargo, text).map_err(|e| e.to_string())?;
        let metadata = checked(
            Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(&dest)
                .env("RUSTUP_TOOLCHAIN", o.rust_toolchain),
        )?;
        check_single_library(&metadata, &project.package)?;
        let (originals, versions) = versions(&project.cycles, |file| {
            std::fs::read_to_string(dest.join(file)).map_err(|e| format!("{file}: {e}"))
        })?;
        let workload = Workload {
            catalogue_sha256: FROZEN_SHA256.to_owned(),
            project: project.id.clone(),
            commit: project.commit.clone(),
            originals,
            versions,
        };
        write_json(&dest.join(WORKLOAD_FILE), &workload)?;
        prepared.push(dest);
    }
    Ok(prepared)
}

/// Options of a measured run.
pub struct RunOptions {
    /// M6 `sail` binary.
    pub m6: PathBuf,
    /// M7 `sail` binary.
    pub m7: PathBuf,
    /// M6 and M7 source commits, as supplied.
    pub commits: [String; 2],
    /// Validated engine runtime installed into the M7 cache.
    pub engine: PathBuf,
    /// Prepared projects (from [`prepare`]).
    pub scratch: PathBuf,
    /// Raw evidence and caches.
    pub out: PathBuf,
    /// The compact result file (`bench/results/NAME.json`).
    pub result: PathBuf,
    /// The raw sidecar, kept outside the repository.
    pub raw: PathBuf,
    /// Measured rounds.
    pub rounds: usize,
    /// Per-invocation deadline.
    pub timeout: Duration,
    /// Execution environment label.
    pub environment: String,
    /// Projects to run (empty: all).
    pub projects: Vec<String>,
    /// Cycle-id substring filter.
    pub filter: Option<String>,
    /// Continue an existing result file.
    pub resume: bool,
    /// Verus's Rust toolchain.
    pub rust_toolchain: String,
    /// Pinned Verus release.
    pub verus_release: String,
    /// Pinned Z3 version.
    pub z3_version: String,
    /// Run start, Unix seconds.
    pub started_unix: u64,
    /// Machine description.
    pub machine: BTreeMap<String, String>,
}

fn file_sha(path: &Path) -> Result<String, String> {
    std::fs::read(path)
        .map(|b| sha256_hex(&b))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn arm_binary(sail: &Path, commit: &str) -> Result<ArmBinary, String> {
    let shim = sail.with_file_name("sail-z3");
    Ok(ArmBinary {
        commit: commit.to_owned(),
        sail_sha256: file_sha(sail)?,
        shim_sha256: shim.is_file().then(|| file_sha(&shim)).transpose()?,
    })
}

/// The run's mutable state: the arms' daemons and the result file.
struct Session<'a> {
    o: &'a RunOptions,
    out: PathBuf,
    binaries: [PathBuf; 2],
    envs: [Env; 2],
    daemons: [Option<OwnedDaemon>; 2],
    file: RepairFile,
    raw: RawFile,
}

fn idx(arm: Arm) -> usize {
    match arm {
        Arm::M6 => 0,
        Arm::M7 => 1,
    }
}

/// What one arm runs for a job.
#[derive(Clone, Copy, PartialEq, Eq)]
enum What {
    /// The measured arm: M6 focused then full, or one M7 repair batch.
    Cycle,
    /// M6 focused verification only (context warm-up).
    FocusOnly,
    /// A plain full `verify --incremental` (dependency warm-up).
    FullVerify,
}

/// One arm's measured or warm-up invocation group.
struct Job<'a> {
    project: &'a Project,
    workspace: &'a Path,
    cycle: &'a str,
    kind: &'a str,
    function: &'a str,
    expected: &'a str,
    round: Option<usize>,
    position: usize,
    raw: PathBuf,
}

fn split_function(function: &str) -> (String, String) {
    let relative = function.split_once("::").map_or(function, |(_, r)| r);
    relative.rsplit_once("::").map_or_else(
        || (String::new(), relative.to_owned()),
        |(m, f)| (m.to_owned(), f.to_owned()),
    )
}

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| (*s).to_owned()).collect()
}

/// The invocations (name, arguments) an arm runs for a job.
fn plan(arm: Arm, what: What, job: &Job) -> Vec<(&'static str, Vec<String>)> {
    let package = ["--package", job.project.package.as_str()];
    let mut verify = strings(&["verify", "--incremental", "--output", "json"]);
    verify.extend(strings(&package));
    let (module, leaf) = split_function(job.function);
    let mut focus = verify.clone();
    focus.extend(strings(&[
        "--",
        "--verify-only-module",
        &module,
        "--verify-function",
        &leaf,
    ]));
    match (what, arm) {
        (What::FullVerify, _) => vec![("verify", verify)],
        (What::FocusOnly, _) => vec![("focus", focus)],
        (What::Cycle, Arm::M6) => vec![("focus", focus), ("full", verify)],
        (What::Cycle, Arm::M7) => {
            let mut repair = strings(&[
                "repair-check",
                "--engine",
                "interactive",
                "--function",
                job.function,
                "--output",
                "json",
            ]);
            repair.extend(strings(&package));
            repair.push("--lib".to_owned());
            vec![("repair", repair)]
        }
    }
}

impl Session<'_> {
    /// The arm's running daemon, (re)starting it if needed.
    fn daemon(&mut self, arm: Arm) -> Result<&OwnedDaemon, String> {
        let i = idx(arm);
        let slot = self.daemons.get_mut(i).ok_or("no daemon slot")?;
        let had = slot.is_some();
        if !slot.as_mut().is_some_and(OwnedDaemon::alive) {
            *slot = None;
            let env = self.envs.get(i).ok_or("no environment")?;
            let binary = self.binaries.get(i).ok_or("no binary")?;
            let log = self.out.join(format!("daemon-{}.log", arm.name()));
            *slot = Some(OwnedDaemon::start(binary, env, &log)?);
            if had {
                self.file
                    .notes
                    .push(format!("{} daemon exited and was restarted", arm.name()));
            }
        }
        slot.as_ref().ok_or_else(|| "daemon unavailable".to_owned())
    }

    /// Runs one arm of a job and returns its sample. Never fails.
    fn run_arm(&mut self, arm: Arm, what: What, job: &Job) -> Sample {
        let mut sample = Sample {
            project: job.project.id.clone(),
            cycle: job.cycle.to_owned(),
            kind: job.kind.to_owned(),
            function: job.function.to_owned(),
            expected: job.expected.to_owned(),
            arm: Some(arm),
            round: job.round,
            position: job.position,
            raw_dir: job
                .raw
                .strip_prefix(&self.out)
                .unwrap_or(&job.raw)
                .display()
                .to_string(),
            ..Sample::default()
        };
        let timeout = self.o.timeout;
        let (binary, env) = match (self.binaries.get(idx(arm)), self.envs.get(idx(arm))) {
            (Some(b), Some(e)) => (b.clone(), e.clone()),
            _ => return sample,
        };
        let daemon = match self.daemon(arm) {
            Ok(d) => d,
            Err(e) => {
                sample.phases.push(Phase {
                    name: "daemon".to_owned(),
                    error: Some(e),
                    ..Phase::default()
                });
                return sample;
            }
        };
        let before = daemon.stats();
        let steps = plan(arm, what, job);
        let several = steps.len() > 1;
        let mut documents = Vec::new();
        for (name, args) in steps {
            let r = Call {
                name,
                binary: &binary,
                args,
                cwd: job.workspace,
                env: &env,
                daemon,
                dir: if several {
                    job.raw.join(name)
                } else {
                    job.raw.clone()
                },
                timeout,
                observe: name == "repair",
            }
            .invoke();
            sample.peak_rss_bytes = sample.peak_rss_bytes.max(r.peak_rss);
            sample.first_actionable_ms = match name {
                // M6's first actionable evidence is its focused result.
                "focus" => Some(r.phase.wall_ms),
                "repair" => r.first_actionable_ms,
                _ => sample.first_actionable_ms,
            };
            sample.phases.push(r.phase);
            documents.push(r.document);
        }
        let first = documents.first().and_then(Option::as_ref);
        let last = documents.last().and_then(Option::as_ref);
        match (arm, what) {
            (Arm::M7, What::Cycle) => {
                let (facts, preparation) = m7_facts(last);
                sample.facts = facts;
                sample.preparation = preparation;
            }
            (_, What::FullVerify) => sample.facts = m6_facts(None, last),
            _ => sample.facts = m6_facts(first, last),
        }
        sample.wall_ms = sample.phases.iter().map(|p| p.wall_ms).sum();
        if let (Some(b), Some(a)) = (before, daemon.stats()) {
            sample.cache = Some(CacheRun::between("sample", &b, &a));
        }
        sample
    }

    /// Records a sample, then persists the raw sidecar and the compact
    /// result naming its digest.
    fn record(&mut self, s: Sample) -> Result<(), String> {
        progress(&s);
        let record = if s.round.is_some() {
            self.file.samples.push(s);
            self.file
                .samples
                .last()
                .map(|s| RawRecord::of("sample", self.file.samples.len() - 1, s))
        } else {
            self.file.warmups.push(s);
            self.file
                .warmups
                .last()
                .map(|s| RawRecord::of("warmup", self.file.warmups.len() - 1, s))
        };
        self.raw.records.extend(record);
        self.persist()
    }

    fn persist(&mut self) -> Result<(), String> {
        self.file.raw_sha256 = Some(write_json(&self.o.raw, &self.raw)?);
        write_json(&self.o.result, &self.file).map(|_| ())
    }

    /// `engine install` of the validated runtime into the M7 cache, before
    /// its daemon starts.
    fn install_engine(&mut self, dir: &Path) -> Result<(), String> {
        let raw = dir.join("engine-install");
        let mut phase = Phase {
            name: "engine-install".to_owned(),
            args: vec![
                "engine".to_owned(),
                "install".to_owned(),
                "--from".to_owned(),
                self.o.engine.display().to_string(),
                "--output".to_owned(),
                "json".to_owned(),
            ],
            ..Phase::default()
        };
        let started = Instant::now();
        let mut cmd = Command::new(&self.o.m7);
        cmd.args(&phase.args);
        if let Some(env) = self.envs.get(idx(Arm::M7)) {
            env.apply(&mut cmd);
        }
        match cmd.output() {
            Ok(output) => {
                let _ = std::fs::create_dir_all(&raw);
                let _ = std::fs::write(raw.join("stdout.json"), &output.stdout);
                let _ = std::fs::write(raw.join("stderr.txt"), &output.stderr);
                phase.exit_code = output.status.code();
                if !output.status.success() {
                    phase.error = Some(String::from_utf8_lossy(&output.stderr).trim().to_owned());
                }
            }
            Err(e) => phase.error = Some(e.to_string()),
        }
        phase.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let sample = Sample {
            cycle: "engine".to_owned(),
            kind: "warm-up: engine install".to_owned(),
            arm: Some(Arm::M7),
            wall_ms: phase.wall_ms,
            phases: vec![phase],
            raw_dir: raw
                .strip_prefix(&self.out)
                .unwrap_or(&raw)
                .display()
                .to_string(),
            ..Sample::default()
        };
        self.record(sample)
    }

    /// Dependency and solver warm-up per arm, then each distinct function's
    /// focused (M6) and repair (M7) context on the original source.
    /// Recorded, never gated; a failure is kept and the run continues.
    fn warm_up(
        &mut self,
        project: &Project,
        workspace: &Path,
        cycles: &[&Cycle],
        dir: &Path,
    ) -> Result<(), String> {
        let base = Job {
            project,
            workspace,
            cycle: "project",
            kind: "warm-up: dependencies and solver cache",
            function: "",
            expected: "",
            round: None,
            position: 0,
            raw: PathBuf::new(),
        };
        for arm in Arm::BOTH {
            let job = Job {
                raw: dir.join(format!("dependencies-{}", arm.name())),
                ..base
            };
            let s = self.run_arm(arm, What::FullVerify, &job);
            self.record(s)?;
        }
        let mut seen = BTreeSet::new();
        let functions: Vec<&str> = cycles
            .iter()
            .map(|c| c.function.as_str())
            .filter(|f| seen.insert(*f))
            .collect();
        for (i, function) in functions.into_iter().enumerate() {
            let job = Job {
                cycle: function,
                kind: "warm-up: focused context",
                function,
                raw: dir.join(format!("focus-{i}")),
                ..base
            };
            let s = self.run_arm(Arm::M6, What::FocusOnly, &job);
            self.record(s)?;
            // A first repair run can establish build-script declarations
            // and end non-Current; one more attempt is allowed.
            for attempt in 0..2 {
                let job = Job {
                    cycle: function,
                    kind: "warm-up: repair context",
                    function,
                    raw: dir.join(format!("repair-{i}-{attempt}")),
                    ..base
                };
                let s = self.run_arm(Arm::M7, What::Cycle, &job);
                let ready = s.facts.accepted == Some(true)
                    && s.facts.full_currentness.as_deref() == Some("current");
                self.record(s)?;
                if ready {
                    break;
                }
            }
        }
        Ok(())
    }

    fn done(&self, project: &str, round: usize, cycle: &str, arm: Arm) -> bool {
        self.file.samples.iter().any(|s| {
            s.project == project && s.round == Some(round) && s.cycle == cycle && s.arm == Some(arm)
        })
    }

    /// Warm-up and every measured round of one project.
    fn run_project(&mut self, project: &Project, warm_dir: &Path) -> Result<(), String> {
        let workspace = self.o.scratch.join(&project.id);
        let w: Workload = std::fs::read_to_string(workspace.join(WORKLOAD_FILE))
            .map_err(|e| {
                format!(
                    "{}: {e} (run `sail-bench repair prepare`)",
                    workspace.display()
                )
            })
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))?;
        if w.catalogue_sha256 != FROZEN_SHA256 || w.commit != project.commit {
            return Err(format!(
                "{} was prepared for another catalogue",
                workspace.display()
            ));
        }
        let filter = self.o.filter.clone();
        let cycles: Vec<&Cycle> = project
            .cycles
            .iter()
            .filter(|c| filter.as_ref().is_none_or(|x| c.id.contains(x.as_str())))
            .collect();
        apply(&workspace, &w, None)?;
        self.warm_up(project, &workspace, &cycles, &warm_dir.join(&project.id))?;
        for round in 0..self.file.rounds {
            for cycle in &cycles {
                if Arm::BOTH
                    .iter()
                    .all(|a| self.done(&project.id, round, &cycle.id, *a))
                {
                    continue;
                }
                let digest = apply(&workspace, &w, Some(cycle))?;
                for (position, arm) in Arm::order(round).into_iter().enumerate() {
                    if self.done(&project.id, round, &cycle.id, arm) {
                        continue;
                    }
                    let job = Job {
                        project,
                        workspace: &workspace,
                        cycle: &cycle.id,
                        kind: &cycle.kind,
                        function: &cycle.function,
                        expected: &cycle.expected,
                        round: Some(round),
                        position,
                        raw: self
                            .out
                            .join(&project.id)
                            .join(round.to_string())
                            .join(&cycle.id)
                            .join(arm.name()),
                    };
                    let _ = std::fs::remove_dir_all(&job.raw);
                    let mut s = self.run_arm(arm, What::Cycle, &job);
                    s.source_sha256.clone_from(&digest);
                    self.record(s)?;
                }
            }
        }
        apply(&workspace, &w, None).map(|_| ())
    }
}

/// Writes every edited file's original, then the cycle's edited text.
fn apply(workspace: &Path, w: &Workload, cycle: Option<&Cycle>) -> Result<Option<String>, String> {
    for (file, text) in &w.originals {
        std::fs::write(workspace.join(file), text).map_err(|e| format!("{file}: {e}"))?;
    }
    let Some(c) = cycle else { return Ok(None) };
    let text = w
        .versions
        .get(&c.id)
        .ok_or_else(|| format!("no frozen text for {}", c.id))?;
    std::fs::write(workspace.join(&c.file), text).map_err(|e| format!("{}: {e}", c.file))?;
    Ok(Some(sha256_hex(text.as_bytes())))
}

fn progress(s: &Sample) {
    let codes: Vec<&str> = super::judge::violations(s).iter().map(|v| v.code).collect();
    let verdict = if s.round.is_none() {
        let failed = s
            .phases
            .iter()
            .any(|p| p.error.is_some() || p.timed_out || p.exit_code.is_none());
        if failed { "infra failure" } else { "recorded" }.to_owned()
    } else if codes.is_empty() {
        "ok".to_owned()
    } else {
        codes.join(",")
    };
    eprintln!(
        "{:<18} {:<8} {:<40} {} {:>9.0} ms  first actionable {:>9}  {verdict}",
        s.project,
        s.round
            .map_or_else(|| "warm-up".to_owned(), |r| format!("round {r}")),
        s.cycle,
        s.arm.map_or("--", Arm::name),
        s.wall_ms,
        s.first_actionable_ms
            .map_or_else(|| "n/a".to_owned(), |x| format!("{x:.0} ms")),
    );
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Loads the raw sidecar when resuming, noting a missing or changed one.
fn open_raw(o: &RunOptions, f: &mut RepairFile) -> Result<RawFile, String> {
    let fresh = RawFile {
        schema_version: 1,
        workload: RAW_WORKLOAD.to_owned(),
        result: file_name(&o.result),
        records: Vec::new(),
    };
    if !o.resume || !o.raw.exists() {
        if o.resume && !f.samples.is_empty() {
            f.notes.push(format!(
                "raw sidecar {} was missing at resumption; it holds only later records",
                file_name(&o.raw)
            ));
        }
        return Ok(fresh);
    }
    let bytes = std::fs::read(&o.raw).map_err(|e| format!("{}: {e}", o.raw.display()))?;
    if f.raw_sha256.as_deref() != Some(sha256_hex(&bytes).as_str()) {
        f.notes.push(format!(
            "raw sidecar {} differed from its recorded digest at resumption",
            file_name(&o.raw)
        ));
    }
    let raw: RawFile = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if raw.workload != RAW_WORKLOAD {
        return Err(format!("{} is not a repair raw file", o.raw.display()));
    }
    Ok(raw)
}

/// Loads or creates the compact result file.
fn open_result(
    o: &RunOptions,
    catalogue: &Catalogue,
    projects: &[String],
) -> Result<RepairFile, String> {
    let [m6_commit, m7_commit] = &o.commits;
    let arms = BTreeMap::from([
        (Arm::M6, arm_binary(&o.m6, m6_commit)?),
        (Arm::M7, arm_binary(&o.m7, m7_commit)?),
    ]);
    if o.result.exists() {
        if !o.resume {
            return Err(format!(
                "{} exists; pass --resume to continue it",
                o.result.display()
            ));
        }
        let text = std::fs::read_to_string(&o.result).map_err(|e| e.to_string())?;
        let mut f: RepairFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if f.arms != arms || f.catalogue_sha256 != FROZEN_SHA256 || f.cycle_filter != o.filter {
            return Err(
                "the existing result file was made with other binaries, catalogue or filter"
                    .to_owned(),
            );
        }
        f.rounds = f.rounds.max(o.rounds);
        f.notes
            .push(format!("resumed at Unix time {}", o.started_unix));
        return Ok(f);
    }
    if o.raw.exists() && !o.resume {
        return Err(format!(
            "{} exists; pass --resume to continue it",
            o.raw.display()
        ));
    }
    Ok(RepairFile {
        schema_version: 2,
        raw_file: Some(file_name(&o.raw)),
        workload: WORKLOAD.to_owned(),
        environment: o.environment.clone(),
        catalogue_sha256: FROZEN_SHA256.to_owned(),
        minimum_rounds: catalogue.minimum_rounds,
        minimum_median_speedup: catalogue.minimum_median_speedup,
        rounds: o.rounds,
        catalogue: catalogue.entries(),
        projects: projects.to_vec(),
        cycle_filter: o.filter.clone(),
        arms,
        verus_release: o.verus_release.clone(),
        z3_version: o.z3_version.clone(),
        started_unix: o.started_unix,
        machine: o.machine.clone(),
        ..RepairFile::default()
    })
}

/// Runs the workload, recording every sample.
///
/// # Errors
///
/// Only for setup that makes measuring impossible (unreadable catalogue or
/// workspace, an existing result without `--resume`, an unwritable result
/// file). Failing invocations are recorded, never returned.
pub fn run(o: &RunOptions, catalogue: &Catalogue) -> Result<RepairFile, String> {
    let selected: Vec<&Project> = catalogue
        .projects
        .iter()
        .filter(|p| o.projects.is_empty() || o.projects.contains(&p.id))
        .collect();
    let names: Vec<String> = selected.iter().map(|p| p.id.clone()).collect();
    let mut file = open_result(o, catalogue, &names)?;
    let raw = open_raw(o, &mut file)?;
    std::fs::create_dir_all(&o.out).map_err(|e| e.to_string())?;
    let out = o.out.canonicalize().map_err(|e| e.to_string())?;
    let tag = sha256_hex(out.as_os_str().as_encoded_bytes());
    let env = |arm: Arm| -> Result<Env, String> {
        let cache = out.join(format!("cache-{}", arm.name()));
        std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        Ok(Env {
            cache,
            socket: PathBuf::from(format!(
                "/tmp/sail-rb-{}-{}.sock",
                tag.get(..10).unwrap_or(&tag),
                arm.name()
            )),
            rust_toolchain: o.rust_toolchain.clone(),
        })
    };
    let warm_dir = out.join(format!("session-{}", file.notes.len()));
    let mut session = Session {
        o,
        binaries: [o.m6.clone(), o.m7.clone()],
        envs: [env(Arm::M6)?, env(Arm::M7)?],
        daemons: [None, None],
        out,
        file,
        raw,
    };
    session.install_engine(&warm_dir)?;
    for project in selected {
        session.run_project(project, &warm_dir)?;
    }
    session.persist()?;
    Ok(session.file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cycle(id: &str, from: &str, find: &str, replace: &str) -> Cycle {
        Cycle {
            id: id.to_owned(),
            kind: "body".to_owned(),
            function: "c::m::f".to_owned(),
            file: "a.rs".to_owned(),
            expected: "verified".to_owned(),
            from: from.to_owned(),
            find: find.to_owned(),
            replace: replace.to_owned(),
        }
    }

    #[test]
    fn versions_chain_and_refuse_ambiguous_edits() {
        let read = |_: &str| Ok("x + 1\n".to_owned());
        let cycles = [
            cycle("fail", "original", "+ 1", "+ 2"),
            cycle("fix", "fail", "+ 2", "+ 1"),
        ];
        let r = versions(&cycles, read);
        let Ok((originals, versions)) = r else {
            unreachable!("{r:?}")
        };
        assert_eq!(originals.get("a.rs").map(String::as_str), Some("x + 1\n"));
        assert_eq!(versions.get("fail").map(String::as_str), Some("x + 2\n"));
        assert_eq!(versions.get("fix").map(String::as_str), Some("x + 1\n"));
        let ambiguous = [cycle("a", "original", "x", "y")];
        assert!(super::versions(&ambiguous, |_| Ok("x x".to_owned())).is_err());
        let unknown = [cycle("a", "nope", "x", "y")];
        assert!(super::versions(&unknown, |_| Ok("x".to_owned())).is_err());
    }

    #[test]
    fn vest_manifest_patch() {
        let text =
            "[dependencies]\nvstd = { version = \"=0.0.0-2025\", default-features = false }\n";
        let r = patch_manifest(text, "0.0.0-2026", Path::new("/v/vstd"));
        let Ok(r) = r else { unreachable!("{r:?}") };
        assert!(
            r.contains("vstd = { version = \"=0.0.0-2026\", default-features = false }"),
            "{r}"
        );
        assert!(
            r.ends_with("[patch.crates-io]\nvstd = { path = \"/v/vstd\" }\n"),
            "{r}"
        );
        assert!(patch_manifest(&format!("{text}{text}"), "1", Path::new("/v")).is_err());
        assert!(patch_manifest("[patch.crates-io]\n", "1", Path::new("/v")).is_err());
    }

    #[test]
    fn function_split_matches_verus_filters() {
        assert_eq!(
            split_function("human_eval_verus::human_eval_013::compute_gcd"),
            ("human_eval_013".to_owned(), "compute_gcd".to_owned())
        );
        assert_eq!(
            split_function("vest_lib::primitives::base128::lemma"),
            ("primitives::base128".to_owned(), "lemma".to_owned())
        );
    }

    #[test]
    fn plans_match_the_protocol() {
        let project = Project {
            id: "p".to_owned(),
            commit: String::new(),
            package: "pkg".to_owned(),
            target: "pkg".to_owned(),
            target_kind: "lib".to_owned(),
            root: String::new(),
            cargo_wrapper: false,
            cycles: Vec::new(),
        };
        let job = Job {
            project: &project,
            workspace: Path::new("/"),
            cycle: "c",
            kind: "body",
            function: "pkg::m::f",
            expected: "verified",
            round: Some(0),
            position: 0,
            raw: PathBuf::new(),
        };
        let m6 = plan(Arm::M6, What::Cycle, &job);
        let names: Vec<&str> = m6.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["focus", "full"]);
        assert_eq!(
            m6.first().map(|(_, a)| a.join(" ")).as_deref(),
            Some(
                "verify --incremental --output json --package pkg -- --verify-only-module m --verify-function f"
            )
        );
        assert_eq!(
            m6.get(1).map(|(_, a)| a.join(" ")).as_deref(),
            Some("verify --incremental --output json --package pkg")
        );
        let m7 = plan(Arm::M7, What::Cycle, &job);
        assert_eq!(
            m7.first().map(|(_, a)| a.join(" ")).as_deref(),
            Some(
                "repair-check --engine interactive --function pkg::m::f --output json --package pkg --lib"
            )
        );
    }

    fn options(dir: &Path, resume: bool) -> RunOptions {
        RunOptions {
            m6: PathBuf::from("/bin/sh"),
            m7: PathBuf::from("/bin/sh"),
            commits: ["a".to_owned(), "b".to_owned()],
            engine: PathBuf::new(),
            scratch: PathBuf::new(),
            out: dir.to_path_buf(),
            result: dir.join("r.json"),
            raw: dir.join("r.raw.json"),
            rounds: 1,
            timeout: Duration::from_secs(1),
            environment: "test".to_owned(),
            projects: Vec::new(),
            filter: None,
            resume,
            rust_toolchain: String::new(),
            verus_release: String::new(),
            z3_version: String::new(),
            started_unix: 0,
            machine: BTreeMap::new(),
        }
    }

    #[test]
    fn resume_reads_the_compact_result_and_its_raw_sidecar() {
        let dir = std::env::temp_dir().join(format!("sail-bench-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/repair-edits.json");
        let Ok(catalogue) = Catalogue::load(&path) else {
            unreachable!()
        };
        let fresh = options(&dir, false);
        let Ok(mut file) = open_result(&fresh, &catalogue, &[]) else {
            unreachable!()
        };
        assert_eq!(file.raw_file.as_deref(), Some("r.raw.json"));
        let Ok(mut raw) = open_raw(&fresh, &mut file) else {
            unreachable!()
        };
        let sample = Sample {
            round: Some(0),
            phases: vec![Phase {
                args: vec!["x".to_owned()],
                ..Phase::default()
            }],
            ..Sample::default()
        };
        file.samples.push(sample.clone());
        raw.records.push(RawRecord::of("sample", 0, &sample));
        file.raw_sha256 = write_json(&fresh.raw, &raw).ok();
        assert!(write_json(&fresh.result, &file).is_ok());
        // Without --resume, neither file is overwritten.
        assert!(open_result(&fresh, &catalogue, &[]).is_err());

        let resumed = options(&dir, true);
        let Ok(mut again) = open_result(&resumed, &catalogue, &[]) else {
            unreachable!()
        };
        assert_eq!(again.samples.len(), 1);
        let Ok(back) = open_raw(&resumed, &mut again) else {
            unreachable!()
        };
        assert_eq!(
            back.records.first().map(|r| r.args.clone()),
            Some(vec![vec!["x".to_owned()]])
        );
        assert!(!again.notes.iter().any(|n| n.contains("differed")));

        let _ = std::fs::write(&resumed.raw, b"{\"workload\": \"repair-raw\"}");
        let mut third = again.clone();
        assert!(open_raw(&resumed, &mut third).is_ok());
        assert!(third.notes.iter().any(|n| n.contains("differed")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_library_check() {
        let one = r#"{"packages":[{"name":"p","targets":[{"kind":["lib"]}]}]}"#;
        assert!(check_single_library(one, "p").is_ok());
        let two = r#"{"packages":[{"name":"p","targets":[{"kind":["lib"]},{"kind":["bin"]}]}]}"#;
        assert!(check_single_library(two, "p").is_err());
        assert!(check_single_library(one, "q").is_err());
        // Vest's `vest_lib`: a test target is outside default selection.
        let vest = r#"{"packages":[{"name":"p","targets":[{"kind":["lib"]},{"kind":["test"]}]}]}"#;
        assert!(check_single_library(vest, "p").is_ok());
        let none = r#"{"packages":[{"name":"p","targets":[{"kind":["test"]}]}]}"#;
        assert!(check_single_library(none, "p").is_err());
    }
}
