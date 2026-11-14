//! Uncached, separate plain/traced executions of an exact diagnostic prefix.
//!
//! The caller authenticates the solver, retained input and function binding.
//! A successful comparison describes these replays, never the original proof.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::quantifiers::{Counter, Counts, MAX_TRACE_BYTES};
use crate::{PortfolioError, Solver};

mod comparison;
pub use comparison::StatisticChange;
use comparison::compare;

const MAX_COMMANDS: usize = 100_000;
const MAX_OUTPUT: u64 = 4 * 1024 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(300);

/// One comparable pair of separate diagnostic executions.
#[derive(Debug, Serialize)]
pub struct Measurement {
    /// Directory retaining original prefix, streams, trace and exit records.
    pub directory: PathBuf,
    /// Zero-based original process check selected for replay.
    pub check: usize,
    /// Number of original SMT check commands, independent of internal trace events.
    pub original_checks: usize,
    /// Exact prefix length; commands after the selected check are absent.
    pub prefix_bytes: usize,
    /// Plain replay elapsed time in seconds, never a proof performance claim.
    pub plain_seconds: f64,
    /// Traced replay elapsed time in seconds.
    pub traced_seconds: f64,
    /// Trace counts cumulative across every check in this prefix.
    pub counts: Counts,
    /// Original allocation/memory/clock differences; never resource-limit counters.
    pub statistic_changes: Vec<StatisticChange>,
}

fn refuse(message: impl Into<String>) -> PortfolioError {
    PortfolioError::Options(format!("diagnostic profile replay: {}", message.into()))
}

fn create(path: &Path) -> Result<File, PortfolioError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| PortfolioError::io(path, &error))
}

fn save(path: &Path, bytes: &[u8]) -> Result<(), PortfolioError> {
    create(path)?
        .write_all(bytes)
        .map_err(|error| PortfolioError::io(path, &error))
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, PortfolioError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|error| PortfolioError::io(path, &error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(refuse("capture exceeds its read bound"));
    }
    Ok(bytes)
}

fn oversized(paths: &[(&Path, u64)]) -> Result<bool, PortfolioError> {
    for (path, limit) in paths {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() || metadata.len() > *limit => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PortfolioError::io(path, &error)),
        }
    }
    Ok(false)
}

fn execute(solver: &Solver, directory: &Path, traced: bool) -> Result<f64, PortfolioError> {
    let arm = if traced { "traced" } else { "plain" };
    let output = directory.join(format!("{arm}.stdout"));
    let error = directory.join(format!("{arm}.stderr"));
    let trace = directory.join("z3.trace");
    let input = directory.join("prefix.smt2");
    let mut command = Command::new(&solver.path);
    command.args(["-smt2", "-in"]);
    if traced {
        let mut option = std::ffi::OsString::from("trace_file_name=");
        option.push(&trace);
        command.arg("trace=true").arg(option);
    }
    let started = Instant::now();
    let mut child = command
        .stdin(File::open(&input).map_err(|failure| PortfolioError::io(&input, &failure))?)
        .stdout(Stdio::from(create(&output)?))
        .stderr(Stdio::from(create(&error)?))
        .spawn()
        .map_err(|failure| PortfolioError::io(&solver.path, &failure))?;
    // Every return after spawn must stop and reap the child, including metadata
    // errors. Spooling avoids pipe writers/readers surviving a timeout.
    let result = (|| {
        loop {
            let paths = [
                (output.as_path(), MAX_OUTPUT),
                (error.as_path(), MAX_OUTPUT),
                (trace.as_path(), MAX_TRACE_BYTES as u64),
            ];
            if oversized(&paths)? {
                return Err(refuse("capture byte limit exceeded"));
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|failure| refuse(format!("cannot observe {arm} process: {failure}")))?
            {
                save(
                    &directory.join(format!("{arm}.exit")),
                    format!("{status}\n").as_bytes(),
                )?;
                if !status.success() {
                    return Err(refuse(format!("{arm} process did not exit successfully")));
                }
                return Ok(started.elapsed().as_secs_f64());
            }
            if started.elapsed() >= solver.timeout {
                return Err(refuse(format!("{arm} process exceeded its wall limit")));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

/// Replay an unchanged original prefix in fresh plain and traced processes.
///
/// `directory` must not exist. It is created beneath an existing caller-owned
/// private parent, must stay private/immutable to other actors during execution,
/// and retains every original capture on success or refusal. The solver must be
/// the caller's separately authenticated pinned Z3; this function authenticates
/// no executable, source, session, function, mode, historical status or proof.
/// It never invokes a shim/cache/daemon and never changes the original input.
///
/// Both executions receive the identical complete prefix, including earlier
/// checks. Every expected check, all non-statistics stdout bytes, stderr,
/// resource-limit counters and other statistics must match. Only original
/// allocation/memory/clock observations may vary, with exact values retained.
/// The complete trace must name pinned Z3. Internal begin-check events can be
/// absent for valid trivial/BV checks and are reported separately from actual
/// matched SMT checks. Counts describe the whole prefix, not solely the selected
/// function/check. Even equal `unknown` responses supply no proof evidence.
///
/// Input/read bounds are fixed; output-file sizes are polled every 5 ms, so a
/// producer can overshoot the file limit before termination. No hard filesystem
/// quota or process-tree containment is claimed. The trusted pinned solver must
/// not spawn descendants. Original options must not redirect/disable tracing;
/// an absent, malformed or inconsistent trace refuses.
///
/// # Errors
/// `SAIL5101` for file/process I/O; `SAIL5102` for invalid bounds, incomplete,
/// differing, over-budget or unsuccessful diagnostic executions.
pub fn measure(
    solver: &Solver,
    input: &[u8],
    check: usize,
    directory: &Path,
) -> Result<Measurement, PortfolioError> {
    if solver.timeout.is_zero() || solver.timeout > MAX_TIMEOUT {
        return Err(refuse("wall limit must be within (0, 300] seconds"));
    }
    let selected = super::prefix(input, check)?;
    if selected.commands > MAX_COMMANDS {
        return Err(refuse("prefix command count exceeds its analysis bound"));
    }
    let checks = check
        .checked_add(1)
        .ok_or_else(|| refuse("check overflow"))?;
    std::fs::create_dir(directory).map_err(|failure| PortfolioError::io(directory, &failure))?;
    let directory = std::fs::canonicalize(directory)
        .map_err(|failure| PortfolioError::io(directory, &failure))?;
    save(&directory.join("prefix.smt2"), selected.input)?;
    let plain_seconds = execute(solver, &directory, false)?;
    let traced_seconds = execute(solver, &directory, true)?;
    let plain = read(&directory.join("plain.stdout"), MAX_OUTPUT)?;
    let traced = read(&directory.join("traced.stdout"), MAX_OUTPUT)?;
    let statistic_changes = compare(selected.input, &plain, &traced, checks)?;
    if read(&directory.join("plain.stderr"), MAX_OUTPUT)?
        != read(&directory.join("traced.stderr"), MAX_OUTPUT)?
    {
        return Err(refuse("plain and traced stderr differ"));
    }
    let mut counter = Counter::new();
    counter
        .push(&read(&directory.join("z3.trace"), MAX_TRACE_BYTES as u64)?)
        .map_err(|failure| refuse(failure.to_string()))?;
    let counts = counter
        .finish()
        .map_err(|failure| refuse(failure.to_string()))?;
    Ok(Measurement {
        directory,
        check,
        original_checks: checks,
        prefix_bytes: selected.input.len(),
        plain_seconds,
        traced_seconds,
        counts,
        statistic_changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_requires_exact_outputs_and_complete_nonerror_answers() {
        let input = b"(check-sat)\n(check-sat)";
        assert!(compare(input, b"unsat\nsat\n", b"unsat\nsat\n", 2).is_ok());
        for output in [
            b"unsat\n".as_slice(),
            b"unsat\n(error \"bad\")\n",
            b"",
            b"unsat\nother\n",
        ] {
            assert!(compare(input, output, output, 2).is_err());
        }
        assert!(compare(input, b"unsat\nsat\n", b"unsat\nunknown\n", 2).is_err());
    }

    #[test]
    fn invalid_timeout_and_prefix_do_not_create_output() {
        let destination =
            std::env::temp_dir().join(format!("sail-profile-invalid-{}", std::process::id()));
        let solver = Solver {
            path: PathBuf::from("unused-solver"),
            timeout: Duration::ZERO,
        };
        assert!(measure(&solver, b"(check-sat)", 0, &destination).is_err());
        assert!(!destination.exists());
        let solver = Solver {
            timeout: Duration::from_secs(1),
            ..solver
        };
        assert!(measure(&solver, b"(assert", 0, &destination).is_err());
        assert!(!destination.exists());
    }
}
