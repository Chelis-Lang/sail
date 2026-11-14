//! Authenticate original Kani and Rust tool bytes without executing them.
//!
//! Prefixes must be caller-owned and immutable while inspected and used. This
//! is a bounded observation of original payloads, never a filesystem snapshot,
//! execution, source closure, proof receipt or permission to reuse a result.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod stage;
pub use stage::Stage;

const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const RUST_FILES: usize = 7_017;
const RUST_BYTES: u64 = 1_586_050_167;
const RUST_TREE: &str = "f2fdf9b82a0521dd8c88fc10cef8e217cf42d855252786492564d82fa7cc4dbe";

/// A tool-payload refusal; no backend outcome follows.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            output,
            "error[SAIL6303]: tool payload authentication: {}",
            self.0
        )
    }
}

impl std::error::Error for Error {}

fn refuse(reason: impl std::fmt::Display) -> Error {
    Error(reason.to_string())
}

// Field order is part of the canonical inventory encoding: sorted ASCII paths,
// compact UTF-8 JSON and these lexicographically sorted record fields.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    bytes: u64,
    executable: bool,
    sha256: String,
}

/// Exact tool prefixes whose original payloads matched at inspection time.
/// The caller must preserve their ownership and immutability throughout use.
#[derive(Debug)]
pub struct Tools {
    bundle: PathBuf,
    rust: PathBuf,
    rust_tree_sha256: String,
}

impl Tools {
    /// Inspect every original bundle and Rust payload file on native Linux x86-64.
    /// No program is executed and no toolchain setup or symlink is created.
    ///
    /// # Errors
    /// `SAIL6303` on unsupported hosts, mismatched, excessive or unreadable trees.
    pub fn authenticate(bundle: &Path, rust: &Path) -> Result<Self, Error> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(refuse(
                "only the pinned native Linux x86-64 payload is supported",
            ));
        }
        let bundle = bundle.canonicalize().map_err(refuse)?;
        let rust = rust.canonicalize().map_err(refuse)?;
        let pin: serde_json::Value =
            serde_json::from_str(include_str!("../resources/engine/kani/bundle-linux.json"))
                .map_err(refuse)?;
        let expected: BTreeMap<String, Payload> = serde_json::from_value(
            pin.get("files")
                .cloned()
                .ok_or_else(|| refuse("missing embedded bundle files"))?,
        )
        .map_err(refuse)?;
        let (actual, _) = inventory(&bundle, &BTreeSet::new())?;
        if actual != expected {
            return Err(refuse("original Kani bundle inventory differs"));
        }
        let generated = generated_metadata();
        let (actual, _) = inventory(&rust, &generated)?;
        if actual.len() != RUST_FILES
            || actual.values().map(|file| file.bytes).sum::<u64>() != RUST_BYTES
        {
            return Err(refuse("original Rust payload counts differ"));
        }
        let encoded = serde_json::to_vec(&actual).map_err(refuse)?;
        let rust_tree_sha256 = format!("{:x}", Sha256::digest(encoded));
        if rust_tree_sha256 != RUST_TREE {
            return Err(refuse("original Rust payload inventory differs"));
        }
        Ok(Self {
            bundle,
            rust,
            rust_tree_sha256,
        })
    }

    /// Canonical original Kani bundle prefix, without an injected Rustup link.
    #[must_use]
    pub fn bundle(&self) -> &Path {
        &self.bundle
    }

    /// Canonical original Rust prefix; generated Rustup metadata is untrusted.
    #[must_use]
    pub fn rust(&self) -> &Path {
        &self.rust
    }

    /// Complete original Rust file inventory digest, not a printed version.
    #[must_use]
    pub fn rust_tree_sha256(&self) -> &str {
        &self.rust_tree_sha256
    }

    /// Reinspect original bytes before using a prior observation.
    ///
    /// # Errors
    /// `SAIL6303` when either complete payload no longer matches.
    pub fn recheck(&self) -> Result<(), Error> {
        Self::authenticate(&self.bundle, &self.rust).map(|_| ())
    }
}

fn generated_metadata() -> BTreeSet<String> {
    let mut names: BTreeSet<String> = [
        "components",
        "rust-installer-version",
        "multirust-channel-manifest.toml",
        "multirust-config.toml",
    ]
    .into_iter()
    .map(|name| format!("lib/rustlib/{name}"))
    .collect();
    for name in [
        "cargo",
        "rustc",
        "rust-std",
        "rustc-dev",
        "llvm-tools-preview",
    ] {
        names.insert(format!(
            "lib/rustlib/manifest-{name}-x86_64-unknown-linux-gnu"
        ));
    }
    names.insert("lib/rustlib/manifest-rust-src".into());
    names
}

fn inventory(
    root: &Path,
    generated: &BTreeSet<String>,
) -> Result<(BTreeMap<String, Payload>, usize), Error> {
    inventory_with_toolchain(root, generated, None)
}

fn inventory_with_toolchain(
    root: &Path,
    generated: &BTreeSet<String>,
    toolchain: Option<&Path>,
) -> Result<(BTreeMap<String, Payload>, usize), Error> {
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = BTreeMap::new();
    let mut entries = 0_usize;
    let mut total = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 64
            || !std::fs::symlink_metadata(&directory)
                .map_err(refuse)?
                .is_dir()
        {
            return Err(refuse("unsupported tool directory"));
        }
        for entry in std::fs::read_dir(&directory).map_err(refuse)? {
            let path = entry.map_err(refuse)?.path();
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(refuse("tool inventory exceeds its entry bound"));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(refuse)?
                .to_str()
                .ok_or_else(|| refuse("non-UTF-8 tool path"))?
                .to_owned();
            if relative.len() > 4096 || !relative.is_ascii() || relative.contains('\\') {
                return Err(refuse("unsupported tool path"));
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(refuse)?;
            if relative == "toolchain"
                && depth == 0
                && let Some(expected) = toolchain
            {
                if !metadata.is_symlink() || std::fs::read_link(&path).map_err(refuse)? != expected
                {
                    return Err(refuse("staged Rust toolchain route differs"));
                }
                continue;
            }
            if metadata.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            if !metadata.is_file() {
                return Err(refuse("linked or special tool file"));
            }
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| refuse("tool size overflow"))?;
            if total > MAX_BYTES {
                return Err(refuse("tool payload exceeds its byte bound"));
            }
            let executable = metadata.permissions().mode() & 0o111 != 0;
            if generated.contains(&relative) {
                if executable || metadata.len() > 4 * 1024 * 1024 {
                    return Err(refuse("unsupported generated tool metadata"));
                }
                // Validate readability too; these bytes never authenticate tools.
                hash_file(&path, metadata.len())?;
                continue;
            }
            let sha256 = hash_file(&path, metadata.len())?;
            files.insert(
                relative,
                Payload {
                    bytes: metadata.len(),
                    executable,
                    sha256,
                },
            );
        }
    }
    Ok((files, entries))
}

fn hash_file(path: &Path, length: u64) -> Result<String, Error> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(refuse)?;
    let file = File::from(fd);
    let metadata = file.metadata().map_err(refuse)?;
    if !metadata.is_file() || metadata.len() != length || length > MAX_BYTES {
        return Err(refuse("tool file changed or exceeds its bound"));
    }
    let mut input = file.take(length + 1);
    let mut total = 0_u64;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = input.read(&mut buffer).map_err(refuse)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        digest.update(&buffer[..count]);
    }
    if total != length {
        return Err(refuse("tool file changed during inspection"));
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_record_encoding_matches_original_archive_inventory_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let files = BTreeMap::from([(
            "bin/tool",
            Payload {
                bytes: 7,
                executable: true,
                sha256: "a".repeat(64),
            },
        )]);
        assert_eq!(
            serde_json::to_string(&files)?,
            "{\"bin/tool\":{\"bytes\":7,\"executable\":true,\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}"
        );
        Ok(())
    }

    #[test]
    fn payload_observation_refuses_links_and_detects_changed_original_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-kani-payload-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        std::fs::write(root.join("tool"), b"original")?;
        let (before, _) = inventory(&root, &BTreeSet::new())?;
        std::fs::write(root.join("tool"), b"modified")?;
        let (after, _) = inventory(&root, &BTreeSet::new())?;
        assert_ne!(before, after);
        std::os::unix::fs::symlink(root.join("tool"), root.join("alias"))?;
        assert!(inventory(&root, &BTreeSet::new()).is_err());
        std::fs::remove_file(root.join("alias"))?;
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            root.join("fifo"),
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )?;
        assert!(inventory(&root, &BTreeSet::new()).is_err());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
