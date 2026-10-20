//! Candidate ordinary selection, limited to the independently admitted runtime.

use super::super::research::Mode;
use std::path::Path;

// Exact manifests of the runtimes ordinary reuse is qualified on, one per
// platform. Changing one requires new runtime and ordinary-reuse
// qualification on that platform. They stay separate constants so a re-pin
// cannot select reuse unqualified; tests require each to equal
// `manifest_sha256` in its engine/verus/artifact-*.json pin.
const LINUX_MANIFEST: &str = "cd78756b462127d2551fd35dfc4dd1259a26aeb2501428b5702fa30bcdc96a04";
const MACOS_MANIFEST: &str = "4392062ae1674fb473818d3bb730b4a2d2dba831dd2e9dddf98c5c57a8ed91fe";

/// The runtime manifest ordinary reuse is qualified on for `host`, if any.
fn qualified_manifest(host: &str) -> Option<&'static str> {
    match host {
        "x86_64-unknown-linux-gnu" => Some(LINUX_MANIFEST),
        "aarch64-apple-darwin" => Some(MACOS_MANIFEST),
        _ => None,
    }
}

pub(super) fn requested(mode: Mode) -> bool {
    choose(mode, crate::toolchain::host(), || true)
}

/// An explicit vstd experiment never also selects ordinary retained reuse.
/// The old retained switch remains a research qualification route, not a user
/// requirement. Ordinary selection must validate the platform's qualified
/// runtime.
///
/// # Errors
/// Not selected. Carries the manifest digest when the only reason is that an
/// otherwise valid engine is not the platform's qualified runtime
/// ([`qualified_manifest`]).
pub(super) fn admitted(mode: Mode, launcher: &Path, z3: &Path) -> Result<(), Option<String>> {
    let mut unqualified = None;
    let selected = choose(mode, crate::toolchain::host(), || {
        match normal_runtime(launcher, z3, crate::toolchain::host()) {
            Runtime::Qualified => true,
            Runtime::Unqualified(manifest) => {
                unqualified = Some(manifest);
                false
            }
            Runtime::Absent => false,
        }
    });
    if selected { Ok(()) } else { Err(unqualified) }
}

/// The result note recorded when ordinary retained reuse is off only because
/// the engine's manifest is not this platform's qualified runtime.
pub(crate) fn unqualified_note(manifest: &str) -> String {
    let qualified = qualified_manifest(crate::toolchain::host()).unwrap_or_default();
    format!(
        "SAIL3136: ordinary retained reuse is off because the engine's manifest {} is not the qualified runtime {}; verification is unaffected",
        manifest.get(..12).unwrap_or(manifest),
        qualified.get(..12).unwrap_or(qualified)
    )
}

fn choose(mode: Mode, host: &str, validate: impl FnOnce() -> bool) -> bool {
    match mode {
        Mode::RetainedContent => true,
        Mode::Off if qualified_manifest(host).is_some() => validate(),
        Mode::Off | Mode::VstdBoundary => false,
    }
}

/// What the engine behind a launcher is, for ordinary selection.
#[derive(Debug, PartialEq, Eq)]
enum Runtime {
    /// The exact qualified runtime for this platform.
    Qualified,
    /// A valid, correctly placed engine with another manifest digest.
    Unqualified(String),
    /// No valid engine at this launcher and solver.
    Absent,
}

fn normal_runtime(launcher: &Path, z3: &Path, host: &str) -> Runtime {
    match placed_manifest(launcher, z3) {
        Some(manifest) if Some(manifest.as_str()) == qualified_manifest(host) => Runtime::Qualified,
        Some(manifest) => Runtime::Unqualified(manifest),
        None => Runtime::Absent,
    }
}

/// The manifest digest of the valid engine whose launcher and solver these are.
fn placed_manifest(launcher: &Path, z3: &Path) -> Option<String> {
    let launcher = launcher.canonicalize().ok()?;
    // Installed::at checks the complete inventory, expected fork/features,
    // toolchain and solver. No stat-only or cross-invocation memo qualifies it.
    let build = launcher.ancestors().nth(5)?;
    let installed =
        crate::engine::Installed::at(build, &crate::toolchain::Pins::embedded().ok()?).ok()?;
    (installed.verus.canonicalize().ok()? == launcher
        && installed.verus.with_file_name("z3").canonicalize().ok()? == z3.canonicalize().ok()?)
    .then_some(installed.manifest_sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn ordinary_selection_needs_a_qualified_platform_and_runtime_authority() {
        for (mode, host, valid, expected, checked) in [
            (Mode::Off, "x86_64-unknown-linux-gnu", true, true, true),
            (Mode::Off, "x86_64-unknown-linux-gnu", false, false, true),
            (Mode::Off, "aarch64-apple-darwin", true, true, true),
            (Mode::Off, "aarch64-apple-darwin", false, false, true),
            (Mode::Off, "x86_64-apple-darwin", true, false, false),
            (Mode::Off, "aarch64-unknown-linux-gnu", true, false, false),
            (
                Mode::VstdBoundary,
                "x86_64-unknown-linux-gnu",
                true,
                false,
                false,
            ),
            (
                Mode::RetainedContent,
                "aarch64-apple-darwin",
                false,
                true,
                false,
            ),
        ] {
            let called = Cell::new(false);
            assert_eq!(
                choose(mode, host, || {
                    called.set(true);
                    valid
                }),
                expected
            );
            assert_eq!(called.get(), checked);
        }
    }

    #[test]
    fn qualified_manifest_is_the_committed_linux_runtime_pin()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Moving the runtime pin without requalifying ordinary reuse fails
        // here instead of silently turning instant revert off.
        let pin: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../engine/verus/artifact-linux.json"
        )))?;
        assert_eq!(pin["manifest_sha256"].as_str(), Some(LINUX_MANIFEST));
        assert_eq!(
            qualified_manifest("x86_64-unknown-linux-gnu"),
            Some(LINUX_MANIFEST)
        );
        Ok(())
    }

    #[test]
    fn qualified_manifest_is_the_committed_macos_runtime_pin()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        // The same rule for the macOS pin and its own qualification.
        let pin: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../engine/verus/artifact-macos.json"
        )))?;
        assert_eq!(pin["manifest_sha256"].as_str(), Some(MACOS_MANIFEST));
        assert_eq!(
            qualified_manifest("aarch64-apple-darwin"),
            Some(MACOS_MANIFEST)
        );
        Ok(())
    }

    #[test]
    fn absent_or_misplaced_runtime_cannot_select_ordinary_reuse() {
        assert_eq!(
            normal_runtime(
                Path::new("/not-a-runtime/verus"),
                Path::new("/z3"),
                crate::toolchain::host()
            ),
            Runtime::Absent
        );
        assert_eq!(
            normal_runtime(
                Path::new("verus"),
                Path::new("z3"),
                crate::toolchain::host()
            ),
            Runtime::Absent
        );
    }

    #[test]
    fn another_valid_engine_manifest_is_not_the_qualified_runtime()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-retained-runtime-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let runtime = crate::source_build::checkout_dir(&root).join("source/target-verus/release");
        std::fs::create_dir_all(&runtime)?;
        for name in ["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"] {
            std::fs::write(runtime.join(name), name)?;
        }
        let pins = crate::toolchain::Pins::embedded()?;
        crate::source_build::Manifest {
            schema_version: crate::source_build::SCHEMA_VERSION,
            commit: pins.verus.commit.clone(),
            rust_toolchain: pins.verus.rust_toolchain.clone(),
            z3_sha256: pins
                .assets
                .get(crate::toolchain::host())
                .ok_or("host pin")?
                .z3_sha256
                .clone(),
            features: crate::engine::features(),
            artifacts: crate::source_build::artifact_hashes(&root)?,
            build_seconds: 0,
            test_binaries: Vec::new(),
        }
        .save(&root)?;
        // This fixture passes the existing inventory validator, but an
        // arbitrary coherent manifest is not the exact admitted Linux build.
        assert!(crate::engine::Installed::at(&root, &pins).is_ok());
        let manifest = crate::engine::Installed::at(&root, &pins)?.manifest_sha256;
        assert_eq!(
            normal_runtime(
                &runtime.join("verus"),
                &runtime.join("z3"),
                crate::toolchain::host()
            ),
            Runtime::Unqualified(manifest.clone())
        );
        assert!(unqualified_note(&manifest).starts_with("SAIL3136: "));
        // Another solver is not this engine's placement, so it is no manifest note.
        assert_eq!(
            normal_runtime(
                &runtime.join("verus"),
                &runtime.join("cargo-verus"),
                crate::toolchain::host()
            ),
            Runtime::Absent
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
