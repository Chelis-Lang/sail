//! Cargo metadata and declared input revision tracking.

use super::files::digest_all;
use crate::{Error, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
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

pub(super) fn metadata(
    workspace: &Path,
    args: &[String],
    cargo: &Path,
    toolchain: &super::CargoToolchain,
) -> Result<Value> {
    let mut command = metadata_command(workspace, args, cargo);
    toolchain.apply(&mut command);
    read_metadata(command, args)
}

fn managed_metadata_command(
    workspace: &Path,
    args: &[String],
    cargo: &Path,
    toolchain: &super::CargoToolchain,
    cache: &Path,
) -> Result<Command> {
    let mut command = metadata_command(workspace, args, cargo);
    toolchain.apply(&mut command);
    // Metadata does not build anything. Keep its excluded output directory
    // independent of the caller's target override: managed compilation uses
    // an explicit owned target, even when the caller names a source directory.
    crate::adapter::configure_metadata_target(
        &mut command,
        &crate::adapter::metadata_target(cache)?,
    )?;
    Ok(command)
}

pub(super) fn managed_metadata(
    workspace: &Path,
    args: &[String],
    cargo: &Path,
    toolchain: &super::CargoToolchain,
    cache: &Path,
) -> Result<Value> {
    read_metadata(
        managed_metadata_command(workspace, args, cargo, toolchain, cache)?,
        args,
    )
}

fn read_metadata(mut command: Command, args: &[String]) -> Result<Value> {
    let output = command
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
    let mut whole_workspace = false;
    let mut arguments = args.iter().take_while(|arg| arg.as_str() != "--");
    while let Some(argument) = arguments.next() {
        whole_workspace |= argument == "--workspace" || argument == "--all";
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
    if selected.is_empty() && !whole_workspace {
        // Without a package selection Cargo builds the workspace's default
        // members, which Cargo resolves from the invoking directory: inside a
        // member's directory that is the member alone. Selecting them here
        // keeps scripts that only other members reach out of required
        // coverage, as `-p` does (issue #157). Cargo metadata from the same
        // directory reports the same set; a unit outside it refuses Current.
        // Without either field the selection stays empty: every package is
        // then a candidate, as before.
        let defaults = data["workspace_default_members"].as_array().map_or_else(
            || data["resolve"]["root"].as_str().map(|root| vec![root]),
            |members| members.iter().map(Value::as_str).collect(),
        );
        selected = defaults
            .unwrap_or_default()
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
    Some(selected)
}

/// Paths seen during one walk. A set compared by components, like the
/// sorted sets it feeds, but hashed: a walk tests every entry once, and
/// ordered inserts compared paths component by component (about a third of
/// a vest snapshot's time).
type Seen = HashSet<PathBuf>;

fn collect(
    path: &Path,
    files: &mut Vec<PathBuf>,
    visited: &mut Seen,
    roots: &mut Vec<PathBuf>,
) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    // A link is a root even when its target was already collected, so the
    // snapshot does not depend on the order of its inputs.
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        roots.push(canonical.clone());
        roots.push(path.to_owned());
    }
    if !visited.insert(canonical.clone()) {
        return true;
    }
    if canonical.is_file() {
        files.push(canonical);
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
    files: &mut Vec<PathBuf>,
    visited: &mut Seen,
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
                    files.push(path);
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

/// The ordered set of `paths`, exactly as inserting each into a
/// `BTreeSet<PathBuf>` builds it. Sorting by a byte key that orders as
/// `Path`'s component comparison does replaces thousands of component-wise
/// comparisons with byte comparisons; equal keys are equal paths.
pub(super) fn path_set(paths: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    let mut keyed: Vec<(Vec<u8>, PathBuf)> = Vec::with_capacity(paths.len());
    let mut paths = paths.into_iter();
    while let Some(path) = paths.next() {
        let Some(key) = order_key(&path) else {
            return keyed
                .into_iter()
                .map(|(_, path)| path)
                .chain(std::iter::once(path))
                .chain(paths)
                .fold(BTreeSet::new(), |mut set, path| {
                    set.insert(path);
                    set
                });
        };
        keyed.push((key, path));
    }
    // Path equality ignores some spelling differences (`a//b`, `a/b/`).
    // Keep the first raw spelling, as repeated BTreeSet::insert did: the
    // revision hashes its original OsStr bytes, not normalized components.
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.dedup_by(|a, b| a.0 == b.0);
    // Already sorted and unique: the set is built without comparing again
    // beyond one check per element.
    keyed.into_iter().map(|(_, path)| path).collect()
}

/// A byte string whose order is `Path`'s order: the components in turn, each
/// as its kind (in `Component`'s declaration order) and bytes, closed by a
/// zero byte, which no path component contains. `None` for a Windows prefix,
/// whose order is not byte order; the caller then compares paths directly.
fn order_key(path: &Path) -> Option<Vec<u8>> {
    use std::path::Component;
    let mut key = Vec::with_capacity(path.as_os_str().len() + 16);
    for component in path.components() {
        let kind = match component {
            Component::Prefix(_) => return None,
            Component::RootDir => 1,
            Component::CurDir => 2,
            Component::ParentDir => 3,
            Component::Normal(_) => 4,
        };
        key.push(kind);
        key.extend_from_slice(component.as_os_str().as_encoded_bytes());
        key.push(0);
    }
    Some(key)
}

pub(super) fn snapshot_metadata(data: &Value, extras: &[PathBuf]) -> InputSnapshot {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    snapshot_metadata_home(data, extras, home.as_deref())
}

/// What one invocation's input snapshot selected: the metadata the snapshot
/// reads and the extra inputs. The next run of the same invocation takes its
/// snapshot from it while Cargo reads the metadata, and keeps that snapshot
/// only if its own selection is equal. It never authorizes anything.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct SnapshotHint {
    pub version: u32,
    pub projection: Value,
    pub extras: Vec<PathBuf>,
}

pub(super) const SNAPSHOT_HINT_VERSION: u32 = 1;

/// Where the snapshot hint of an invocation (directory and arguments) lives.
pub(super) fn hint_path(cache: &Path, invocation: &Path, args: &[String]) -> PathBuf {
    let key = super::files::digest(&serde_json::to_vec(&(invocation, args)).unwrap_or_default());
    cache.join("snapshot-hints").join(format!("{key}.json"))
}

/// The metadata fields [`snapshot_metadata_home`] reads, besides each
/// package's `manifest_path`.
pub(super) const SNAPSHOT_FIELDS: [&str; 5] = [
    "workspace_root",
    "sail_invocation_directory",
    "sail_cache_directory",
    "target_directory",
    "sail_source_inputs",
];

pub(super) fn snapshot_metadata_home(
    data: &Value,
    extras: &[PathBuf],
    cargo_home: Option<&Path>,
) -> InputSnapshot {
    stamped_snapshot(data, extras, cargo_home).0
}

/// [`snapshot_metadata`], with the stamp of its observed paths taken around
/// its content reads, for [`unchanged_since`].
pub(super) fn snapshot_metadata_stamped(
    data: &Value,
    extras: &[PathBuf],
) -> (InputSnapshot, Option<String>) {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    stamped_snapshot(data, extras, home.as_deref())
}

/// Whether a complete snapshot taken earlier is still the snapshot its
/// selection would read now: every observed file and directory still has
/// the stamp it had around the snapshot's content reads.
pub(super) fn unchanged_since(snapshot: &InputSnapshot, stamp: Option<&String>) -> bool {
    snapshot.complete
        && stamp.is_some()
        && stamp_paths(&snapshot.observation_paths).as_ref() == stamp
}

fn stamped_snapshot(
    data: &Value,
    extras: &[PathBuf],
    cargo_home: Option<&Path>,
) -> (InputSnapshot, Option<String>) {
    let workspace = PathBuf::from(data["workspace_root"].as_str().unwrap_or_default());
    let invocation = Path::new(
        data["sail_invocation_directory"]
            .as_str()
            .unwrap_or_else(|| workspace.to_str().unwrap_or_default()),
    );
    let mut files = Vec::new();
    let mut visited = Seen::new();
    let mut excluded: Vec<PathBuf> = ["sail_cache_directory", "target_directory"]
        .into_iter()
        .filter_map(|name| data[name].as_str().map(|p| invocation.join(p)))
        .collect();
    if let Ok(path) = crate::cache_dir() {
        excluded.push(invocation.join(path));
    }
    let excluded_canonical: HashSet<_> = excluded
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
            files.push(path);
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
    let observation_paths = path_set(observation_paths);
    let files = path_set(files);
    let stamp_before = stamp_paths(&observation_paths);
    let (revision, readable, digests) = contents_revision(&files);
    complete &= readable;
    complete &= stamp_before.is_some() && stamp_before == stamp_paths(&observation_paths);
    roots.sort();
    roots.dedup();
    (
        InputSnapshot {
            workspace,
            revision,
            roots,
            excluded,
            complete,
            observation_paths,
            digests,
        },
        stamp_before,
    )
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
    let mut recorded = Vec::with_capacity(files.len());
    let mut hasher = Sha256::new();
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    for (path, digest) in files.iter().zip(digest_all(&paths)) {
        let bytes = path.as_os_str().as_encoded_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        if let Ok(hash) = digest {
            hasher.update(hash.as_bytes());
            recorded.push((path.clone(), hash));
        } else {
            complete = false;
            hasher.update(b"unreadable");
        }
    }
    // Already in the set's order: the map is built without ordered inserts.
    (
        format!("{:x}", hasher.finalize()),
        complete,
        recorded.into_iter().collect(),
    )
}

// A run-local mutation stamp is separate from logical proof keys: reverting an
// edit between completed runs may reuse a proof, reverting during a run may not.
pub(super) fn mutation_stamp(snapshot: &InputSnapshot) -> Option<String> {
    let mut paths = snapshot.observation_paths.clone();
    // Package roots share most ancestors (the registry source directory, the
    // home directory): each ancestor is examined once.
    let mut examined = HashSet::new();
    for path in &snapshot.roots {
        paths.insert(path.clone());
        if let Ok(canonical) = path.canonicalize() {
            paths.insert(canonical);
        }
        for alias in path.ancestors() {
            if examined.insert(alias)
                && std::fs::symlink_metadata(alias).is_ok_and(|m| m.file_type().is_symlink())
            {
                paths.insert(alias.to_owned());
            }
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

/// Hashes Cargo's declared workspace and resolved dependency inputs, reading
/// Cargo's metadata with `cargo` under `toolchain`.
///
/// # Errors
/// Returns `SAIL3103` when Cargo metadata cannot be obtained.
pub fn snapshot(
    workspace: &Path,
    args: &[String],
    cargo: &Path,
    toolchain: &super::CargoToolchain,
) -> Result<InputSnapshot> {
    let invocation =
        std::path::absolute(workspace).map_err(|e| Error::new("SAIL3103", e.to_string()))?;
    let mut data = metadata(workspace, args, cargo, toolchain)?;
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
    #[test]
    fn only_managed_metadata_ignores_the_callers_target_override() {
        let workspace = std::path::Path::new("workspace");
        let cargo = std::path::Path::new("cargo");
        let cache = std::path::Path::new("owned-cache");
        let caller = crate::managed::CargoToolchain::Caller;
        let managed = super::managed_metadata_command(workspace, &[], cargo, &caller, cache)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let expected =
            crate::adapter::metadata_target(cache).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(
            managed.get_envs().any(
                |(key, value)| key == "CARGO_TARGET_DIR" && value == Some(expected.as_os_str())
            )
        );
        let ordinary = super::metadata_command(workspace, &[], cargo);
        assert!(
            !ordinary
                .get_envs()
                .any(|(key, _)| key == "CARGO_TARGET_DIR")
        );
    }

    #[test]
    fn managed_metadata_runs_with_the_selected_toolchain() {
        let toolchain = |selection: &crate::managed::CargoToolchain| {
            let command = super::managed_metadata_command(
                std::path::Path::new("workspace"),
                &[],
                std::path::Path::new("cargo"),
                selection,
                std::path::Path::new("owned-cache"),
            )
            .unwrap_or_else(|error| unreachable!("{error}"));
            command
                .get_envs()
                .find(|(key, _)| *key == "RUSTUP_TOOLCHAIN")
                .and_then(|(_, value)| value.map(std::ffi::OsStr::to_owned))
        };
        assert_eq!(
            toolchain(&crate::managed::CargoToolchain::Pinned("1.99.1".into())),
            Some("1.99.1".into())
        );
        assert_eq!(toolchain(&crate::managed::CargoToolchain::Caller), None);
    }

    /// The keyed sort builds exactly the set that ordered inserts build,
    /// including paths whose byte order differs from their component order
    /// and spellings that name the same path.
    #[test]
    fn path_set_is_the_ordered_set_of_its_paths() {
        use std::collections::BTreeSet;
        use std::path::PathBuf;
        let spellings = [
            "/a/b",
            "/a-b",
            "/a/b/c",
            "/a/b-c",
            "/a/b.c",
            "/a/b ",
            "/a/..",
            "/a/-x",
            "/a/.x",
            "/a//b",
            "/a/./b",
            "/a/b/",
            "/a",
            "/",
            "a/b",
            "./a/b",
            "../a",
            "a",
            "",
            "/z/\u{e9}",
            "/z/e",
            "/z/E",
        ];
        // Repetition also exercises duplicate partitioning in an unstable sort.
        let paths: Vec<PathBuf> = spellings
            .iter()
            .cycle()
            .take(spellings.len() * 3)
            .map(PathBuf::from)
            .collect();
        let raw = |set: &BTreeSet<PathBuf>| {
            set.iter()
                .map(|path| path.as_os_str().as_encoded_bytes().to_vec())
                .collect::<Vec<_>>()
        };
        for paths in [paths.clone(), paths.into_iter().rev().collect()] {
            // FromIterator may choose another equal representative; the
            // pre-optimization implementation inserted each path in order.
            let mut expected = BTreeSet::new();
            for path in &paths {
                expected.insert(path.clone());
            }
            let built = super::path_set(paths);
            assert_eq!(raw(&built), raw(&expected));
        }
    }

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

    /// Without `-p`, Cargo builds the default members it resolved from the
    /// invoking directory; the selection is the same set (issue #157).
    #[test]
    fn selection_without_package_is_cargos_default_members() {
        let data = serde_json::json!({
            "packages":[{"id":"a-id","name":"a","version":"1"},
                        {"id":"b-id","name":"b","version":"1"}],
            "workspace_members":["a-id","b-id"],
            "workspace_default_members":["a-id"],
            "resolve":{"root":"a-id","nodes":[]}});
        assert_eq!(
            super::selected_packages(&data, &[]),
            Some(vec!["a-id".to_owned()])
        );
        // Arguments after `--` belong to Verus.
        assert_eq!(
            super::selected_packages(&data, &["--".into(), "-p".into(), "b".into()]),
            Some(vec!["a-id".to_owned()])
        );
        // An explicit selection replaces the default.
        assert_eq!(
            super::selected_packages(&data, &["-p".into(), "b".into()]),
            Some(vec!["b-id".to_owned()])
        );
        // --workspace selects every member: no narrower closure.
        for flag in ["--workspace", "--all"] {
            assert_eq!(
                super::selected_packages(&data, &[flag.to_owned()]),
                Some(Vec::<String>::new())
            );
        }
        // A virtual workspace root: every default member, no resolve root.
        let mut root = data.clone();
        root["workspace_default_members"] = serde_json::json!(["a-id", "b-id"]);
        root["resolve"]["root"] = Value::Null;
        assert_eq!(
            super::selected_packages(&root, &[]),
            Some(vec!["a-id".to_owned(), "b-id".to_owned()])
        );
        // Older Cargo without default members: the resolve root.
        let mut older = data.clone();
        older
            .as_object_mut()
            .map(|o| o.remove("workspace_default_members"));
        assert_eq!(
            super::selected_packages(&older, &[]),
            Some(vec!["a-id".to_owned()])
        );
        older["resolve"]["root"] = Value::Null;
        assert_eq!(
            super::selected_packages(&older, &[]),
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
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            roots.push(canonical.clone());
            roots.push(path.to_owned());
        }
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
        let mut walked = (Vec::new(), Seen::new(), Vec::new());
        let mut expected = (BTreeSet::new(), BTreeSet::new(), Vec::new());
        let complete = collect(&package, &mut walked.0, &mut walked.1, &mut walked.2);
        let mut found = (
            path_set(walked.0),
            walked.1.into_iter().collect::<BTreeSet<_>>(),
            walked.2,
        );
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
        // A link back into collected content is still a root of its own.
        assert!(found.2.contains(&package.join("src/nested/cycle")));
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

    #[test]
    fn the_order_of_extra_inputs_does_not_change_a_snapshot() {
        // The runner sorts build-script paths so an unchanged invocation keeps
        // its snapshot hint; that is only sound if the order never mattered.
        let root = std::env::temp_dir().join(format!("sail-extras-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ws = root.join("ws");
        for (path, text) in [
            ("ws/Cargo.toml", "[package]\nname = \"ws\"\n"),
            ("ws/src/lib.rs", ""),
            ("one/build.rs", "fn main() {}"),
            ("one/data/x", "x"),
            ("two/build.rs", "fn main() {}"),
        ] {
            let file = root.join(path);
            assert!(
                file.parent()
                    .is_some_and(|d| std::fs::create_dir_all(d).is_ok())
            );
            assert!(std::fs::write(&file, text).is_ok());
        }
        // Alias one input through a link, so first-seen spellings would show.
        #[cfg(unix)]
        assert!(std::os::unix::fs::symlink(root.join("one"), root.join("alias")).is_ok());
        let data = serde_json::json!({
            "workspace_root": ws,
            "packages": [{"manifest_path": ws.join("Cargo.toml")}],
        });
        let extras = [
            root.join("two/build.rs"),
            root.join("alias"),
            root.join("one/build.rs"),
            root.join("one"),
        ];
        let reference = super::snapshot_metadata_home(&data, &extras, None);
        assert!(reference.complete);
        let mut reversed = extras.to_vec();
        reversed.reverse();
        let mut sorted = extras.to_vec();
        sorted.sort();
        for order in [reversed, sorted] {
            let snapshot = super::snapshot_metadata_home(&data, &order, None);
            assert_eq!(snapshot.revision, reference.revision);
            assert_eq!(snapshot.digests, reference.digests);
            assert_eq!(snapshot.roots, reference.roots);
            assert_eq!(snapshot.observation_paths, reference.observation_paths);
            assert_eq!(snapshot.complete, reference.complete);
        }
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
