//! Command line of the `sail-bench profile` trace analysis.
//!
//! ```text
//! sail-bench profile <trace-dir>... [--json out.json]
//! ```

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::error::BenchError;
use crate::trace;

/// Usage text.
pub const USAGE: &str = "\
usage:
  sail-bench profile <trace-dir>... [--json out.json]
A trace directory holds <id>.jsonl/.in.smt2/.out files, or subdirectories of them.";

/// Parsed arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// Trace directories.
    pub dirs: Vec<PathBuf>,
    /// `--json`.
    pub json: Option<PathBuf>,
}

/// Parses arguments (without the program name), starting with `profile`.
///
/// # Errors
///
/// [`BenchError::Usage`] for anything malformed.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, BenchError> {
    let mut it = args.into_iter();
    match it.next().as_deref() {
        Some("profile") => {}
        Some(other) => {
            return Err(BenchError::Usage(format!(
                "unknown command `{other}`\n{USAGE}"
            )));
        }
        None => return Err(BenchError::Usage(USAGE.to_owned())),
    }
    let mut a = Args {
        dirs: Vec::new(),
        json: None,
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
    Ok(a)
}

/// Runs a parsed command, returning the human-readable summary.
///
/// # Errors
///
/// Any [`BenchError`].
pub fn execute(a: &Args) -> Result<String, BenchError> {
    let mut loaded = Vec::new();
    for files in trace::discover_all(&a.dirs)? {
        let t = trace::load(&files)?;
        loaded.push((files, t));
    }
    let report = crate::profile::profile(&loaded);
    if let Some(path) = &a.json {
        let mut text =
            serde_json::to_string_pretty(&report).map_err(|e| BenchError::Json(e.to_string()))?;
        text.push('\n');
        std::fs::write(path, text).map_err(|e| BenchError::io(path, &e))?;
    }
    Ok(crate::profile::render(&report))
}

/// Entry point for the `profile` subcommand: parses `args` (starting with
/// the subcommand), runs, prints the summary.
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
        let a = args("profile t1 t2 --json o.json");
        let Ok(a) = a else { unreachable!("{a:?}") };
        assert_eq!(a.dirs, [PathBuf::from("t1"), PathBuf::from("t2")]);
        assert_eq!(a.json, Some(PathBuf::from("o.json")));
    }

    #[test]
    fn rejects_bad_usage() {
        for bad in [
            "",
            "frobnicate t",
            "profile",
            "seeds t",
            "profile t --bogus",
        ] {
            assert!(
                matches!(args(bad), Err(BenchError::Usage(_))),
                "{bad:?} should be rejected"
            );
        }
    }
}
