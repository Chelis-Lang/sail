//! Running a fresh pinned Z3 on a complete input.
//!
//! [`run`] starts `<z3> -smt2 -in`, writes the whole input, closes stdin and
//! collects stdout until the solver exits. A wall-clock guard kills the
//! solver if it runs past the timeout. That guard is a safety net for the
//! tooling only: such a run is reported as [`Outcome::InfraTimeout`], never
//! as a solver answer.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// A solver process could not be started or driven (`SAIL5001`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverError(pub String);

impl SolverError {
    /// The stable error code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        "SAIL5001"
    }
}

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}]: solver: {}", self.code(), self.0)
    }
}

impl std::error::Error for SolverError {}

/// The solver to replay against.
#[derive(Debug, Clone)]
pub struct Solver {
    /// Path to the pinned Z3.
    pub path: PathBuf,
    /// The safety-net wall-clock limit per process.
    pub timeout: Duration,
}

/// The identity of the solver binary, recorded with every result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The path given.
    pub path: String,
    /// The first line of `--version` output.
    pub version: Option<String>,
    /// SHA-256 of the binary, as hex.
    pub sha256: Option<String>,
}

impl Solver {
    /// Reads the solver's identity (`--version` and binary hash).
    #[must_use]
    pub fn identity(&self) -> Identity {
        let version = Command::new(&self.path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .map(str::to_owned)
            });
        let sha256 = std::fs::read(&self.path)
            .ok()
            .map(|b| hex(&Sha256::digest(&b)));
        Identity {
            path: self.path.display().to_string(),
            version,
            sha256,
        }
    }
}

/// Lower-case hex of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// How a solver run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The solver exited by itself.
    Exited {
        /// Its exit code, if it exited normally.
        code: Option<i32>,
    },
    /// The safety-net timeout killed it. Not a solver answer.
    InfraTimeout,
}

/// The result of one solver run.
#[derive(Debug, Clone)]
pub struct Run {
    /// Everything the solver wrote to stdout.
    pub stdout: Vec<u8>,
    /// Everything it wrote to stderr.
    pub stderr: Vec<u8>,
    /// How it ended.
    pub outcome: Outcome,
    /// Wall-clock duration.
    pub elapsed: Duration,
}

fn drain(mut source: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = source.read_to_end(&mut buf);
        buf
    })
}

/// Runs the solver on `input` in a fresh process.
///
/// # Errors
///
/// [`SolverError`] if the process cannot be started or waited for.
pub fn run(solver: &Solver, input: &[u8]) -> Result<Run, SolverError> {
    run_path(&solver.path, solver.timeout, input)
}

fn run_path(path: &Path, timeout: Duration, input: &[u8]) -> Result<Run, SolverError> {
    let started = Instant::now();
    let mut child = Command::new(path)
        .args(["-smt2", "-in"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| SolverError(format!("cannot start {}: {e}", path.display())))?;
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let writer = child.stdin.take().map(|mut stdin| {
        let input = input.to_vec();
        thread::spawn(move || {
            // A solver that exits early closes the pipe; that is not an
            // error here, its output says what happened.
            let _ = stdin.write_all(&input);
        })
    });
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break Outcome::Exited {
                    code: status.code(),
                };
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break Outcome::InfraTimeout;
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(e) => {
                let _ = child.kill();
                return Err(SolverError(format!("waiting for solver: {e}")));
            }
        }
    };
    if let Some(w) = writer {
        let _ = w.join();
    }
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(Run {
        stdout,
        stderr,
        outcome,
        elapsed: started.elapsed(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn collects_output_of_a_fake_solver() {
        // `cat` ignores the flags only on some platforms, so use `sh`.
        let dir = std::env::temp_dir().join(format!("sail-solver-process-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let script = dir.join("fake-z3");
        let ok = std::fs::write(&script, "#!/bin/sh\ncat\n").is_ok()
            && Command::new("chmod")
                .arg("+x")
                .arg(&script)
                .status()
                .is_ok_and(|s| s.success());
        assert!(ok);
        let r = run_path(&script, Duration::from_secs(30), b"hello\n");
        let Ok(r) = r else { unreachable!("{r:?}") };
        assert_eq!(r.stdout, b"hello\n");
        assert_eq!(r.outcome, Outcome::Exited { code: Some(0) });

        let slow = dir.join("slow-z3");
        let ok = std::fs::write(&slow, "#!/bin/sh\nexec sleep 30\n").is_ok()
            && Command::new("chmod")
                .arg("+x")
                .arg(&slow)
                .status()
                .is_ok_and(|s| s.success());
        assert!(ok);
        let r = run_path(&slow, Duration::from_millis(100), b"");
        let Ok(r) = r else { unreachable!("{r:?}") };
        assert_eq!(r.outcome, Outcome::InfraTimeout);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
