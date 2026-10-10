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
        let build = build_dir(cache, pins);
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
    let focus = if selected.is_empty() {
        None
    } else {
        let mut result = full.clone();
        result.targets = selected;
        result
            .notes
            .push("Explicit focused evidence; independent full validation follows".into());
        Some(result)
    };
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
