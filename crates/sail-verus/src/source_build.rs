//! A source build of Verus at the pinned commit (ADR 0019).
//!
//! The deep tier's `rust_verify_test` conformance needs Verus's own test
//! suite, which runs only against a source build: its harness finds
//! `rust_verify`, `vstd.vir` and the macro libraries in the build's
//! `target-verus/<profile>` directory. `cargo xtask build-verus-source`
//! makes the build with Verus's own tooling (`vargo`), under
//! [`build_dir`], never inside this repository, and records what it built
//! in a [`Manifest`]. The harness ([`crate::rvt`]) reads the manifest.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The manifest's file name, in the build directory.
pub const MANIFEST: &str = "sail-build.json";

/// Takes exclusive ownership of this source build until the returned file drops.
///
/// Deep suites share materialized test directories, and rebuilding prunes runtime
/// files. Concurrent use must fail before either operation touches those files.
///
/// # Errors
///
/// `SAIL3043` if another process owns the build or its lock cannot be opened.
pub fn lock_build(build: &Path) -> Result<std::fs::File> {
    std::fs::create_dir_all(build).map_err(|e| {
        Error::new(
            "SAIL3043",
            format!("cannot create {}: {e}", build.display()),
        )
    })?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(build.join("sail-build.lock"))
        .map_err(|e| Error::new("SAIL3043", format!("cannot open source build lock: {e}")))?;
    file.try_lock().map_err(|e| {
        Error::new(
            "SAIL3043",
            format!(
                "source build {} is in use or cannot be locked: {e}",
                build.display()
            ),
        )
    })?;
    Ok(file)
}

/// The manifest schema this crate reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Where the source build of `commit` lives: `<cache>/verus-src-build/<commit>`.
#[must_use]
pub fn build_dir(cache: &Path, commit: &str) -> PathBuf {
    cache.join("verus-src-build").join(commit)
}

/// The Verus checkout inside a build directory.
#[must_use]
pub fn checkout_dir(build: &Path) -> PathBuf {
    build.join("verus")
}

/// Fetches the Verus source at `commit` (shallow) into the build's checkout
/// directory, unless it is already there, and returns that directory.
///
/// # Errors
///
/// `SAIL3014` if git fails.
pub fn fetch_source(build: &Path, commit: &str) -> Result<PathBuf> {
    let dir = checkout_dir(build);
    crate::corpus::fetch_git(crate::corpus::VERUS_REPO_URL, commit, &dir, None)?;
    Ok(dir)
}

/// One `rust_verify_test` test binary (one per file in `tests/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestBinary {
    /// The test target's name, for example `modes`.
    pub name: String,
    /// The executable, relative to the build directory.
    pub path: PathBuf,
}

/// What a source build contains, written once the build succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The Verus commit built.
    pub commit: String,
    /// The Rust toolchain it was built with.
    pub rust_toolchain: String,
    /// SHA-256 of the z3 it was built and tested with (the pinned one).
    pub z3_sha256: String,
    /// Cargo features enabled (none so far: Verus's `singular` feature is
    /// not built; ADR 0019).
    pub features: Vec<String>,
    /// How long the build took, in seconds.
    pub build_seconds: u64,
    /// The `rust_verify_test` test binaries, sorted by name.
    pub test_binaries: Vec<TestBinary>,
}

impl Manifest {
    /// Reads the manifest of the build in `build`.
    ///
    /// # Errors
    ///
    /// `SAIL3040` if there is no build there or its manifest cannot be read.
    pub fn load(build: &Path) -> Result<Self> {
        let path = build.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Error::new(
                "SAIL3040",
                format!(
                    "no Verus source build at {} ({e}); run `cargo xtask build-verus-source`",
                    build.display()
                ),
            )
        })?;
        let manifest: Self = serde_json::from_str(&text)
            .map_err(|e| Error::new("SAIL3040", format!("cannot parse {}: {e}", path.display())))?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(Error::new(
                "SAIL3040",
                format!(
                    "{} has schema {}, expected {SCHEMA_VERSION}; rebuild with `cargo xtask build-verus-source --force`",
                    path.display(),
                    manifest.schema_version
                ),
            ));
        }
        Ok(manifest)
    }

    /// Writes the manifest into `build`.
    ///
    /// # Errors
    ///
    /// `SAIL3040` if it cannot be written.
    pub fn save(&self, build: &Path) -> Result<()> {
        let path = build.join(MANIFEST);
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| Error::new("SAIL3040", format!("cannot encode the manifest: {e}")))?;
        std::fs::write(&path, json)
            .map_err(|e| Error::new("SAIL3040", format!("cannot write {}: {e}", path.display())))
    }

    /// Whether every test binary the manifest names exists.
    #[must_use]
    pub fn is_complete(&self, build: &Path) -> bool {
        !self.test_binaries.is_empty()
            && self
                .test_binaries
                .iter()
                .all(|b| build.join(&b.path).is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_build_ownership_is_exclusive_and_released_on_drop() {
        let build = std::env::temp_dir().join(format!("sail-build-lock-{}", std::process::id()));
        let first = lock_build(&build).unwrap_or_else(|e| unreachable!("first lock: {e}"));
        assert!(lock_build(&build).is_err_and(|e| e.code == "SAIL3043"));
        drop(first);
        let second = lock_build(&build).unwrap_or_else(|e| unreachable!("released lock: {e}"));
        drop(second);
        let _ = std::fs::remove_dir_all(build);
    }

    #[test]
    fn build_dirs_are_per_commit() {
        assert_eq!(
            build_dir(Path::new("/c"), "abc"),
            Path::new("/c/verus-src-build/abc")
        );
    }

    #[test]
    fn manifests_round_trip_and_check_their_binaries() {
        let dir = std::env::temp_dir().join(format!("sail-srcbuild-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(dir.join("bin")).is_ok());
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            commit: "abc".to_owned(),
            rust_toolchain: "1.98.1".to_owned(),
            z3_sha256: "00".to_owned(),
            features: Vec::new(),
            build_seconds: 1,
            test_binaries: vec![TestBinary {
                name: "basic".to_owned(),
                path: PathBuf::from("bin/basic-123"),
            }],
        };
        assert!(m.save(&dir).is_ok());
        assert_eq!(Manifest::load(&dir).ok(), Some(m.clone()));
        assert!(!m.is_complete(&dir));
        assert!(std::fs::write(dir.join("bin/basic-123"), "").is_ok());
        assert!(m.is_complete(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Manifest::load(&dir).is_err_and(|e| e.code == "SAIL3040"));
    }
}
