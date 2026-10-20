//! A source build of Verus at the pinned commit (ADR 0019).
//!
//! The deep tier's `rust_verify_test` conformance needs Verus's own test
//! suite, which runs only against a source build: its harness finds
//! `rust_verify`, `vstd.vir` and the macro libraries in the build's
//! `target-verus/<profile>` directory. `cargo xtask build-verus-source`
//! makes the build with Verus's own tooling (`vargo`), under
//! [`build_dir`], never inside this repository, and records what it built
//! in a [`Manifest`]. The harness ([`crate::rvt`]) reads the manifest.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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

/// A feature and toolchain scoped source-build directory. Default builds retain
/// their historical location; selected features always have a distinct identity.
#[must_use]
pub fn variant_dir(cache: &Path, commit: &str, toolchain: &str, features: &[String]) -> PathBuf {
    if features.is_empty() {
        return build_dir(cache, commit);
    }
    let id = format!("{commit}:{toolchain}:{features:?}");
    let hash = format!("{:x}", Sha256::digest(id.as_bytes()));
    cache
        .join("verus-src-build")
        .join(format!("{commit}-{}", &hash[..16]))
}

/// Identifies a source build of the pinned fork with its full test suite.
///
/// This has a separate directory from both stock source builds and installed
/// engine runtimes, whose manifests contain no test executables.
#[must_use]
pub fn fork_test_features() -> Vec<String> {
    let mut features = crate::engine::features();
    // The suite uses vargo's vstd layout, not the release runtime's separately
    // rebuilt vstd. Both its namespace and provenance must state that layout.
    features.retain(|feature| feature != crate::engine::VSTD_BUILD);
    features.push("vstd-build:vargo".to_owned());
    features.push("suite:rust-verify-test+toolchain-manifest".to_owned());
    features
}

fn safe_relative(path: &Path) -> bool {
    path.components()
        .all(|p| matches!(p, std::path::Component::Normal(_)))
}

pub(crate) fn hash_file(path: &Path) -> Result<String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).map_err(|e| Error::new("SAIL3040", e.to_string()))?;
    let mut hash = Sha256::new();
    let mut chunk = vec![0u8; 65536];
    loop {
        let n = file
            .read(&mut chunk)
            .map_err(|e| Error::new("SAIL3040", e.to_string()))?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Hashes every runtime file directly in target-verus/release.
///
/// # Errors
/// `SAIL3040` if an artifact cannot be read.
pub fn artifact_hashes(build: &Path) -> Result<BTreeMap<PathBuf, String>> {
    // Read every runtime file in full, several at once; the first failure in
    // inventory order is reported, as when reading them one by one.
    let files = artifact_files(build)?;
    let hashes = crate::parallel::in_order(&files, |(path, _)| hash_file(path));
    files
        .into_iter()
        .zip(hashes)
        .map(|((_, relative), hash)| Ok((relative, hash?)))
        .collect()
}

/// One managed run's [`artifact_hashes`] of its runtime, read at most once
/// in that run (#286). The run's shipped-proof identity and its retained-reuse
/// admission both bind the complete runtime; the second asks within the same
/// run and gets the bytes the first read. Each run starts with an empty one,
/// so nothing carries over between runs, and a build directory other than the
/// first one asked about is read afresh. A failed read is a failure for the
/// whole run, which both callers treat as "no reuse".
#[derive(Debug, Default)]
pub(crate) struct RunDigests(std::sync::OnceLock<(PathBuf, Option<BTreeMap<PathBuf, String>>)>);

impl RunDigests {
    /// [`artifact_hashes`] of `build`, or `None` if any artifact is unreadable.
    pub(crate) fn of(&self, build: &Path) -> Option<BTreeMap<PathBuf, String>> {
        let canonical = build.canonicalize().ok()?;
        let (first, hashes) = self
            .0
            .get_or_init(|| (canonical.clone(), artifact_hashes(&canonical).ok()));
        if *first == canonical {
            hashes.clone()
        } else {
            artifact_hashes(&canonical).ok()
        }
    }
}

/// Enumerates the complete runtime inventory with a caller's same-phase hasher.
pub(crate) fn artifact_hashes_with(
    build: &Path,
    mut hash: impl FnMut(&Path) -> Result<String>,
) -> Result<BTreeMap<PathBuf, String>> {
    artifact_files(build)?
        .into_iter()
        .map(|(path, relative)| Ok((relative, hash(&path)?)))
        .collect()
}

/// Every runtime file: its path and its path relative to `build`.
fn artifact_files(build: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let dir = checkout_dir(build).join("source/target-verus/release");
    let entries = std::fs::read_dir(&dir).map_err(|e| Error::new("SAIL3040", e.to_string()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| Error::new("SAIL3040", e.to_string()))?
            .path();
        if path.is_file() {
            let relative = path
                .strip_prefix(build)
                .map_err(|e| Error::new("SAIL3040", e.to_string()))?
                .to_path_buf();
            files.push((path, relative));
        }
    }
    Ok(files)
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
    /// SHA-256 of the executable produced by the recorded source build.
    /// Older manifests deserialize without a digest, but cannot be reused.
    #[serde(default)]
    pub sha256: String,
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
    /// Cargo features enabled; optional Singular builds are isolated (ADR 0021).
    pub features: Vec<String>,
    /// SHA-256 of runtime artifacts, relative to this build.
    #[serde(default)]
    pub artifacts: BTreeMap<PathBuf, String>,
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

    /// Whether the recorded runtime and every named test binary match their bytes.
    ///
    /// Manifests predating test-binary digests require a rebuild. Names and
    /// executable paths must be nonempty and unique within the suite.
    #[must_use]
    pub fn is_complete(&self, build: &Path) -> bool {
        let mut names = BTreeSet::new();
        let mut paths = BTreeSet::new();
        !self.test_binaries.is_empty()
            && self.artifacts.iter().all(|(p, hash)| {
                safe_relative(p) && hash_file(&build.join(p)).ok().as_ref() == Some(hash)
            })
            && self.test_binaries.iter().all(|b| {
                !b.name.is_empty()
                    && names.insert(&b.name)
                    && paths.insert(&b.path)
                    && safe_relative(&b.path)
                    && build.join(&b.path).is_file()
                    && hash_file(&build.join(&b.path)).ok().as_ref() == Some(&b.sha256)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One run reads its runtime once; another run, or another build, reads
    /// the bytes as they are then (#286).
    #[test]
    fn run_digests_are_read_once_per_run_and_never_shared_between_runs()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-run-digests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (first, second) = (root.join("first"), root.join("second"));
        for build in [&first, &second] {
            let runtime = checkout_dir(build).join("source/target-verus/release");
            std::fs::create_dir_all(&runtime)?;
            std::fs::write(runtime.join("verus"), b"launcher")?;
        }
        let launcher = checkout_dir(&first).join("source/target-verus/release/verus");
        let run = RunDigests::default();
        let read = run.of(&first).ok_or("unreadable")?;
        assert_eq!(read, artifact_hashes(&first.canonicalize()?)?);
        std::fs::write(&launcher, b"replaced")?;
        assert_eq!(run.of(&first), Some(read.clone()));
        assert_eq!(
            run.of(&second),
            Some(artifact_hashes(&second.canonicalize()?)?)
        );
        let next = RunDigests::default().of(&first).ok_or("unreadable")?;
        assert_ne!(next, read);
        assert!(RunDigests::default().of(&root.join("absent")).is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

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
            rust_toolchain: "1.99.1".to_owned(),
            z3_sha256: "00".to_owned(),
            features: Vec::new(),
            build_seconds: 1,
            artifacts: BTreeMap::new(),
            test_binaries: vec![TestBinary {
                name: "basic".to_owned(),
                path: PathBuf::from("bin/basic-123"),
                sha256: format!("{:x}", Sha256::digest(b"built test executable")),
            }],
        };
        assert!(m.save(&dir).is_ok());
        assert_eq!(Manifest::load(&dir).ok(), Some(m.clone()));
        assert!(!m.is_complete(&dir));
        assert!(std::fs::write(dir.join("bin/basic-123"), "built test executable").is_ok());
        assert!(m.is_complete(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Manifest::load(&dir).is_err_and(|e| e.code == "SAIL3040"));
    }

    #[test]
    fn test_binary_changes_and_incomplete_entries_are_rejected() {
        let dir = std::env::temp_dir().join(format!("sail-suite-hash-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&dir).is_ok());
        assert!(std::fs::write(dir.join("runtime"), "runtime").is_ok());
        assert!(std::fs::write(dir.join("test"), "original").is_ok());
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            commit: "abc".to_owned(),
            rust_toolchain: "1".to_owned(),
            z3_sha256: "hash".to_owned(),
            features: vec![],
            build_seconds: 0,
            artifacts: BTreeMap::from([(
                "runtime".into(),
                hash_file(&dir.join("runtime")).unwrap_or_default(),
            )]),
            test_binaries: vec![TestBinary {
                name: "test".to_owned(),
                path: "test".into(),
                sha256: hash_file(&dir.join("test")).unwrap_or_default(),
            }],
        };
        assert!(manifest.is_complete(&dir));
        // Only suite bytes change; the runtime and its recorded digest stay fixed.
        for bytes in ["modified", ""] {
            assert!(std::fs::write(dir.join("test"), bytes).is_ok());
            assert!(!manifest.is_complete(&dir));
        }
        assert!(std::fs::write(dir.join("test"), "original").is_ok());
        assert!(manifest.is_complete(&dir));
        for digest in ["", "not-a-digest"] {
            let mut incomplete = manifest.clone();
            incomplete.test_binaries[0].sha256 = digest.to_owned();
            assert!(!incomplete.is_complete(&dir));
        }
        let mut duplicate = manifest.clone();
        duplicate
            .test_binaries
            .push(manifest.test_binaries[0].clone());
        assert!(!duplicate.is_complete(&dir));
        duplicate.test_binaries[1].name = "other-name".into();
        assert!(!duplicate.is_complete(&dir));
        assert!(std::fs::write(dir.join("other-test"), "original").is_ok());
        duplicate.test_binaries[1].name = "test".into();
        duplicate.test_binaries[1].path = "other-test".into();
        assert!(!duplicate.is_complete(&dir));
        let mut unnamed = manifest.clone();
        unnamed.test_binaries[0].name.clear();
        assert!(!unnamed.is_complete(&dir));
        let mut legacy = serde_json::to_value(&manifest).unwrap_or_default();
        assert!(
            legacy["test_binaries"][0]
                .as_object_mut()
                .and_then(|entry| entry.remove("sha256"))
                .is_some()
        );
        let legacy: Manifest =
            serde_json::from_value(legacy).unwrap_or_else(|e| unreachable!("legacy manifest: {e}"));
        assert!(!legacy.is_complete(&dir));
        assert!(std::fs::remove_file(dir.join("test")).is_ok());
        assert!(!manifest.is_complete(&dir));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn feature_builds_and_toolchains_have_distinct_locations() {
        let features = vec!["singular".to_owned()];
        let cache = Path::new("/cache");
        assert_ne!(
            variant_dir(cache, "commit", "1", &features),
            build_dir(cache, "commit")
        );
        assert_ne!(
            variant_dir(cache, "commit", "1", &features),
            variant_dir(cache, "commit", "2", &features)
        );
    }

    #[test]
    fn fork_suite_identity_binds_its_actual_vstd_layout() {
        let suite = fork_test_features();
        let engine = crate::engine::features();
        assert!(suite.iter().any(|feature| feature == "vstd-build:vargo"));
        assert!(
            !suite
                .iter()
                .any(|feature| feature == crate::engine::VSTD_BUILD)
        );
        assert!(
            suite
                .iter()
                .any(|feature| feature.starts_with("sail-fork:"))
        );
        assert!(
            suite
                .iter()
                .any(|feature| feature.starts_with("source-lock:"))
        );
        assert!(suite.iter().any(|feature| feature.starts_with("host:")));
        assert_ne!(
            variant_dir(Path::new("cache"), "commit", "toolchain", &suite),
            variant_dir(Path::new("cache"), "commit", "toolchain", &engine)
        );
    }

    #[test]
    fn corrupt_runtime_artifacts_and_traversal_are_rejected() {
        let dir = std::env::temp_dir().join(format!("sail-build-hash-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&dir).is_ok());
        assert!(std::fs::write(dir.join("solver"), "original").is_ok());
        let mut manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            commit: "abc".to_owned(),
            rust_toolchain: "1".to_owned(),
            z3_sha256: "hash".to_owned(),
            features: vec![],
            build_seconds: 0,
            test_binaries: vec![TestBinary {
                name: "test".to_owned(),
                path: "solver".into(),
                sha256: hash_file(&dir.join("solver")).unwrap_or_default(),
            }],
            artifacts: BTreeMap::from([(
                "solver".into(),
                hash_file(&dir.join("solver")).unwrap_or_default(),
            )]),
        };
        assert!(manifest.is_complete(&dir));
        assert!(std::fs::write(dir.join("solver"), "corrupt").is_ok());
        assert!(!manifest.is_complete(&dir));
        manifest.test_binaries[0].path = "../solver".into();
        manifest.artifacts.clear();
        assert!(!manifest.is_complete(&dir));
        let _ = std::fs::remove_dir_all(dir);
    }
}
