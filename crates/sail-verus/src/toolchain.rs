//! The pinned Verus and Z3 (`corpus/toolchain.toml`) and where they are
//! installed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Error, Result};

/// The contents of `corpus/toolchain.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Pins {
    /// The pinned Verus release.
    pub verus: VerusPin,
    /// The pinned Z3.
    pub z3: Z3Pin,
    /// Release archives by host target triple.
    pub assets: BTreeMap<String, AssetPin>,
}

/// The pinned Verus release.
#[derive(Debug, Clone, Deserialize)]
pub struct VerusPin {
    /// Release version, for example `0.2026.10.04.426d8b0`.
    pub release: String,
    /// Git tag of the release.
    pub tag: String,
    /// Full commit SHA of the release.
    pub commit: String,
    /// The Rust toolchain Verus itself needs.
    pub rust_toolchain: String,
}

/// The pinned Z3.
#[derive(Debug, Clone, Deserialize)]
pub struct Z3Pin {
    /// The version string Z3 reports.
    pub version: String,
}

/// One release archive.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetPin {
    /// Download URL.
    pub url: String,
    /// SHA-256 of the archive.
    pub sha256: String,
    /// Directory the archive unpacks to.
    pub dir: String,
    /// SHA-256 of the bundled `z3`, empty if not yet recorded.
    pub z3_sha256: String,
}

impl Pins {
    /// Reads the pins from `path`.
    ///
    /// # Errors
    ///
    /// `SAIL3002` if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::new("SAIL3002", format!("cannot read {}: {e}", path.display())))?;
        toml::from_str(&text)
            .map_err(|e| Error::new("SAIL3002", format!("cannot parse {}: {e}", path.display())))
    }
}

/// The host's target triple, as used for asset keys.
#[must_use]
pub fn host() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}

/// An installed, pinned Verus.
#[derive(Debug, Clone)]
pub struct Installed {
    /// The pins it was installed from.
    pub pins: Pins,
    /// The `verus` launcher.
    pub verus: PathBuf,
    /// `cargo-verus`, next to `verus` (only needed by `cargo-verus` units).
    pub cargo_verus: PathBuf,
    /// The bundled, pinned `z3`.
    pub z3: PathBuf,
}

impl Installed {
    /// Locates the install made by `cargo xtask fetch-verus` for `pins`:
    /// `$SAIL_VERUS_HOME`, else `<cache>/verus/<release>`.
    ///
    /// # Errors
    ///
    /// `SAIL3003` if there is no pinned asset for this host or the install
    /// is missing.
    pub fn locate(pins: Pins) -> Result<Self> {
        let asset = pins.assets.get(host()).ok_or_else(|| {
            Error::new(
                "SAIL3003",
                format!("no pinned Verus asset for host {}", host()),
            )
        })?;
        let root = match std::env::var_os("SAIL_VERUS_HOME").filter(|v| !v.is_empty()) {
            Some(home) => PathBuf::from(home),
            None => crate::cache_dir()?.join("verus").join(&pins.verus.release),
        };
        let dir = root.join(&asset.dir);
        let (verus, z3) = (dir.join("verus"), dir.join("z3"));
        let cargo_verus = dir.join("cargo-verus");
        if !verus.is_file() || !z3.is_file() {
            return Err(Error::new(
                "SAIL3003",
                format!(
                    "pinned Verus not installed in {} (run `cargo xtask fetch-verus`)",
                    dir.display()
                ),
            ));
        }
        Ok(Self {
            pins,
            verus,
            cargo_verus,
            z3,
        })
    }
}
