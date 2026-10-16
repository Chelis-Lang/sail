//! The pinned Verus and Z3 (`corpus/toolchain.toml`) and where they are
//! installed.
//!
//! The file has one primary pin, the release the corpus is validated
//! against, and optional `[secondary."<release>"]` pins: secondary
//! compatibility targets (ADR 0010 item 4). Setting `SAIL_VERUS_RELEASE` to
//! a secondary release makes [`Pins::load`] return that release instead, so
//! the harness, the benchmarks and `cargo xtask fetch-verus` all use it.

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
    /// Secondary compatibility targets, by release.
    #[serde(default)]
    pub secondary: BTreeMap<String, SecondaryPin>,
    /// Whether a secondary release was selected (`SAIL_VERUS_RELEASE`).
    #[serde(skip)]
    pub is_secondary: bool,
    /// When a secondary release is selected: the primary release and its
    /// archives (a secondary release may use the primary's Z3).
    #[serde(skip)]
    pub primary: Option<(VerusPin, BTreeMap<String, AssetPin>)>,
}

/// A secondary Verus release and its archives.
#[derive(Debug, Clone, Deserialize)]
pub struct SecondaryPin {
    /// The release.
    pub verus: VerusPin,
    /// Its Z3.
    pub z3: Z3Pin,
    /// Its archives by host target triple.
    pub assets: BTreeMap<String, AssetPin>,
}

/// Environment variable selecting a secondary release.
pub const ENV_RELEASE: &str = "SAIL_VERUS_RELEASE";

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
    /// `"primary"` if this archive bundles no `z3` and the primary
    /// release's (same version, checked by `z3_sha256`) is used instead.
    #[serde(default)]
    pub z3: Option<String>,
}

impl Pins {
    /// The compiled-in supported pins, selecting any requested secondary release.
    ///
    /// # Errors
    ///
    /// `SAIL3002` for malformed embedded pins or `SAIL3004` for an unknown release.
    pub fn embedded() -> Result<Self> {
        let pins: Self = toml::from_str(include_str!("../../../corpus/toolchain.toml"))
            .map_err(|e| Error::new("SAIL3002", format!("cannot parse embedded pins: {e}")))?;
        match std::env::var(ENV_RELEASE).ok().filter(|r| !r.is_empty()) {
            Some(release) => pins.select(&release),
            None => Ok(pins),
        }
    }
    /// Reads the pins from `path`.
    ///
    /// # Errors
    ///
    /// `SAIL3002` if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::new("SAIL3002", format!("cannot read {}: {e}", path.display())))?;
        let pins: Self = toml::from_str(&text)
            .map_err(|e| Error::new("SAIL3002", format!("cannot parse {}: {e}", path.display())))?;
        match std::env::var(ENV_RELEASE).ok().filter(|r| !r.is_empty()) {
            Some(release) => pins.select(&release),
            None => Ok(pins),
        }
    }

    /// These pins with `release` as the active one: the primary release
    /// itself, or one of the secondary releases.
    ///
    /// # Errors
    ///
    /// `SAIL3004` if `release` is neither.
    pub fn select(self, release: &str) -> Result<Self> {
        if release == self.verus.release {
            return Ok(self);
        }
        let Some(s) = self.secondary.get(release).cloned() else {
            let known: Vec<&str> = std::iter::once(self.verus.release.as_str())
                .chain(self.secondary.keys().map(String::as_str))
                .collect();
            return Err(Error::new(
                "SAIL3004",
                format!(
                    "Verus release {release} is not pinned (pinned: {})",
                    known.join(", ")
                ),
            ));
        };
        Ok(Self {
            verus: s.verus,
            z3: s.z3,
            assets: s.assets,
            secondary: self.secondary,
            is_secondary: true,
            primary: Some((self.verus, self.assets)),
        })
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
    /// The pinned `z3`: bundled with this release, or the primary
    /// release's when this one bundles none.
    pub z3: PathBuf,
    /// Whether `z3` is bundled with this release. If not, plain Verus must be
    /// told where it is (`VERUS_Z3_PATH`), or its launcher finds none.
    pub z3_bundled: bool,
}

impl Installed {
    fn from_source(pins: Pins, build: &Path) -> Result<Self> {
        let manifest = crate::source_build::Manifest::load(build)?;
        let expected = pins.assets.get(host()).map(|a| a.z3_sha256.as_str());
        if manifest.commit != pins.verus.commit
            || manifest.rust_toolchain != pins.verus.rust_toolchain
            || Some(manifest.z3_sha256.as_str()) != expected
            || manifest.features.iter().any(|f| f != "singular")
            || manifest.artifacts.is_empty()
            || !manifest.is_complete(build)
        {
            return Err(Error::new(
                "SAIL3040",
                "source build identity or artifact hashes do not match the active pins",
            ));
        }
        let dir = crate::source_build::checkout_dir(build).join("source/target-verus/release");
        let verus = dir.join("verus");
        let cargo_verus = dir.join("cargo-verus");
        let z3 = dir.join("z3");
        if crate::source_build::hash_file(&z3).ok().as_deref() != Some(manifest.z3_sha256.as_str())
        {
            return Err(Error::new(
                "SAIL3040",
                "source build solver binary does not match its pinned hash",
            ));
        }
        if ![&verus, &cargo_verus, &z3].iter().all(|p| p.is_file()) {
            return Err(Error::new(
                "SAIL3040",
                "source build has missing runtime executables",
            ));
        }
        Ok(Self {
            pins,
            verus,
            cargo_verus,
            z3,
            z3_bundled: true,
        })
    }

    /// Locates the install made by `cargo xtask fetch-verus` for `pins`:
    /// `$SAIL_VERUS_HOME`, else `<cache>/verus/<release>`.
    ///
    /// # Errors
    ///
    /// `SAIL3003` if there is no pinned asset for this host or the install
    /// is missing.
    pub fn locate(pins: Pins) -> Result<Self> {
        if let Some(build) = std::env::var_os("SAIL_VERUS_SOURCE_BUILD") {
            return Self::from_source(pins, Path::new(&build));
        }
        Self::locate_release(pins)
    }

    /// Locates the pinned binary release, ignoring `SAIL_VERUS_SOURCE_BUILD`.
    ///
    /// This supplies the release's solver to a separately validated source
    /// suite. `SAIL_VERUS_HOME` and secondary-release selection retain their
    /// normal meaning; this does not admit a source build as a release.
    ///
    /// # Errors
    ///
    /// `SAIL3003` if the host has no pinned asset or the release is not installed.
    pub fn locate_release(pins: Pins) -> Result<Self> {
        let asset = pins.assets.get(host()).ok_or_else(|| {
            Error::new(
                "SAIL3003",
                format!("no pinned Verus asset for host {}", host()),
            )
        })?;
        // SAIL_VERUS_HOME names the primary release's install; secondary
        // releases always install under the cache.
        let root_of = |release: &str, primary: bool| -> Result<PathBuf> {
            match std::env::var_os("SAIL_VERUS_HOME").filter(|v| !v.is_empty() && primary) {
                Some(home) => Ok(PathBuf::from(home)),
                None => Ok(crate::cache_dir()?.join("verus").join(release)),
            }
        };
        let dir = root_of(&pins.verus.release, !pins.is_secondary)?.join(&asset.dir);
        let verus = dir.join("verus");
        let (z3, z3_bundled) = match (asset.z3.as_deref(), &pins.primary) {
            (Some("primary"), Some((pv, passets))) => {
                let pdir = passets
                    .get(host())
                    .map(|a| a.dir.clone())
                    .unwrap_or_default();
                (root_of(&pv.release, true)?.join(pdir).join("z3"), false)
            }
            _ => (dir.join("z3"), true),
        };
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
            z3_bundled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Pins;

    const PINS: &str = r#"
[verus]
release = "1.0"
tag = "release/1.0"
commit = "aaa"
rust_toolchain = "1.98.1"

[z3]
version = "4.16.0"

[assets.h]
url = "u1"
sha256 = "s1"
dir = "d"
z3_sha256 = ""

[secondary."0.9".verus]
release = "0.9"
tag = "release/0.9"
commit = "bbb"
rust_toolchain = "1.98.0"

[secondary."0.9".z3]
version = "4.16.0"

[secondary."0.9".assets.h]
url = "u2"
sha256 = "s2"
dir = "d"
z3_sha256 = ""
"#;

    #[test]
    fn selects_primary_or_secondary_releases() {
        let pins: Pins = match toml::from_str(PINS) {
            Ok(p) => p,
            Err(e) => unreachable!("{e}"),
        };
        let primary = pins.clone().select("1.0");
        assert!(
            primary
                .as_ref()
                .is_ok_and(|p| !p.is_secondary && p.verus.commit == "aaa")
        );
        let secondary = pins.clone().select("0.9");
        assert!(secondary.as_ref().is_ok_and(|p| p.is_secondary
            && p.verus.commit == "bbb"
            && p.verus.rust_toolchain == "1.98.0"
            && p.assets.get("h").is_some_and(|a| a.url == "u2")));
        assert!(pins.select("0.8").is_err());
    }
}
