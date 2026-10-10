//! Optional source-integrated focus/full proof engine (ADR 0027).
//!
//! The engine is a separately hashed pinned Verus build. Stock releases and
//! stock source builds are never substituted silently.
use std::path::{Path, PathBuf};

use crate::source_build::{self, Manifest};
use crate::toolchain::Pins;
use crate::{Error, Result};

/// Patch bytes applied only to the engine's owned source checkout.
pub const PATCH: &str = include_str!("../../../engine/verus/interactive.patch");
/// Source-event sidecar contract version.
pub const EVENT_VERSION: u32 = 1;
/// Exact dependency resolution used for the optional source worker.
pub const SOURCE_LOCK: &str = include_str!("../../../engine/verus/Cargo.lock");

/// Exact source patch identifier; part of build and execution identities.
#[must_use]
pub fn patch_sha256() -> String {
    crate::managed::digest(PATCH.as_bytes())
}

/// Features identifying this distinct source variant.
#[must_use]
pub fn features() -> Vec<String> {
    vec![
        format!("sail-interactive:{}", patch_sha256()),
        format!("host:{}", crate::toolchain::host()),
        format!(
            "source-lock:{}",
            crate::managed::digest(SOURCE_LOCK.as_bytes())
        ),
    ]
}

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
}

impl Installed {
    /// Validates the managed engine build before any proof is started.
    ///
    /// # Errors
    /// `SAIL3120` if missing, misidentified, or corrupted. Run
    /// `cargo xtask build-verus-engine` to install the explicit variant.
    pub fn locate(cache: &Path, pins: &Pins) -> Result<Self> {
        Self::at(&build_dir(cache, pins), pins)
    }

    /// Validates an explicitly supplied build without installing or modifying it.
    ///
    /// # Errors
    /// `SAIL3120` when its identity or artifact bytes do not match the engine.
    pub fn at(build: &Path, pins: &Pins) -> Result<Self> {
        let build = build.to_owned();
        let manifest = Manifest::load(&build).map_err(|e| {
            Error::new(
                "SAIL3120",
                format!("{e}; run cargo xtask build-verus-engine"),
            )
        })?;
        let valid = manifest.commit == pins.verus.commit
            && manifest.rust_toolchain == pins.verus.rust_toolchain
            && manifest.features == features()
            && !manifest.artifacts.is_empty()
            && manifest.artifacts.iter().all(|(path, hash)| {
                path.components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
                    && source_build::hash_file(&build.join(path)).ok().as_ref() == Some(hash)
            })
            && pins
                .assets
                .get(crate::toolchain::host())
                .is_some_and(|asset| asset.z3_sha256 == manifest.z3_sha256);
        let runtime = source_build::checkout_dir(&build).join("source/target-verus/release");
        if !valid
            || !["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"]
                .iter()
                .all(|name| runtime.join(name).is_file())
        {
            return Err(Error::new(
                "SAIL3120",
                "interactive engine identity or runtime artifact validation failed",
            ));
        }
        Ok(Self {
            cargo_verus: runtime.join("cargo-verus"),
            verus: runtime.join("verus"),
            manifest_sha256: source_build::hash_file(&build.join(source_build::MANIFEST))?,
            build,
        })
    }
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
    /// Current exact source patch digest.
    pub patch_sha256: String,
    /// Exact dependency resolution digest.
    pub source_lock_sha256: String,
    /// Complete build-manifest digest.
    pub manifest_sha256: String,
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
        version: 1,
        commit: pins.verus.commit.clone(),
        rust_toolchain: pins.verus.rust_toolchain.clone(),
        patch_sha256: patch_sha256(),
        source_lock_sha256: crate::managed::digest(SOURCE_LOCK.as_bytes()),
        manifest_sha256: installed.manifest_sha256,
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
    pub stock_wrapper: PathBuf,
    pub artifacts: Vec<PathBuf>,
}

fn request(config: &crate::managed::Config, function: Option<String>) -> Result<(Request, String)> {
    let installed = Installed::locate(&config.workspace.join(&config.cache), &Pins::embedded()?)?;
    let stock_wrapper = config
        .cargo_verus
        .canonicalize()
        .map_err(|e| Error::new("SAIL3120", e.to_string()))?
        .with_file_name("verus");
    let request = Request {
        function,
        cargo_verus: installed.cargo_verus,
        manifest: installed.build.join(source_build::MANIFEST),
        stock_wrapper,
        artifacts: Manifest::load(&installed.build)?
            .artifacts
            .keys()
            .map(|path| installed.build.join(path))
            .collect(),
    };
    Ok((request, installed.manifest_sha256))
}

/// Performs explicit interactive whole-target verification using the optional build.
///
/// # Errors
/// `SAIL3120` for an unavailable engine; ordinary verification remains available.
pub fn verify(
    config: &crate::managed::Config,
    err: &mut dyn std::io::Write,
) -> Result<sail_api::project::ProjectResult> {
    let (request, _) = request(config, None)?;
    crate::managed::run_engine(config, err, Some(request))
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
    let full = crate::managed::run_engine(&config, err, Some(request))?;
    let run = config
        .workspace
        .join(&config.cache)
        .join("projects")
        .join(crate::managed::digest(
            Path::new(&full.workspace).as_os_str().as_encoded_bytes(),
        ))
        .join(&full.context_sha256)
        .join("runs")
        .join(&full.run_id);
    let mut selected = Vec::new();
    for entry in std::fs::read_dir(run.join("sidecars"))
        .map_err(|e| Error::new("SAIL3121", e.to_string()))?
    {
        let entry = entry.map_err(|e| Error::new("SAIL3121", e.to_string()))?;
        let path = entry.path().join("focus.report.json");
        if !path.is_file() {
            continue;
        }
        let focus: FocusTarget = serde_json::from_slice(
            &std::fs::read(path).map_err(|e| Error::new("SAIL3121", e.to_string()))?,
        )
        .map_err(|e| Error::new("SAIL3121", e.to_string()))?;
        if focus.function != function {
            continue;
        }
        if let Some(target) = full.targets.iter().find(|target| {
            target.requested
                && target.package_id == focus.package_id
                && target.target.replace('-', "_") == focus.target
                && target.kind.contains(&focus.kind)
        }) {
            let mut target = target.clone();
            target.report = focus.report;
            target.complete = false;
            target.reused = false;
            selected.push(target);
        }
    }
    if selected.len() > 1 {
        return Err(Error::new(
            "SAIL3122",
            "repair function matched more than one selected Cargo target",
        ));
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
    if focus.is_none() && full.exit_code == Some(0) {
        return Err(Error::new(
            "SAIL3122",
            "exact repair function was not established in the selected target",
        ));
    }
    Ok(sail_api::repair::BatchResult {
        batched: focus.is_some(),
        focus,
        compiler_invocations: full.execution.verifier_invocations,
        full,
        manifest_sha256,
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct FocusTarget {
    pub function: String,
    pub package_id: String,
    pub target: String,
    pub kind: String,
    pub report: sail_api::output::Report,
}
