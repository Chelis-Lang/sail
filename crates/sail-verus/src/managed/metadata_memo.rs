//! SPIKE (A4): serves Cargo-Verus's own `cargo metadata` probe from the
//! runner's identical read, made moments earlier in the same run.
//!
//! The runner records the exact stdout of its metadata read only when the
//! read printed nothing on stderr and every metadata input (the guard of the
//! previous run's selection: manifests, lockfile, Cargo configuration) kept
//! its stamp across that read. The Cargo alias serves it only to a call with
//! byte-identical arguments, the same directory and metadata target, after
//! checking the same stamps again. Any mismatch runs the real Cargo.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The private transport setting naming the memo file.
pub(crate) const MEMO_ENV: &str = "SAIL_SPIKE_METADATA_MEMO";

#[derive(Serialize, Deserialize)]
struct Memo {
    argv: Vec<String>,
    cwd: PathBuf,
    target: PathBuf,
    paths: BTreeSet<PathBuf>,
    stamp: String,
    stdout: PathBuf,
}

/// Writes the memo into `run`; `None` when there is nothing to serve.
pub(super) fn write(
    run: &Path,
    argv: Vec<String>,
    cwd: &Path,
    target: PathBuf,
    guard: &super::input::MetadataGuard,
    stdout: Vec<u8>,
) -> Option<PathBuf> {
    if std::env::var("SAIL_SPIKE_METADATA_MEMO_OFF").as_deref() == Ok("1") {
        return None;
    }
    let (paths, stamp) = guard.parts();
    let out = run.join("metadata-memo.stdout");
    std::fs::write(&out, stdout).ok()?;
    let memo = Memo {
        argv,
        cwd: std::path::absolute(cwd).ok()?,
        target,
        paths: paths.clone(),
        stamp: stamp.to_owned(),
        stdout: out,
    };
    let path = run.join("metadata-memo.json");
    std::fs::write(&path, serde_json::to_vec(&memo).ok()?).ok()?;
    Some(path)
}

/// In the Cargo alias: the memo's stdout when it may serve `args`, or
/// `None` for the real Cargo to run.
pub(crate) fn eligible(args: &[OsString], target: Option<&OsString>) -> Option<Vec<u8>> {
    let path = super::transport::var(MEMO_ENV)?;
    let memo = serde_json::from_slice::<Memo>(&std::fs::read(&path).ok()?).ok()?;
    let same_args = args.len() == memo.argv.len()
        && args
            .iter()
            .zip(&memo.argv)
            .all(|(a, b)| a.to_str() == Some(b));
    let same_cwd = std::env::current_dir()
        .ok()
        .and_then(|d| std::path::absolute(d).ok())
        .is_some_and(|d| d == memo.cwd);
    let same_target = target.is_some_and(|t| Path::new(t) == memo.target);
    (same_args && same_cwd && same_target && super::input::stamp_matches(&memo.paths, &memo.stamp))
        .then(|| std::fs::read(&memo.stdout).ok())
        .flatten()
}

/// Writes `stdout` as Cargo would have.
pub(crate) fn emit(stdout: &[u8]) -> Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(stdout)
        .and_then(|()| out.flush())
        .map_err(|e| crate::Error::new("SAIL3111", e.to_string()))
}
