//! The history-replay workload (charter: "History replay").
//!
//! It replays a project's last `N` first-parent commits, ending at its
//! pinned commit, oldest first, the way a developer or CI machine meets
//! them: one `sail serve` whose cache persists across all commits. Every
//! commit runs plain Verus and Verus through `sail-z3`, and every sail run is
//! judged against plain Verus. Every commit is verified with the pinned
//! Verus. Commits written for another Verus release may fail under it; their
//! outcome is recorded as observed, and conformance stays differential.
//!
//! The checkout lives at a fixed scratch path and units are prepared into a
//! fixed directory, so file paths do not change between commits; the
//! content-addressed keys see only what really changed.

use std::path::{Path, PathBuf};
use std::process::Command;

use sail_verus::corpus::{Project, Unit};
use sail_verus::run::{RunRecord, Solver, prepare_unit, run_unit};
use sail_verus::toolchain::Installed;
use serde::{Deserialize, Serialize};

use crate::cold::{Judgement, Samples, judge};
use crate::daemon::{CacheRun, Daemon};
use crate::edit::outcome;
use crate::warm::Setup;

/// One commit's results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitResult {
    /// Full SHA.
    pub sha: String,
    /// First line of the commit message.
    pub subject: String,
    /// Unit id.
    pub unit: String,
    /// Plain Verus's outcome.
    pub outcome: Option<String>,
    /// Plain Verus.
    pub plain: Samples,
    /// sail with the cache carried over from earlier commits.
    pub sail: Samples,
    /// Cache counters for the sail run.
    pub cache: Vec<CacheRun>,
    /// Same as plain Verus.
    pub same: usize,
    /// Different from plain Verus (a bug).
    pub different: usize,
    /// Plain Verus varied and sail matched one run.
    pub unstable: usize,
    /// The harness could not run it (checkout, build setup, or a run).
    pub infra_errors: usize,
    /// Why, for an infra error.
    pub note: Option<String>,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Fetches enough history to list the last `count` first-parent commits up
/// to `project.commit` into `repo`, and returns them oldest first with their
/// subjects.
///
/// # Errors
///
/// A message if git fails.
pub fn history(
    project: &Project,
    repo: &Path,
    count: usize,
) -> Result<Vec<(String, String)>, String> {
    std::fs::create_dir_all(repo).map_err(|e| e.to_string())?;
    if !repo.join(".git").is_dir() {
        git(repo, &["init", "-q"])?;
        git(repo, &["remote", "add", "origin", &project.url])?;
    }
    let depth = (count + 1).to_string();
    git(
        repo,
        &["fetch", "-q", "--depth", &depth, "origin", &project.commit],
    )?;
    let n = count.to_string();
    let log = git(
        repo,
        &[
            "log",
            "--first-parent",
            "--format=%H %s",
            "-n",
            &n,
            &project.commit,
        ],
    )?;
    let mut commits: Vec<(String, String)> = log
        .lines()
        .filter_map(|l| l.split_once(' ').map(|(h, s)| (h.to_owned(), s.to_owned())))
        .collect();
    commits.reverse();
    Ok(commits)
}

/// Replays `commits` of `project`: for each, checks it out, prepares its
/// units and runs plain and sail with the persistent cache.
///
/// # Errors
///
/// A message if the daemon cannot be started; per-commit problems are
/// recorded as infra errors instead.
#[allow(clippy::too_many_arguments)] // the workload's whole context
pub fn run(
    setup: &Setup,
    project: &Project,
    repo: &Path,
    commits: &[(String, String)],
    verus_source: Option<&Path>,
    unit_filter: Option<&str>,
    cache_root: &Path,
    mut progress: impl FnMut(&CommitResult),
) -> Result<Vec<CommitResult>, String> {
    let daemon = Daemon::start(setup.sail, &setup.socket(), cache_root)?;
    let prepared_dir: PathBuf =
        std::path::absolute(setup.scratch.join("replay-prepared")).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for (sha, subject) in commits {
        let base = CommitResult {
            sha: sha.clone(),
            subject: subject.clone(),
            ..CommitResult::default()
        };
        let units = git(repo, &["checkout", "-q", "--detach", sha])
            .and_then(|_| project.units(repo).map_err(|e| e.to_string()));
        let units = match units {
            Ok(u) => u,
            Err(e) => {
                let r = CommitResult {
                    infra_errors: 1,
                    note: Some(e),
                    ..base
                };
                progress(&r);
                results.push(r);
                continue;
            }
        };
        for unit in units
            .iter()
            .filter(|u| unit_filter.is_none_or(|f| u.id.contains(f)))
        {
            let mut r = CommitResult {
                unit: unit.id.clone(),
                ..base.clone()
            };
            match prepare_unit(setup.installed, unit, verus_source, &prepared_dir) {
                Ok(prepared) => one(setup, &mut r, &prepared, &daemon),
                Err(e) => {
                    r.infra_errors = 1;
                    r.note = Some(e.to_string());
                }
            }
            progress(&r);
            results.push(r);
        }
    }
    Ok(results)
}

fn one(setup: &Setup, r: &mut CommitResult, unit: &Unit, daemon: &Daemon) {
    let sail_solver = Solver::Shim {
        shim: setup.shim.to_path_buf(),
        trace_dir: None,
        daemon_socket: Some(daemon.socket().to_path_buf()),
    };
    let work = setup.scratch.join("work");
    let installed: &Installed = setup.installed;
    let p = run_unit(installed, unit, &Solver::Plain, &work.join("plain"));
    let before = daemon.stats();
    let s = run_unit(installed, unit, &sail_solver, &work.join("sail"));
    if let (Some(b), Some(a)) = (before, daemon.stats()) {
        r.cache.push(CacheRun::between("commit", &b, &a));
    }
    let (Ok(p), Ok(s)) = (p, s) else {
        r.infra_errors += 1;
        r.note = Some("a run could not be executed".to_owned());
        return;
    };
    r.outcome = Some(outcome(&p).to_owned());
    r.plain.push(&p);
    r.sail.push(&s);
    let plains: Vec<RunRecord> = vec![p];
    match judge(&plains, &s).0 {
        Judgement::Same => r.same += 1,
        Judgement::Unstable => r.unstable += 1,
        Judgement::Different => {
            r.different += 1;
            eprintln!("{} {} DIFFERENT", r.sha, r.unit);
        }
    }
}
