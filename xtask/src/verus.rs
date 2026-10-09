//! `cargo xtask fetch-verus`: installs the pinned Verus release.
//!
//! Reads `corpus/toolchain.toml`, downloads the release archive for the host
//! platform with `curl`, checks its SHA-256 before unpacking it with `unzip`,
//! then checks the bundled z3's SHA-256. Installs are idempotent: a verified
//! install is left alone.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::{Result, workspace_root};

pub(crate) fn host() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}

pub(crate) fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(buf.get(..n).unwrap_or_default());
    }
    Ok(hasher.finalize().iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    }))
}

fn install_root(release: &str) -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("SAIL_VERUS_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok_or("neither SAIL_VERUS_HOME, XDG_CACHE_HOME nor HOME is set")?;
    Ok(cache.join("sail").join("verus").join(release))
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run {cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} exited with {status}"))
    }
}

pub(crate) fn check_z3(z3: &Path, asset: &sail_verus::toolchain::AssetPin) -> Result<()> {
    let actual = file_sha256(z3)?;
    if asset.z3_sha256.is_empty() {
        println!(
            "note: no z3_sha256 pinned for {}; record this value in corpus/toolchain.toml: {actual}",
            host()
        );
        return Ok(());
    }
    if actual == asset.z3_sha256 {
        Ok(())
    } else {
        Err(format!(
            "{} has SHA-256 {actual}, expected {}",
            z3.display(),
            asset.z3_sha256
        ))
    }
}

/// Installs the pinned Verus release for the host and prints where it is.
pub fn fetch() -> Result<()> {
    let path = workspace_root().join("corpus").join("toolchain.toml");
    // Honours SAIL_VERUS_RELEASE (a secondary pin), like the harness does.
    let pins = sail_verus::toolchain::Pins::load(&path).map_err(|e| e.to_string())?;
    let asset = pins
        .assets
        .get(host())
        .ok_or_else(|| format!("no pinned Verus asset for host {}", host()))?;
    // SAIL_VERUS_HOME names the primary release's install only.
    let root = if pins.is_secondary {
        sail_verus::cache_dir()
            .map_err(|e| e.to_string())?
            .join("verus")
            .join(&pins.verus.release)
    } else {
        install_root(&pins.verus.release)?
    };
    let install = root.join(&asset.dir);
    // A release that bundles no z3 uses the primary's (`z3 = "primary"`).
    let bundled = asset.z3.as_deref() != Some("primary");
    let local_z3 = install.join("z3");
    let installed = install.join("verus").is_file()
        && (!bundled || (local_z3.is_file() && check_z3(&local_z3, asset).is_ok()));

    if !installed {
        let downloads = root.join("downloads");
        std::fs::create_dir_all(&downloads)
            .map_err(|e| format!("cannot create {}: {e}", downloads.display()))?;
        let name = asset.url.rsplit('/').next().unwrap_or("verus.zip");
        let archive = downloads.join(name);
        if !(archive.is_file() && file_sha256(&archive)? == asset.sha256) {
            let partial = downloads.join(format!("{name}.part"));
            println!("downloading {}", asset.url);
            run(Command::new("curl")
                .args(["-sSfL", "-o"])
                .arg(&partial)
                .arg(&asset.url))?;
            let actual = file_sha256(&partial)?;
            if actual != asset.sha256 {
                let _ = std::fs::remove_file(&partial);
                return Err(format!(
                    "{name} has SHA-256 {actual}, expected {}",
                    asset.sha256
                ));
            }
            std::fs::rename(&partial, &archive)
                .map_err(|e| format!("cannot move {}: {e}", partial.display()))?;
        }
        let staging = root.join("staging");
        let _ = std::fs::remove_dir_all(&staging);
        run(Command::new("unzip")
            .arg("-q")
            .arg(&archive)
            .arg("-d")
            .arg(&staging))?;
        let _ = std::fs::remove_dir_all(&install);
        std::fs::rename(staging.join(&asset.dir), &install)
            .map_err(|e| format!("cannot install into {}: {e}", install.display()))?;
        let _ = std::fs::remove_dir_all(&staging);
    }
    let z3 = sail_verus::toolchain::Installed::locate(pins.clone())
        .map_err(|e| {
            format!("{e} (for a release using the primary's z3, fetch the primary first)")
        })?
        .z3;
    check_z3(&z3, asset)?;

    println!(
        "Verus {} (z3 {}) installed:",
        pins.verus.release, pins.z3.version
    );
    println!("  verus:          {}", install.join("verus").display());
    println!("  z3:             {}", z3.display());
    println!("  rust toolchain: {}", pins.verus.rust_toolchain);
    Ok(())
}
