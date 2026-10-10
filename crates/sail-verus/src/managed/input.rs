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
    /// Resolved package directories, including path, registry and git dependencies.
    pub roots: Vec<PathBuf>,
    /// Absolute Cargo/Sail output directories excluded from input tracking.
    pub excluded: Vec<PathBuf>,
    /// False if declared inputs could not be read completely.
    pub complete: bool,
    pub(crate) observation_paths: BTreeSet<PathBuf>,
    #[cfg(target_os = "linux")]
    pub(super) content_paths: BTreeSet<PathBuf>,
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

fn collect(
    path: &Path,
    files: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
    roots: &mut Vec<PathBuf>,
) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    if !visited.insert(canonical.clone()) {
        return true;
    }
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        roots.push(canonical.clone());
        roots.push(path.to_owned());
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
        if [".git", ".claude", ".codex"].iter().any(|s| name == *s) {
            continue;
        }
        complete &= collect(&entry.path(), files, visited, roots);
    }
    complete
}

pub(super) fn snapshot_metadata(data: &Value, extras: &[PathBuf]) -> InputSnapshot {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    snapshot_metadata_home(data, extras, home.as_deref())
}

pub(super) fn snapshot_metadata_home(
    data: &Value,
    extras: &[PathBuf],
    cargo_home: Option<&Path>,
) -> InputSnapshot {
    let workspace = PathBuf::from(data["workspace_root"].as_str().unwrap_or_default());
    let invocation = Path::new(
        data["sail_invocation_directory"]
            .as_str()
            .unwrap_or_else(|| workspace.to_str().unwrap_or_default()),
    );
    let mut files = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut excluded: Vec<PathBuf> = ["sail_cache_directory", "target_directory"]
        .into_iter()
        .filter_map(|name| data[name].as_str().map(|p| invocation.join(p)))
        .collect();
    if let Ok(path) = crate::cache_dir() {
        excluded.push(invocation.join(path));
    }
    let excluded_canonical: BTreeSet<_> = excluded
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();
    visited.extend(excluded_canonical.iter().cloned());
    excluded.sort();
    excluded.dedup();
    let mut roots = Vec::new();
    let mut complete = !workspace.as_os_str().is_empty();
    for p in data["packages"].as_array().into_iter().flatten() {
        if let Some(parent) = p["manifest_path"]
            .as_str()
            .and_then(|s| Path::new(s).parent())
        {
            roots.push(parent.to_owned());
            complete &= collect(parent, &mut files, &mut visited, &mut roots);
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

    files.extend(
        configuration_paths(invocation, cargo_home)
            .into_iter()
            .filter(|p| p.is_file()),
    );
    let source_inputs: Vec<PathBuf> = data["sail_source_inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    roots.extend(source_inputs.iter().cloned());
    for path in extras.iter().chain(&source_inputs) {
        complete &= collect(path, &mut files, &mut visited, &mut roots);
    }
    let mut observation_paths = files.clone();
    observation_paths.extend(
        visited
            .into_iter()
            .filter(|p| !excluded_canonical.contains(p)),
    );
    let stamp_before = stamp_paths(&observation_paths);
    let (revision, readable) = contents_revision(&files);
    complete &= readable;
    complete &= stamp_before.is_some() && stamp_before == stamp_paths(&observation_paths);
    roots.sort();
    roots.dedup();
    InputSnapshot {
        workspace,
        revision,
        roots,
        excluded,
        complete,
        observation_paths,
        #[cfg(target_os = "linux")]
        content_paths: files,
    }
}

// Preserve the original exact sorted path spelling and digest encoding.
fn contents_revision(files: &BTreeSet<PathBuf>) -> (String, bool) {
    let mut complete = true;
    let mut hasher = Sha256::new();
    for path in files {
        let bytes = path.as_os_str().as_encoded_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        if let Ok(hash) = file_digest(path) {
            hasher.update(hash.as_bytes());
        } else {
            complete = false;
            hasher.update(b"unreadable");
        }
    }
    (format!("{:x}", hasher.finalize()), complete)
}

#[cfg(target_os = "linux")]
impl InputSnapshot {
    // The caller must retain complete native namespace observation plus read
    // leases covering this inventory from its coherent cold collection through
    // both read barriers. This hashes ALL mutable contents on EVERY invocation;
    // it cannot discover new paths and is not standalone freshness evidence.
    #[cfg(test)]
    pub(super) fn rehash_guarded_inventory(&self) -> Self {
        let (revision, readable) = contents_revision(&self.content_paths);
        let mut refreshed = self.clone();
        refreshed.revision = revision;
        refreshed.complete &= readable;
        refreshed
    }
}

// A run-local mutation stamp is separate from logical proof keys: reverting an
// edit between completed runs may reuse a proof, reverting during a run may not.
pub(super) fn mutation_stamp(snapshot: &InputSnapshot) -> Option<String> {
    let mut paths = snapshot.observation_paths.clone();
    for path in &snapshot.roots {
        paths.insert(path.clone());
        if let Ok(canonical) = path.canonicalize() {
            paths.insert(canonical);
        }
        for alias in path
            .ancestors()
            .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            paths.insert(alias.to_owned());
        }
        // Linux drains an exact native queue for absent configuration paths.
        // Other platforms retain conservative parent stamps because native
        // callbacks and cookies cannot establish a synchronous absence barrier.
        #[cfg(not(target_os = "linux"))]
        if !path.exists()
            && let Some(parent) = path.ancestors().skip(1).find(|p| p.exists())
        {
            paths.insert(parent.to_owned());
            if let Ok(canonical) = parent.canonicalize() {
                paths.insert(canonical);
            }
        }
    }
    stamp_paths(&paths)
}

#[cfg(unix)]
fn stamp_paths(paths: &BTreeSet<PathBuf>) -> Option<String> {
    use std::os::unix::fs::MetadataExt as _;
    let mut stamp = Sha256::new();
    for path in paths {
        let name = path.as_os_str().as_encoded_bytes();
        stamp.update((name.len() as u64).to_le_bytes());
        stamp.update(name);
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                stamp.update(b"missing");
                continue;
            }
            Err(_) => return None,
        };
        for number in [
            metadata.dev(),
            metadata.ino(),
            metadata.size(),
            u64::from(metadata.mode()),
        ] {
            stamp.update(number.to_le_bytes());
        }
        for number in [
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ] {
            stamp.update(number.to_le_bytes());
        }
    }
    Some(format!("{:x}", stamp.finalize()))
}

#[cfg(not(unix))]
fn stamp_paths(_paths: &BTreeSet<PathBuf>) -> Option<String> {
    None
}

/// Hashes Cargo's declared workspace and resolved dependency inputs.
///
/// # Errors
/// Returns `SAIL3103` when Cargo metadata cannot be obtained.
pub fn snapshot(workspace: &Path, args: &[String], cargo: &Path) -> Result<InputSnapshot> {
    let invocation =
        std::path::absolute(workspace).map_err(|e| Error::new("SAIL3103", e.to_string()))?;
    let mut data = metadata(workspace, args, cargo)?;
    data["sail_invocation_directory"] =
        serde_json::Value::String(invocation.to_string_lossy().into_owned());
    let mut snapshot = snapshot_metadata(&data, &[]);
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    snapshot
        .roots
        .extend(configuration_paths(&invocation, home.as_deref()));
    snapshot.roots.sort();
    snapshot.roots.dedup();
    Ok(snapshot)
}

// Used for build-script paths outside the package/workspace root.
pub(super) fn snapshot_paths(paths: &[PathBuf]) -> (String, bool) {
    let data = serde_json::json!({"workspace_root":"/", "packages":[]});
    let snapshot = snapshot_metadata(&data, paths);
    (snapshot.revision, snapshot.complete)
}

pub(super) fn configuration_paths(workspace: &Path, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for parent in workspace.ancestors() {
        files.extend(
            [
                ".cargo/config",
                ".cargo/config.toml",
                "rust-toolchain",
                "rust-toolchain.toml",
            ]
            .into_iter()
            .map(|name| parent.join(name)),
        );
    }
    if let Some(home) = cargo_home {
        let home = workspace.join(home);
        files.extend([home.join("config"), home.join("config.toml")]);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn retained_inventory_preserves_exact_non_utf8_sorted_encoding_and_rehashes_bytes() {
        use std::os::unix::{ffi::OsStringExt as _, fs::symlink};
        let root = std::env::temp_dir().join(format!("sail-inventory-{}", std::process::id()));
        let package = root.join("package");
        assert!(std::fs::create_dir_all(&package).is_ok());
        let outside = root.join("external.rs");
        assert!(std::fs::write(&outside, b"outside").is_ok());
        let odd = package.join(std::ffi::OsString::from_vec(b"source-\xff.rs".to_vec()));
        assert!(std::fs::write(&odd, b"initial").is_ok());
        assert!(std::fs::write(package.join("Cargo.toml"), b"manifest").is_ok());
        assert!(symlink(&outside, package.join("alias.rs")).is_ok());
        let data = serde_json::json!({"workspace_root":package,
            "packages":[{"manifest_path":package.join("Cargo.toml")}]});
        let snapshot = snapshot_metadata_home(&data, &[], None);
        assert!(snapshot.complete);
        let mut oracle = Sha256::new();
        let mut files = vec![outside.clone(), odd.clone(), package.join("Cargo.toml")];
        files.sort();
        for path in &files {
            let bytes = path.as_os_str().as_encoded_bytes();
            oracle.update((bytes.len() as u64).to_le_bytes());
            oracle.update(bytes);
            let contents = std::fs::read(path).unwrap_or_else(|e| unreachable!("fixture:{e}"));
            oracle.update(format!("{:x}", Sha256::digest(contents)).as_bytes());
        }
        assert_eq!(snapshot.revision, format!("{:x}", oracle.finalize()));
        assert_eq!(
            snapshot.revision,
            snapshot.rehash_guarded_inventory().revision
        );
        assert!(std::fs::write(&odd, b"changed").is_ok());
        let refreshed = snapshot.rehash_guarded_inventory();
        assert!(refreshed.complete);
        assert_ne!(snapshot.revision, refreshed.revision);
        assert_eq!(
            refreshed.revision,
            snapshot_metadata_home(&data, &[], None).revision
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn relative_cargo_home_resolves_against_recorded_invocation() {
        let root =
            std::env::temp_dir().join(format!("sail-relative-cargo-home-{}", std::process::id()));
        let invocation = root.join("client");
        let package = root.join("package");
        assert!(std::fs::create_dir_all(invocation.join("cargo-home")).is_ok());
        assert!(std::fs::create_dir_all(&package).is_ok());
        assert!(std::fs::write(package.join("Cargo.toml"), "manifest").is_ok());
        let path = invocation.join("cargo-home/config.toml");
        assert!(std::fs::write(&path, "[build]\njobs=1\n").is_ok());
        let data = serde_json::json!({"workspace_root":root,"sail_invocation_directory":invocation,
            "packages":[{"manifest_path":package.join("Cargo.toml")}]});
        let before = snapshot_metadata_home(&data, &[], Some(Path::new("cargo-home")));
        assert!(std::fs::write(&path, "[build]\njobs=2\n").is_ok());
        let after = snapshot_metadata_home(&data, &[], Some(Path::new("cargo-home")));
        assert!(before.complete && after.complete);
        assert_ne!(before.revision, after.revision);
        assert!(configuration_paths(&invocation, Some(Path::new("cargo-home"))).contains(&path));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn invoking_member_cwd_configuration_participates_in_revision() {
        let root = std::env::temp_dir().join(format!("sail-cwd-config-{}", std::process::id()));
        let group = root.join("groups");
        let member = group.join("member");
        assert!(std::fs::create_dir_all(group.join(".cargo")).is_ok());
        assert!(std::fs::create_dir_all(&member).is_ok());
        assert!(std::fs::write(member.join("Cargo.toml"), "manifest").is_ok());
        let config = group.join(".cargo/config.toml");
        assert!(std::fs::write(&config, "[build]\njobs=1\n").is_ok());
        let data = serde_json::json!({"workspace_root":root,"sail_invocation_directory":member,
            "packages":[{"manifest_path":member.join("Cargo.toml")}]});
        let before = snapshot_metadata_home(&data, &[], None);
        assert!(std::fs::write(&config, "[build]\njobs=2\n").is_ok());
        let after = snapshot_metadata_home(&data, &[], None);
        assert!(before.complete && after.complete);
        assert_ne!(before.revision, after.revision);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    use std::fs::{File, FileTimes};

    #[test]
    fn mutation_stamp_detects_restored_content_and_mtime_without_callbacks() {
        let root = std::env::temp_dir().join(format!("sail-mutation-stamp-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        assert!(std::fs::create_dir_all(root.join("target")).is_ok());
        let source = root.join("source.rs");
        assert!(std::fs::write(&source, "original").is_ok());
        let data = serde_json::json!({"workspace_root":root,"target_directory":root.join("target"),"packages":[{"manifest_path":root.join("Cargo.toml")}]});
        let before = snapshot_metadata(&data, &[]);
        let stamp_before = mutation_stamp(&before);
        assert!(std::fs::write(root.join("target/output"), "owned output").is_ok());
        assert_eq!(stamp_before, mutation_stamp(&snapshot_metadata(&data, &[])));
        let file = File::open(&source).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let mtime = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(std::fs::write(&source, "transient").is_ok());
        assert!(std::fs::write(&source, "original").is_ok());
        assert!(file.set_times(FileTimes::new().set_modified(mtime)).is_ok());
        let after = snapshot_metadata(&data, &[]);
        assert!(before.complete && after.complete);
        assert_eq!(before.revision, after.revision);
        assert_ne!(stamp_before, mutation_stamp(&after));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn runtime_alias_target_restoration_changes_stamp() {
        let root = std::env::temp_dir().join(format!("sail-runtime-stamp-{}", std::process::id()));
        assert!(std::fs::create_dir_all(root.join("project")).is_ok());
        let runtime = root.join("runtime");
        let alias = root.join("alias");
        assert!(std::fs::write(&runtime, "original").is_ok());
        assert!(std::os::unix::fs::symlink(&runtime, &alias).is_ok());
        let data = serde_json::json!({"workspace_root":root.join("project"),"packages":[]});
        let mut before = snapshot_metadata(&data, &[]);
        before.roots.push(alias);
        let stamp = mutation_stamp(&before);
        let file = File::open(&runtime).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let mtime = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(std::fs::write(&runtime, "transient").is_ok());
        assert!(std::fs::write(&runtime, "original").is_ok());
        assert!(file.set_times(FileTimes::new().set_modified(mtime)).is_ok());
        assert_ne!(stamp, mutation_stamp(&before));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn content_revision_covers_registry_dependencies_and_restored_mtime() {
        let root = std::env::temp_dir().join(format!("sail-input-test-{}", std::process::id()));
        let dependency = root.join("dep");
        let project = root.join("project");
        assert!(std::fs::create_dir_all(dependency.join("src/target")).is_ok());
        assert!(std::fs::create_dir_all(&project).is_ok());
        let source = dependency.join("src/target/mod.rs");
        assert!(std::fs::write(&source, "good").is_ok());
        let data = serde_json::json!({"workspace_root":project,"packages":[{"source":"registry+https://example.invalid/index","manifest_path":dependency.join("Cargo.toml")}]});
        let before = snapshot_metadata(&data, &[]);
        let file = File::open(&source).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let mtime = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(std::fs::write(&source, "fail").is_ok());
        assert!(file.set_times(FileTimes::new().set_modified(mtime)).is_ok());
        let after = snapshot_metadata(&data, &[]);
        assert!(before.complete && after.complete);
        assert_ne!(before.revision, after.revision);
        assert_eq!(after.roots, vec![dependency]);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
