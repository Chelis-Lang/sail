//! Candidate ordinary selection, limited to the independently admitted runtime.

use super::super::research::Mode;
use std::path::Path;

// Exact manifest inside Linux archive 689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928.
// Changing this authority requires new runtime and ordinary-reuse qualification.
const LINUX_MANIFEST: &str = "a845cd2fcba411148566d8214c1285f083689df6a8ffa5479afcf3845382b033";

pub(super) fn requested(mode: Mode) -> bool {
    choose(mode, crate::toolchain::host(), || true)
}

/// An explicit vstd experiment never also selects ordinary retained reuse.
/// The old retained switch remains a research qualification route, not a user
/// requirement. Ordinary selection must validate the qualified Linux image.
pub(super) fn admitted(mode: Mode, launcher: &Path, z3: &Path) -> bool {
    choose(mode, crate::toolchain::host(), || {
        normal_runtime(launcher, z3).is_some()
    })
}

fn choose(mode: Mode, host: &str, validate: impl FnOnce() -> bool) -> bool {
    match mode {
        Mode::RetainedContent => true,
        Mode::Off if host == "x86_64-unknown-linux-gnu" => validate(),
        Mode::Off | Mode::VstdBoundary => false,
    }
}

fn normal_runtime(launcher: &Path, z3: &Path) -> Option<()> {
    let launcher = launcher.canonicalize().ok()?;
    // Installed::at checks the complete inventory, expected fork/features,
    // toolchain and solver. No stat-only or cross-invocation memo qualifies it.
    let build = launcher.ancestors().nth(5)?;
    let installed =
        crate::engine::Installed::at(build, &crate::toolchain::Pins::embedded().ok()?).ok()?;
    if installed.manifest_sha256 != LINUX_MANIFEST
        || installed.verus.canonicalize().ok()? != launcher
        || installed.verus.with_file_name("z3").canonicalize().ok()? != z3.canonicalize().ok()?
    {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn ordinary_selection_is_linux_only_and_requires_runtime_authority() {
        for (mode, host, valid, expected, checked) in [
            (Mode::Off, "x86_64-unknown-linux-gnu", true, true, true),
            (Mode::Off, "x86_64-unknown-linux-gnu", false, false, true),
            (Mode::Off, "aarch64-apple-darwin", true, false, false),
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
    fn absent_or_misplaced_runtime_cannot_select_ordinary_reuse() {
        assert!(normal_runtime(Path::new("/not-a-runtime/verus"), Path::new("/z3")).is_none());
        assert!(normal_runtime(Path::new("verus"), Path::new("z3")).is_none());
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
        assert!(normal_runtime(&runtime.join("verus"), &runtime.join("z3")).is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
