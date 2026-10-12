//! Command line of the `sail-bench` trace-analysis subcommands.
//!
//! ```text
//! sail-bench profile <trace-dir>... [--json out.json]
//! sail-bench seeds   <trace-dir>... --z3 <path> [--seeds N] [--jobs J] [--json out.json]
//! sail-bench history <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
//! sail-bench presolver <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
//!                      [--save DIR] [--replay-every N] [--rlimit R]
//! ```
//!
//! Common options: `--timeout-secs T` (safety net per solver process,
//! default 600), `--examples K` (examples kept in reports, default 20),
//! `--all-checks` (report every check in the JSON), `--quiet` (no
//! progress on stderr).

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use serde::Serialize;

use crate::error::BenchError;
use crate::solver::Solver;
use crate::trace;

/// Usage text.
pub const USAGE: &str = "\
usage:
  sail-bench profile <trace-dir>... [--json out.json]
  sail-bench seeds   <trace-dir>... --z3 <path> [--seeds N] [--jobs J] [--json out.json]
  sail-bench history <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
  sail-bench presolver <trace-dir>... --z3 <path> [--jobs J] [--json out.json]
                       [--save DIR] [--replay-every N] [--rlimit R]
options:
  --timeout-secs T   safety-net wall-clock limit per solver process (default 600)
  --examples K       examples kept in reports (default 20)
  --all-checks       report every check in the JSON output
  --quiet            no progress output on stderr
A trace directory holds <id>.jsonl/.in.smt2/.out files, or subdirectories of them.";

/// Parsed arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// `profile`, `seeds` or `history`.
    pub command: String,
    /// Trace directories.
    pub dirs: Vec<PathBuf>,
    /// `--json`.
    pub json: Option<PathBuf>,
    /// `--z3`.
    pub z3: Option<PathBuf>,
    /// `--seeds`.
    pub seeds: u64,
    /// `--jobs`.
    pub jobs: usize,
    /// `--timeout-secs`.
    pub timeout_secs: u64,
    /// `--examples`.
    pub examples: usize,
    /// `--all-checks`.
    pub all_checks: bool,
    /// `--quiet`.
    pub quiet: bool,
    /// `--save` (presolver: where to save example queries).
    pub save: Option<PathBuf>,
    /// `--replay-every` (presolver).
    pub replay_every: u64,
    /// `--rlimit` (presolver oracle queries).
    pub rlimit: u64,
}

fn number<T: std::str::FromStr>(flag: &str, value: Option<&str>) -> Result<T, BenchError> {
    value
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| BenchError::Usage(format!("{flag} needs a number")))
}

/// Parses arguments (without the program name).
///
/// # Errors
///
/// [`BenchError::Usage`] for anything malformed.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, BenchError> {
    let mut it = args.into_iter();
    let command = it
        .next()
        .ok_or_else(|| BenchError::Usage(USAGE.to_owned()))?;
    if !matches!(
        command.as_str(),
        "profile" | "seeds" | "history" | "presolver"
    ) {
        return Err(BenchError::Usage(format!(
            "unknown command `{command}`\n{USAGE}"
        )));
    }
    let mut a = Args {
        command,
        dirs: Vec::new(),
        json: None,
        z3: None,
        seeds: 8,
        jobs: 4,
        timeout_secs: 600,
        examples: 20,
        all_checks: false,
        quiet: false,
        save: None,
        replay_every: 0,
        rlimit: 50_000_000,
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--json" => {
                a.json = Some(
                    it.next()
                        .map(PathBuf::from)
                        .ok_or_else(|| BenchError::Usage("--json needs a path".to_owned()))?,
                );
            }
            "--z3" => {
                a.z3 = Some(
                    it.next()
                        .map(PathBuf::from)
                        .ok_or_else(|| BenchError::Usage("--z3 needs a path".to_owned()))?,
                );
            }
            "--seeds" => a.seeds = number("--seeds", it.next().as_deref())?,
            "--jobs" => a.jobs = number("--jobs", it.next().as_deref())?,
            "--timeout-secs" => a.timeout_secs = number("--timeout-secs", it.next().as_deref())?,
            "--examples" => a.examples = number("--examples", it.next().as_deref())?,
            "--save" => {
                a.save = Some(
                    it.next()
                        .map(PathBuf::from)
                        .ok_or_else(|| BenchError::Usage("--save needs a path".to_owned()))?,
                );
            }
            "--replay-every" => a.replay_every = number("--replay-every", it.next().as_deref())?,
            "--rlimit" => a.rlimit = number("--rlimit", it.next().as_deref())?,
            "--all-checks" => a.all_checks = true,
            "--quiet" => a.quiet = true,
            flag if flag.starts_with("--") => {
                return Err(BenchError::Usage(format!(
                    "unknown option `{flag}`\n{USAGE}"
                )));
            }
            dir => a.dirs.push(PathBuf::from(dir)),
        }
    }
    if a.dirs.is_empty() {
        return Err(BenchError::Usage(format!(
            "no trace directory given\n{USAGE}"
        )));
    }
    if a.command != "profile" && a.z3.is_none() {
        return Err(BenchError::Usage(format!(
            "`{}` needs --z3 <path>",
            a.command
        )));
    }
    if a.seeds == 0 || a.jobs == 0 || a.timeout_secs == 0 {
        return Err(BenchError::Usage(
            "--seeds, --jobs and --timeout-secs must be positive".to_owned(),
        ));
    }
    Ok(a)
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), BenchError> {
    let mut text =
        serde_json::to_string_pretty(value).map_err(|e| BenchError::Json(e.to_string()))?;
    text.push('\n');
    std::fs::write(path, text).map_err(|e| BenchError::io(path, &e))
}

/// Runs a parsed command, returning the human-readable summary.
///
/// # Errors
///
/// Any [`BenchError`].
pub fn execute(a: &Args) -> Result<String, BenchError> {
    crate::set_quiet(a.quiet);
    let sessions = trace::discover_all(&a.dirs)?;
    let solver = a.z3.as_ref().map(|path| Solver {
        path: path.clone(),
        timeout: Duration::from_secs(a.timeout_secs),
    });
    match (a.command.as_str(), solver) {
        ("profile", _) => {
            let mut loaded = Vec::with_capacity(sessions.len());
            for files in sessions {
                let t = trace::load(&files)?;
                loaded.push((files, t));
            }
            let report = crate::profile::profile(&loaded);
            if let Some(path) = &a.json {
                write_json(path, &report)?;
            }
            Ok(crate::profile::render(&report))
        }
        ("seeds", Some(solver)) => {
            let report = crate::seeds::seeds(
                &solver,
                &sessions,
                &crate::seeds::SeedsOptions {
                    seeds: a.seeds,
                    jobs: a.jobs,
                    examples: a.examples,
                    all_checks: a.all_checks,
                },
            );
            if let Some(path) = &a.json {
                write_json(path, &report)?;
            }
            Ok(crate::seeds::render(&report))
        }
        ("history", Some(solver)) => {
            let report = crate::history::history(
                &solver,
                &sessions,
                &crate::history::HistoryOptions {
                    jobs: a.jobs,
                    examples: a.examples,
                    all_checks: a.all_checks,
                },
            );
            if let Some(path) = &a.json {
                write_json(path, &report)?;
            }
            Ok(crate::history::render(&report))
        }
        ("presolver", Some(solver)) => {
            let report = crate::presolver::presolver(
                &solver,
                &a.dirs,
                &crate::presolver::Options {
                    jobs: a.jobs,
                    examples: a.examples,
                    save: a.save.clone(),
                    replay_every: a.replay_every,
                    rlimit: a.rlimit,
                },
            )?;
            if let Some(path) = &a.json {
                write_json(path, &report)?;
            }
            Ok(crate::presolver::render(&report))
        }
        _ => Err(BenchError::Usage(USAGE.to_owned())),
    }
}

/// Entry point for the `profile`, `seeds` and `history` subcommands:
/// parses `args` (starting with the subcommand), runs, prints the summary.
#[must_use]
pub fn run(args: Vec<String>) -> ExitCode {
    let result = parse(args).and_then(|a| execute(&a));
    match result {
        Ok(summary) => {
            let _ = std::io::stdout().write_all(summary.as_bytes());
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "{e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args, BenchError> {
        parse(s.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn parses_commands() {
        let a = args("seeds t1 t2 --z3 /z3 --seeds 4 --jobs 2 --json o.json");
        let Ok(a) = a else { unreachable!("{a:?}") };
        assert_eq!(a.dirs, [PathBuf::from("t1"), PathBuf::from("t2")]);
        assert_eq!((a.seeds, a.jobs), (4, 2));
        assert_eq!(a.json, Some(PathBuf::from("o.json")));
        assert!(args("profile t").is_ok());
    }

    #[test]
    fn rejects_bad_usage() {
        for bad in [
            "",
            "frobnicate t",
            "profile",
            "seeds t",
            "seeds t --z3",
            "seeds t --z3 z --seeds x",
            "history t --z3 z --jobs 0",
            "profile t --bogus",
        ] {
            assert!(
                matches!(args(bad), Err(BenchError::Usage(_))),
                "{bad:?} should be rejected"
            );
        }
    }
}
