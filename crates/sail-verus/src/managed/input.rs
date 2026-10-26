//! Cargo metadata and declared input revision tracking.

use super::files::{digest_all_tagged, read_outcome_keys_match};
use crate::{Error, Result};
use sail_acceptance::inputs::{
    InputFact, InputValue, OriginalCapture, OriginalGuard, compare_readback, reads_complete,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

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
    /// Source path aliases and their canonical destinations, including linked ancestors.
    pub(super) bindings: BTreeMap<PathBuf, PathBuf>,
    /// Original capture provenance; projections share its entire observed namespace.
    pub(super) capture: Option<SnapshotGuard>,
}

#[derive(Debug)]
pub(super) struct SnapshotGuard {
    guard: OriginalGuard,
    paths: Arc<BTreeSet<PathBuf>>,
}

impl Clone for SnapshotGuard {
    fn clone(&self) -> Self {
        Self {
            guard: self.guard.duplicate(),
            paths: Arc::clone(&self.paths),
        }
    }
}

impl SnapshotGuard {
    fn finish(
        original: OriginalCapture,
        after: Option<&str>,
        revision: &str,
        paths: &BTreeSet<PathBuf>,
    ) -> Option<Self> {
        original
            .finish(after.map(str::as_bytes), revision.as_bytes())
            .map(|guard| Self {
                guard,
                paths: Arc::new(paths.clone()),
            })
    }
}

/// Native ordering proposals confer no authority: the source-shared core checks
/// the complete forward/inverse permutation and every adjacent key.
pub(super) fn proposed_order(facts: &[InputFact<'_>]) -> (Vec<usize>, Vec<usize>) {
    crate::parallel::proposed_fact_order(facts)
}

fn snapshot_facts<'a>(
    digests: &'a BTreeMap<PathBuf, String>,
    bindings: &'a BTreeMap<PathBuf, PathBuf>,
) -> Vec<InputFact<'a>> {
    digests
        .iter()
        .map(|(path, hash)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(hash.as_bytes()),
        })
        .chain(bindings.iter().map(|(alias, target)| InputFact {
            namespace: 1,
            name: alias.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(target.as_os_str().as_encoded_bytes()),
        }))
        .collect()
}

fn bindings_current(bindings: &BTreeMap<PathBuf, PathBuf>) -> bool {
    let aliases: Vec<_> = bindings.keys().map(PathBuf::as_path).collect();
    let reads = crate::parallel::in_order(&aliases, |&alias| (alias, alias.canonicalize()));
    bindings_readback(bindings, &reads)
}

fn bindings_readback(
    bindings: &BTreeMap<PathBuf, PathBuf>,
    reads: &[(&Path, std::io::Result<PathBuf>)],
) -> bool {
    if bindings.is_empty() && reads.is_empty() {
        return true;
    }
    let expected: Vec<_> = bindings
        .iter()
        .map(|(alias, target)| InputFact {
            namespace: 1,
            name: alias.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(target.as_os_str().as_encoded_bytes()),
        })
        .collect();
    let observed: Vec<_> = reads
        .iter()
        .map(|(alias, read)| InputFact {
            namespace: 1,
            name: alias.as_os_str().as_encoded_bytes(),
            value: match read {
                Ok(target) => InputValue::Present(target.as_os_str().as_encoded_bytes()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => InputValue::Absent,
                Err(_) => InputValue::Unreadable,
            },
        })
        .collect();
    let (before_order, before_inverse) = proposed_order(&expected);
    let (after_order, after_inverse) = proposed_order(&observed);
    compare_readback(
        &expected,
        &observed,
        &before_order,
        &before_inverse,
        &after_order,
        &after_inverse,
    )
    .is_equal()
}

fn project_guard(
    observed: &InputSnapshot,
    revision: &str,
    digests: &BTreeMap<PathBuf, String>,
    bindings: &BTreeMap<PathBuf, PathBuf>,
) -> Option<SnapshotGuard> {
    let selected = snapshot_facts(digests, bindings);
    project_guard_facts(observed, revision, &selected)
}

fn project_guard_facts(
    observed: &InputSnapshot,
    revision: &str,
    selected: &[InputFact<'_>],
) -> Option<SnapshotGuard> {
    let capture = observed.capture.as_ref()?;
    let source = snapshot_facts(&observed.digests, &observed.bindings);
    let (source_order, source_inverse) = proposed_order(&source);
    let (selected_order, selected_inverse) = proposed_order(selected);
    let guard = capture.guard.project(
        observed.revision.as_bytes(),
        revision.as_bytes(),
        &source,
        selected,
        &source_order,
        &source_inverse,
        &selected_order,
        &selected_inverse,
    )?;
    Some(SnapshotGuard {
        guard,
        paths: Arc::clone(&capture.paths),
    })
}

#[derive(Default)]
struct Bindings {
    paths: BTreeMap<PathBuf, PathBuf>,
    examined: HashSet<PathBuf>,
}

impl Bindings {
    fn observe(&mut self, path: &Path) -> bool {
        let mut complete = true;
        for ancestor in path.ancestors() {
            if !self.examined.insert(ancestor.to_owned()) {
                continue;
            }
            match std::fs::symlink_metadata(ancestor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    if let Ok(target) = ancestor.canonicalize() {
                        self.paths.insert(ancestor.to_owned(), target);
                    } else {
                        complete = false;
                    }
                }
                Ok(_) => {}
                Err(_) => complete = false,
            }
        }
        complete
    }
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
    bindings: &mut Bindings,
) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    let complete = bindings.observe(path);
    // A link is a root even when its target was already collected, so the
    // snapshot does not depend on the order of its inputs.
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        roots.push(canonical.clone());
        roots.push(path.to_owned());
    }
    if !visited.insert(canonical.clone()) {
        return complete;
    }
    if canonical.is_file() {
        files.push(canonical);
        return complete;
    }
    collect_directory(&canonical, files, visited, roots, bindings) && complete
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
    bindings: &mut Bindings,
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
                    complete &= collect_directory(&path, files, visited, roots, bindings);
                }
            }
            _ => complete &= collect(&path, files, visited, roots, bindings),
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
        && snapshot.capture.as_ref().is_some_and(|capture| {
            let now = stamp_paths(&capture.paths);
            capture.guard.retained(
                snapshot.revision.as_bytes(),
                stamp.map(String::as_bytes),
                now.as_deref().map(str::as_bytes),
            )
        })
}

/// A selected revision from this run's complete source observation. Every
/// selected path and directory must belong to that observation. The original
/// content-read stamp is checked after selection; any mutation or uncertainty
/// takes a new complete snapshot instead. Compiler identities are not memoized.
#[cfg(test)]
pub(super) fn snapshot_metadata_observed(
    data: &Value,
    extras: &[PathBuf],
    observed: &InputSnapshot,
    stamp: Option<&String>,
) -> InputSnapshot {
    if observed.complete
        && stamp.is_some()
        && let Some(snapshot) = snapshot_metadata_projected(data, extras, observed)
        && unchanged_since(observed, stamp)
    {
        return snapshot;
    }
    snapshot_metadata(data, extras)
}

/// Reobserves this invocation's complete selection without a second content
/// read when its entire namespace and original mutation stamp still agree.
/// Preserve the original stamp; never bind old bytes to a newly taken stamp.
pub(super) fn snapshot_metadata_reobserved(
    data: &Value,
    extras: &[PathBuf],
    observed: &InputSnapshot,
    stamp: Option<&String>,
) -> (InputSnapshot, Option<String>) {
    if observed.complete
        && stamp.is_some()
        && let Some(snapshot) = snapshot_metadata_projected(data, extras, observed)
        && snapshot.observation_paths == observed.observation_paths
        && unchanged_since(observed, stamp)
    {
        return (snapshot, stamp.cloned());
    }
    snapshot_metadata_stamped(data, extras)
}

/// Projects the same selected bytes without re-reading them. The caller must
/// check the observation's original stamp after the entire projection batch;
/// an unchecked projection never authorizes a receipt.
pub(super) fn snapshot_metadata_projected(
    data: &Value,
    extras: &[PathBuf],
    observed: &InputSnapshot,
) -> Option<InputSnapshot> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    projected_snapshot(data, extras, home.as_deref(), observed)
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
    let mut bindings = Bindings::default();
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
            complete &= collect(parent, &mut files, &mut visited, &mut roots, &mut bindings);
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

    for path in configuration_paths(invocation, cargo_home) {
        if path.is_file() {
            complete &= bindings.observe(&path);
            files.push(path);
        }
    }
    let source_inputs: Vec<PathBuf> = data["sail_source_inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    roots.extend(source_inputs.iter().cloned());
    for path in extras.iter().chain(&source_inputs) {
        complete &= collect(path, &mut files, &mut visited, &mut roots, &mut bindings);
    }
    let mut observation_paths = files.clone();
    observation_paths.extend(bindings.paths.keys().cloned());
    observation_paths.extend(
        visited
            .into_iter()
            .filter(|p| !excluded_canonical.contains(p)),
    );
    let observation_paths = path_set(observation_paths);
    let files = path_set(files);
    let stamp_before = stamp_paths(&observation_paths);
    let original = OriginalCapture::begin(stamp_before.as_deref().map(str::as_bytes));
    let (revision, readable, digests) = contents_revision(&files, &bindings.paths);
    complete &= readable;
    let stamp_after = stamp_paths(&observation_paths);
    let capture = SnapshotGuard::finish(
        original,
        stamp_after.as_deref(),
        &revision,
        &observation_paths,
    );
    complete &= capture.is_some();
    complete &= bindings_current(&bindings.paths);
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
            bindings: bindings.paths,
            capture,
        },
        stamp_before,
    )
}

/// The selected namespace, validated against one complete source observation.
struct Projection {
    workspace: PathBuf,
    roots: Vec<PathBuf>,
    files: BTreeSet<PathBuf>,
    bindings: BTreeMap<PathBuf, PathBuf>,
    ranges: Vec<PathBuf>,
}

impl Projection {
    /// Borrow the exact selected digests in order; no input is reread or omitted.
    fn reads<'a>(
        &'a self,
        observed: &'a InputSnapshot,
    ) -> impl Iterator<Item = (&'a Path, Result<&'a str>)> {
        let mut available = observed
            .digests
            .range(self.files.first().cloned().unwrap_or_default()..)
            .peekable();
        self.files.iter().map(move |path| {
            while available.peek().is_some_and(|(key, _)| *key < path) {
                available.next();
            }
            match available.peek().filter(|(key, _)| *key == path) {
                Some((key, digest)) => (key.as_path(), Ok(digest.as_str())),
                None => (
                    path.as_path(),
                    Err(Error::new("SAIL3101", "missing observed input")),
                ),
            }
        })
    }
}

/// Selects a subset of a complete walk without walking those directories
/// again. The caller must check the original walk's mutation stamp after
/// selection. Uncovered roots and aliases retain the ordinary fresh walk.
fn projected_snapshot(
    data: &Value,
    extras: &[PathBuf],
    cargo_home: Option<&Path>,
    observed: &InputSnapshot,
) -> Option<InputSnapshot> {
    let projection = projected_selection(data, extras, cargo_home, observed)?;
    let mut observation_paths: BTreeSet<PathBuf> = projection
        .ranges
        .iter()
        .flat_map(|root| {
            observed
                .observation_paths
                .range(root.clone()..)
                .take_while(move |path| path.starts_with(root))
                .cloned()
        })
        .collect();
    // Merge ordered sets while keeping the observation's original spellings.
    observation_paths = observation_paths
        .union(&projection.files)
        .cloned()
        .collect();
    observation_paths.extend(projection.bindings.keys().cloned());
    let (revision, complete, digests) = fold_contents(
        &projection.files,
        &projection.bindings,
        projection.reads(observed),
    );
    let capture = project_guard(observed, &revision, &digests, &projection.bindings)?;
    Some(InputSnapshot {
        workspace: projection.workspace,
        revision,
        roots: projection.roots,
        excluded: observed.excluded.clone(),
        complete,
        observation_paths,
        digests,
        bindings: projection.bindings,
        capture: Some(capture),
    })
}

/// Only the logical revision of the same validated selection. The caller
/// still checks the original whole-observation stamp after its batch; no
/// incomplete snapshot or new proof authority is manufactured here.
pub(super) fn snapshot_revision_projected(
    data: &Value,
    extras: &[PathBuf],
    observed: &InputSnapshot,
) -> Option<(String, bool)> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
    let projection = projected_selection(data, extras, home.as_deref(), observed)?;
    let (revision, complete) = fold_revision(
        &projection.files,
        &projection.bindings,
        projection.reads(observed),
        |_, _| {},
    );
    let selected: Vec<_> = projection
        .reads(observed)
        .map(|(path, read)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: read.map_or(InputValue::Unreadable, |hash| {
                InputValue::Present(hash.as_bytes())
            }),
        })
        .chain(projection.bindings.iter().map(|(alias, target)| InputFact {
            namespace: 1,
            name: alias.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(target.as_os_str().as_encoded_bytes()),
        }))
        .collect();
    project_guard_facts(observed, &revision, &selected)?;
    Some((revision, complete))
}

#[allow(
    clippy::too_many_lines,
    reason = "The selected namespace and aliases are validated together."
)]
fn projected_selection(
    data: &Value,
    extras: &[PathBuf],
    cargo_home: Option<&Path>,
    observed: &InputSnapshot,
) -> Option<Projection> {
    if !observed.complete {
        return None;
    }
    let workspace = PathBuf::from(data["workspace_root"].as_str()?);
    let invocation = Path::new(
        data["sail_invocation_directory"]
            .as_str()
            .unwrap_or(workspace.to_str()?),
    );
    let mut roots: Vec<PathBuf> = data["packages"]
        .as_array()?
        .iter()
        .map(|package| {
            Some(
                Path::new(package["manifest_path"].as_str()?)
                    .parent()?
                    .to_owned(),
            )
        })
        .collect::<Option<_>>()?;
    let source_inputs: Vec<PathBuf> = data["sail_source_inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|path| path.as_str().map(PathBuf::from))
        .collect::<Option<_>>()?;
    let mut selected = HashSet::new();
    for root in &roots {
        if root.canonicalize().ok()? != *root || !observed.observation_paths.contains(root) {
            return None;
        }
        selected.insert(root.clone());
    }
    for root in source_inputs.iter().chain(extras) {
        if !observed.observation_paths.contains(root) {
            // A declared directory can be reached through an ancestor alias
            // (Homebrew's opt/openssl/include). The fresh collector walks its
            // canonical directory too. Keep direct links on the strict alias
            // path below, and refuse any previously unobserved destination.
            if std::fs::symlink_metadata(root)
                .ok()?
                .file_type()
                .is_symlink()
            {
                return None;
            }
            let canonical = root.canonicalize().ok()?;
            if !observed.observation_paths.contains(&canonical) {
                return None;
            }
            selected.insert(canonical);
            continue;
        }
        // Compiler-loaded files already inside a selected canonical package
        // need no second realpath. Their fresh bytes and namespace are in
        // the observation, whose original stamp is checked by the caller.
        if !root.ancestors().any(|parent| selected.contains(parent)) {
            if root.canonicalize().ok()? != *root {
                return None;
            }
            selected.insert(root.clone());
        }
    }
    roots.extend(source_inputs);
    let contains = |path: &Path| path.ancestors().any(|parent| selected.contains(parent));
    // Disjoint canonical roots select contiguous ranges in Path's component
    // order. Avoid a per-file scan of all package roots or ancestor hashes.
    let mut ranges: Vec<PathBuf> = selected
        .iter()
        .filter(|root| {
            !root
                .ancestors()
                .skip(1)
                .any(|parent| selected.contains(parent))
        })
        .cloned()
        .collect();
    ranges.sort_unstable();
    let mut files: BTreeSet<PathBuf> = ranges
        .iter()
        .flat_map(|root| {
            observed
                .digests
                .range(root.clone()..)
                .take_while(move |(path, _)| path.starts_with(root))
                .map(|(path, _)| path.clone())
        })
        .collect();
    let explicit: BTreeSet<PathBuf> = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
    ]
    .map(|name| workspace.join(name))
    .into_iter()
    .chain(configuration_paths(invocation, cargo_home))
    .filter(|path| path.is_file())
    .collect();
    // Collector aliases are roots outside its canonical namespace. Explicit
    // manifests/config files may instead be raw paths in that namespace, so
    // inspect those too. Absent watch-only configuration names carry no bytes.
    let candidates = observed
        .bindings
        .keys()
        .chain(
            observed
                .roots
                .iter()
                .filter(|root| !observed.observation_paths.contains(*root)),
        )
        .chain(&explicit);
    for alias in candidates {
        let metadata = match std::fs::symlink_metadata(alias) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        if !metadata.file_type().is_symlink() || !(contains(alias) || explicit.contains(alias)) {
            continue;
        }
        let target = alias.canonicalize().ok()?;
        // An internal link adds no files beyond the selected complete walk.
        // Its parent directory binds any replacement of that link. External
        // aliases, and links without that parent observation, use fresh reads.
        if !contains(&target) || !observed.observation_paths.contains(alias.parent()?) {
            return None;
        }
        if contains(alias) {
            roots.extend([alias.clone(), target]);
        }
    }
    // Select exactly the aliases the fresh collector would observe. Include
    // linked ancestors of declared inputs and internal links in selected
    // package walks; foreign packages' links do not bind this revision.
    let mut selected_bindings = Bindings::default();
    for path in roots.iter().chain(extras).chain(&explicit) {
        if !selected_bindings.observe(path) {
            return None;
        }
    }
    for alias in observed.bindings.keys().filter(|alias| contains(alias)) {
        if !selected_bindings.observe(alias) {
            return None;
        }
    }
    if selected_bindings
        .paths
        .iter()
        .any(|(alias, target)| observed.bindings.get(alias) != Some(target))
    {
        return None;
    }
    let bindings = selected_bindings.paths;
    for path in explicit {
        if !observed.digests.contains_key(&path) {
            return None;
        }
        files.insert(path);
    }
    roots.sort();
    roots.dedup();
    Some(Projection {
        workspace,
        roots,
        files,
        bindings,
        ranges,
    })
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

/// Source alias resolutions relevant to the selected dependency closure.
pub(super) fn closure_bindings(
    snapshot: &InputSnapshot,
    data: &Value,
    closure: Option<&BTreeSet<String>>,
) -> BTreeMap<PathBuf, PathBuf> {
    let Some(closure) = closure else {
        return snapshot.bindings.clone();
    };
    let scope = Scope::new(data, closure);
    snapshot
        .bindings
        .iter()
        .filter(|(alias, _)| {
            scope.contains(alias) || scope.declared.iter().any(|path| path.starts_with(alias))
        })
        .map(|(alias, target)| (alias.clone(), target.clone()))
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
    declared: std::collections::HashSet<PathBuf>,
}
impl Scope {
    fn new(data: &Value, closure: &BTreeSet<String>) -> Self {
        let mut scope = Self {
            inside: std::collections::HashSet::new(),
            outside: std::collections::HashSet::new(),
            manifests: std::collections::HashSet::new(),
            declared: data["sail_source_inputs_by_package"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(id, _)| closure.contains(*id))
                .flat_map(|(_, paths)| paths.as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect(),
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
        self.declared.contains(path)
            || path.ancestors().any(|d| self.inside.contains(d))
            || !path.ancestors().any(|d| self.outside.contains(d))
    }
}

// Preserve the original exact sorted path spelling and digest encoding. Every
// file is still read completely between the caller's stamps; only the reads
// overlap, and their digests are folded in the original sorted order.
fn contents_revision(
    files: &BTreeSet<PathBuf>,
    bindings: &BTreeMap<PathBuf, PathBuf>,
) -> (String, bool, BTreeMap<PathBuf, String>) {
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    fold_contents(files, bindings, digest_all_tagged(&paths))
}

fn fold_contents<'a, S: AsRef<str>>(
    files: &BTreeSet<PathBuf>,
    bindings: &BTreeMap<PathBuf, PathBuf>,
    reads: impl IntoIterator<Item = (&'a Path, Result<S>)>,
) -> (String, bool, BTreeMap<PathBuf, String>) {
    let mut recorded = Vec::with_capacity(files.len());
    let (revision, complete) = fold_revision(files, bindings, reads, |path, hash| {
        recorded.push((path.to_owned(), hash.to_owned()));
    });
    (revision, complete, recorded.into_iter().collect())
}

/// Fold independently keyed outcomes in the original requested path order,
/// optionally retaining digests. Revision-only callers avoid building a map.
fn fold_revision<'a, S: AsRef<str>>(
    files: &BTreeSet<PathBuf>,
    bindings: &BTreeMap<PathBuf, PathBuf>,
    reads: impl IntoIterator<Item = (&'a Path, Result<S>)>,
    mut record: impl FnMut(&Path, &str),
) -> (String, bool) {
    // A parallel result owns the path actually read alongside its outcome.
    // Validate all returned keys before reducing them to a keyed lookup.
    let reads: Vec<_> = reads.into_iter().collect();
    let required: Vec<_> = files
        .iter()
        .map(|path| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(&[]),
        })
        .collect();
    let observed: Vec<_> = reads
        .iter()
        .map(|(path, read)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: read.as_ref().map_or(InputValue::Unreadable, |hash| {
                InputValue::Present(hash.as_ref().as_bytes())
            }),
        })
        .collect();
    let (required_order, required_inverse) = proposed_order(&required);
    let (observed_order, observed_inverse) = proposed_order(&observed);
    let complete = reads_complete(
        &required,
        &observed,
        &required_order,
        &required_inverse,
        &observed_order,
        &observed_inverse,
    );
    let by_name: BTreeMap<_, _> = reads
        .iter()
        .filter_map(|(path, read)| {
            read.as_ref()
                .ok()
                .map(|hash| (path.as_os_str().as_encoded_bytes(), hash))
        })
        .collect();
    let mut hasher = Sha256::new();
    for path in files {
        let bytes = path.as_os_str().as_encoded_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        if let Some(hash) = by_name.get(bytes) {
            hasher.update(hash.as_ref().as_bytes());
            record(path, hash.as_ref());
        } else {
            hasher.update(b"unreadable");
        }
    }
    bind_paths(&mut hasher, bindings);
    (format!("{:x}", hasher.finalize()), complete)
}

pub(super) fn bind_paths(hasher: &mut Sha256, bindings: &BTreeMap<PathBuf, PathBuf>) {
    if bindings.is_empty() {
        return;
    }
    hasher.update(b"sail-source-path-bindings-v1");
    for (alias, target) in bindings {
        for path in [alias, target] {
            let bytes = path.as_os_str().as_encoded_bytes();
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
}

// A run-local mutation stamp is separate from logical proof keys: reverting an
// edit between completed runs may reuse a proof, reverting during a run may not.
pub(super) fn mutation_stamp(snapshot: &InputSnapshot) -> Option<String> {
    stamp_paths(&mutation_paths(snapshot)?)
}

fn mutation_paths(snapshot: &InputSnapshot) -> Option<BTreeSet<PathBuf>> {
    stamp_root_paths(
        snapshot.roots.iter().map(PathBuf::as_path),
        snapshot.observation_paths.clone(),
    )
}

/// The complete stat-stamp vector behind `expected`, this snapshot's mutation
/// stamp: every stamped path with its full stamp, or `None` when absent. It is
/// returned only when the same `lstat` results fold to exactly `expected`, so
/// the vector describes the state the run's own currentness check accepted.
#[cfg(unix)]
pub(super) fn mutation_vector(
    snapshot: &InputSnapshot,
    expected: &str,
) -> Option<Vec<(PathBuf, Option<super::noop::Stamp>)>> {
    let paths = mutation_paths(snapshot)?;
    let names: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    let stats = crate::parallel::in_order(&names, |&path| (path, std::fs::symlink_metadata(path)));
    if stamp_readback(&paths, &stats)? != expected {
        return None;
    }
    stats
        .into_iter()
        .map(|(path, metadata)| match metadata {
            Ok(metadata) => Some((path.to_owned(), Some(super::noop::Stamp::of(&metadata)))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Some((path.to_owned(), None))
            }
            Err(_) => None,
        })
        .collect()
}

#[cfg(not(unix))]
pub(super) fn mutation_vector(
    _snapshot: &InputSnapshot,
    _expected: &str,
) -> Option<Vec<(PathBuf, Option<super::noop::Stamp>)>> {
    None
}

// The retained readback brackets the entire exact recorded domain, independent
// of the content-read executor. A supplied inventory is not a compiler-closure
// proof; this only guards the paths that the original record actually binds.
pub(super) fn readback_stamp(roots: &BTreeSet<PathBuf>) -> Option<String> {
    stamp_roots(roots.iter().map(PathBuf::as_path), roots.clone())
}

fn stamp_roots<'a>(
    roots: impl IntoIterator<Item = &'a Path>,
    paths: BTreeSet<PathBuf>,
) -> Option<String> {
    stamp_paths(&stamp_root_paths(roots, paths)?)
}

fn stamp_root_paths<'a>(
    roots: impl IntoIterator<Item = &'a Path>,
    mut paths: BTreeSet<PathBuf>,
) -> Option<BTreeSet<PathBuf>> {
    let roots: Vec<&Path> = roots.into_iter().collect();
    // Preserve the same exact root domain and conservative error policy while
    // overlapping independent path observations.
    let resolved = crate::parallel::in_order(&roots, |&path| match path.canonicalize() {
        Ok(resolved) => Some(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(path.to_owned()),
        Err(_) => None,
    });
    paths.extend(resolved.into_iter().collect::<Option<Vec<_>>>()?);
    // Package roots share most ancestors (the registry source directory, the
    // home directory): each ancestor is examined once. The resolutions and
    // link checks are independent, so they overlap; the stamped set is the
    // same as examining them one by one.
    let mut examined = HashSet::new();
    let ancestors: Vec<&Path> = roots
        .iter()
        .flat_map(|path| path.ancestors())
        .filter(|alias| examined.insert(*alias))
        .collect();
    let links = crate::parallel::in_order(&ancestors, |&alias| {
        match std::fs::symlink_metadata(alias) {
            Ok(metadata) => Some(metadata.file_type().is_symlink()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(_) => None,
        }
    })
    .into_iter()
    .collect::<Option<Vec<_>>>()?;
    paths.extend(
        ancestors
            .iter()
            .zip(links)
            .filter(|(_, link)| *link)
            .map(|(alias, _)| (*alias).to_owned()),
    );
    Some(paths)
}

// The `lstat` calls overlap (about 13,000 paths per vest snapshot); the stamp
// folds their results in the original sorted order, exactly as one by one.
#[cfg(unix)]
fn stamp_paths(paths: &BTreeSet<PathBuf>) -> Option<String> {
    let names: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    let stats = crate::parallel::in_order(&names, |&path| (path, std::fs::symlink_metadata(path)));
    stamp_readback(paths, &stats)
}

#[cfg(unix)]
fn stamp_readback(
    paths: &BTreeSet<PathBuf>,
    stats: &[(&Path, std::io::Result<std::fs::Metadata>)],
) -> Option<String> {
    use std::os::unix::fs::MetadataExt as _;
    let names: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    if !read_outcome_keys_match(&names, stats) {
        return None;
    }
    let by_name: BTreeMap<_, _> = stats
        .iter()
        .map(|(path, metadata)| (path.as_os_str().as_encoded_bytes(), metadata))
        .collect();
    let mut stamp = Sha256::new();
    for path in paths {
        let name = path.as_os_str().as_encoded_bytes();
        stamp.update((name.len() as u64).to_le_bytes());
        stamp.update(name);
        let metadata = match by_name.get(name)? {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                stamp.update([0u8]);
                continue;
            }
            Err(_) => return None,
        };
        stamp.update([1u8]);
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
    #[cfg(unix)]
    #[test]
    fn mutation_stamps_cover_resolved_roots_and_linked_ancestors() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-stamp-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("real/src"))?;
        std::fs::write(root.join("real/src/lib.rs"), "x")?;
        std::os::unix::fs::symlink(root.join("real"), root.join("link"))?;
        let root = root.canonicalize()?;
        let snapshot = |roots: Vec<std::path::PathBuf>| super::InputSnapshot {
            workspace: root.clone(),
            revision: String::new(),
            roots,
            excluded: Vec::new(),
            complete: true,
            observation_paths: std::collections::BTreeSet::new(),
            digests: std::collections::BTreeMap::new(),
            bindings: std::collections::BTreeMap::new(),
            capture: None,
        };
        let linked = snapshot(vec![root.join("link/src/../src/lib.rs")]);
        let explicit = super::InputSnapshot {
            observation_paths: [root.join("real/src/lib.rs"), root.join("link")]
                .into_iter()
                .collect(),
            ..snapshot(Vec::new())
        };
        assert!(super::mutation_stamp(&linked).is_some());
        assert_eq!(
            super::mutation_stamp(&linked),
            super::mutation_stamp(&explicit)
        );
        std::fs::remove_dir_all(root)
    }

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
        assert_eq!(
            snapshot.bindings,
            BTreeMap::from([(package.join("alias.rs"), outside.clone())])
        );
        oracle.update(b"sail-source-path-bindings-v1");
        for path in [package.join("alias.rs"), outside.clone()] {
            let bytes = path.as_os_str().as_encoded_bytes();
            oracle.update((bytes.len() as u64).to_le_bytes());
            oracle.update(bytes);
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
    #[cfg(unix)]
    #[test]
    fn observed_revision_binds_retargeting_between_already_hashed_sources() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-observed-binding-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        std::fs::write(root.join("Cargo.toml"), "fixture")?;
        std::fs::write(root.join("good.rs"), "good")?;
        std::fs::write(root.join("bad.rs"), "bad")?;
        let alias = root.join("lib.rs");
        std::os::unix::fs::symlink(root.join("good.rs"), &alias)?;
        let data = serde_json::json!({"workspace_root":root,
            "sail_invocation_directory":root,
            "packages":[{"manifest_path":root.join("Cargo.toml")}]});
        let (observed, stamp) = snapshot_metadata_stamped(&data, &[]);
        let projected = snapshot_metadata_projected(&data, &[], &observed)
            .ok_or_else(|| std::io::Error::other("complete source projection refused"))?;
        assert_eq!(projected.revision, observed.revision);
        assert_eq!(projected.bindings, observed.bindings);
        assert_eq!(
            snapshot_revision_projected(&data, &[], &observed),
            Some((projected.revision.clone(), projected.complete))
        );
        let mut unguarded = observed.clone();
        unguarded.capture = None;
        assert!(snapshot_revision_projected(&data, &[], &unguarded).is_none());
        let mut rebound = observed.clone();
        rebound.revision.push_str("-rebound");
        assert!(snapshot_revision_projected(&data, &[], &rebound).is_none());
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(root.join("bad.rs"), &alias)?;
        let fresh = snapshot_metadata(&data, &[]);
        let (reobserved, _) = snapshot_metadata_reobserved(&data, &[], &observed, stamp.as_ref());
        assert_eq!(fresh.digests, observed.digests);
        assert_ne!(fresh.bindings, observed.bindings);
        assert_ne!(fresh.revision, observed.revision);
        assert_eq!(reobserved.revision, fresh.revision);
        assert_eq!(reobserved.bindings, fresh.bindings);
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(root.join("good.rs"), &alias)?;
        assert_eq!(snapshot_metadata(&data, &[]).revision, observed.revision);
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn content_capture_refuses_missing_extra_and_failed_read_outcomes() {
        let a = Path::new("a.rs");
        let b = Path::new("b.rs");
        let c = Path::new("c.rs");
        let files = BTreeSet::from([a.to_owned(), b.to_owned()]);
        let bindings = BTreeMap::new();
        let (revision, complete, digests) =
            fold_contents(&files, &bindings, [(a, Ok("one")), (b, Ok("two"))]);
        assert!(complete);
        assert_eq!(digests.len(), 2);
        let reordered = fold_contents(&files, &bindings, [(b, Ok("two")), (a, Ok("one"))]);
        assert_eq!(reordered, (revision, true, digests));
        for reads in [
            vec![(a, Ok("one"))],
            vec![(a, Ok("one")), (b, Ok("two")), (c, Ok("extra"))],
            vec![(a, Ok("one")), (a, Ok("two"))],
            vec![
                (a, Ok("one")),
                (b, Err(Error::new("SAIL3101", "read failed"))),
            ],
        ] {
            assert!(!fold_contents(&files, &bindings, reads).1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn metadata_stamps_validate_returned_paths_before_folding() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-tagged-stats-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        let absent = root.join("absent.toml");
        let extra = root.join("extra.rs");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        let files = BTreeSet::from([a.clone(), b.clone(), absent.clone()]);
        let paths = [a.as_path(), b.as_path(), absent.as_path()];
        let read =
            || crate::parallel::in_order(&paths, |&path| (path, std::fs::symlink_metadata(path)));
        let before = stamp_readback(&files, &read());
        assert!(before.is_some());
        let mut reordered = read();
        reordered.reverse();
        assert_eq!(stamp_readback(&files, &reordered), before);
        reordered.pop();
        assert!(stamp_readback(&files, &reordered).is_none());
        let mut unexpected = read();
        unexpected.push((extra.as_path(), std::fs::symlink_metadata(&extra)));
        assert!(stamp_readback(&files, &unexpected).is_none());
        let duplicate = [
            (a.as_path(), std::fs::symlink_metadata(&a)),
            (a.as_path(), std::fs::symlink_metadata(&a)),
            (absent.as_path(), std::fs::symlink_metadata(&absent)),
        ];
        assert!(stamp_readback(&files, &duplicate).is_none());
        std::fs::write(&a, "changed contents")?;
        let mut changed = read();
        changed.reverse();
        assert_ne!(stamp_readback(&files, &changed), before);
        std::fs::remove_dir_all(root)
    }

    #[cfg(unix)]
    #[test]
    fn raw_alias_reads_cannot_cancel_a_retarget_with_reordering() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-tagged-aliases-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let a = root.join("a");
        let b = root.join("b");
        let first = root.join("first.rs");
        let second = root.join("second.rs");
        std::fs::write(&first, "first")?;
        std::fs::write(&second, "second")?;
        std::os::unix::fs::symlink(&first, &a)?;
        std::os::unix::fs::symlink(&second, &b)?;
        let expected = BTreeMap::from([(a.clone(), first.clone()), (b.clone(), second.clone())]);
        let names = [a.as_path(), b.as_path()];
        let read = || crate::parallel::in_order(&names, |&name| (name, name.canonicalize()));
        let mut reads = read();
        reads.reverse();
        assert!(bindings_readback(&expected, &reads));
        reads.pop();
        assert!(!bindings_readback(&expected, &reads));
        let duplicate = [
            (a.as_path(), a.canonicalize()),
            (a.as_path(), a.canonicalize()),
        ];
        assert!(!bindings_readback(&expected, &duplicate));
        std::fs::remove_file(&a)?;
        std::fs::remove_file(&b)?;
        std::os::unix::fs::symlink(&second, &a)?;
        std::os::unix::fs::symlink(&first, &b)?;
        let mut retargeted = read();
        retargeted.reverse();
        assert!(!bindings_readback(&expected, &retargeted));
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn raw_content_reads_keep_their_own_paths_under_permutation() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-tagged-inputs-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        let files = BTreeSet::from([a.clone(), b.clone()]);
        let paths = [a.as_path(), b.as_path()];
        let before = fold_contents(&files, &BTreeMap::new(), digest_all_tagged(&paths));
        assert!(before.1);
        let mut reads = digest_all_tagged(&paths);
        reads.reverse();
        assert_eq!(fold_contents(&files, &BTreeMap::new(), reads), before);
        // Swapping both physical contents and result order used to cancel if
        // expected paths were reattached to untagged results by position.
        std::fs::write(&a, "second")?;
        std::fs::write(&b, "first")?;
        let mut changed = digest_all_tagged(&paths);
        changed.reverse();
        let after = fold_contents(&files, &BTreeMap::new(), changed);
        assert!(after.1);
        assert_ne!(after.0, before.0);
        assert_ne!(after.2, before.2);
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn observed_subset_has_the_same_content_and_namespace_as_a_fresh_read() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-observed-subset-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let a = root.join("a");
        let b = root.join("a-b");
        std::fs::create_dir_all(&a)?;
        std::fs::create_dir_all(&b)?;
        std::fs::write(a.join("Cargo.toml"), "a")?;
        std::fs::write(a.join("lib.rs"), "first")?;
        std::fs::write(b.join("Cargo.toml"), "b")?;
        std::fs::write(b.join("lib.rs"), "second")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(a.join("lib.rs"), a.join("internal.rs"))?;
        let data = serde_json::json!({"workspace_root":root,
            "sail_invocation_directory":root,
            "packages":[{"manifest_path":a.join("Cargo.toml")},
                        {"manifest_path":b.join("Cargo.toml")}]});
        let (mut observed, stamp) = snapshot_metadata_stamped(&data, &[]);
        assert!(observed.complete);
        // The real runner adds absent configuration paths to its watch roots.
        observed.roots.push(root.join(".cargo/config.toml"));
        let mut subset = data.clone();
        subset["workspace_root"] = serde_json::json!(a);
        subset["packages"] = serde_json::json!([{"manifest_path":a.join("Cargo.toml")}]);
        assert!(projected_snapshot(&subset, &[], None, &observed).is_some());
        let projected = snapshot_metadata_observed(&subset, &[], &observed, stamp.as_ref());
        let fresh = snapshot_metadata(&subset, &[]);
        assert!(projected.complete && fresh.complete);
        assert_eq!(projected.revision, fresh.revision);
        assert_eq!(projected.digests, fresh.digests);
        assert_eq!(projected.observation_paths, fresh.observation_paths);
        assert_eq!(projected.roots, fresh.roots);
        assert_eq!(projected.bindings, fresh.bindings);
        assert_eq!(projected.excluded, fresh.excluded);
        assert_eq!(
            snapshot_revision_projected(&subset, &[], &observed),
            Some((fresh.revision.clone(), fresh.complete))
        );
        // Also select the later disjoint range: earlier digests must never
        // supply a later file's value or leak into its namespace.
        let mut later = data.clone();
        later["workspace_root"] = serde_json::json!(b);
        later["packages"] = serde_json::json!([{"manifest_path":b.join("Cargo.toml")}]);
        let projected = snapshot_metadata_observed(&later, &[], &observed, stamp.as_ref());
        let fresh = snapshot_metadata(&later, &[]);
        assert!(projected.complete && fresh.complete);
        assert_eq!(projected.revision, fresh.revision);
        assert_eq!(projected.digests, fresh.digests);
        assert_eq!(projected.observation_paths, fresh.observation_paths);
        assert_eq!(
            snapshot_revision_projected(&later, &[], &observed),
            Some((fresh.revision.clone(), fresh.complete))
        );
        // A newly declared external include was absent from the observation.
        // It must be freshly read, including after another content change.
        let outside = root.join("external.rs");
        std::fs::write(&outside, "new input")?;
        let extras = [outside.clone()];
        let projected = snapshot_metadata_observed(&subset, &extras, &observed, stamp.as_ref());
        assert_eq!(
            projected.revision,
            snapshot_metadata(&subset, &extras).revision
        );
        std::fs::write(&outside, "changed input")?;
        let changed = snapshot_metadata_observed(&subset, &extras, &observed, stamp.as_ref());
        assert_ne!(projected.revision, changed.revision);
        assert_eq!(
            changed.revision,
            snapshot_metadata(&subset, &extras).revision
        );
        #[cfg(unix)]
        {
            let (observed_before, stamp_before) = snapshot_metadata_stamped(&data, &[]);
            let prior =
                snapshot_metadata_observed(&subset, &[], &observed_before, stamp_before.as_ref());
            let directory = std::fs::File::open(&a)?;
            let modified = directory.metadata()?.modified()?;
            std::fs::remove_file(a.join("internal.rs"))?;
            std::os::unix::fs::symlink(&outside, a.join("internal.rs"))?;
            directory.set_times(std::fs::FileTimes::new().set_modified(modified))?;
            let changed =
                snapshot_metadata_observed(&subset, &[], &observed_before, stamp_before.as_ref());
            assert_ne!(prior.revision, changed.revision);
            assert_eq!(changed.revision, snapshot_metadata(&subset, &[]).revision);
            // A package symlink can read bytes outside its own directory.
            // The observation must preserve that input even when its mtime
            // is restored after an edit.
            std::os::unix::fs::symlink(&outside, a.join("external.rs"))?;
            let (observed, stamp) = snapshot_metadata_stamped(&data, &[]);
            let prior = snapshot_metadata_observed(&subset, &[], &observed, stamp.as_ref());
            assert_eq!(prior.revision, snapshot_metadata(&subset, &[]).revision);
            let file = std::fs::OpenOptions::new().write(true).open(&outside)?;
            let modified = file.metadata()?.modified()?;
            std::fs::write(&outside, "another input")?;
            file.set_times(std::fs::FileTimes::new().set_modified(modified))?;
            let changed = snapshot_metadata_observed(&subset, &[], &observed, stamp.as_ref());
            assert_ne!(prior.revision, changed.revision);
            assert_eq!(changed.revision, snapshot_metadata(&subset, &[]).revision);
        }
        std::fs::remove_dir_all(root)
    }

    #[cfg(unix)]
    #[test]
    fn declared_headers_through_an_ancestor_alias_match_fresh_reads() -> std::io::Result<()> {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("sail-header-alias-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let app = root.join("app");
        let native = root.join("native");
        let other = root.join("other");
        std::fs::create_dir_all(&app)?;
        for directory in [&native, &other] {
            std::fs::create_dir_all(directory.join("include"))?;
        }
        std::fs::write(app.join("Cargo.toml"), "manifest")?;
        std::fs::write(native.join("include/header.h"), "first")?;
        std::fs::write(other.join("include/header.h"), "second")?;
        let alias = root.join("opt");
        symlink(&native, &alias)?;
        let extras = [alias.join("include")];
        let data = serde_json::json!({"workspace_root":app,
            "packages":[{"manifest_path":app.join("Cargo.toml")}]});
        let (observed, stamp) = snapshot_metadata_stamped(&data, &extras);
        assert!(snapshot_metadata_projected(&data, &extras, &observed).is_some());
        let (same, same_stamp) =
            snapshot_metadata_reobserved(&data, &extras, &observed, stamp.as_ref());
        let fresh = snapshot_metadata(&data, &extras);
        assert_eq!(same.revision, fresh.revision);
        assert_eq!(same.roots, fresh.roots);
        assert_eq!(same.observation_paths, fresh.observation_paths);
        assert_eq!(same_stamp, stamp);
        std::fs::remove_file(&alias)?;
        symlink(&other, &alias)?;
        assert!(snapshot_metadata_projected(&data, &extras, &observed).is_none());
        let (changed, _) = snapshot_metadata_reobserved(&data, &extras, &observed, stamp.as_ref());
        assert_ne!(changed.revision, observed.revision);
        assert_eq!(changed.revision, snapshot_metadata(&data, &extras).revision);
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn observed_revisions_reread_mutations_even_when_modification_time_is_restored()
    -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-observed-mutation-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let source = root.join("lib.rs");
        std::fs::write(&source, "old")?;
        let data = serde_json::json!({"workspace_root":root,
            "packages":[{"manifest_path":root.join("Cargo.toml")}]});
        let (observed, stamp) = snapshot_metadata_stamped(&data, &[]);
        let (same, same_stamp) =
            snapshot_metadata_reobserved(&data, &[], &observed, stamp.as_ref());
        assert_eq!(same.revision, observed.revision);
        assert_eq!(same.observation_paths, observed.observation_paths);
        assert_eq!(same_stamp, stamp);
        let mut wrong_revision = observed.clone();
        wrong_revision.revision.push_str("-unobserved");
        assert!(!unchanged_since(&wrong_revision, stamp.as_ref()));
        let file = std::fs::OpenOptions::new().write(true).open(&source)?;
        let original_time = file.metadata()?.modified()?;
        std::fs::write(&source, "new")?;
        file.set_times(std::fs::FileTimes::new().set_modified(original_time))?;
        // A caller cannot pair old bytes with a newly sampled mutation stamp.
        let fresh_stamp = stamp_paths(&observed.observation_paths);
        assert!(!unchanged_since(&observed, fresh_stamp.as_ref()));
        let changed = snapshot_metadata_observed(&data, &[], &observed, stamp.as_ref());
        assert!(changed.complete);
        assert_ne!(changed.revision, observed.revision);
        let (reobserved, new_stamp) =
            snapshot_metadata_reobserved(&data, &[], &observed, stamp.as_ref());
        assert_eq!(reobserved.revision, changed.revision);
        assert_ne!(new_stamp, stamp);
        assert_eq!(changed.revision, snapshot_metadata(&data, &[]).revision);
        // New and removed names cannot reuse a former namespace.
        let added = root.join("new-module.rs");
        std::fs::write(&added, "module")?;
        let with_new = snapshot_metadata_observed(&data, &[], &observed, stamp.as_ref());
        assert_ne!(with_new.revision, changed.revision);
        assert_eq!(with_new.revision, snapshot_metadata(&data, &[]).revision);
        std::fs::remove_file(&source)?;
        let removed = snapshot_metadata_observed(&data, &[], &observed, stamp.as_ref());
        assert_eq!(removed.revision, snapshot_metadata(&data, &[]).revision);
        assert!(!removed.digests.contains_key(&source));
        // An incomplete observation is never a source of cached bytes.
        let mut incomplete = observed.clone();
        incomplete.complete = false;
        assert_eq!(
            snapshot_metadata_observed(&data, &[], &incomplete, stamp.as_ref()).revision,
            snapshot_metadata(&data, &[]).revision
        );
        std::fs::remove_dir_all(root)
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
    fn discovering_a_declared_spelling_preserves_stamp_and_alias_guards() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-declared-spelling-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src"))?;
        let root = root.canonicalize()?;
        std::fs::create_dir_all(root.join("good/nested"))?;
        std::fs::create_dir_all(root.join("bad/nested"))?;
        std::fs::write(root.join("good/proof.rs"), "assert(true)")?;
        std::fs::write(root.join("bad/proof.rs"), "assert(false)")?;
        let alias = root.join("alias");
        std::os::unix::fs::symlink(root.join("good/nested"), &alias)?;
        let mut data = serde_json::json!({"workspace_root":root,
            "packages":[{"manifest_path":root.join("Cargo.toml")}]});
        let before = snapshot_metadata_home(&data, &[], None);
        let stamp = mutation_stamp(&before);
        assert!(before.complete && stamp.is_some());
        // Resolve .. through the real filesystem: alias/../proof is good/proof,
        // not root/proof. Discovering this declaration adds no unseen input.
        data["sail_source_inputs"] = serde_json::json!([
            root.join("src/../good/proof.rs"),
            root.join("alias/../proof.rs")
        ]);
        let declared = snapshot_metadata_home(&data, &[], None);
        assert!(declared.complete);
        assert_eq!(declared.digests, before.digests);
        assert_eq!(declared.bindings, before.bindings);
        assert_eq!(declared.revision, before.revision);
        assert_eq!(mutation_stamp(&declared), stamp);
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(root.join("bad/nested"), &alias)?;
        let changed = snapshot_metadata_home(&data, &[], None);
        assert_eq!(changed.digests, before.digests);
        assert_ne!(changed.bindings, before.bindings);
        assert_ne!(changed.revision, before.revision);
        assert_ne!(mutation_stamp(&changed), stamp);
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(root.join("good/nested"), &alias)?;
        let restored = snapshot_metadata_home(&data, &[], None);
        assert_eq!(restored.revision, before.revision);
        assert_ne!(mutation_stamp(&restored), stamp);
        std::fs::remove_dir_all(root)
    }

    #[cfg(unix)]
    #[test]
    fn mutation_stamp_keeps_absent_configuration_roots() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-absent-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let data = serde_json::json!({"workspace_root":root,"packages":[]});
        let mut snapshot = snapshot_metadata_home(&data, &[], None);
        let missing = root.join("config.toml");
        snapshot.roots.push(missing.clone());
        let stamp = mutation_stamp(&snapshot);
        assert!(stamp.is_some());
        std::fs::write(missing, "[build]\njobs=1\n")?;
        assert_ne!(mutation_stamp(&snapshot), stamp);
        std::fs::remove_dir_all(root)
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
        // The temporary directory's real path: on macOS it sits behind the
        // `/var` -> `/private/var` link, and collected roots are real paths.
        let root = std::env::temp_dir()
            .canonicalize()?
            .join(format!("sail-collect-test-{}", std::process::id()));
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
        let complete = collect(
            &package,
            &mut walked.0,
            &mut walked.1,
            &mut walked.2,
            &mut Bindings::default(),
        );
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
            bindings: std::collections::BTreeMap::new(),
            capture: None,
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
