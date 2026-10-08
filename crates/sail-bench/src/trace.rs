//! Reading the shim's trace directories (`SAIL_TRACE_DIR`, trace versions 1
//! and 2; version 2 adds `prefix_key`, the v2 key of ADR 0008).
//!
//! Each solver process the shim served left three files, all named by the
//! session id `<pid>-<nanos>`: `<id>.in.smt2`, `<id>.out` and `<id>.jsonl`
//! (see `sail-shim`'s `trace` module for the record format). A trace
//! directory either holds such files directly, or holds one subdirectory of
//! them per run (for example one per corpus entry); [`discover`] accepts
//! both.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;

/// The newest trace format version this module reads (it also reads 1).
pub const TRACE_VERSION: u32 = 2;

/// One record of a `<id>.jsonl` trace. Unknown record types are kept as
/// [`Record::Other`] so newer traces still load.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Record {
    /// The session started.
    Start {
        /// The trace format version.
        trace_version: u32,
    },
    /// A satisfiability check.
    Check(CheckRecord),
    /// An `:all-statistics` reply.
    Stats {
        /// The check this reply followed, if any.
        after_check: Option<u64>,
        /// Its `rlimit-count`.
        rlimit_count: Option<u64>,
    },
    /// The solver replied with an error.
    SolverError {
        /// The error reply.
        text: String,
    },
    /// The session ended.
    End(EndRecord),
    /// A record type this version does not know.
    #[serde(other)]
    Other,
}

/// A `check` record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CheckRecord {
    /// Index of the check within its session.
    pub seq: u64,
    /// Query key v1, if the snapshot was exact.
    pub key: Option<String>,
    /// Query key v2 (whole process prefix), from trace version 2 on.
    #[serde(default)]
    pub prefix_key: Option<String>,
    /// The paired answer, if pairing held.
    pub answer: Option<String>,
    /// Microseconds from session start until the check was sent.
    pub sent_us: u64,
    /// Microseconds from session start until the answer arrived.
    pub answered_us: Option<u64>,
    /// Push depth at the check.
    pub depth: u64,
    /// Number of assertion-stack commands at the check.
    pub stack_len: u64,
    /// Commands applied before the check.
    pub position: u64,
    /// Checks before this one.
    pub prior_checks: u64,
    /// The `:rlimit` option in force.
    pub rlimit: Option<String>,
    /// Why the session model was inexact, if it was.
    pub inexact: Option<String>,
}

impl CheckRecord {
    /// Solve time in microseconds (answered minus sent), if answered.
    #[must_use]
    pub fn solve_us(&self) -> Option<u64> {
        self.answered_us.map(|a| a.saturating_sub(self.sent_us))
    }
}

/// An `end` record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EndRecord {
    /// The solver's exit code.
    pub exit_code: Option<i32>,
    /// Commands seen.
    pub commands: u64,
    /// Checks seen.
    pub checks: u64,
    /// Replies seen.
    pub responses: u64,
    /// Whether the tap kept up and every write succeeded.
    pub complete: bool,
    /// Whether pairing held to the end.
    pub paired: bool,
}

/// The files of one captured solver session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFiles {
    /// A readable id: `<subdir>/<pid>-<nanos>` or `<pid>-<nanos>`.
    pub id: String,
    /// `<id>.jsonl`.
    pub jsonl: PathBuf,
    /// `<id>.in.smt2`.
    pub input: PathBuf,
    /// `<id>.out`.
    pub output: PathBuf,
}

/// A parsed `<id>.jsonl`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionTrace {
    /// The trace version from the start record.
    pub trace_version: Option<u32>,
    /// Check records, in order.
    pub checks: Vec<CheckRecord>,
    /// Solver error replies.
    pub solver_errors: Vec<String>,
    /// The end record, if the session ended cleanly.
    pub end: Option<EndRecord>,
}

/// Parses the text of a `<id>.jsonl` trace.
///
/// # Errors
///
/// [`BenchError::TraceFormat`] for a line that is not a valid record, or
/// a trace version other than [`TRACE_VERSION`].
pub fn parse_jsonl(text: &str, path: &Path) -> Result<SessionTrace, BenchError> {
    let mut trace = SessionTrace::default();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(line).map_err(|e| BenchError::TraceFormat {
            path: path.to_path_buf(),
            line: n + 1,
            message: e.to_string(),
        })?;
        match record {
            Record::Start { trace_version } => {
                if !(1..=TRACE_VERSION).contains(&trace_version) {
                    return Err(BenchError::TraceFormat {
                        path: path.to_path_buf(),
                        line: n + 1,
                        message: format!(
                            "trace version {trace_version}, expected 1 to {TRACE_VERSION}"
                        ),
                    });
                }
                trace.trace_version = Some(trace_version);
            }
            Record::Check(c) => trace.checks.push(c),
            Record::SolverError { text } => trace.solver_errors.push(text),
            Record::End(e) => trace.end = Some(e),
            Record::Stats { .. } | Record::Other => {}
        }
    }
    Ok(trace)
}

/// Reads and parses one session's trace.
///
/// # Errors
///
/// I/O errors and [`parse_jsonl`] errors.
pub fn load(files: &SessionFiles) -> Result<SessionTrace, BenchError> {
    let text = fs::read_to_string(&files.jsonl).map_err(|e| BenchError::io(&files.jsonl, &e))?;
    parse_jsonl(&text, &files.jsonl)
}

/// Lists the sessions under `dir`, sorted by id: the `*.jsonl` files directly
/// in it, or, if there are none, those in its immediate subdirectories.
///
/// # Errors
///
/// [`BenchError::Io`] if a directory cannot be read.
pub fn discover(dir: &Path) -> Result<Vec<SessionFiles>, BenchError> {
    let mut found = sessions_in(dir, None)?;
    if found.is_empty() {
        let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(|e| BenchError::io(dir, &e))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        for sub in subdirs {
            let name = sub
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.extend(sessions_in(&sub, Some(&name))?);
        }
    }
    Ok(found)
}

fn sessions_in(dir: &Path, prefix: Option<&str>) -> Result<Vec<SessionFiles>, BenchError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| BenchError::io(dir, &e))? {
        let path = entry.map_err(|e| BenchError::io(dir, &e))?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        out.push(SessionFiles {
            id: prefix.map_or_else(|| stem.to_owned(), |p| format!("{p}/{stem}")),
            jsonl: path.clone(),
            input: dir.join(format!("{stem}.in.smt2")),
            output: dir.join(format!("{stem}.out")),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// The sessions under every directory in `dirs`, in order.
///
/// # Errors
///
/// As [`discover`].
pub fn discover_all(dirs: &[PathBuf]) -> Result<Vec<SessionFiles>, BenchError> {
    let mut all = Vec::new();
    for dir in dirs {
        all.extend(discover(dir)?);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"type":"start","trace_version":1,"pid":1,"args":["-smt2","-in"],"solver":{"path":"z3","version":"4.16.0","sha256":null}}
{"type":"stats","after_check":null,"rlimit_count":29457}
{"type":"check","seq":0,"key":"ab","answer":"unsat","sent_us":100,"answered_us":350,"depth":1,"stack_len":3,"position":20,"prior_checks":0,"rlimit":"30000000","inexact":null}
{"type":"check","seq":1,"key":null,"answer":null,"sent_us":400,"answered_us":null,"depth":1,"stack_len":3,"position":30,"prior_checks":1,"rlimit":"30000000","inexact":"Unmodelled"}
{"type":"future-record","x":1}
{"type":"end","exit_code":0,"commands":40,"checks":2,"responses":9,"complete":true,"paired":false}
"#;

    #[test]
    fn parses_records() {
        let t = parse_jsonl(SAMPLE, Path::new("x.jsonl"));
        let Ok(t) = t else {
            unreachable!("{t:?}");
        };
        assert_eq!(t.trace_version, Some(1));
        assert_eq!(t.checks.len(), 2);
        assert_eq!(t.checks[0].solve_us(), Some(250));
        assert_eq!(t.checks[1].solve_us(), None);
        assert_eq!(t.checks[1].inexact.as_deref(), Some("Unmodelled"));
        assert_eq!(t.end.as_ref().map(|e| e.paired), Some(false));
    }

    #[test]
    fn rejects_bad_lines_and_versions() {
        let err = parse_jsonl("{\"type\":\"check\"}\n", Path::new("x.jsonl"));
        assert!(matches!(err, Err(BenchError::TraceFormat { line: 1, .. })));
        let err = parse_jsonl(
            "{\"type\":\"start\",\"trace_version\":3}\n",
            Path::new("x.jsonl"),
        );
        assert!(matches!(err, Err(BenchError::TraceFormat { .. })));
        let ok = parse_jsonl(
            "{\"type\":\"start\",\"trace_version\":2}\n",
            Path::new("x.jsonl"),
        );
        assert!(ok.is_ok());
    }
}
