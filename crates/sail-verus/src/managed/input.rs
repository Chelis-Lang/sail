//! Cargo metadata and declared input revision tracking.

use super::files::digest_all;
use crate::{Error, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
    /// The content digest of every readable input file in `revision`.
    pub(super) digests: BTreeMap<PathBuf, String>,
}

fn metadata_command(workspace: &Path, args: &[String], cargo: &Path) -> Command {
    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace)
        .args(["metadata", "--format-version", "1"]);
    let mut it = args.iter().take_while(|a| a.as_str() != "--");
    while let Some(a) = it.next() {
        if ["--manifest-path", "--features", "-F"].contains(&a.as_str()) {
            cmd.arg(a);
            if let Some(v) = it.next() {
                cmd.arg(v);
            }
        } else if a.starts_with("--manifest-path=")
            || a.starts_with("--features=")
            || a.starts_with("-F")
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
    cmd
}

pub(super) fn metadata(workspace: &Path, args: &[String], cargo: &Path) -> Result<Value> {
    let output = metadata_command(workspace, args, cargo)
        .output()
        .map_err(|e| Error::new("SAIL3103", e.to_string()))?;
    if !output.status.success() {
        return Err(Error::new(
            "SAIL3103",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let mut data: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| Error::new("SAIL3103", e.to_string()))?;
    // Preserve explicit package roles for disabled-script scheduling checks.
    // Unknown or ambiguous package specs retain original unobserved cleaning.
    data["sail_selected_package_ids"] =
        serde_json::to_value(selected_packages(&data, args)).unwrap_or(Value::Null);
    Ok(data)
}

fn selected_packages(data: &Value, args: &[String]) -> Option<Vec<String>> {
    let packages = data["packages"].as_array()?;
    let mut selected = Vec::new();
    let mut arguments = args.iter().take_while(|arg| arg.as_str() != "--");
    while let Some(argument) = arguments.next() {
        let spec = if argument == "-p" || argument == "--package" {
            Some(arguments.next()?.as_str())
        } else if let Some(spec) = argument.strip_prefix("--package=") {
            Some(spec)
        } else if let Some(spec) = argument.strip_prefix("-p") {
            Some(spec.strip_prefix('=').unwrap_or(spec))
        } else {
            if argument.starts_with("--package") {
                return None;
            }
            None
        };
        let Some(spec) = spec else { continue };
        let mut matches = packages.iter().filter(|package| {
            package["id"].as_str() == Some(spec)
                || package["name"].as_str() == Some(spec)
                || spec.split_once('@').is_some_and(|(name, version)| {
                    package["name"].as_str() == Some(name)
                        && package["version"].as_str() == Some(version)
                })
        });
        let package = matches.next()?;
        matches.next().is_none().then_some(())?;
        selected.push(package["id"].as_str()?.to_owned());
    }
    Some(selected)
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
    collect_directory(&canonical, files, visited, roots)
}

/// The entries of the canonical directory `directory`, as [`collect`] finds
/// them. An entry that is not a symbolic link is already its own canonical
/// path, so only links are resolved: one `realpath` per link instead of one
/// per file (about 100 ms per snapshot on vest's 11,600 input files).
fn collect_directory(
    directory: &Path,
    files: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
    roots: &mut Vec<PathBuf>,
) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
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
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_file() => {
                if visited.insert(path.clone()) {
                    files.insert(path);
                }
            }
            Ok(kind) if kind.is_dir() => {
                if visited.insert(path.clone()) {
                    complete &= collect_directory(&path, files, visited, roots);
                }
            }
            _ => complete &= collect(&path, files, visited, roots),
        }
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
    let (revision, readable, digests) = contents_revision(&files);
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
        digests,
    }
}

/// `snapshot` narrowed for proof-time kernel coverage of one selection.
///
/// A root inside the directory of a resolved package outside `closure` is
/// dropped, unless it is also inside the directory of a package in `closure`
/// (nested packages stay covered). The workspace manifest, lockfile and every
/// member manifest are added, since Cargo reads them to load the workspace.
/// The caller must refuse coverage if Cargo compiles a unit outside
/// `closure`. Without a closure the snapshot is returned unchanged.
#[cfg(test)]
pub(super) fn closure_scope(
    snapshot: &InputSnapshot,
    data: &Value,
    closure: Option<&BTreeSet<String>>,
) -> InputSnapshot {
    let mut scoped = snapshot.clone();
    let Some(closure) = closure else {
        return scoped;
    };
    let scope = Scope::new(data, closure);
    scoped.roots.retain(|root| scope.contains(root));
    scoped.roots.extend(scope.manifests);
    scoped.roots.sort();
    scoped.roots.dedup();
    scoped
}

/// The input digests of `snapshot` that one selection depends on, narrowed by
/// the same rule as `closure_scope`: files of packages outside `closure` are
/// dropped, and the workspace manifest, lockfile and member manifests stay.
/// Without a closure every digest is kept.
pub(super) fn closure_digests(
    snapshot: &InputSnapshot,
    data: &Value,
    closure: Option<&BTreeSet<String>>,
) -> BTreeMap<PathBuf, String> {
    let Some(closure) = closure else {
        return snapshot.digests.clone();
    };
    let scope = Scope::new(data, closure);
    snapshot
        .digests
        .iter()
        .filter(|(path, _)| scope.contains(path) || scope.manifests.contains(path.as_path()))
        .map(|(path, hash)| (path.clone(), hash.clone()))
        .collect()
}

/// Package directories inside and outside one selection's closure, and the
/// manifests Cargo reads to load the workspace.
///
/// Sets, not lists: a run checks every input file (thousands, with registry
/// dependencies) against every package directory, and testing a file's
/// ancestors for membership replaces a scan of all packages per file
/// (about 230 ms per vest run).
struct Scope {
    inside: std::collections::HashSet<PathBuf>,
    outside: std::collections::HashSet<PathBuf>,
    manifests: std::collections::HashSet<PathBuf>,
}
impl Scope {
    fn new(data: &Value, closure: &BTreeSet<String>) -> Self {
        let mut scope = Self {
            inside: std::collections::HashSet::new(),
            outside: std::collections::HashSet::new(),
            manifests: std::collections::HashSet::new(),
        };
        for package in data["packages"].as_array().into_iter().flatten() {
            let Some(manifest) = package["manifest_path"].as_str().map(Path::new) else {
                continue;
            };
            let Some(directory) = manifest.parent() else {
                continue;
            };
            // An unidentified package is never excluded from coverage.
            if package["id"].as_str().is_none_or(|id| closure.contains(id)) {
                scope.inside.insert(directory.to_owned());
            } else {
                scope.outside.insert(directory.to_owned());
            }
            if data["workspace_members"]
                .as_array()
                .is_some_and(|members| members.contains(&package["id"]))
            {
                scope.manifests.insert(manifest.to_owned());
            }
        }
        if let Some(workspace) = data["workspace_root"].as_str().map(Path::new) {
            scope.manifests.extend(
                ["Cargo.toml", "Cargo.lock"]
                    .iter()
                    .map(|name| workspace.join(name))
                    .filter(|path| path.is_file()),
            );
        }
        scope
    }
    /// Inside a closure package directory (nested packages stay covered), or
    /// outside every package directory that is not in the closure. A path
    /// starts with a directory exactly when the directory is one of its
    /// ancestors (both compare by component).
    fn contains(&self, path: &Path) -> bool {
        path.ancestors().any(|d| self.inside.contains(d))
            || !path.ancestors().any(|d| self.outside.contains(d))
    }
}

// Preserve the original exact sorted path spelling and digest encoding. Every
// file is still read completely between the caller's stamps; only the reads
// overlap, and their digests are folded in the original sorted order.
fn contents_revision(files: &BTreeSet<PathBuf>) -> (String, bool, BTreeMap<PathBuf, String>) {
    let mut complete = true;
    let mut recorded = BTreeMap::new();
    let mut hasher = Sha256::new();
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    for (path, digest) in files.iter().zip(digest_all(&paths)) {
        let bytes = path.as_os_str().as_encoded_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        if let Ok(hash) = digest {
            hasher.update(hash.as_bytes());
            recorded.insert(path.clone(), hash);
        } else {
            complete = false;
            hasher.update(b"unreadable");
        }
    }
    (format!("{:x}", hasher.finalize()), complete, recorded)
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
    }
    stamp_paths(&paths)
}

// The `lstat` calls overlap (about 13,000 paths per vest snapshot); the stamp
// folds their results in the original sorted order, exactly as one by one.
#[cfg(unix)]
fn stamp_paths(paths: &BTreeSet<PathBuf>) -> Option<String> {
    use std::os::unix::fs::MetadataExt as _;
    let mut stamp = Sha256::new();
    let paths: Vec<&PathBuf> = paths.iter().collect();
    let stats = crate::parallel::in_order(&paths, |path| std::fs::symlink_metadata(path));
    for (path, metadata) in paths.into_iter().zip(stats) {
        let name = path.as_os_str().as_encoded_bytes();
        stamp.update((name.len() as u64).to_le_bytes());
        stamp.update(name);
        let metadata = match metadata {
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
    /// Scope membership by ancestor sets is the original definition: inside
    /// a closure package directory, or outside every other package directory.
    #[test]
    fn scope_membership_matches_directory_prefixes() {
        use std::collections::BTreeSet;
        use std::path::Path;
        let data = serde_json::json!({
            "workspace_root": "/nonexistent-ws",
            "workspace_members": ["a"],
            "packages": [
                {"id": "a", "manifest_path": "/ws/a/Cargo.toml"},
                {"id": "b", "manifest_path": "/ws/a/nested/Cargo.toml"},
                {"id": "c", "manifest_path": "/reg/c-1.0/Cargo.toml"},
                {"manifest_path": "/reg/unknown/Cargo.toml"}
            ]
        });
        let closure: BTreeSet<String> = ["a".to_owned()].into();
        let scope = super::Scope::new(&data, &closure);
        let inside = ["/ws/a", "/reg/unknown"];
        let outside = ["/ws/a/nested", "/reg/c-1.0"];
        for path in [
            "/ws/a/src/lib.rs",
            "/ws/a/nested/src/lib.rs",
            "/reg/c-1.0/src/lib.rs",
            "/reg/c-1.0x/src/lib.rs",
            "/reg/unknown/x.rs",
            "/elsewhere/config.toml",
            "/ws/a",
            "/reg/c-1.0",
        ] {
            let path = Path::new(path);
            let original = inside.iter().any(|d| path.starts_with(d))
                || !outside.iter().any(|d| path.starts_with(d));
            assert_eq!(scope.contains(path), original, "{}", path.display());
        }
        assert!(scope.manifests.contains(Path::new("/ws/a/Cargo.toml")));
    }

    #[test]
    fn metadata_command_forwards_each_exact_cargo_feature_alias() {
        use std::ffi::OsStr;
        use std::path::Path;
        for arguments in [
            vec!["--features", "enabled"],
            vec!["--features=enabled"],
            vec!["-F", "enabled"],
            vec!["-Fenabled"],
            vec!["-F=enabled"],
        ] {
            let args = arguments
                .iter()
                .map(|v| (*v).to_owned())
                .collect::<Vec<_>>();
            // Inspect the actual constructor used by metadata(), not a second
            // imitation parser. The pinned Cargo parser owns alias semantics.
            let command =
                super::metadata_command(Path::new("workspace"), &args, Path::new("cargo"));
            let expected = ["metadata", "--format-version", "1"]
                .into_iter()
                .chain(arguments)
                .map(OsStr::new)
                .collect::<Vec<_>>();
            assert_eq!(command.get_args().collect::<Vec<_>>(), expected);
            assert_eq!(command.get_program(), OsStr::new("cargo"));
            assert_eq!(command.get_current_dir(), Some(Path::new("workspace")));
            // The original request remains byte-for-byte unchanged for its
            // independent proof/compiler context serialization.
            assert_eq!(
                args.iter().map(String::as_str).collect::<Vec<_>>(),
                expected
                    .iter()
                    .skip(3)
                    .map(|v| v.to_str().unwrap_or_default())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn metadata_command_retains_repeated_features_and_first_verifier_separator() {
        use std::ffi::OsStr;
        use std::path::Path;
        let args = [
            "--manifest-path",
            "member/Cargo.toml",
            "-F",
            "a",
            "--features=b",
            "-Fc,d",
            "-F=e",
            "--locked",
            "-pother",
            "--",
            "-Fignored",
            "--features",
            "ignored",
        ]
        .map(str::to_owned);
        let command = super::metadata_command(Path::new("workspace"), &args, Path::new("cargo"));
        let expected = [
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            "member/Cargo.toml",
            "-F",
            "a",
            "--features=b",
            "-Fc,d",
            "-F=e",
            "--locked",
        ]
        .map(OsStr::new);
        assert_eq!(command.get_args().collect::<Vec<_>>(), expected);
        let missing_value = ["-F".to_owned(), "--".to_owned(), "enabled".to_owned()];
        let command =
            super::metadata_command(Path::new("workspace"), &missing_value, Path::new("cargo"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["metadata", "--format-version", "1", "-F"].map(OsStr::new)
        );
    }

    #[test]
    fn disabled_script_explicit_selection_requires_unique_cargo_package_identity() {
        let data = serde_json::json!({"packages":[
            {"id":"root-id","name":"root","version":"1"},
            {"id":"disabled-id","name":"disabled","version":"2"}]});
        for args in [
            vec!["--package", "root"],
            vec!["-proot"],
            vec!["-p=root"],
            vec!["--package=root@1"],
            vec!["-p", "root-id"],
        ] {
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(
                super::selected_packages(&data, &args),
                Some(vec!["root-id".into()])
            );
        }
        assert_eq!(
            super::selected_packages(&data, &["--package=disabled".into()]),
            Some(vec!["disabled-id".into()])
        );
        for args in [
            vec!["--package"],
            vec!["--package="],
            vec!["-punknown"],
            vec!["--package=root@2"],
            vec!["--package-malformed"],
        ] {
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(super::selected_packages(&data, &args), None);
        }
        let mut ambiguous = data.clone();
        ambiguous["packages"][1]["name"] = "root".into();
        assert_eq!(
            super::selected_packages(&ambiguous, &["-p".into(), "root".into()]),
            None
        );
        assert_eq!(
            super::selected_packages(&data, &["--".into(), "-p".into(), "disabled".into()]),
            Some(Vec::<String>::new())
        );
    }

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
        // Every hashed file's digest is retained under its exact path.
        assert_eq!(snapshot.digests.keys().cloned().collect::<Vec<_>>(), files);
        assert!(std::fs::write(&odd, b"changed").is_ok());
        let refreshed = snapshot_metadata_home(&data, &[], None);
        assert!(refreshed.complete);
        assert_ne!(snapshot.revision, refreshed.revision);
        assert_ne!(snapshot.digests.get(&odd), refreshed.digests.get(&odd));
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

    /// [`collect`] as it was before it resolved only symbolic links: every
    /// entry canonicalized.
    fn collect_resolving_every_entry(
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
            complete &= collect_resolving_every_entry(&entry.path(), files, visited, roots);
        }
        complete
    }

    #[cfg(unix)]
    #[test]
    fn collect_finds_what_resolving_every_entry_finds() -> std::io::Result<()> {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("sail-collect-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("package");
        let outside = root.join("outside");
        std::fs::create_dir_all(package.join("src/nested/deeper"))?;
        std::fs::create_dir_all(package.join(".git"))?;
        std::fs::create_dir_all(outside.join("shared"))?;
        std::fs::write(package.join("Cargo.toml"), "[package]")?;
        std::fs::write(package.join("src/lib.rs"), "lib")?;
        std::fs::write(package.join("src/nested/deeper/a.rs"), "a")?;
        std::fs::write(package.join(".git/HEAD"), "ignored")?;
        std::fs::write(outside.join("shared/b.rs"), "b")?;
        std::fs::write(outside.join("c.rs"), "c")?;
        symlink(outside.join("shared"), package.join("src/linked-dir"))?;
        symlink(outside.join("c.rs"), package.join("src/linked-file.rs"))?;
        symlink(package.join("src"), package.join("src/nested/cycle"))?;
        symlink(root.join("missing"), package.join("src/dangling"))?;
        let mut found = (BTreeSet::new(), BTreeSet::new(), Vec::new());
        let mut expected = (BTreeSet::new(), BTreeSet::new(), Vec::new());
        let complete = collect(&package, &mut found.0, &mut found.1, &mut found.2);
        let reference = collect_resolving_every_entry(
            &package,
            &mut expected.0,
            &mut expected.1,
            &mut expected.2,
        );
        found.2.sort();
        expected.2.sort();
        assert_eq!((complete, &found), (reference, &expected));
        assert!(!complete, "a dangling link leaves the walk incomplete");
        assert_eq!(found.0.len(), 5, "{:?}", found.0);
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn closure_scope_drops_only_unselected_package_directories() {
        use std::collections::BTreeSet;
        use std::path::PathBuf;
        let root = std::env::temp_dir().join(format!("sail-scope-test-{}", std::process::id()));
        let ws = root.join("ws");
        for dir in [
            "ws/lib/src",
            "ws/bench/src",
            "registry/criterion",
            "registry/syn",
        ] {
            assert!(std::fs::create_dir_all(root.join(dir)).is_ok());
        }
        assert!(std::fs::write(ws.join("Cargo.toml"), "[workspace]").is_ok());
        assert!(std::fs::write(ws.join("Cargo.lock"), "").is_ok());
        let package = |id: &str, dir: &PathBuf| serde_json::json!({"id": id, "manifest_path": dir.join("Cargo.toml")});
        let data = serde_json::json!({
        "workspace_root": ws, "workspace_members": ["lib", "bench"],
        "packages": [
            package("lib", &ws.join("lib")), package("bench", &ws.join("bench")),
            package("syn", &root.join("registry/syn")),
            package("criterion", &root.join("registry/criterion")),
            {"manifest_path": root.join("registry/unknown/Cargo.toml")}
        ]});
        let snapshot = super::InputSnapshot {
            workspace: ws.clone(),
            revision: String::new(),
            roots: vec![
                ws.join("lib"),
                ws.join("bench"),
                root.join("registry/syn"),
                root.join("registry/criterion"),
                root.join("registry/unknown"),
                PathBuf::from("/opt/toolchain/lib/librustc.so"),
            ],
            excluded: vec![ws.join("target")],
            complete: true,
            observation_paths: BTreeSet::new(),
            digests: std::collections::BTreeMap::new(),
        };
        let unchanged = super::closure_scope(&snapshot, &data, None);
        assert_eq!(unchanged.roots, snapshot.roots);
        let closure = BTreeSet::from(["lib".to_owned(), "syn".to_owned()]);
        let scoped = super::closure_scope(&snapshot, &data, Some(&closure));
        let mut expected = vec![
            ws.join("lib"),
            root.join("registry/syn"),
            root.join("registry/unknown"),
            PathBuf::from("/opt/toolchain/lib/librustc.so"),
            ws.join("lib/Cargo.toml"),
            ws.join("bench/Cargo.toml"),
            ws.join("Cargo.toml"),
            ws.join("Cargo.lock"),
        ];
        expected.sort();
        assert_eq!(scoped.roots, expected);
        assert_eq!(scoped.excluded, snapshot.excluded);
        // A selected package nested inside an unselected one stays covered.
        let nested = serde_json::json!({"workspace_root": ws, "packages": [
            package("outer", &ws), package("lib", &ws.join("lib"))]});
        let snapshot = super::InputSnapshot {
            roots: vec![ws.clone(), ws.join("lib")],
            ..snapshot
        };
        let only_lib = BTreeSet::from(["lib".to_owned()]);
        let scoped = super::closure_scope(&snapshot, &nested, Some(&only_lib));
        assert!(scoped.roots.contains(&ws.join("lib")));
        assert!(!scoped.roots.contains(&ws));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
