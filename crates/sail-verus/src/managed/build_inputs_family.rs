//! Transactional ordinary script observations for an exclusively owned target.
//!
//! This is not a proof receipt. A family export is consumed before any target
//! mutation; an interrupted writer cannot leave old declarations authorizing
//! newer generated output. Every context keeps its own fresh observation copy.

use super::{BuildInput, current};
use crate::managed::files::{digest, json};
use crate::managed::input::PathStamp;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(unix)]
use std::io::Read as _;
use std::io::Write;
use std::path::{Path, PathBuf};

#[path = "build_inputs_sources.rs"]
mod sources;

#[path = "build_inputs_schedule.rs"]
pub(in crate::managed) mod schedule;

const MAX_BYTES: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    version: u32,
    compiler: String,
    records: Vec<BuildInput>,
    sources: Option<sources::Inventory>,
    checksum: String,
}

struct Encoding(Vec<u8>);
impl Write for Encoding {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("compiler script observation limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize) -> Option<Vec<u8>> {
    let mut writer = Encoding(Vec::new());
    serde_json::to_writer(&mut writer, value).ok()?;
    Some(writer.0)
}
fn checksum(export: &Export) -> Option<String> {
    encode(&(
        export.version,
        &export.compiler,
        &export.records,
        &export.sources,
    ))
    .map(|v| digest(&v))
}

#[cfg(unix)]
fn bytes(path: &Path) -> Option<Vec<u8>> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(u64::try_from(MAX_BYTES).ok()?.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_BYTES {
        return None;
    }
    Some(bytes)
}
#[cfg(not(unix))]
fn bytes(_path: &Path) -> Option<Vec<u8>> {
    // Only the Unix nonblocking regular-file loader is audited. An unavailable
    // export always takes original script cleaning, including an older context.
    None
}

fn read(path: &Path) -> Option<Export> {
    serde_json::from_slice(&bytes(path)?).ok()
}

fn output_matches(record: &BuildInput, target: &Path) -> bool {
    let Some((path, sha)) = &record.output else {
        return false;
    };
    let Some(target) = target.canonicalize().ok() else {
        return false;
    };
    let Some(actual) = path.canonicalize().ok().filter(|p| p.starts_with(&target)) else {
        return false;
    };
    let Some(bytes) = bytes(&actual).filter(|bytes| digest(bytes) == *sha) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let Some(directory) = record.manifest.parent() else {
        return false;
    };
    let mut paths = Vec::new();
    let mut keys = std::collections::BTreeSet::new();
    for line in text.lines() {
        let line = line
            .strip_prefix("cargo::")
            .or_else(|| line.strip_prefix("cargo:"))
            .unwrap_or(line);
        if let Some(path) = line.strip_prefix("rerun-if-changed=") {
            paths.push(directory.join(path));
        }
        if let Some(key) = line.strip_prefix("rerun-if-env-changed=") {
            keys.insert(key.to_owned());
        }
    }
    if paths.is_empty() && keys.is_empty() {
        paths.push(directory.to_owned());
    }
    paths == record.paths && keys == record.env.keys().cloned().collect()
}

fn matches(record: &BuildInput, data: &Value, target: &Path) -> bool {
    record.covered
        && output_matches(record, target)
        && record.env.keys().all(|key| allowed_environment(key))
        && package_matches(record, data)
        && current(record, None) == (record.digest.clone(), true)
}

// Parent values do not describe Cargo-injected or removed child variables.
// RUSTC_BOOTSTRAP is pinned by the invocation. CARGO_TARGET_DIR is different:
// compilation preserves the exact caller value (including absence); only
// metadata's separate child receives the owned metadata target override.
//
// `CARGO_CFG_*` and `CARGO_FEATURE_*` are Cargo-injected too, but what Cargo
// injects is fixed by what the contract already pins (#294). Feature values
// are the unit's features, which select its output directory
// (`output_matches` binds that exact path). Cfg values are the target's
// `rustc --print cfg` under the effective rustflags: the toolchain, target,
// caller `CARGO_*`/`RUST*` environment and Cargo configuration bytes are all
// part of the compiler family's identity, so a family's target cannot see
// other values. Cargo's own `rerun-if-env-changed` check reads these names
// from its own environment, as the recorded parent value does. rustix 0.38
// declares both kinds; refusing them cleaned rustix and every package that
// depends on it on every run.
fn allowed_environment(key: &str) -> bool {
    if matches!(std::env::var(key), Err(std::env::VarError::NotUnicode(_))) {
        return false;
    }
    matches!(key, "RUSTC_BOOTSTRAP" | "CARGO_TARGET_DIR")
        || key.starts_with("CARGO_CFG_")
        || key.starts_with("CARGO_FEATURE_")
        || !(key.starts_with("SAIL_")
            || key.starts_with("VERUS_")
            || key.starts_with("CARGO_")
            || key.starts_with("RUSTUP_")
            || key.starts_with("DEP_")
            || [
                "CARGO",
                "RUSTC",
                "RUSTDOC",
                "RUSTC_LINKER",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "RUSTFLAGS",
                "OUT_DIR",
                "HOST",
                "TARGET",
                "PROFILE",
                "OPT_LEVEL",
                "DEBUG",
                "NUM_JOBS",
                "LD_LIBRARY_PATH",
                "DYLD_LIBRARY_PATH",
                "DYLD_FALLBACK_LIBRARY_PATH",
                "LIBPATH",
                "PATH",
            ]
            .contains(&key))
}

fn package_matches(record: &BuildInput, data: &Value) -> bool {
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|package| {
            package["id"] == record.package_id
                && package["name"] == record.package_name
                && package["manifest_path"].as_str().map(Path::new)
                    == Some(record.manifest.as_path())
                && package["targets"].as_array().is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
                    })
                })
        })
}

fn eligible(records: Vec<BuildInput>, data: &Value, target: &Path) -> Vec<BuildInput> {
    let mut invalid = BTreeSet::new();
    let mut units = BTreeSet::new();
    for record in &records {
        let output = record.output.as_ref().map(|(path, _)| path.as_path());
        if !matches(record, data, target) || !units.insert((record.package_id.as_str(), output)) {
            invalid.insert(record.package_id.clone());
        }
    }
    // A single unchanged unit must not authorize other units of the package.
    // A package that depends on a refused one loses its contracts too, so the
    // refusal reaches everything that may have consumed the changed script.
    let invalid = dependents(data, invalid);
    records
        .into_iter()
        .filter(|r| !invalid.contains(&r.package_id))
        .collect()
}

/// `invalid` and every package of Cargo's resolve graph that reaches one of
/// them through any dependency kind. Without a graph only `invalid` is known.
fn dependents(data: &Value, mut invalid: BTreeSet<String>) -> BTreeSet<String> {
    let Some(nodes) = data["resolve"]["nodes"].as_array() else {
        return invalid;
    };
    if invalid.is_empty() {
        return invalid;
    }
    loop {
        let before = invalid.len();
        for node in nodes {
            let Some(id) = node["id"].as_str() else {
                continue;
            };
            if !invalid.contains(id)
                && node["dependencies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|dependency| invalid.contains(dependency))
            {
                invalid.insert(id.to_owned());
            }
        }
        if invalid.len() == before {
            return invalid;
        }
    }
}

/// Recheck after the original native guards start, before any cached output use.
/// Remove all observations of a package if even one unit lost its contract.
pub(in crate::managed) fn recheck(
    local: &Path,
    data: &Value,
    target: &Path,
) -> Result<Vec<String>> {
    let previous = super::read::<Vec<BuildInput>>(local).unwrap_or_default();
    let packages: std::collections::BTreeMap<_, _> = previous
        .iter()
        .map(|r| (r.package_id.clone(), r.package_name.clone()))
        .collect();
    let retained = eligible(previous, data, target);
    let retained_ids: BTreeSet<_> = retained.iter().map(|r| r.package_id.as_str()).collect();
    let clean: BTreeSet<_> = packages
        .iter()
        .filter(|(id, _)| !retained_ids.contains(id.as_str()))
        .map(|(_, name)| name.clone())
        .collect();
    let clean = clean.into_iter().collect();
    super::super::files::json(local, &retained)?;
    Ok(clean)
}

/// Must run under the existing family lock, before any clean or invocation.
/// Missing exports cannot fall back to an older context-local observation.
pub(in crate::managed) fn begin(
    path: &Path,
    compiler: &str,
    target: &Path,
    local: &Path,
    data: &Value,
) -> Result<bool> {
    let export = read(path).filter(|export| {
        export.version == 2
            && export.compiler == compiler
            && checksum(export).as_ref() == Some(&export.checksum)
    });
    // Remove even unreadable, stale or malformed exports. Refuse mutation if
    // removal fails: leaving an old certificate alongside new output is unsafe.
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::new("SAIL3102", error.to_string())),
    }
    let (mut records, available) = export.map_or_else(
        || (Vec::new(), false),
        |export| {
            let count = export.records.len();
            let records = eligible(export.records, data, target);
            let mut available = count != 0 && count == records.len();
            // A refused contract (and its dependents) is dropped on its own:
            // the other packages keep theirs, as without an inventory. Before
            // this, one edited package whose script declares its whole
            // directory cleaned every script package, vstd included (#119).
            if let Some(sources) = export.sources {
                let source_state = local.with_file_name("source-inputs.json");
                // Admission validates every generated path against the
                // retained contracts; a refused inventory is not imported.
                let admitted = sources
                    .admit(data, &records, target)
                    .and_then(|inputs| sources::merge(&source_state, inputs));
                if let Some(inputs) = admitted {
                    // This is only an inventory. The caller subsequently reads
                    // every declared byte and acquires all original owners.
                    if super::super::files::json(&source_state, &inputs).is_err() {
                        available = false;
                    }
                } else {
                    available = false;
                }
            }
            (records, available)
        },
    );
    let admitted: BTreeSet<_> = records.iter().map(|r| r.package_id.clone()).collect();
    // Preserve earlier declarations only as observation roots, never as reuse
    // authorization. This keeps the original declarations-establishment next
    // run's inventory while forcing regeneration of every uncertified package.
    for mut prior in super::read::<Vec<BuildInput>>(local).unwrap_or_default() {
        if !admitted.contains(&prior.package_id) && package_matches(&prior, data) {
            prior.covered = false;
            prior.output = None;
            records.push(prior);
        }
    }
    super::super::files::json(local, &records)?;
    Ok(available)
}

/// The stamp of each path (`None`: absent).
pub(in crate::managed) type Stamps = BTreeMap<PathBuf, Option<PathStamp>>;

/// What changed across a Historical run, for [`scripts_unchanged_by_run`].
pub(in crate::managed) struct RunChanges<'a> {
    /// Content digests of the run's inputs before it.
    pub before: &'a BTreeMap<PathBuf, String>,
    /// Content digests of the run's inputs after it.
    pub after: &'a BTreeMap<PathBuf, String>,
    /// Every input path or directory whose `lstat` stamp differs across the
    /// run, or that only one side covers (`input::changed_stamps`).
    pub stamps: &'a BTreeMap<PathBuf, (Option<PathStamp>, Option<PathStamp>)>,
    /// Every path the input stamps cover after the run.
    pub stamped: &'a Stamps,
    /// Stamps of the parent directories of every package directory and of
    /// the workspace, before and after the run ([`parent_paths`]); `None`
    /// when either side could not be taken.
    pub parents: Option<(&'a Stamps, &'a Stamps)>,
    /// Cargo's workspace root.
    pub workspace: &'a Path,
    /// The packages (by name) this run cleaned with `cargo clean -p`, so
    /// that Cargo ran their build scripts again.
    pub cleaned: &'a BTreeSet<String>,
}

/// The directories whose stamps [`RunChanges::parents`] holds: the parent of
/// every package directory (as Cargo names it and canonical) and of the
/// workspace root.
pub(in crate::managed) fn parent_paths(data: &Value, workspace: &Path) -> BTreeSet<PathBuf> {
    let mut parents = BTreeSet::new();
    let roots = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| package["manifest_path"].as_str())
        .filter_map(|manifest| Path::new(manifest).parent())
        .chain(std::iter::once(workspace));
    for root in roots {
        for form in forms(root) {
            if let Some(parent) = form.parent() {
                parents.insert(parent.to_owned());
            }
        }
    }
    parents
}

fn forms(path: &Path) -> Vec<PathBuf> {
    let mut forms = vec![path.to_owned()];
    if let Ok(canonical) = path.canonicalize() {
        forms.push(canonical);
    }
    forms
}

/// Whether a run whose inputs changed (Historical) left every build-script
/// contract it observed intact, so it may still publish them (ADR 0046/0047
/// amendments). Every path whose content or `lstat` stamp differs across the
/// run must be explained, and no path may be, or lie inside, a recorded
/// script's declared path:
///
/// - a path inside `workspace` and outside every build-script package (a
///   log written into the project, #259); or
/// - the directory of a build-script package this run cleaned, which is the
///   same directory at both ends (device, inode and mode) and changed only in
///   its entries' bookkeeping: a script that creates and removes a temporary
///   entry where Cargo runs it, as rustix 0.38's compiler probe does (#294).
///   Every other path of that package is unchanged, its declared paths do not
///   include the directory, and its parent is unchanged, so the directory was
///   not renamed or replaced.
///
/// A changed directory above a declared path also needs an unchanged parent.
/// Declared variables are checked against the record when publishing.
/// Anything else returns false, keeping the export unpublished.
pub(in crate::managed) fn scripts_unchanged_by_run(
    local: &Path,
    data: &Value,
    changes: &RunChanges<'_>,
) -> bool {
    explained(local, data, changes, true)
}

/// Whether the only change across a run is build scripts this run cleaned
/// creating and removing temporary entries in their own package directories,
/// exactly as [`scripts_unchanged_by_run`]'s second case describes, with
/// every content digest unchanged. Nothing such a script did can be an input
/// of the run: an entry a compiler read is a recorded input that is now
/// missing (Unknown), the script's declared paths do not include the
/// directory, and nothing else in the package changed. Such a run is Current
/// (#294); every other stamp change keeps it Historical.
pub(in crate::managed) fn only_script_churn(
    local: &Path,
    data: &Value,
    changes: &RunChanges<'_>,
) -> bool {
    changes.before == changes.after
        && !changes.stamps.is_empty()
        && explained(local, data, changes, false)
}

/// Every build-script package's directory (as Cargo names it and canonical),
/// with the package's name; `None` if a script package has neither.
fn script_directories(data: &Value) -> Option<Vec<(PathBuf, &str)>> {
    let mut scripts = Vec::new();
    for package in data["packages"].as_array().into_iter().flatten() {
        let script = package["targets"].as_array().is_some_and(|targets| {
            targets.iter().any(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "custom-build"))
            })
        });
        if script {
            let root = package["manifest_path"]
                .as_str()
                .and_then(|manifest| Path::new(manifest).parent())?;
            let name = package["name"].as_str()?;
            scripts.extend(forms(root).into_iter().map(|form| (form, name)));
        }
    }
    Some(scripts)
}

fn explained(local: &Path, data: &Value, changes: &RunChanges<'_>, workspace_rule: bool) -> bool {
    let Some(records) = super::read::<Vec<BuildInput>>(local) else {
        return false;
    };
    if records.is_empty() || records.iter().any(|record| !record.covered) {
        return false;
    }
    let workspace = forms(changes.workspace);
    let Some(scripts) = script_directories(data) else {
        return false;
    };
    let declared: BTreeSet<PathBuf> = records
        .iter()
        .flat_map(|record| record.paths.iter().flat_map(|path| forms(path)))
        .collect();
    let parent_unchanged = |path: &Path| -> bool {
        let Some(parent) = path.parent() else {
            return false;
        };
        if changes.stamps.contains_key(parent) {
            return false;
        }
        if changes.stamped.contains_key(parent) {
            return true;
        }
        let Some((before, after)) = changes.parents else {
            return false;
        };
        matches!(
            (before.get(parent), after.get(parent)),
            (Some(Some(b)), Some(Some(a))) if b == a
        )
    };
    let in_workspace = |forms: &[PathBuf]| -> bool {
        forms
            .iter()
            .any(|form| workspace.iter().any(|root| form.starts_with(root)))
            && !forms
                .iter()
                .any(|form| scripts.iter().any(|(root, _)| form.starts_with(root)))
    };
    let content: BTreeSet<&PathBuf> = changes
        .before
        .iter()
        .filter(|(path, digest)| changes.after.get(*path) != Some(*digest))
        .map(|(path, _)| path)
        .chain(
            changes
                .after
                .keys()
                .filter(|path| !changes.before.contains_key(*path)),
        )
        .collect();
    if content.is_empty() && changes.stamps.is_empty() {
        // Nothing changed: not a Historical run this decides for.
        return false;
    }
    for path in &content {
        if !workspace_rule {
            return false;
        }
        let path_forms = forms(path);
        if path_forms
            .iter()
            .any(|form| declared.iter().any(|d| form.starts_with(d)))
            || !in_workspace(&path_forms)
        {
            return false;
        }
    }
    for (path, (before, after)) in changes.stamps {
        let path_forms = forms(path);
        if path_forms
            .iter()
            .any(|form| declared.iter().any(|d| form.starts_with(d)))
        {
            return false;
        }
        let directory = matches!((before, after), (Some(b), Some(a)) if b.same_directory(a));
        // A changed directory above a declared path: it was not renamed.
        if directory
            && declared.iter().any(|d| d != path && d.starts_with(path))
            && !parent_unchanged(path)
        {
            return false;
        }
        if workspace_rule && in_workspace(&path_forms) {
            continue;
        }
        let cleaned_root = scripts
            .iter()
            .any(|(root, name)| root == path && changes.cleaned.contains(*name));
        let inside_changed = changes
            .stamps
            .keys()
            .chain(content.iter().copied())
            .any(|other| other != path && other.starts_with(path));
        if !(directory && cleaned_root && !inside_changed && parent_unchanged(path)) {
            return false;
        }
    }
    true
}

/// Optional publication after the original final Current transaction. The
/// runner admits either successful verification or its separately sealed full
/// negative outcome; neither this export nor a negative outcome is a proof.
/// Encoding/publication failure leaves no reusable family export.
pub(in crate::managed) fn publish(
    path: &Path,
    compiler: &str,
    target: &Path,
    local: &Path,
    data: &Value,
) {
    let records = eligible(
        super::read::<Vec<BuildInput>>(local).unwrap_or_default(),
        data,
        target,
    );
    let sources = sources::Inventory::capture(
        &local.with_file_name("source-inputs.json"),
        data,
        &records,
        target,
    );
    publish_inventory(path, compiler, records, sources);
}

fn publish_inventory(
    path: &Path,
    compiler: &str,
    records: Vec<BuildInput>,
    sources: Option<sources::Inventory>,
) {
    let mut export = Export {
        version: 2,
        compiler: compiler.to_owned(),
        records,
        sources,
        checksum: String::new(),
    };
    let Some(sum) = checksum(&export) else {
        return;
    };
    export.checksum = sum;
    // Check the complete deterministic envelope before the original atomic
    // writer allocates its identical document. This bounds the new encoding,
    // not existing Cargo loaders or overall managed invocation memory/RSS.
    if encode(&export).is_some() {
        let _ = json(path, &export);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::PathBuf;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Fixture {
        root: PathBuf,
        target: PathBuf,
        input: PathBuf,
        local: PathBuf,
        export: PathBuf,
        data: Value,
    }
    impl Fixture {
        fn new(name: &str) -> std::io::Result<Self> {
            let root = std::env::temp_dir()
                .join(format!("sail-script-family-{name}-{}", std::process::id()));
            let target = root.join("target");
            std::fs::create_dir_all(&target)?;
            let input = root.join("external.txt");
            std::fs::write(&input, "good")?;
            let manifest = root.join("Cargo.toml");
            std::fs::write(&manifest, "[package]\nname='registry-script'\n")?;
            let data = serde_json::json!({"packages":[{
                "id":"registry-pkg", "name":"registry-script", "manifest_path":manifest,
                "source":"registry+https://example.invalid/index", "targets":[{"kind":["custom-build"]}]
            }]});
            Ok(Self {
                local: root.join("context-a.json"),
                export: root.join("family.json"),
                root,
                target,
                input,
                data,
            })
        }
        fn record(&self, unit: &str, input: &Path, keys: &[&str]) -> std::io::Result<BuildInput> {
            let output = self.target.join(unit).join("output");
            std::fs::create_dir_all(output.parent().unwrap_or(&self.target))?;
            let mut text = format!("cargo:rerun-if-changed={}\n", input.display());
            for key in keys {
                let _ = writeln!(text, "cargo:rerun-if-env-changed={key}");
            }
            std::fs::write(&output, &text)?;
            let mut record = BuildInput {
                package_id: "registry-pkg".to_owned(),
                package_name: "registry-script".to_owned(),
                manifest: self.root.join("Cargo.toml"),
                paths: vec![input.to_owned()],
                env: keys
                    .iter()
                    .map(|k| ((*k).to_owned(), super::super::effective_value(k, None)))
                    .collect(),
                covered: true,
                digest: String::new(),
                output: Some((output, digest(text.as_bytes()))),
            };
            record.digest = current(&record, None).0;
            Ok(record)
        }
        fn export(&self, records: &[BuildInput]) -> Result<()> {
            json(&self.local, &records)?;
            publish(
                &self.export,
                "compiler",
                &self.target,
                &self.local,
                &self.data,
            );
            Ok(())
        }
        fn begin(&self, local: &Path, compiler: &str) -> Result<()> {
            begin(&self.export, compiler, &self.target, local, &self.data).map(|_| ())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    /// A workspace with one member and a registry package with a build script
    /// that declares a workspace file.
    struct Run {
        root: PathBuf,
        workspace: PathBuf,
        script: PathBuf,
        declared: PathBuf,
        local: PathBuf,
        data: Value,
        /// Whether the last [`Run::unchanged_with`] run would be Current.
        current: std::cell::Cell<bool>,
    }
    impl Run {
        fn new(name: &str, covered: bool) -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let root =
                std::env::temp_dir().join(format!("sail-script-run-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let workspace = root.join("ws");
            let script = root.join("registry/pkg-1.0.0");
            std::fs::create_dir_all(workspace.join("src"))?;
            std::fs::create_dir_all(&script)?;
            let declared = workspace.join("shared.txt");
            let data = serde_json::json!({"packages":[
                {"id":"member","name":"member","manifest_path":workspace.join("Cargo.toml"),
                 "targets":[{"kind":["lib"]}]},
                {"id":"pkg","name":"pkg","manifest_path":script.join("Cargo.toml"),
                 "targets":[{"kind":["lib"]},{"kind":["custom-build"]}]}
            ]});
            let record = BuildInput {
                package_id: "pkg".to_owned(),
                package_name: "pkg".to_owned(),
                manifest: script.join("Cargo.toml"),
                paths: vec![declared.clone()],
                env: BTreeMap::from([("PKG_CONFIG".to_owned(), None)]),
                digest: "recorded".to_owned(),
                covered,
                output: None,
            };
            let local = root.join("context.json");
            json(&local, &[record])?;
            Ok(Self {
                root,
                workspace,
                script,
                declared,
                local,
                data,
                current: std::cell::Cell::new(false),
            })
        }
        fn unchanged(&self, changed: &[PathBuf]) -> bool {
            self.unchanged_with(changed, &BTreeMap::new(), &BTreeSet::new(), &[])
        }
        /// `changed` content, `stamps` changed stamps, `cleaned` packages and
        /// the parent directories whose stamps changed across the run.
        fn unchanged_with(
            &self,
            changed: &[PathBuf],
            stamps: &BTreeMap<PathBuf, (Option<PathStamp>, Option<PathStamp>)>,
            cleaned: &BTreeSet<String>,
            moved_parents: &[PathBuf],
        ) -> bool {
            let before: BTreeMap<PathBuf, String> = [
                self.workspace.join("src/lib.rs"),
                self.declared.clone(),
                self.script.join("build.rs"),
            ]
            .into_iter()
            .map(|path| (path, "before".to_owned()))
            .collect();
            let mut after = before.clone();
            for path in changed {
                after.insert(path.clone(), "after".to_owned());
            }
            let parents_before: BTreeMap<PathBuf, Option<PathStamp>> =
                parent_paths(&self.data, &self.workspace)
                    .into_iter()
                    .map(|path| (path, Some(stamp(true, 1))))
                    .collect();
            let mut parents_after = parents_before.clone();
            for parent in moved_parents {
                parents_after.insert(parent.clone(), Some(stamp(true, 2)));
            }
            let stamped: BTreeMap<PathBuf, Option<PathStamp>> = stamps
                .iter()
                .map(|(path, (_, after))| (path.clone(), *after))
                .collect();
            let run_changes = RunChanges {
                before: &before,
                after: &after,
                stamps,
                stamped: &stamped,
                parents: Some((&parents_before, &parents_after)),
                workspace: &self.workspace,
                cleaned,
            };
            let historical = scripts_unchanged_by_run(&self.local, &self.data, &run_changes);
            // A run that is Current through script churn alone also keeps
            // every contract.
            let current = only_script_churn(&self.local, &self.data, &run_changes);
            assert!(!current || historical, "current without intact contracts");
            self.current.set(current);
            historical
        }
        /// A directory that gained and lost an entry across the run.
        fn churned(path: &Path) -> (PathBuf, (Option<PathStamp>, Option<PathStamp>)) {
            (
                path.to_owned(),
                (Some(stamp(true, 1)), Some(stamp(true, 2))),
            )
        }
    }
    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn historical_run_publishes_only_when_no_script_input_changed() -> TestResult {
        let run = Run::new("changes", true)?;
        let log = run.workspace.join("verify.log");
        // A log written into the workspace, or a member's own source, leaves
        // every script contract intact.
        assert!(run.unchanged(std::slice::from_ref(&log)));
        assert!(run.unchanged(&[run.workspace.join("src/lib.rs")]));
        // Changed: a script's declared path, any file of a script package,
        // anything outside the workspace (registry sources included).
        assert!(!run.unchanged(&[log.clone(), run.declared.clone()]));
        assert!(!run.unchanged(&[run.script.join("build.rs")]));
        assert!(!run.unchanged(&[run.script.join("src/lib.rs")]));
        assert!(!run.unchanged(&[run.root.join("registry/other-1.0.0/src/lib.rs")]));
        assert!(!run.unchanged(&[run.root.join("elsewhere.txt")]));
        // Nothing changed: not a Historical run this decides for.
        assert!(!run.unchanged(&[]));
        Ok(())
    }

    #[test]
    fn historical_run_never_publishes_unobserved_or_missing_contracts() -> TestResult {
        let uncovered = Run::new("uncovered", false)?;
        assert!(!uncovered.unchanged(&[uncovered.workspace.join("verify.log")]));
        let missing = Run::new("missing", true)?;
        std::fs::remove_file(&missing.local)?;
        assert!(!missing.unchanged(&[missing.workspace.join("verify.log")]));
        let mut nameless = Run::new("nameless", true)?;
        nameless.data["packages"][1]["manifest_path"] = Value::Null;
        assert!(!nameless.unchanged(&[nameless.workspace.join("verify.log")]));
        Ok(())
    }

    /// A stamp of a directory (or file) whose times are `time`.
    fn stamp(directory: bool, time: i64) -> PathStamp {
        PathStamp {
            dev: 1,
            ino: 7,
            size: 4096,
            mode: if directory { 0o040_755 } else { 0o100_644 },
            mtime: (time, 0),
            ctime: (time, 0),
        }
    }

    #[test]
    fn historical_run_publishes_after_a_cleaned_script_churned_its_own_directory() -> TestResult {
        // rustix 0.38's build script runs rustc with `-o -` in its package
        // directory, and rustc creates and removes a temporary `rmeta*`
        // directory there (#294). Content is unchanged; only that
        // directory's times are.
        let run = Run::new("churn", true)?;
        let cleaned = BTreeSet::from(["pkg".to_owned()]);
        let churn = BTreeMap::from([Run::churned(&run.script)]);
        assert!(run.unchanged_with(&[], &churn, &cleaned, &[]));
        // Nothing such a script did is an input: the run is Current.
        assert!(run.current.get());
        // Together with a log in the workspace, as before: Historical, but the
        // contracts are intact.
        let mut both = churn.clone();
        both.insert(
            run.workspace.clone(),
            (Some(stamp(true, 1)), Some(stamp(true, 2))),
        );
        assert!(run.unchanged_with(&[run.workspace.join("verify.log")], &both, &cleaned, &[]));
        assert!(!run.current.get());
        assert!(run.unchanged_with(&[], &both, &cleaned, &[]));
        assert!(!run.current.get());
        // Sail did not make Cargo run that script: someone else changed it.
        assert!(!run.unchanged_with(&[], &churn, &BTreeSet::new(), &[]));
        assert!(!run.unchanged_with(&[], &churn, &BTreeSet::from(["member".to_owned()]), &[]));
        // The directory may have been renamed or replaced.
        let registry = run.root.join("registry");
        assert!(!run.unchanged_with(&[], &churn, &cleaned, std::slice::from_ref(&registry)));
        let mut replaced = stamp(true, 2);
        replaced.ino = 8;
        let swapped =
            BTreeMap::from([(run.script.clone(), (Some(stamp(true, 1)), Some(replaced)))]);
        assert!(!run.unchanged_with(&[], &swapped, &cleaned, &[]));
        // A file, an absent side, or a path below the directory changed too.
        let file = BTreeMap::from([(
            run.script.clone(),
            (Some(stamp(false, 1)), Some(stamp(false, 2))),
        )]);
        assert!(!run.unchanged_with(&[], &file, &cleaned, &[]));
        let appeared = BTreeMap::from([(run.script.clone(), (None, Some(stamp(true, 2))))]);
        assert!(!run.unchanged_with(&[], &appeared, &cleaned, &[]));
        let mut below = churn.clone();
        below.insert(
            run.script.join("src"),
            (Some(stamp(true, 1)), Some(stamp(true, 2))),
        );
        assert!(!run.unchanged_with(&[], &below, &cleaned, &[]));
        assert!(!run.unchanged_with(&[run.script.join("build.rs")], &churn, &cleaned, &[]));
        // Another package's directory, though cleaned.
        let other = BTreeMap::from([Run::churned(&run.root.join("registry/other-1.0.0"))]);
        assert!(!run.unchanged_with(&[], &other, &cleaned, &[]));
        Ok(())
    }

    #[test]
    fn historical_run_never_publishes_a_directory_a_script_declared() -> TestResult {
        // With no declarations Cargo watches the whole package: a temporary
        // entry there may be what the script read.
        let run = Run::new("whole", true)?;
        let mut records: Vec<BuildInput> = super::super::read(&run.local).ok_or("records")?;
        records[0].paths = vec![run.script.clone()];
        json(&run.local, &records)?;
        let cleaned = BTreeSet::from(["pkg".to_owned()]);
        let churn = BTreeMap::from([Run::churned(&run.script)]);
        assert!(!run.unchanged_with(&[], &churn, &cleaned, &[]));
        Ok(())
    }

    #[test]
    fn historical_run_checks_workspace_stamps_like_workspace_content() -> TestResult {
        let run = Run::new("stamps", true)?;
        let none = BTreeSet::new();
        // A member source touched or restored: its stamp alone changed.
        let touched = BTreeMap::from([(
            run.workspace.join("src/lib.rs"),
            (Some(stamp(false, 1)), Some(stamp(false, 2))),
        )]);
        assert!(run.unchanged_with(&[], &touched, &none, &[]));
        assert!(!run.current.get());
        // A declared path restored during the run is not intact.
        let declared = BTreeMap::from([(
            run.declared.clone(),
            (Some(stamp(false, 1)), Some(stamp(false, 2))),
        )]);
        assert!(!run.unchanged_with(&[], &declared, &none, &[]));
        // The workspace directory gained a log; the declared file inside it is
        // intact as long as the workspace directory itself was not moved.
        let log = run.workspace.join("verify.log");
        let workspace = BTreeMap::from([Run::churned(&run.workspace)]);
        assert!(run.unchanged_with(std::slice::from_ref(&log), &workspace, &none, &[]));
        assert!(!run.unchanged_with(
            std::slice::from_ref(&log),
            &workspace,
            &none,
            std::slice::from_ref(&run.root)
        ));
        // A script package's file, though only its stamp changed.
        let script = BTreeMap::from([(
            run.script.join("src/lib.rs"),
            (Some(stamp(false, 1)), Some(stamp(false, 2))),
        )]);
        assert!(!run.unchanged_with(&[], &script, &none, &[]));
        Ok(())
    }

    #[test]
    fn selector_reuses_script_contract_but_consumes_generation_before_any_mutation() -> TestResult {
        let f = Fixture::new("selector")?;
        f.export(&[f.record("host", &f.input, &[])?])?;
        let other = f.root.join("context-b.json");
        f.begin(&other, "compiler")?;
        assert!(!f.export.exists());
        assert_eq!(super::super::stale(&other, &f.data).0, Vec::<String>::new());
        // B rewrites shared output then fails. Even restoring A's external input
        // and mtime cannot revive the consumed generation or old local A ledger.
        let file = std::fs::File::open(&f.input)?;
        let times = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        std::fs::write(f.target.join("host/output"), "cargo:rustc-cfg=wrong\n")?;
        std::fs::write(&f.input, "good")?;
        file.set_times(std::fs::FileTimes::new().set_modified(times))?;
        f.begin(&f.local, "compiler")?;
        let (clean, _, complete) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, vec!["registry-script"]);
        assert!(!complete);
        Ok(())
    }
    #[test]
    fn post_watch_recheck_cleans_restored_mtime_change_before_cached_output_use() -> TestResult {
        let f = Fixture::new("post-watch")?;
        f.export(&[f.record("host", &f.input, &[])?])?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        let file = std::fs::File::open(&f.input)?;
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        Ok(())
    }
    #[test]
    fn both_output_units_survive_update_and_an_earlier_input_change_refuses_package() -> TestResult
    {
        let f = Fixture::new("units")?;
        let second = f.root.join("target-input.txt");
        std::fs::write(&second, "also")?;
        let records = [
            f.record("host", &f.input, &[])?,
            f.record("target", &second, &[])?,
        ];
        let artifacts = f.root.join("artifacts.jsonl");
        let mut text = String::new();
        for record in &records {
            let output = record.output.as_ref().map_or(&f.target, |(path, _)| path);
            writeln!(
                text,
                "{}",
                serde_json::json!({"reason":"build-script-executed", "package_id":record.package_id,
                "out_dir":output.parent().unwrap_or(&f.target).join("out")})
            )?;
        }
        std::fs::write(&artifacts, text)?;
        super::super::update(&f.local, &artifacts, &f.data)?;
        let updated: Vec<BuildInput> = super::super::read(&f.local).ok_or("unit ledger missing")?;
        assert_eq!(updated.len(), 2);
        publish(&f.export, "compiler", &f.target, &f.local, &f.data);
        f.begin(&f.local, "compiler")?;
        let file = std::fs::File::open(&f.input)?;
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        assert!(super::super::read::<Vec<BuildInput>>(&f.local).is_some_and(|v| v.is_empty()));
        Ok(())
    }
    #[test]
    fn content_contract_output_registry_and_compiler_changes_cannot_import() -> TestResult {
        let f = Fixture::new("bindings")?;
        let pristine = f.record("host", &f.input, &[])?;
        for field in [
            "input",
            "declarations",
            "output",
            "registry",
            "compiler",
            "incomplete",
            "duplicate",
            "old-version",
            "checksum",
        ] {
            std::fs::write(&f.input, "good")?;
            let record = f.record("host", &f.input, &[])?;
            let records = if field == "duplicate" {
                vec![record.clone(), record]
            } else {
                vec![record]
            };
            f.export(&records)?;
            let mut data = f.data.clone();
            match field {
                "input" => std::fs::write(&f.input, "fail")?,
                "declarations" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.records[0].paths.clear();
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "output" => {
                    std::fs::write(f.target.join("host/output"), "cargo:rustc-cfg=changed\n")?;
                }
                "registry" => {
                    data["packages"][0]["id"] = Value::String("different-registry-pkg".to_owned());
                }
                "incomplete" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.records[0].covered = false;
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "old-version" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.version = 1;
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "checksum" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.checksum = "invalid".to_owned();
                    json(&f.export, &export)?;
                }
                _ => {}
            }
            begin(
                &f.export,
                if field == "compiler" {
                    "different-tool-family"
                } else {
                    "compiler"
                },
                &f.target,
                &f.local,
                &data,
            )?;
            assert!(
                super::super::read::<Vec<BuildInput>>(&f.local)
                    .is_some_and(|v| v.iter().all(|r| !r.covered)),
                "refusal: {field}"
            );
        }
        assert!(pristine.covered);
        Ok(())
    }
    #[test]
    fn effective_bootstrap_is_bound_and_unaudited_injected_environment_refuses_reuse() -> TestResult
    {
        let f = Fixture::new("environment")?;
        let record = f.record("host", &f.input, &["RUSTC_BOOTSTRAP"])?;
        let parent =
            BTreeMap::from([("RUSTC_BOOTSTRAP".to_owned(), "different-parent".to_owned())]);
        assert_eq!(current(&record, Some(&parent)), current(&record, None));
        assert_eq!(record.env["RUSTC_BOOTSTRAP"].as_deref(), Some("1"));
        let ordinary = f.record("env", &f.input, &["LANG"])?;
        let changed =
            BTreeMap::from([("LANG".to_owned(), "sail-changed-language-input".to_owned())]);
        assert_ne!(current(&ordinary, Some(&changed)).0, ordinary.digest);
        f.export(&[record])?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        for key in [
            "OUT_DIR",
            "HOST",
            "TARGET",
            "PROFILE",
            "OPT_LEVEL",
            "DEBUG",
            "NUM_JOBS",
            "SAIL_MANAGED_RUNTIME",
            "CARGO",
            "CARGO_PKG_VERSION",
            "CARGO_ENCODED_RUSTFLAGS",
            "VERUS_DRIVER_ARGS",
            "RUSTUP_TOOLCHAIN",
        ] {
            f.export(&[f.record("host", &f.input, &[key])?])?;
            f.begin(&f.local, "compiler")?;
            assert_eq!(
                super::super::stale(&f.local, &f.data).0,
                vec!["registry-script"],
                "declared {key}"
            );
        }
        // What Cargo injects for these is fixed by the unit and the family
        // (#294): rustix 0.38 declares both.
        for key in ["CARGO_FEATURE_USE_LIBC", "CARGO_CFG_MIRI"] {
            f.export(&[f.record("host", &f.input, &[key])?])?;
            f.begin(&f.local, "compiler")?;
            assert_eq!(
                super::super::stale(&f.local, &f.data).0,
                Vec::<String>::new(),
                "declared {key}"
            );
        }
        let mut record = f.record("host", &f.input, &["SAIL_TEST_FAMILY_INPUT"])?;
        record.env.insert(
            "SAIL_TEST_FAMILY_INPUT".to_owned(),
            Some("stale-value".to_owned()),
        );
        assert!(!matches(&record, &f.data, &f.target));
        Ok(())
    }

    #[test]
    fn caller_target_contracts_import_only_with_the_exact_declared_value() -> TestResult {
        let f = Fixture::new("caller-target")?;
        let record = f.record("host", &f.input, &["CARGO_TARGET_DIR"])?;
        assert!(matches(&record, &f.data, &f.target));
        f.export(std::slice::from_ref(&record))?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );

        let absent = BTreeMap::new();
        let present = BTreeMap::from([(
            "CARGO_TARGET_DIR".to_owned(),
            f.target.join("caller-selected").display().to_string(),
        )]);
        assert_ne!(
            current(&record, Some(&absent)).0,
            current(&record, Some(&present)).0
        );
        let changed = if record.env["CARGO_TARGET_DIR"].is_none() {
            &present
        } else {
            &absent
        };
        let mut stale = record;
        stale.digest = current(&stale, Some(changed)).0;
        assert!(!matches(&stale, &f.data, &f.target));
        f.export(&[stale])?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        Ok(())
    }
    #[test]
    fn malformed_oversize_and_nonregular_exports_discard_older_local_observations() -> TestResult {
        let f = Fixture::new("invalid")?;
        for text in [b"{".to_vec(), vec![b'x'; MAX_BYTES + 1]] {
            json(&f.local, &vec![f.record("host", &f.input, &[])?])?;
            std::fs::write(&f.export, text)?;
            f.begin(&f.local, "compiler")?;
            assert_eq!(
                super::super::stale(&f.local, &f.data).0,
                vec!["registry-script"]
            );
        }
        std::os::unix::fs::symlink(&f.local, &f.export)?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        let fifo = std::process::Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(&f.export)
            .status()?;
        assert!(fifo.success());
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        Ok(())
    }
    #[test]
    fn distinct_package_ids_with_one_name_cannot_hide_post_watch_invalidation() -> TestResult {
        let f = Fixture::new("same-name")?;
        let mut data = f.data.clone();
        let mut package = data["packages"][0].clone();
        package["id"] = Value::String("registry-pkg-other-version".to_owned());
        data["packages"]
            .as_array_mut()
            .ok_or("packages absent")?
            .push(package);
        let other_input = f.root.join("other.txt");
        std::fs::write(&other_input, "also")?;
        let mut second = f.record("other-version", &other_input, &[])?;
        second.package_id = "registry-pkg-other-version".to_owned();
        json(&f.local, &vec![f.record("host", &f.input, &[])?, second])?;
        std::fs::write(&f.input, "fail")?;
        assert_eq!(
            recheck(&f.local, &data, &f.target)?,
            vec!["registry-script"]
        );
        let retained: Vec<BuildInput> = super::super::read(&f.local).ok_or("local absent")?;
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].package_id, "registry-pkg-other-version");
        Ok(())
    }
    #[test]
    fn missing_generation_keeps_declaration_watch_roots_but_forces_fresh_script() -> TestResult {
        let f = Fixture::new("declaration-transition")?;
        let record = f.record("host", &f.input, &[])?;
        json(&f.local, &vec![record])?;
        f.begin(&f.local, "compiler")?;
        let (clean, paths, covered) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, vec!["registry-script"]);
        assert_eq!(paths, vec![f.input.clone()]);
        assert!(!covered);
        let observation: Vec<BuildInput> =
            super::super::read(&f.local).ok_or("watch paths missing")?;
        assert!(observation.iter().all(|r| !r.covered && r.output.is_none()));
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        let fresh = f.record("host", &f.input, &[])?;
        let output = fresh
            .output
            .as_ref()
            .map(|(path, _)| path)
            .ok_or("output absent")?;
        let artifacts = f.root.join("artifacts.jsonl");
        std::fs::write(
            &artifacts,
            format!(
                "{}\n",
                serde_json::json!({
                    "reason":"build-script-executed", "package_id":fresh.package_id,
                    "out_dir":output.parent().ok_or("parent absent")?.join("out")
                })
            ),
        )?;
        super::super::update(&f.local, &artifacts, &f.data)?;
        let established: Vec<BuildInput> =
            super::super::read(&f.local).ok_or("fresh record absent")?;
        assert_eq!(established.len(), 1);
        assert!(established[0].covered && established[0].output.is_some());
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        publish(&f.export, "compiler", &f.target, &f.local, &f.data);
        assert!(f.export.is_file());
        f.begin(&f.local, "compiler")?;
        let (clean, paths, covered) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, Vec::<String>::new());
        assert_eq!(paths, vec![f.input.clone()]);
        assert!(covered);
        Ok(())
    }
    #[test]
    fn removed_dependency_does_not_force_cleaning_a_retired_package() -> TestResult {
        let f = Fixture::new("removed")?;
        json(&f.local, &vec![f.record("host", &f.input, &[])?])?;
        let data = serde_json::json!({"packages":[]});
        begin(&f.export, "compiler", &f.target, &f.local, &data)?;
        let (clean, paths, complete) = super::super::stale(&f.local, &data);
        assert!(clean.is_empty() && paths.is_empty() && complete);
        Ok(())
    }
    /// Issue #119: a script without declarations watches its whole package,
    /// so an edit refuses that package's contract. Only it and the packages
    /// that depend on it lose their contracts, even when the export carries a
    /// source inventory; an unrelated dependency (vstd) keeps its contract.
    #[test]
    fn edited_package_refuses_only_its_contract_and_its_dependents() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-script-family-edit-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        // The inventory admits only canonical paths (macOS /var is a link).
        let root = root.canonicalize()?;
        let target = root.join("cache/target");
        let mut packages = Vec::new();
        let mut records = Vec::new();
        let mut by_package = std::collections::BTreeMap::new();
        for name in ["edited", "dependent", "top", "vstd"] {
            let directory = root.join(name);
            std::fs::create_dir_all(directory.join("src"))?;
            let manifest = directory.join("Cargo.toml");
            std::fs::write(&manifest, format!("[package]\nname='{name}'\n"))?;
            let source = directory.join("src/lib.rs");
            std::fs::write(&source, "pub fn f() {}\n")?;
            let output = target.join(format!("build/{name}/output"));
            std::fs::create_dir_all(output.parent().ok_or("unit")?.join("out"))?;
            // No declarations: Cargo watches the whole package directory.
            std::fs::write(&output, "")?;
            let mut record = BuildInput {
                package_id: name.to_owned(),
                package_name: name.to_owned(),
                manifest: manifest.clone(),
                paths: vec![directory.clone()],
                env: BTreeMap::new(),
                digest: String::new(),
                covered: true,
                output: Some((output, digest(b""))),
            };
            record.digest = current(&record, None).0;
            records.push(record);
            by_package.insert(name.to_owned(), vec![source]);
            packages.push(
                serde_json::json!({"id":name, "name":name, "version":"0.0.0",
                "source":null, "manifest_path":manifest,
                "targets":[{"kind":["lib"]},{"kind":["custom-build"]}]}),
            );
        }
        let data = serde_json::json!({"sail_cache_directory":root.join("cache"),
            "packages":packages, "resolve":{"root":null,"nodes":[
                // `top` reaches `edited` only through `dependent` and is
                // listed first, so the refusal needs a second pass.
                {"id":"top","dependencies":["dependent"]},
                {"id":"edited","dependencies":["vstd"]},
                {"id":"dependent","dependencies":["edited","vstd"]},
                {"id":"vstd","dependencies":[]}]}});
        let local = root.join("context/build-inputs.json");
        std::fs::create_dir_all(local.parent().ok_or("context")?)?;
        json(&local, &records)?;
        let source_state = local.with_file_name("source-inputs.json");
        let paths: Vec<PathBuf> = by_package.values().flatten().cloned().collect();
        json(
            &source_state,
            &crate::managed::source_inputs::Inputs {
                paths,
                covered: true,
                by_package: Some(by_package),
            },
        )?;
        let export = root.join("family.json");
        publish(&export, "compiler", &target, &local, &data);
        let published = read(&export).ok_or("export missing")?;
        assert!(published.sources.is_some(), "the inventory is exported");
        assert_eq!(published.records.len(), 4);
        // A comment-only edit to the package with the undeclared script.
        std::fs::write(root.join("edited/src/lib.rs"), "pub fn f() {}\n// edit\n")?;
        begin(&export, "compiler", &target, &local, &data)?;
        let (clean, _, covered) = super::super::stale(&local, &data);
        assert_eq!(clean, vec!["dependent", "edited", "top"]);
        assert!(!covered);
        let kept: Vec<BuildInput> = super::super::read(&local).ok_or("local missing")?;
        assert!(
            kept.iter()
                .any(|r| r.package_id == "vstd" && r.covered && r.output.is_some()),
            "the unrelated contract survives"
        );
        assert!(
            kept.iter()
                .filter(|r| r.package_id != "vstd")
                .all(|r| !r.covered && r.output.is_none())
        );
        // A change to the shared dependency refuses everything above it.
        for record in &mut records {
            record.digest = current(record, None).0;
        }
        json(&local, &records)?;
        publish(&export, "compiler", &target, &local, &data);
        std::fs::write(root.join("vstd/src/lib.rs"), "pub fn g() {}\n")?;
        begin(&export, "compiler", &target, &local, &data)?;
        assert_eq!(
            super::super::stale(&local, &data).0,
            vec!["dependent", "edited", "top", "vstd"]
        );
        // Without a resolve graph only the refused package itself is known.
        assert_eq!(
            dependents(&serde_json::json!({}), BTreeSet::from(["vstd".to_owned()])),
            BTreeSet::from(["vstd".to_owned()])
        );
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }
}
