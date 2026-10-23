//! Sail's Verus: the runtime built from one pinned commit of Sail's Verus
//! fork (`engine/verus/fork.toml`), and the source-integrated focus/full
//! proof engine it contains (ADR 0027).
//!
//! Every verification Sail runs uses this runtime (ADR 0043): ordinary
//! verification through [`Installed::locate_verified`], and the repair
//! engine through [`Installed::locate`]. Stock releases and stock source
//! builds are never substituted silently.
use std::path::{Path, PathBuf};

use crate::source_build::{self, Manifest};
use crate::toolchain::Pins;
use crate::{Error, Result};

/// The fork pin: repository, branch, exact commit and upstream base.
pub const FORK: &str = include_str!("../../../engine/verus/fork.toml");
/// Source-event sidecar contract version.
pub const EVENT_VERSION: u32 = 1;
/// Exact dependency resolution used for the optional source worker.
pub const SOURCE_LOCK: &str = include_str!("../../../engine/verus/Cargo.lock");

/// The `[fork]` table of `engine/verus/fork.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Fork {
    /// Repository URL (public; fetched without credentials).
    pub repo: String,
    /// Branch the commit is on.
    pub branch: String,
    /// Full commit SHA the engine is built from.
    pub commit: String,
    /// Upstream Verus commit the branch is based on.
    pub upstream: String,
}

/// The parsed fork pin.
///
/// # Errors
/// `SAIL3120` if `engine/verus/fork.toml` is malformed or its commit is not a
/// full 40-digit hexadecimal SHA.
pub fn fork() -> Result<Fork> {
    #[derive(serde::Deserialize)]
    struct File {
        fork: Fork,
    }
    let file: File = toml::from_str(FORK)
        .map_err(|e| Error::new("SAIL3120", format!("invalid engine/verus/fork.toml: {e}")))?;
    let full_sha = |s: &str| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if !full_sha(&file.fork.commit) || !full_sha(&file.fork.upstream) {
        return Err(Error::new(
            "SAIL3120",
            "engine/verus/fork.toml must pin full commit SHAs",
        ));
    }
    Ok(file.fork)
}

/// Exact fork commit; part of build and execution identities. A malformed
/// pin yields an identity no build can have, so validation fails closed.
#[must_use]
pub fn fork_commit() -> String {
    fork().map_or_else(|_| "invalid-fork-pin".to_owned(), |fork| fork.commit)
}

/// Features identifying this distinct source variant.
#[must_use]
pub fn features() -> Vec<String> {
    vec![
        format!("sail-fork:{}", fork_commit()),
        format!("host:{}", crate::toolchain::host()),
        format!(
            "source-lock:{}",
            crate::managed::digest(SOURCE_LOCK.as_bytes())
        ),
        VSTD_BUILD.to_owned(),
    ]
}

/// The Verus Cargo features a build's manifest records: the plain entries of
/// its `features` list (for example `singular`), as opposed to the
/// `prefix:value` tags that identify a source variant.
#[must_use]
pub fn cargo_features(features: &[String]) -> Vec<String> {
    features
        .iter()
        .filter(|feature| !feature.contains(':'))
        .cloned()
        .collect()
}

/// The Verus Cargo features the engine runtime is built with. Engine
/// validation requires its manifest's `features` to equal [`features`], so
/// this is what the validated manifest records: none today, like the stock
/// release.
#[must_use]
pub fn runtime_cargo_features() -> Vec<String> {
    cargo_features(&features())
}

/// The features named by a project's `verus-feature:<name>` requirements
/// that are not in `available`, in order and without duplicates.
#[must_use]
pub fn missing_features<'a>(
    requires: impl IntoIterator<Item = &'a String>,
    available: &[String],
) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for feature in requires
        .into_iter()
        .filter_map(|requirement| requirement.strip_prefix("verus-feature:"))
    {
        if !available.iter().any(|f| f == feature) && !missing.iter().any(|f| f == feature) {
            missing.push(feature.to_owned());
        }
    }
    missing
}

/// Verus's diagnostic for a construct that needs a Cargo feature its build
/// lacks, for example ``Please cargo build with `--features singular` to use
/// integer_ring attribute``. Verus emits it only for a function it would
/// verify, so it is exactly the case in which the engine cannot answer.
const FEATURE_DIAGNOSTIC: &str = "Please cargo build with `--features ";

/// The first Verus Cargo feature that `output` (a verifier's stdout or
/// stderr, rendered or JSON) reports as required but missing from
/// `available`.
#[must_use]
pub fn required_feature(output: &[u8], available: &[String]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let mut rest = text.as_ref();
    while let Some(at) = rest.find(FEATURE_DIAGNOSTIC) {
        rest = &rest[at + FEATURE_DIAGNOSTIC.len()..];
        let name = rest.split('`').next().unwrap_or_default();
        let valid = !name.is_empty()
            && name.len() < rest.len()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if valid && !available.iter().any(|f| f == name) {
            return Some(name.to_owned());
        }
    }
    None
}

/// The marker a target's sidecar holds when the engine lacked a Verus
/// feature and the target fell back to stock verification (SAIL3124). It
/// contains the feature's name.
pub(crate) const MISSING_FEATURE: &str = "engine.missing-feature";

/// The features the engine lacked in this run, from the targets that fell
/// back to stock verification because of them.
fn missing_in_run(run: &Path) -> Vec<String> {
    let mut missing = std::collections::BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(run.join("sidecars")) {
        for entry in entries.filter_map(std::result::Result::ok) {
            if let Some(bytes) = bounded_document(&entry.path().join(MISSING_FEATURE)) {
                let name = String::from_utf8_lossy(&bytes).trim().to_owned();
                if !name.is_empty() {
                    missing.insert(name);
                }
            }
        }
    }
    missing.into_iter().collect()
}

/// SAIL3124: records, as a warning and a result note, that targets fell back
/// to the configured stock Verus because the engine lacks a feature they
/// need. Returns the fault for repair evidence.
fn missing_feature_fallback(
    run: &Path,
    notes: &mut Vec<String>,
    err: &mut dyn std::io::Write,
) -> Option<sail_api::output::ErrorInfo> {
    let missing = missing_in_run(run);
    if missing.is_empty() {
        return None;
    }
    let names = missing
        .iter()
        .map(|f| format!("`{f}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let message = format!(
        "the interactive engine is built without Verus feature {names}, which this project needs; the affected targets were verified by the configured stock Verus and cannot accept a repair"
    );
    let _ = writeln!(err, "warning[SAIL3124]: {message}");
    notes.push(format!("SAIL3124: {message}"));
    Some(sail_api::output::ErrorInfo {
        code: "SAIL3124".into(),
        message,
    })
}

/// How the engine's vstd is built: like the stock release, with
/// `cargo-verus build --manifest-path vstd/Cargo.toml` after the ordinary
/// build, so vstd's recorded source paths (and therefore diagnostics that
/// point into vstd) match the release's (`seq.rs`, not vargo's
/// `vstd/seq.rs`).
pub const VSTD_BUILD: &str = "vstd-build:cargo-verus-release";

/// Engine-specific source directory; never a stock source build directory.
#[must_use]
pub fn build_dir(cache: &Path, pins: &Pins) -> PathBuf {
    source_build::variant_dir(
        cache,
        &pins.verus.commit,
        &pins.verus.rust_toolchain,
        &features(),
    )
}

/// The engine runtime bundled with a release: `engine/<variant>` beside the
/// directory holding the running executable, so `<release>/bin/sail` finds
/// `<release>/engine/<variant>` (docs/releasing.md). The variant name is the
/// cache variant's, which binds the host and fork commit, so a bundle built
/// for another host or commit is never found. Symlinks to the executable are
/// resolved first.
#[must_use]
pub fn bundled_dir(pins: &Pins) -> Option<PathBuf> {
    let variant = build_dir(Path::new(""), pins).file_name()?.to_owned();
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    Some(exe.parent()?.parent()?.join("engine").join(variant))
}

/// Names a fork build directory to use instead of the cache and the bundle
/// (CI images and development; ADR 0043). It passes the same validation.
pub const BUILD_ENV: &str = "SAIL_VERUS_BUILD";

/// The fork build to use: `$SAIL_VERUS_BUILD` when set, else the cache
/// variant when anything is installed there (`cargo xtask
/// build-verus-engine` or `sail engine install`), else a bundled release
/// runtime ([`bundled_dir`]) when present, else the cache variant, whose
/// absence reports `SAIL3120`. Every candidate passes the same identity and
/// artifact validation before use.
#[must_use]
pub fn resolve_build(cache: &Path, pins: &Pins) -> PathBuf {
    if let Some(build) = std::env::var_os(BUILD_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(build);
    }
    let cached = build_dir(cache, pins);
    if cached.symlink_metadata().is_ok() {
        return cached;
    }
    bundled_dir(pins)
        .filter(|bundled| bundled.join(source_build::MANIFEST).is_file())
        .unwrap_or(cached)
}

/// An engine install whose runtime bytes and base pins were checked.
#[derive(Debug, Clone)]
pub struct Installed {
    /// Owned source variant root.
    pub build: PathBuf,
    /// Engine Cargo-Verus frontend.
    pub cargo_verus: PathBuf,
    /// Engine Verus launcher.
    pub verus: PathBuf,
    /// Exact validated build manifest hash.
    pub manifest_sha256: String,
    /// The Verus Cargo features the manifest records ([`cargo_features`]).
    pub cargo_features: Vec<String>,
}

impl Installed {
    /// Validates the managed engine build before any proof is started.
    ///
    /// # Errors
    /// `SAIL3120` if missing, misidentified, or corrupted. Run
    /// `cargo xtask build-verus-engine` to install the explicit variant.
    pub fn locate(cache: &Path, pins: &Pins) -> Result<Self> {
        Self::at(&resolve_build(cache, pins), pins)
    }

    /// [`Installed::locate`] for every ordinary verification (ADR 0043):
    /// the same identity checks, with the full artifact digests read once per
    /// install. A successful full check records each artifact's device,
    /// inode, size and modification time under
    /// `<cache>/runtime-identities/`; while the runtime directory holds
    /// exactly the manifest's files with those identities, later calls skip
    /// rereading them (as ADR 0041 identifies toolchain files). Any
    /// difference, or an unreadable record, repeats the full check.
    ///
    /// # Errors
    /// `SAIL3120` if missing, misidentified, or corrupted.
    pub fn locate_verified(cache: &Path, pins: &Pins) -> Result<Self> {
        let build = resolve_build(cache, pins);
        let (manifest, manifest_sha256) = preflight(&build, pins)?;
        let record = cache
            .join("runtime-identities")
            .join(format!("verus-{manifest_sha256}.json"));
        let stats = runtime_stats(&build);
        let recorded = std::fs::read(&record)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RuntimeRecord>(&bytes).ok());
        let current = match (&stats, recorded) {
            (Some(stats), Some(recorded)) => {
                recorded.build == build.canonicalize().ok()
                    && recorded.stats == *stats
                    && stats.keys().eq(manifest.artifacts.keys())
            }
            _ => false,
        };
        if !current {
            let installed = Self::at(&build, pins)?;
            if let (Some(stats), Some(parent)) = (stats, record.parent()) {
                let entry = RuntimeRecord {
                    build: build.canonicalize().ok(),
                    stats,
                };
                // Best effort: without the record the next run checks again.
                if std::fs::create_dir_all(parent).is_ok() {
                    let _ = crate::managed::json(&record, &entry);
                }
            }
            return Ok(installed);
        }
        let runtime = runtime_dir(&build);
        Ok(Self {
            cargo_verus: runtime.join("cargo-verus"),
            verus: runtime.join("verus"),
            manifest_sha256,
            cargo_features: cargo_features(&manifest.features),
            build,
        })
    }

    /// Validates an explicitly supplied build without installing or modifying it.
    ///
    /// # Errors
    /// `SAIL3120` when its identity or artifact bytes do not match the engine.
    pub fn at(build: &Path, pins: &Pins) -> Result<Self> {
        Self::at_in_run(build, pins, &source_build::RunDigests::default())
    }

    /// [`Installed::at`] with the artifact digests a managed run has already
    /// read for `build`, read now if it has not.
    pub(crate) fn at_in_run(
        build: &Path,
        pins: &Pins,
        digests: &source_build::RunDigests,
    ) -> Result<Self> {
        let (manifest, manifest_sha256) = preflight(build, pins)?;
        if digests.of(build).as_ref() != Some(&manifest.artifacts) {
            return Err(invalid());
        }
        let runtime = runtime_dir(build);
        Ok(Self {
            cargo_verus: runtime.join("cargo-verus"),
            verus: runtime.join("verus"),
            manifest_sha256,
            cargo_features: cargo_features(&manifest.features),
            build: build.to_owned(),
        })
    }
}

/// A validated runtime's file identities ([`Installed::locate_verified`]).
#[derive(serde::Serialize, serde::Deserialize)]
struct RuntimeRecord {
    build: Option<PathBuf>,
    stats: std::collections::BTreeMap<PathBuf, String>,
}

/// The stat identity of every file directly in the runtime directory, keyed
/// as the manifest keys its artifacts; `None` if any cannot be read.
fn runtime_stats(build: &Path) -> Option<std::collections::BTreeMap<PathBuf, String>> {
    let mut stats = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(runtime_dir(build)).ok()? {
        let path = entry.ok()?.path();
        if path.is_file() {
            let relative = path.strip_prefix(build).ok()?.to_path_buf();
            stats.insert(relative, crate::managed::stat_identity(&path).ok()?);
        }
    }
    Some(stats)
}

fn runtime_dir(build: &Path) -> PathBuf {
    source_build::checkout_dir(build).join("source/target-verus/release")
}

/// How to obtain Sail's Verus (the fork runtime, which is also the repair
/// engine), appended to every "missing" or "mismatched" error so the user is
/// never left without a next step.
pub const OBTAIN: &str = "Release archives bundle Sail's Verus under `engine/` \
beside `bin/sail`, so from a release install this means the install is damaged \
or incomplete: unpack the archive again into a new directory and use its \
`bin/sail` without moving files out of that directory. A build named by \
SAIL_VERUS_BUILD is used first, then one installed in the Sail cache \
(`verus-src-build/` under ~/.cache/sail or SAIL_CACHE_DIR), then the bundled \
one; remove a stale one to use the bundle. In a Sail source checkout, download \
the prebuilt runtime with `python3 ci/engine-artifact.py fetch --pin \
engine/verus/artifact-linux.json --cache ~/.cache/sail` (artifact-macos.json on \
macOS), or build it with `cargo xtask build-verus-engine` (it clones \
https://github.com/Chelis-Lang/verus and needs the pinned Verus from \
`cargo xtask fetch-verus`). Or import a build with `sail engine install --from \
PATH`. Sail never falls back to another Verus.";

fn invalid() -> Error {
    Error::new(
        "SAIL3120",
        format!(
            "Sail's Verus (the fork runtime and interactive engine) does not match this \
             Sail's pins (fork commit {}, Verus toolchain or z3) or its files changed since it \
             was built (a source-checkout build is rebuilt with `cargo xtask build-verus-engine \
             --force`). {OBTAIN}",
            fork_commit()
        ),
    )
}

/// Install provenance exposed by the explicit engine-management command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Status {
    /// Independently versioned install response.
    pub version: u32,
    /// Verus source commit.
    pub commit: String,
    /// Verus Rust toolchain.
    pub rust_toolchain: String,
    /// Exact Verus fork commit the engine is built from.
    pub fork_commit: String,
    /// Exact dependency resolution digest.
    pub source_lock_sha256: String,
    /// Complete build-manifest digest.
    pub manifest_sha256: String,
    /// Verus Cargo features the runtime is built with (none, like the stock
    /// release); projects that need another fall back to stock (SAIL3124).
    pub verus_features: Vec<String>,
    /// Build location resolved to its real source variant.
    pub build: PathBuf,
    /// Selected engine launcher.
    pub verus: PathBuf,
}

/// Reports an available install only after its full artifact validation.
///
/// # Errors
/// `SAIL3120` when the optional build is unavailable or corrupt.
pub fn status(cache: &Path, pins: &Pins) -> Result<Status> {
    let installed = Installed::locate(cache, pins)?;
    Ok(Status {
        version: 2,
        commit: pins.verus.commit.clone(),
        rust_toolchain: pins.verus.rust_toolchain.clone(),
        fork_commit: fork_commit(),
        source_lock_sha256: crate::managed::digest(SOURCE_LOCK.as_bytes()),
        manifest_sha256: installed.manifest_sha256,
        verus_features: installed.cargo_features,
        build: installed
            .build
            .canonicalize()
            .map_err(|e| Error::new("SAIL3120", e.to_string()))?,
        verus: installed.verus,
    })
}

/// Makes one validated source variant available under an independent cache root.
/// Existing installs are never overwritten; the alias is monitored like every
/// other runtime path during managed execution.
///
/// # Errors
/// `SAIL3120` for a wrong build, conflicting destination, or unsupported platform.
pub fn import(cache: &Path, pins: &Pins, source: &Path) -> Result<Status> {
    let installed = Installed::at(source, pins)?;
    let destination = build_dir(cache, pins);
    let source = installed
        .build
        .canonicalize()
        .map_err(|e| Error::new("SAIL3120", e.to_string()))?;
    if !destination.exists() {
        let parent = destination
            .parent()
            .ok_or_else(|| Error::new("SAIL3120", "invalid engine cache path"))?;
        std::fs::create_dir_all(parent).map_err(|e| Error::new("SAIL3120", e.to_string()))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &destination)
            .map_err(|e| Error::new("SAIL3120", e.to_string()))?;
        #[cfg(not(unix))]
        return Err(Error::new(
            "SAIL3120",
            "source-engine import currently requires Unix",
        ));
    }
    if destination
        .canonicalize()
        .map_err(|e| Error::new("SAIL3120", e.to_string()))?
        != source
    {
        return Err(Error::new(
            "SAIL3120",
            "engine cache already refers to a different source variant",
        ));
    }
    status(cache, pins)
}

/// Immutable engine controls passed to the owned driver, never process globals.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Request {
    pub function: Option<String>,
    pub cargo_verus: PathBuf,
    pub manifest: PathBuf,
    #[serde(default)]
    pub manifest_sha256: String,
    pub stock_wrapper: PathBuf,
    pub artifacts: Vec<PathBuf>,
}

/// Borrowed request identity for compiler artifacts and optional sysroot hints.
/// Proof contexts and driver controls continue to serialize the full `Request`.
#[derive(serde::Serialize)]
pub(crate) struct ArtifactRequest<'a> {
    function: Option<&'a str>,
    cargo_verus: &'a Path,
    manifest: &'a Path,
    manifest_sha256: &'a str,
    stock_wrapper: &'a Path,
    artifacts: &'a [PathBuf],
}

impl Request {
    pub(crate) fn artifact_identity(&self) -> ArtifactRequest<'_> {
        // Enumerate every field so adding a request control requires deciding
        // its artifact identity explicitly. Only the driver-local selector is
        // omitted; no owned strings, paths or artifact vectors are cloned.
        let Self {
            function: _,
            cargo_verus,
            manifest,
            manifest_sha256,
            stock_wrapper,
            artifacts,
        } = self;
        ArtifactRequest {
            function: None,
            cargo_verus,
            manifest,
            manifest_sha256,
            stock_wrapper,
            artifacts,
        }
    }

    /// Provenance for a report this request established, or fell back from.
    pub(crate) fn provenance(
        &self,
        established_by: sail_api::output::EngineEstablishment,
    ) -> sail_api::output::EngineProvenance {
        sail_api::output::EngineProvenance {
            name: "interactive".to_owned(),
            manifest_sha256: self.manifest_sha256.clone(),
            established_by,
        }
    }

    /// Project-level engine identity.
    pub(crate) fn identity(&self) -> sail_api::project::EngineIdentity {
        sail_api::project::EngineIdentity {
            name: "interactive".to_owned(),
            manifest_sha256: self.manifest_sha256.clone(),
        }
    }
}

fn request(config: &crate::managed::Config, function: Option<String>) -> Result<(Request, String)> {
    let pins = Pins::embedded()?;
    let build = resolve_build(&config.workspace.join(&config.cache), &pins);
    // This private request has not authorized execution. The context phase
    // validates every artifact's bytes before probing an engine executable.
    let (manifest, manifest_sha256) = preflight(&build, &pins)?;
    let manifest_path = build.join(source_build::MANIFEST);
    let stock_wrapper = config
        .cargo_verus
        .canonicalize()
        .map_err(|e| Error::new("SAIL3120", e.to_string()))?
        .with_file_name("verus");
    let request = Request {
        function,
        cargo_verus: runtime_dir(&build).join("cargo-verus"),
        manifest: manifest_path,
        manifest_sha256: manifest_sha256.clone(),
        stock_wrapper,
        artifacts: manifest
            .artifacts
            .keys()
            .map(|path| build.join(path))
            .collect(),
    };
    Ok((request, manifest_sha256))
}

fn preflight(build: &Path, pins: &Pins) -> Result<(Manifest, String)> {
    let manifest_path = build.join(source_build::MANIFEST);
    let bytes = std::fs::read(&manifest_path).map_err(|e| {
        let what = if e.kind() == std::io::ErrorKind::NotFound {
            "Sail's Verus (the fork runtime and interactive engine) is not installed".to_owned()
        } else {
            format!("cannot read the build manifest of Sail's Verus ({e})")
        };
        Error::new(
            "SAIL3120",
            format!("{what}: expected {}. {OBTAIN}", manifest_path.display()),
        )
    })?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| Error::new("SAIL3120", format!("invalid engine manifest: {e}")))?;
    let runtime = runtime_dir(build);
    if manifest.schema_version != source_build::SCHEMA_VERSION
        || manifest.commit != pins.verus.commit
        || manifest.rust_toolchain != pins.verus.rust_toolchain
        || manifest.features != features()
        || manifest.artifacts.is_empty()
        || manifest.artifacts.iter().any(|(path, hash)| {
            path.components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
                || hash.is_empty()
        })
        || pins
            .assets
            .get(crate::toolchain::host())
            .is_none_or(|asset| asset.z3_sha256 != manifest.z3_sha256)
        || ["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"]
            .iter()
            .any(|name| !runtime.join(name).is_file())
    {
        return Err(invalid());
    }
    Ok((manifest, crate::managed::digest(&bytes)))
}

/// Validates the complete engine inventory using the context's exact byte reads.
/// This must precede every engine executable probe, including version/sysroot.
pub(crate) fn validate_request(
    request: &Request,
    mut hash: impl FnMut(&Path) -> Result<String>,
) -> Result<()> {
    let build = request
        .manifest
        .parent()
        .ok_or_else(|| Error::new("SAIL3120", "invalid engine manifest path"))?;
    let manifest_hash = hash(&request.manifest)?;
    let (manifest, parsed_sha256) = preflight(build, &Pins::embedded()?)?;
    let artifacts: Vec<_> = manifest
        .artifacts
        .keys()
        .map(|path| build.join(path))
        .collect();
    let runtime = runtime_dir(build);
    let valid = request
        .manifest
        .file_name()
        .is_some_and(|name| name == source_build::MANIFEST)
        && manifest_hash == request.manifest_sha256
        && manifest_hash == parsed_sha256
        && request.cargo_verus == runtime.join("cargo-verus")
        && request.artifacts == artifacts
        && source_build::artifact_hashes_with(build, &mut hash)? == manifest.artifacts;
    if !valid {
        return Err(invalid());
    }
    Ok(())
}

/// Performs explicit interactive whole-target verification using the optional build.
///
/// # Errors
/// `SAIL3120` for an unavailable engine; ordinary verification remains available.
pub fn verify(
    config: &crate::managed::Config,
    err: &mut dyn std::io::Write,
) -> Result<sail_api::project::ProjectResult> {
    verify_with(config, None, err)
}

/// [`verify`], also showing Verus's own stdout on `out` as plain
/// `cargo verus verify` would print it.
///
/// # Errors
///
/// As [`verify`].
pub fn verify_showing_output(
    config: &crate::managed::Config,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<sail_api::project::ProjectResult> {
    verify_with(config, Some(out), err)
}

fn verify_with(
    config: &crate::managed::Config,
    out: Option<&mut dyn std::io::Write>,
    err: &mut dyn std::io::Write,
) -> Result<sail_api::project::ProjectResult> {
    let (request, _) = request(config, None)?;
    // The held workspace keeps `sail cache projects prune` away until the
    // run's records have been read.
    match crate::managed::run_engine_held(config, out, err, Some(request)) {
        Ok((mut result, _workspace)) => {
            let run = run_directory(config, &result);
            launcher_notes(&run, &mut result);
            missing_feature_fallback(&run, &mut result.notes, err);
            Ok(result)
        }
        Err(error) => stock_fallback(config, err, error).map(|(result, _)| result),
    }
}

/// Repair evidence after a batch-level engine fault: the stock full result
/// only, never focused evidence or acceptance.
fn fallback_batch(
    full: sail_api::project::ProjectResult,
    fault: sail_api::output::ErrorInfo,
    manifest_sha256: String,
) -> sail_api::repair::BatchResult {
    sail_api::repair::BatchResult {
        batched: false,
        selection_error: None,
        focus: None,
        compiler_invocations: full.execution.verifier_invocations,
        full: Some(full),
        manifest_sha256,
        engine_fault: Some(fault),
    }
}

/// ADR 0027: an engine fault that escapes every per-target fallback (for
/// example a crash while the engine identifies its toolchain) falls back to
/// independent stock verification.
/// The stock result is labelled and never accepts a repair. Missing or
/// incompatible installations (`SAIL3120`) are rejected instead, and an error
/// that also stops stock verification is returned unchanged.
fn stock_fallback(
    config: &crate::managed::Config,
    err: &mut dyn std::io::Write,
    error: Error,
) -> Result<(
    sail_api::project::ProjectResult,
    sail_api::output::ErrorInfo,
)> {
    if error.code == "SAIL3120" {
        return Err(error);
    }
    let fault = sail_api::output::ErrorInfo {
        code: "SAIL3123".into(),
        message: format!(
            "interactive engine fault ({}): {error}; independent stock verification follows",
            error.code
        ),
    };
    let _ = writeln!(err, "warning[SAIL3123]: {}", fault.message);
    let mut result = crate::managed::run_engine(config, err, None)?;
    result.notes.push(format!(
        "SAIL3123: the interactive engine faulted ({}); this is an independent stock Verus result and cannot accept a repair",
        error.code
    ));
    Ok((result, fault))
}

/// Runs one focused repair pass and independent full validation in compiler batches.
/// Focused proof evidence never authorizes a complete target receipt.
///
/// # Errors
/// `SAIL3120` for unavailable engine or invalid selection; managed runner errors
/// retain their stable codes.
pub fn repair(
    config: &crate::managed::Config,
    function: &str,
    err: &mut dyn std::io::Write,
) -> Result<sail_api::repair::BatchResult> {
    if function.is_empty() || !function.contains("::") || crate::managed::focused(&config.args) {
        return Err(Error::new(
            "SAIL3120",
            "repair requires one exact crate-qualified function and independent full scope",
        ));
    }
    let (request, manifest_sha256) = request(config, Some(function.to_owned()))?;
    let mut config = config.clone();
    config.force = true;
    let (mut full, _workspace) =
        match crate::managed::run_engine_held(&config, None, err, Some(request)) {
            Ok(held) => held,
            Err(error) => {
                let (full, fault) = stock_fallback(&config, err, error)?;
                return Ok(fallback_batch(full, fault, manifest_sha256));
            }
        };
    let run = run_directory(&config, &full);
    launcher_notes(&run, &mut full);
    let engine_fault = missing_feature_fallback(&run, &mut full.notes, err);
    let mut selected = Vec::new();
    let mut selection_error = None;
    let gathered = (|| -> Result<()> {
        for entry in std::fs::read_dir(run.join("sidecars"))
            .map_err(|e| Error::new("SAIL3123", e.to_string()))?
        {
            let entry = entry.map_err(|e| Error::new("SAIL3123", e.to_string()))?;
            let path = entry.path().join("focus.report.json");
            if !path.is_file() {
                continue;
            }
            let focus: FocusTarget = serde_json::from_slice(
                &std::fs::read(path).map_err(|e| Error::new("SAIL3123", e.to_string()))?,
            )
            .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
            if focus.function != function {
                continue;
            }
            if let Some(target) = full.targets.iter().find(|target| {
                target.requested
                    && target.package_id == focus.package_id
                    && target.target == focus.target
                    && target.kind.contains(&focus.kind)
            }) {
                let mut target = target.clone();
                target.report = focus.report;
                target.complete = false;
                target.reused = false;
                selected.push(target);
            }
        }
        Ok(())
    })();
    if let Err(error) = gathered {
        selected.clear();
        selection_error = Some(sail_api::output::ErrorInfo {
            code: error.code.into(),
            message: error.to_string(),
        });
    }
    if selected.len() > 1 {
        selected.clear();
        selection_error = Some(sail_api::output::ErrorInfo {
            code: "SAIL3122".into(),
            message: "repair function matched more than one selected Cargo target".into(),
        });
    }
    let focus = if selected.is_empty() {
        None
    } else {
        let mut result = full.clone();
        result.exit_code = Some(i32::from(
            selected
                .iter()
                .any(|target| target.report.exit_code != Some(0)),
        ));
        result.targets = selected;
        result
            .notes
            .push("Explicit focused evidence; independent full validation follows".into());
        Some(result)
    };
    // A target that fell back for a missing feature has no focused evidence;
    // the fault, not the selector, explains that.
    if focus.is_none() && selection_error.is_none() && engine_fault.is_none() {
        selection_error = Some(sail_api::output::ErrorInfo {
            code: "SAIL3122".into(),
            message: "exact repair function was not established in the selected target".into(),
        });
    }
    Ok(sail_api::repair::BatchResult {
        batched: focus.is_some(),
        selection_error,
        focus,
        compiler_invocations: full.execution.verifier_invocations,
        full: Some(full),
        manifest_sha256,
        engine_fault,
    })
}

fn run_directory(
    config: &crate::managed::Config,
    result: &sail_api::project::ProjectResult,
) -> PathBuf {
    config
        .workspace
        .join(&config.cache)
        .join("projects")
        .join(crate::managed::digest(
            Path::new(&result.workspace).as_os_str().as_encoded_bytes(),
        ))
        .join(&result.context_sha256)
        .join("runs")
        .join(&result.run_id)
}

#[derive(serde::Deserialize)]
struct LauncherAdaptation {
    version: u32,
    run_id: String,
    context_sha256: String,
    project_package_id: String,
    target: String,
    kind: String,
    forwarded: Vec<String>,
    engine_only: bool,
    adaptation: String,
}

impl LauncherAdaptation {
    fn matches(&self, run: &str, context: &str) -> bool {
        self.version == 1
            && self.run_id == run
            && self.context_sha256 == context
            && !self.project_package_id.is_empty()
            && !self.target.is_empty()
            && !self.kind.is_empty()
            && self.engine_only
            && self.forwarded == ["--no-vstd"]
            && self.adaptation == "relocate-exact-effective-token"
    }
}

fn launcher_notes(run: &Path, result: &mut sail_api::project::ProjectResult) {
    use std::io::Read as _;
    let Ok(entries) = std::fs::read_dir(run.join("sidecars")) else {
        return;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let Ok(file) = std::fs::File::open(entry.path().join("launcher-options.json")) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file.take(1024 * 1024 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > 1024 * 1024
        {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<LauncherAdaptation>(&bytes) else {
            continue;
        };
        if record.matches(&result.run_id, &result.context_sha256)
            && result.targets.iter().any(|target| {
                target.package_id == record.project_package_id
                    && target.target == record.target
                    && target.kind.contains(&record.kind)
            })
        {
            result
                .notes
                .push("Engine applies the requested no-vstd macro mode (VI-0011).".into());
            return;
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct FocusTarget {
    pub function: String,
    pub package_id: String,
    pub target: String,
    pub kind: String,
    pub report: sail_api::output::Report,
}

fn completed_driver(run: &Path, sidecar: &Path) -> bool {
    let valid = || -> Option<bool> {
        if sidecar.join("engine.failed.stdout").try_exists().ok()? {
            return Some(false);
        }
        let key = sidecar.file_name()?.to_str()?;
        let invocation: crate::managed::Invocation = serde_json::from_slice(&bounded_document(
            &run.join("invocations").join(format!("{key}.json")),
        )?)
        .ok()?;
        if invocation.identity.key() != key || invocation.run_id != run.file_name()?.to_str()? {
            return Some(false);
        }
        let stdout = bounded_document(&sidecar.join("verus.stdout"))?;
        Some(crate::engine_events::completed(
            &sidecar.join("engine.events.jsonl"),
            &stdout,
            invocation.report.exit_code,
        ))
    };
    valid() == Some(true)
}

/// Complete full verifier outcome for ordinary compiler-output retention only.
/// Failed proofs remain failed and never gain a successful proof receipt.
/// `engine` selects the source engine's sealed terminal; a stock driver has no
/// seal and is judged by its normal exit and whole structured document.
pub(crate) fn completed_compiler_output(
    run: &Path,
    call: &crate::managed::Invocation,
    engine: bool,
) -> bool {
    let sidecar = run.join("sidecars").join(call.identity.key());
    let driver = if engine {
        completed_driver(run, &sidecar)
    } else {
        completed_stock_driver(run, &sidecar)
    };
    if !driver || call.report.error.is_some() || !call.report.invariant_violations().is_empty() {
        return false;
    }
    let Some(stdout) = bounded_document(&sidecar.join("verus.stdout")) else {
        return false;
    };
    let (documents, _) = crate::attribution::json_documents(&stdout);
    let ([document], [krate]) = (documents.as_slice(), call.report.crates.as_slice()) else {
        return false;
    };
    let results = &document["verification-results"];
    let Some(errors) = results["errors"].as_u64() else {
        return false;
    };
    let success = errors == 0;
    results["is-verifying-entire-crate"] == true
        && results["encountered-vir-error"] == false
        && results["encountered-error"] == !success
        && results["success"] == success
        && krate.verus.verified == results["verified"].as_u64()
        && krate.verus.errors == Some(errors)
        && krate.verus.success == Some(success)
        && krate.verus.encountered_vir_error == Some(false)
        && call.report.exit_code == Some(i32::from(!success))
        && call.success == success
        && (!success || crate::managed::receipt_complete(&call.report))
        && (!engine
            || crate::engine_events::terminal_success(&sidecar.join("engine.events.jsonl"))
                == Some(success))
}

/// The stock counterpart of [`completed_driver`]: this run's recorded
/// completion of the same invocation, and a whole document agreeing with its
/// normal exit.
fn completed_stock_driver(run: &Path, sidecar: &Path) -> bool {
    let valid = || -> Option<bool> {
        let key = sidecar.file_name()?.to_str()?;
        let invocation: crate::managed::Invocation = serde_json::from_slice(&bounded_document(
            &run.join("invocations").join(format!("{key}.json")),
        )?)
        .ok()?;
        if invocation.identity.key() != key || invocation.run_id != run.file_name()?.to_str()? {
            return Some(false);
        }
        let stdout = bounded_document(&sidecar.join("verus.stdout"))?;
        Some(crate::engine_events::stock_completed(
            &stdout,
            invocation.report.exit_code,
        ))
    };
    valid() == Some(true)
}

fn bounded_document(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    const LIMIT: u64 = 32 * 1024 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_pin_is_complete_and_based_on_the_pinned_upstream_commit()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fork = fork()?;
        let pins: Pins = toml::from_str(include_str!("../../../corpus/toolchain.toml"))?;
        assert_eq!(fork.upstream, pins.verus.commit);
        assert_eq!(fork.repo, "https://github.com/Chelis-Lang/verus");
        assert_eq!(features()[0], format!("sail-fork:{}", fork.commit));
        Ok(())
    }

    #[test]
    fn prebuilt_runtime_pins_install_where_default_discovery_looks()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pins: Pins = toml::from_str(include_str!("../../../corpus/toolchain.toml"))?;
        for (text, host) in [
            (
                include_str!("../../../engine/verus/artifact-linux.json"),
                "x86_64-unknown-linux-gnu",
            ),
            (
                include_str!("../../../engine/verus/artifact-macos.json"),
                "aarch64-apple-darwin",
            ),
        ] {
            let pin: serde_json::Value = serde_json::from_str(text)?;
            let mut platform_features = features();
            for feature in &mut platform_features {
                if feature.starts_with("host:") {
                    *feature = format!("host:{host}");
                }
            }
            let expected = source_build::variant_dir(
                Path::new("/cache"),
                &pins.verus.commit,
                &pins.verus.rust_toolchain,
                &platform_features,
            );
            assert_eq!(
                pin["variant"].as_str(),
                expected.file_name().and_then(|name| name.to_str()),
                "the installer and default engine discovery must agree for {host}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_bundled_runtime_is_found_beside_the_executable_only_when_the_cache_has_none()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pins: Pins = toml::from_str(include_str!("../../../corpus/toolchain.toml"))?;
        let exe = std::env::current_exe()?.canonicalize()?;
        let bundled = bundled_dir(&pins).ok_or("no bundled location")?;
        let release = exe
            .parent()
            .and_then(Path::parent)
            .ok_or("no release root")?;
        assert_eq!(bundled.parent(), Some(release.join("engine").as_path()));
        assert_eq!(
            bundled.file_name(),
            build_dir(Path::new("/c"), &pins).file_name()
        );
        // No bundle beside the test binary: the (missing) cache variant
        // stays selected, so its absence reports SAIL3120 as before.
        let cache = std::env::temp_dir().join(format!("sail-engine-bundle-{}", std::process::id()));
        assert_eq!(resolve_build(&cache, &pins), build_dir(&cache, &pins));
        // An installed cache variant, even a dangling alias, always wins.
        let cached = build_dir(&cache, &pins);
        std::fs::create_dir_all(&cached)?;
        assert_eq!(resolve_build(&cache, &pins), cached);
        std::fs::remove_dir_all(&cache)?;
        Ok(())
    }

    /// ADR 0043: ordinary verification reads every artifact in full once,
    /// then trusts unchanged stat identities; any change repeats the full
    /// check, and a corrupt runtime is refused.
    #[test]
    fn verified_runtime_is_rehashed_only_when_its_files_change()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-verus-stamp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("cache");
        let pins = Pins::embedded()?;
        let build = build_dir(&cache, &pins);
        let runtime = runtime_dir(&build);
        std::fs::create_dir_all(&runtime)?;
        for name in ["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"] {
            std::fs::write(runtime.join(name), name)?;
        }
        Manifest {
            schema_version: source_build::SCHEMA_VERSION,
            commit: pins.verus.commit.clone(),
            rust_toolchain: pins.verus.rust_toolchain.clone(),
            z3_sha256: pins
                .assets
                .get(crate::toolchain::host())
                .ok_or("host pin")?
                .z3_sha256
                .clone(),
            features: features(),
            artifacts: source_build::artifact_hashes(&build)?,
            build_seconds: 0,
            test_binaries: Vec::new(),
        }
        .save(&build)?;
        let first = Installed::locate_verified(&cache, &pins)?;
        assert_eq!(first.cargo_verus, runtime.join("cargo-verus"));
        let records: Vec<_> = std::fs::read_dir(cache.join("runtime-identities"))?.collect();
        assert_eq!(records.len(), 1);
        // Unchanged: served from the record.
        let second = Installed::locate_verified(&cache, &pins)?;
        assert_eq!(second.manifest_sha256, first.manifest_sha256);
        // Same size, new bytes and a new modification time: refused.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(runtime.join("z3"), "Z3")?;
        assert_eq!(
            Installed::locate_verified(&cache, &pins)
                .map_err(|e| e.code)
                .err(),
            Some("SAIL3120")
        );
        // A file the manifest does not list: refused.
        std::fs::write(runtime.join("z3"), "z3")?;
        std::fs::write(runtime.join("extra"), "extra")?;
        assert!(Installed::locate_verified(&cache, &pins).is_err());
        std::fs::remove_file(runtime.join("extra"))?;
        assert!(Installed::locate_verified(&cache, &pins).is_ok());
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn context_validation_checks_inventory_and_request_manifest()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-engine-phase-{}", std::process::id()));
        let runtime = source_build::checkout_dir(&root).join("source/target-verus/release");
        std::fs::create_dir_all(&runtime)?;
        for name in ["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"] {
            std::fs::write(runtime.join(name), name)?;
        }
        let pins = Pins::embedded()?;
        let manifest = Manifest {
            schema_version: source_build::SCHEMA_VERSION,
            commit: pins.verus.commit.clone(),
            rust_toolchain: pins.verus.rust_toolchain.clone(),
            z3_sha256: pins
                .assets
                .get(crate::toolchain::host())
                .ok_or("host pin")?
                .z3_sha256
                .clone(),
            features: features(),
            artifacts: source_build::artifact_hashes(&root)?,
            build_seconds: 0,
            test_binaries: Vec::new(),
        };
        manifest.save(&root)?;
        let path = root.join(source_build::MANIFEST);
        let request = Request {
            function: Some("demo::selected".to_owned()),
            cargo_verus: runtime.join("cargo-verus"),
            manifest: path.clone(),
            manifest_sha256: source_build::hash_file(&path)?,
            stock_wrapper: PathBuf::new(),
            artifacts: manifest.artifacts.keys().map(|p| root.join(p)).collect(),
        };
        let mut reads = std::collections::BTreeMap::new();
        validate_request(&request, |path| {
            *reads.entry(path.to_owned()).or_insert(0u32) += 1;
            source_build::hash_file(path)
        })?;
        assert_eq!(reads.len(), 6);
        assert!(reads.values().all(|count| *count == 1));
        std::fs::write(runtime.join("z3"), b"corrupt")?;
        assert!(validate_request(&request, source_build::hash_file).is_err());
        std::fs::write(runtime.join("z3"), b"z3")?;
        std::fs::write(runtime.join("unexpected"), b"not declared")?;
        assert!(validate_request(&request, source_build::hash_file).is_err());
        std::fs::remove_file(runtime.join("unexpected"))?;
        std::fs::remove_file(runtime.join("vstd.vir"))?;
        assert!(validate_request(&request, source_build::hash_file).is_err());
        std::fs::write(runtime.join("vstd.vir"), b"vstd.vir")?;
        let mut changed = manifest.clone();
        changed.build_seconds = 1;
        changed.save(&root)?;
        assert!(validate_request(&request, source_build::hash_file).is_err());
        manifest.save(&root)?;
        // A manifest changed between its context hash and parsing cannot
        // authorize new artifact bytes under the original request digest.
        assert!(
            validate_request(&request, |candidate| {
                let hash = source_build::hash_file(candidate)?;
                if candidate == path {
                    changed.save(&root)?;
                }
                Ok(hash)
            })
            .is_err()
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn launcher_note_requires_exact_run_context_and_relocation_provenance() {
        let value = serde_json::json!({"version":1,"run_id":"r","context_sha256":"c", "project_package_id":"p", "target":"demo", "kind":"lib", "forwarded":["--no-vstd"], "engine_only":true, "adaptation":"relocate-exact-effective-token"});
        let parsed = serde_json::from_value::<LauncherAdaptation>(value.clone()).ok();
        assert!(
            parsed
                .as_ref()
                .is_some_and(|record| record.matches("r", "c"))
        );
        for (key, wrong) in [
            ("run_id", serde_json::json!("another")),
            ("context_sha256", serde_json::json!("other")),
            ("engine_only", serde_json::json!(false)),
            ("adaptation", serde_json::json!("unrecognized")),
            ("project_package_id", serde_json::json!("")),
            ("forwarded", serde_json::json!(["--is-vstd"])),
        ] {
            let mut invalid = value.clone();
            invalid[key] = wrong;
            assert!(
                !serde_json::from_value::<LauncherAdaptation>(invalid)
                    .is_ok_and(|record| record.matches("r", "c"))
            );
        }
        let duplicate = serde_json::to_string(&value)
            .unwrap_or_default()
            .replace("\"version\":1", "\"version\":1,\"version\":1");
        assert!(serde_json::from_str::<LauncherAdaptation>(&duplicate).is_err());
    }

    fn driver_fixture(root: &Path) -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
        let identity = crate::managed::TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "args".into(),
            cargo_name: None,
        };
        let sidecar = root.join("sidecars").join(identity.key());
        std::fs::create_dir_all(&sidecar)?;
        std::fs::create_dir_all(root.join("invocations"))?;
        let stdout = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        let (documents, _) = crate::attribution::json_documents(stdout);
        let attribution = crate::attribution::attribute(
            &documents,
            None,
            &crate::attribution::Provenance::NoCache,
            None,
        );
        let report = crate::report::build(&crate::report::Inputs {
            args: &[],
            exit_code: Some(0),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        });
        let invocation = crate::managed::Invocation {
            retained_from: None,
            run_id: root
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("missing run id")?
                .into(),
            identity,
            report,
            success: true,
            dependencies: std::collections::BTreeMap::new(),
            dependency_build: None,
        };
        std::fs::write(
            root.join("invocations")
                .join(format!("{}.json", invocation.identity.key())),
            serde_json::to_vec(&invocation)?,
        )?;
        std::fs::write(sidecar.join("verus.stdout"), stdout)?;
        Ok(sidecar)
    }

    #[cfg(unix)]
    #[test]
    fn real_stock_fallback_is_never_a_completed_driver()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-engine-stock-fallback-{}", std::process::id()));
        let sidecar = driver_fixture(&root)?;
        let seal = include_str!("../tests/fixtures/engine-terminal/terminal-success.jsonl");
        std::fs::write(sidecar.join("engine.events.jsonl"), seal)?;
        assert!(
            completed_driver(&root, &sidecar),
            "control: completed evidence is a completed driver"
        );
        let captured = std::process::Command::new("/bin/sh")
            .args(["-c", "printf 'interrupted'; exit 101"])
            .output()?;
        let mut stock = std::process::Command::new("/bin/sh");
        stock
            .args(["-c", "cat \"$1\"", "stock"])
            .arg(sidecar.join("verus.stdout"));
        let invocations = root.join("invocations");
        let log = crate::adapter::InvocationLog {
            directory: &invocations,
            run_id: root
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("missing run id")?,
            target: "demo",
        };
        let (output, trusted) = crate::adapter::engine_output(&sidecar, stock, captured, &log)?;
        assert!(!trusted);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, std::fs::read(sidecar.join("verus.stdout"))?);
        assert!(sidecar.join("engine.failed.stdout").is_file());
        assert!(sidecar.join("engine.events.jsonl").is_file());
        assert!(
            !completed_driver(&root, &sidecar),
            "valid old terminal/full bytes cannot make a crashed engine complete"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn verus_feature_requirements_are_checked_against_the_engine_manifest() {
        // The engine records no Cargo feature, like the stock release, and
        // the committed runtime pin says the same.
        assert_eq!(runtime_cargo_features(), Vec::<String>::new());
        let pin: serde_json::Value =
            serde_json::from_str(include_str!("../../../engine/verus/artifact-linux.json"))
                .unwrap_or_default();
        assert_eq!(
            pin["verus_features"],
            serde_json::json!(runtime_cargo_features())
        );
        assert_eq!(
            cargo_features(&[
                "singular".to_owned(),
                "sail-fork:abc".to_owned(),
                VSTD_BUILD.to_owned()
            ]),
            ["singular"]
        );
        let requires = [
            "singular".to_owned(),
            "verus-feature:singular".to_owned(),
            "verus-feature:singular".to_owned(),
            "program:z3".to_owned(),
        ];
        assert_eq!(missing_features(&requires, &[]), ["singular"]);
        assert_eq!(
            missing_features(&requires, &["singular".to_owned()]),
            Vec::<String>::new()
        );
        assert_eq!(
            missing_features(&requires[..1], &[]),
            Vec::<String>::new(),
            "only verus-feature requirements name Verus features"
        );

        let rendered =
            b"error: Please cargo build with `--features singular` to use integer_ring attribute\n";
        assert_eq!(required_feature(rendered, &[]).as_deref(), Some("singular"));
        assert_eq!(required_feature(rendered, &["singular".to_owned()]), None);
        let json = br#"{"$message_type":"diagnostic","message":"Please cargo build with `--features singular` to use integer_ring attribute","level":"error"}"#;
        assert_eq!(required_feature(json, &[]).as_deref(), Some("singular"));
        for unrelated in [
            &b"error: assertion failed"[..],
            b"Please cargo build with `--features ",
            b"Please cargo build with `--features $(rm)` to use",
            b"",
        ] {
            assert_eq!(required_feature(unrelated, &[]), None);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_verus_feature_falls_back_to_stock_even_after_a_complete_seal()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-engine-feature-{}", std::process::id()));
        let sidecar = driver_fixture(&root)?;
        let seal = include_str!("../tests/fixtures/engine-terminal/terminal-success.jsonl");
        std::fs::write(sidecar.join("engine.events.jsonl"), seal)?;
        let stdout = std::fs::read(sidecar.join("verus.stdout"))?;
        let engine = |stderr: &str| -> std::io::Result<std::process::Output> {
            let mut output = std::process::Command::new("/bin/sh")
                .args(["-c", "exit 0"])
                .output()?;
            output.stdout.clone_from(&stdout);
            output.stderr = stderr.as_bytes().to_vec();
            Ok(output)
        };
        let invocations = root.join("invocations");
        let run_id = root
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("missing run id")?
            .to_owned();
        let log = crate::adapter::InvocationLog {
            directory: &invocations,
            run_id: &run_id,
            target: "demo",
        };
        let stock = || {
            let mut stock = std::process::Command::new("/bin/sh");
            stock.args(["-c", "printf stock; exit 1"]);
            stock
        };
        // Control: the same complete engine output is trusted.
        let (_, trusted) = crate::adapter::engine_output(&sidecar, stock(), engine("")?, &log)?;
        assert!(trusted);
        assert!(!sidecar.join(MISSING_FEATURE).exists());
        let diagnostic =
            "error: Please cargo build with `--features singular` to use integer_ring attribute\n";
        let (output, trusted) =
            crate::adapter::engine_output(&sidecar, stock(), engine(diagnostic)?, &log)?;
        assert!(!trusted, "the engine's rejection is never the answer");
        assert_eq!(output.stdout, b"stock");
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            std::fs::read_to_string(sidecar.join(MISSING_FEATURE))?,
            "singular"
        );
        assert_eq!(missing_in_run(&root), ["singular"]);
        let mut notes = Vec::new();
        let mut warnings = Vec::new();
        let fault = missing_feature_fallback(&root, &mut notes, &mut warnings)
            .ok_or("missing fallback fault")?;
        assert_eq!(fault.code, "SAIL3124");
        assert!(String::from_utf8(warnings)?.starts_with("warning[SAIL3124]: "));
        assert!(notes.iter().any(|note| note.starts_with("SAIL3124: ")));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn completed_driver_requires_matching_retained_driver_document()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-engine-document-{}", std::process::id()));
        let sidecar = driver_fixture(&root)?;
        let events = include_str!("../tests/fixtures/engine-terminal/terminal-success.jsonl");
        std::fs::write(sidecar.join("engine.events.jsonl"), events)?;
        assert!(completed_driver(&root, &sidecar));
        let output = sidecar.join("verus.stdout");
        let valid_output = std::fs::read(&output)?;
        std::fs::write(&output, b"{\"verification-results\":")?;
        assert!(!completed_driver(&root, &sidecar));
        std::fs::remove_file(&output)?;
        assert!(!completed_driver(&root, &sidecar));
        std::fs::write(&output, valid_output)?;
        let key = sidecar
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("missing sidecar key")?;
        let record = root.join("invocations").join(format!("{key}.json"));
        let mut invocation: crate::managed::Invocation =
            serde_json::from_slice(&std::fs::read(&record)?)?;
        invocation.run_id = "other-run".into();
        std::fs::write(&record, serde_json::to_vec(&invocation)?)?;
        assert!(!completed_driver(&root, &sidecar));
        invocation.run_id = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("missing run id")?
            .into();
        invocation.report.exit_code = Some(101);
        std::fs::write(&record, serde_json::to_vec(&invocation)?)?;
        assert!(!completed_driver(&root, &sidecar));
        std::fs::remove_file(record)?;
        assert!(!completed_driver(&root, &sidecar));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
