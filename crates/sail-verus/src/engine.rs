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
                    && !hash.is_empty()
            })
            && source_build::artifact_hashes(&build).ok().as_ref() == Some(&manifest.artifacts)
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
    pub prepared: PathBuf,
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
        prepared: config.workspace.join(&config.cache).join("engine-prepared"),
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
    let mut result = crate::managed::run_engine(config, err, Some(request))?;
    launcher_notes(&run_directory(config, &result), &mut result);
    Ok(result)
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
    let mut full = crate::managed::run_engine(&config, err, Some(request))?;
    let run = run_directory(&config, &full);
    launcher_notes(&run, &mut full);
    let mut selected = Vec::new();
    let mut selection_error = None;
    let gathered = (|| -> Result<()> {
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
    if focus.is_none() && selection_error.is_none() {
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
        full,
        manifest_sha256,
        preparation: preparation(&run),
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
                    && target.target.replace('-', "_") == record.target
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationEvent {
    version: u32,
    phase: String,
    event: String,
    payload: sail_api::repair::PreparationStats,
}

fn preparation(run: &Path) -> Option<Vec<sail_api::repair::PreparationStats>> {
    use std::io::Read as _;
    let mut paths = std::fs::read_dir(run.join("sidecars"))
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path().join("engine.events.jsonl"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut stats = Vec::new();
    for path in paths {
        if !crate::engine_events::sealed(&path) {
            continue;
        }
        let file = std::fs::File::open(path).ok()?;
        if file.metadata().ok()?.len() > 16 * 1024 * 1024 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 16 * 1024 * 1024 {
            return None;
        }
        let mut summary = None;
        for line in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let value: serde_json::Value = serde_json::from_slice(line).ok()?;
            if value["event"] != "preparation-summary" {
                continue;
            }
            if summary.is_some() {
                return None;
            }
            let event: PreparationEvent = serde_json::from_slice(line).ok()?;
            if event.version != 1
                || event.phase != "full"
                || event.event != "preparation-summary"
                || event.payload.peak_retained_bytes > event.payload.memory_budget_bytes
                || (!event.payload.enabled
                    && (event.payload.batch_hits != 0 || event.payload.persistent_hits != 0))
                || event.payload.target.is_some()
                || event.payload.phase.is_some()
            {
                return None;
            }
            summary = Some(event.payload);
        }
        if let Some(summary) = summary {
            stats.push(summary);
        }
    }
    (!stats.is_empty()).then_some(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn preparation_requires_one_valid_sealed_summary()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-preparation-{}", std::process::id()));
        let sidecar = root.join("sidecars/driver");
        std::fs::create_dir_all(&sidecar)?;
        let path = sidecar.join("engine.events.jsonl");
        let seal =
            r#"{"version":1,"phase":"full","event":"compiler-sealed","payload":{"success":true}}"#;
        let summary = r#"{"version":1,"phase":"full","event":"preparation-summary","payload":{"version":1,"enabled":true,"batch_hits":0,"persistent_hits":2,"misses":1,"peak_retained_bytes":123,"memory_budget_bytes":512,"disk_budget_bytes":512}}"#;
        std::fs::write(&path, format!("{seal}\n"))?;
        assert!(
            preparation(&root).is_none(),
            "old source has no fabricated counters"
        );
        std::fs::write(&path, format!("{summary}\n{seal}\n"))?;
        let stats = preparation(&root).ok_or("valid counters unavailable")?;
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].persistent_hits, 2);
        assert!(stats[0].target.is_none());
        std::fs::write(&path, format!("{summary}\n"))?;
        assert!(
            preparation(&root).is_none(),
            "interrupted source cannot report counters"
        );
        std::fs::write(&path, format!("{summary}\n{summary}\n{seal}\n"))?;
        assert!(
            preparation(&root).is_none(),
            "duplicate summaries are ambiguous"
        );
        let duplicate = summary.replace("\"misses\":1", "\"misses\":1,\"misses\":2");
        std::fs::write(&path, format!("{duplicate}\n{seal}\n"))?;
        assert!(
            preparation(&root).is_none(),
            "duplicate fields cannot be hidden by Value parsing"
        );
        let excessive =
            summary.replace("\"peak_retained_bytes\":123", "\"peak_retained_bytes\":513");
        std::fs::write(&path, format!("{excessive}\n{seal}\n"))?;
        assert!(preparation(&root).is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
