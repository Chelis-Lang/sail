use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{LoweredSource, digest, lower, unsupported};
use crate::{Error, Result};

/// Actual managed backend evidence for an exact original neutral source snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractRun {
    /// Reproducible original-to-shadow binding, not itself a proof.
    pub lowering: LoweredSource,
    /// Owned shadow directory; the caller's source is never written.
    pub shadow: PathBuf,
    /// Actual complete managed backend result, including failures and unknowns.
    pub backend: sail_api::project::ProjectResult,
}

/// Lower one exact source snapshot and run its immutable owned shadow on pinned Verus.
///
/// This returns actual backend records, not per-requirement assurance. Consumers
/// must map observed obligations and independently recheck original currentness.
/// # Errors
/// `SAIL3140` for unsupported source or request controls; `SAIL3141` for shadow
/// drift; existing managed/runtime codes for unavailable execution or backend faults.
pub fn verify(
    original: &str,
    crate_name: &str,
    config: &crate::managed::Config,
    err: &mut dyn Write,
) -> Result<ContractRun> {
    let lowering = lower(original, crate_name)?;
    if !config.args.is_empty() {
        return Err(unsupported(
            "neutral contract verification currently requires whole-target default backend controls",
        ));
    }
    let pins = crate::toolchain::Pins::embedded()?;
    let installed = crate::engine::Installed::locate_verified(&config.cache, &pins)?;
    if canonical(&installed.cargo_verus)? != canonical(&config.cargo_verus)? {
        return Err(unsupported(
            "neutral lowering requires Sail's validated pinned Verus runtime",
        ));
    }
    let solver_hash = pins
        .assets
        .get(crate::toolchain::host())
        .map(|a| a.z3_sha256.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| unsupported("no exact solver pin for neutral backend"))?;
    if digest(&std::fs::read(&config.z3).map_err(io_error)?) != solver_hash {
        return Err(unsupported(
            "neutral backend solver differs from its pinned bytes",
        ));
    }
    let vstd = pins
        .verus
        .vstd
        .as_deref()
        .ok_or_else(|| unsupported("neutral backend has no pinned vstd dependency"))?;
    let manifest =
        serde_json::to_vec_pretty(&lowering.manifest).map_err(|e| drift(e.to_string()))?;
    let key = digest(&manifest);
    let root = config.cache.join("neutral-contracts-v1");
    directory(&root)?;
    let root = canonical(&root)?;
    let lock_path = root.join(format!("{key}.lock"));
    regular_or_absent(&lock_path)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(io_error)?;
    lock.lock().map_err(io_error)?;
    let shadow = root.join(key);
    directory(&shadow)?;
    directory(&shadow.join("src"))?;
    let cargo = format!(
        "[package]\nname = \"{crate_name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n[package.metadata.verus]\nverify = true\n[lib]\npath = \"src/lib.rs\"\n[dependencies]\nvstd = \"={vstd}\"\n[workspace]\n"
    );
    install(&shadow.join("Cargo.toml"), cargo.as_bytes())?;
    install(&shadow.join("src/lib.rs"), lowering.source.as_bytes())?;
    install(&shadow.join("lowering.json"), &manifest)?;
    check(
        &shadow,
        cargo.as_bytes(),
        lowering.source.as_bytes(),
        &manifest,
    )?;
    let mut backend_config = config.clone();
    backend_config.workspace.clone_from(&shadow);
    backend_config.attribution_logs = true;
    let result = crate::managed::run(&backend_config, err);
    // Even a failed backend cannot turn a modified shadow into established evidence.
    check(
        &shadow,
        cargo.as_bytes(),
        lowering.source.as_bytes(),
        &manifest,
    )?;
    let backend = result?;
    drop(lock);
    Ok(ContractRun {
        lowering,
        shadow,
        backend,
    })
}

fn check(shadow: &Path, cargo: &[u8], source: &[u8], manifest: &[u8]) -> Result<()> {
    check_directory(shadow)?;
    check_directory(&shadow.join("src"))?;
    for (path, bytes) in [
        (shadow.join("Cargo.toml"), cargo),
        (shadow.join("src/lib.rs"), source),
        (shadow.join("lowering.json"), manifest),
    ] {
        regular_or_absent(&path)?;
        if std::fs::read(&path).map_err(io_error)? != bytes {
            return Err(drift(
                "owned neutral shadow differs from its exact original binding",
            ));
        }
    }
    for entry in std::fs::read_dir(shadow).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if !["Cargo.toml", "Cargo.lock", "lowering.json", "src"]
            .iter()
            .any(|name| entry.file_name() == *name)
        {
            return Err(drift("unexpected input in owned neutral shadow"));
        }
    }
    let entries = std::fs::read_dir(shadow.join("src"))
        .map_err(io_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(io_error)?;
    if entries.len() != 1 || entries[0].file_name() != "lib.rs" {
        return Err(drift("unexpected source in owned neutral shadow"));
    }
    regular_or_absent(&shadow.join("Cargo.lock"))?;
    Ok(())
}

fn install(path: &Path, expected: &[u8]) -> Result<()> {
    regular_or_absent(path)?;
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(expected).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut actual = Vec::new();
            File::open(path)
                .map_err(io_error)?
                .read_to_end(&mut actual)
                .map_err(io_error)?;
            if actual != expected {
                return Err(drift(
                    "immutable neutral shadow drift; existing evidence is preserved",
                ));
            }
        }
        Err(error) => return Err(io_error(error)),
    }
    Ok(())
}

fn directory(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(io_error)?;
    check_directory(path)
}

fn check_directory(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(drift(
            "neutral shadow directories must not be symbolic links",
        ));
    }
    Ok(())
}

fn regular_or_absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(value) if value.is_file() && !value.file_type().is_symlink() => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(drift("neutral shadow inputs must be regular files")),
        Err(error) => Err(io_error(error)),
    }
}

fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize().map_err(io_error)
}
fn drift(message: impl Into<String>) -> Error {
    Error::new("SAIL3141", message)
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err supplies an owned filesystem diagnostic."
)]
fn io_error(error: std::io::Error) -> Error {
    drift(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "sail-neutral-shadow-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_changed_owned_shadow_is_refused_without_overwriting_evidence() -> TestResult {
        let directory = Directory::new()?;
        let path = directory.0.join("source.rs");
        install(&path, b"original")?;
        install(&path, b"original")?;
        std::fs::write(&path, b"tampered")?;
        assert_eq!(
            install(&path, b"original").map_err(|e| e.code),
            Err("SAIL3141")
        );
        assert_eq!(std::fs::read(&path)?, b"tampered");
        Ok(())
    }

    #[test]
    fn unexpected_inputs_cannot_join_an_existing_shadow() -> TestResult {
        let directory = Directory::new()?;
        super::directory(&directory.0.join("src"))?;
        install(&directory.0.join("Cargo.toml"), b"cargo")?;
        install(&directory.0.join("src/lib.rs"), b"source")?;
        install(&directory.0.join("lowering.json"), b"manifest")?;
        check(&directory.0, b"cargo", b"source", b"manifest")?;
        std::fs::write(directory.0.join("build.rs"), b"effectful input")?;
        assert_eq!(
            check(&directory.0, b"cargo", b"source", b"manifest").map_err(|e| e.code),
            Err("SAIL3141")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_cannot_redirect_shadow_writes() -> TestResult {
        let directory = Directory::new()?;
        let outside = Directory::new()?;
        let target = outside.0.join("outside.rs");
        std::fs::write(&target, b"preserved")?;
        let path = directory.0.join("source.rs");
        std::os::unix::fs::symlink(&target, &path)?;
        assert!(install(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&target)?, b"preserved");
        Ok(())
    }
}
