//! Cargo metadata and declared input revision tracking.

use super::files::file_digest;
use crate::{Error, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Declared workspace inputs used by managed jobs and watch.
#[derive(Debug, Clone)]
pub struct InputSnapshot {
    /// Cargo's canonical workspace root.
    pub workspace: PathBuf,
    /// Digest of paths and file contents.
    pub revision: String,
    /// Local package directories, including external path dependencies.
    pub roots: Vec<PathBuf>,
    /// False if declared inputs could not be read completely.
    pub complete: bool,
}

pub(super) fn metadata(workspace: &Path, args: &[String], cargo: &Path) -> Result<Value> {
    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace)
        .args(["metadata", "--format-version", "1"]);
    let mut it = args.iter().take_while(|a| a.as_str() != "--");
    while let Some(a) = it.next() {
        if ["--manifest-path", "--features"].contains(&a.as_str()) {
            cmd.arg(a);
            if let Some(v) = it.next() {
                cmd.arg(v);
            }
        } else if a.starts_with("--manifest-path=")
            || a.starts_with("--features=")
            || [
                "--all-features",
                "--no-default-features",
                "--offline",
                "--locked",
                "--frozen",
            ]
            .contains(&a.as_str())
        {
            cmd.arg(a);
        }
    }
    let output = cmd
        .output()
        .map_err(|e| Error::new("SAIL3103", e.to_string()))?;
    if !output.status.success() {
        return Err(Error::new(
            "SAIL3103",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| Error::new("SAIL3103", e.to_string()))
}

fn collect(path: &Path, files: &mut BTreeSet<PathBuf>, visited: &mut BTreeSet<PathBuf>) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    if !visited.insert(canonical.clone()) {
        return true;
    }
    if canonical.is_file() {
        files.insert(canonical);
        return true;
    }
    let Ok(entries) = std::fs::read_dir(&canonical) else {
        return false;
    };
    let mut complete = true;
    for entry in entries {
        let Ok(entry) = entry else {
            complete = false;
            continue;
        };
        let name = entry.file_name();
        if ["target", ".git", ".claude", ".codex"]
            .iter()
            .any(|s| name == *s)
        {
            continue;
        }
        complete &= collect(&entry.path(), files, visited);
    }
    complete
}

pub(super) fn snapshot_metadata(data: &Value, extras: &[PathBuf]) -> InputSnapshot {
    let workspace = PathBuf::from(data["workspace_root"].as_str().unwrap_or_default());
    let mut files = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut roots = Vec::new();
    let mut complete = !workspace.as_os_str().is_empty();
    for p in data["packages"].as_array().into_iter().flatten() {
        if p["source"].is_null()
            && let Some(parent) = p["manifest_path"]
                .as_str()
                .and_then(|s| Path::new(s).parent())
        {
            roots.push(parent.to_owned());
            complete &= collect(parent, &mut files, &mut visited);
        }
    }
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
    ] {
        let path = workspace.join(name);
        if path.is_file() {
            files.insert(path);
        }
    }
    for parent in workspace.ancestors() {
        for name in [".cargo/config", ".cargo/config.toml"] {
            let path = parent.join(name);
            if path.is_file() {
                files.insert(path);
            }
        }
    }
    if let Some(home) = std::env::var_os("CARGO_HOME").or_else(|| {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo").into_os_string())
    }) {
        for name in ["config", "config.toml"] {
            let path = PathBuf::from(&home).join(name);
            if path.is_file() {
                files.insert(path);
            }
        }
    }
    for path in extras {
        complete &= collect(path, &mut files, &mut visited);
    }
    let mut hasher = Sha256::new();
    for path in files {
        let bytes = path.as_os_str().as_encoded_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        if let Ok(hash) = file_digest(&path) {
            hasher.update(hash.as_bytes());
        } else {
            complete = false;
            hasher.update(b"unreadable");
        }
    }
    roots.sort();
    roots.dedup();
    InputSnapshot {
        workspace,
        revision: format!("{:x}", hasher.finalize()),
        roots,
        complete,
    }
}

/// Hashes Cargo's declared local workspace and dependency inputs.
///
/// # Errors
/// Returns `SAIL3103` when Cargo metadata cannot be obtained.
pub fn snapshot(workspace: &Path, args: &[String], cargo: &Path) -> Result<InputSnapshot> {
    Ok(snapshot_metadata(&metadata(workspace, args, cargo)?, &[]))
}
