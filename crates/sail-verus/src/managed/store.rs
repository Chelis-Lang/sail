//! Machine-local store of verified registry dependencies (ADR 0045).
//!
//! The first managed run on a machine that verifies a registry dependency
//! such as `vstd` copies that dependency's compiled Cargo units (every unit of
//! its dependency closure, with Cargo's fingerprints), its proof receipt, its
//! build-script contracts and the verifier's recorded output into
//! `<cache>/dependency-store/v1/<key>/`. A later managed run that starts a new
//! Cargo target (a new project, worktree or sandbox) with the same key copies
//! the units in before Cargo runs. Cargo's own fingerprints then decide what is
//! fresh, exactly as for a target directory plain Cargo-Verus had built
//! itself; a seeded unit Cargo does not accept is rebuilt and verified as
//! usual. A seeded target is reported as reused only when the store's receipt
//! passes every receipt check against the bytes now in the new target.
//!
//! Entries are copied into a hidden sibling of the new target and renamed into
//! place, so a killed run never leaves a partial target. Every failure leaves
//! no target and verification proceeds as usual; only a damaged entry (its own
//! checksums or digests fail) prints `warning[SAIL3131]`, and a missing,
//! evicted or mismatched one is a silent miss. Store problems never change a
//! run's result. The store never changes a solver query, a verdict or a
//! diagnostic: it only lets Cargo find a dependency fresh.
//!
//! The v1 native-artifact path is disabled. Its
//! identity does not establish the native compiler/linker inputs that fresh
//! Cargo would read (#187). The format below remains available for investigation;
//! managed runs neither seed nor publish entries until that path is qualified.

use super::files::{digest, json};
use super::receipts::{RECEIPT_VERSION, Receipt, complete, digests_match};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Layout version of the store directory and its manifests.
const VERSION: u32 = 1;
/// Entries kept by default; `SAIL_DEPENDENCY_STORE_ENTRIES` overrides it, and
/// `0` disables the store.
const DEFAULT_ENTRIES: usize = 4;

/// The store directory under the Sail cache.
pub(super) fn root(cache: &Path) -> PathBuf {
    cache.join("dependency-store").join(format!("v{VERSION}"))
}

fn capacity() -> usize {
    std::env::var("SAIL_DEPENDENCY_STORE_ENTRIES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ENTRIES)
}

/// Whether native-artifact store reuse is qualified for managed runs.
///
/// V1 can suppress a native compilation failure after the linker changes at
/// an unchanged `PATH` (#187). Disable both reading and publication regardless
/// of the capacity setting until complete native input identity is qualified.
pub(super) fn enabled() -> bool {
    false
}

// Retain the loader-input controls while the native-artifact path is disabled.
#[cfg(test)]
fn loader_variables(keys: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    keys.into_iter().any(|key| {
        let key = key.as_encoded_bytes();
        key.starts_with(b"LD_") || key.starts_with(b"DYLD_")
    })
}

/// A registry dependency whose verification can be stored and reused.
#[derive(Debug, Clone)]
pub(super) struct Candidate {
    /// Cargo package id of the verified dependency (for example `vstd`).
    pub root: String,
    /// It and every package it reaches through any dependency kind.
    pub closure: BTreeSet<String>,
    /// The store key: everything that determines its build and verification.
    pub key: String,
}

/// The verified dependencies of this selection that the store may hold:
/// packages that declare `package.metadata.verus.verify = true`, are not
/// workspace members or selected targets, and whose whole dependency closure
/// comes from a registry or git source (never a local path, whose bytes are
/// per-workspace). The key binds the compiler family (its arguments other
/// than package and target selection, toolchain, runtime and engine identities, relevant environment and Cargo
/// configuration bytes), the root, the resolved closure (package ids, enabled
/// features and dependency edges) and the workspace's `[profile]` table.
pub(super) fn candidates(
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
    family: &str,
) -> Vec<Candidate> {
    let packages: BTreeMap<&str, &Value> = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["id"].as_str()?, p)))
        .collect();
    let members = workspace_members(data);
    let Some(profile) = profile_table(data) else {
        return Vec::new();
    };
    let selected =
        super::runner::dependency_closure(data, expected.iter().map(|(id, _, _)| id.as_str()));
    let mut out = Vec::new();
    for (id, package) in &packages {
        if members.contains(id)
            || expected.iter().any(|(selected, _, _)| selected == id)
            || !selected.contains(*id)
            || package
                .pointer("/metadata/verus/verify")
                .and_then(Value::as_bool)
                != Some(true)
        {
            continue;
        }
        let closure = super::runner::dependency_closure(data, [*id]);
        let Some(nodes) = foreign_nodes(data, &members, &closure) else {
            continue;
        };
        // An entry records one verified root's receipt and displayed output.
        // Copying a second verified root's artifacts without its own complete
        // entry could suppress that root's report or diagnostics. Store the
        // leaf proof instead; the dependent root then verifies normally.
        if closure.iter().any(|member| {
            member.as_str() != *id
                && packages
                    .get(member.as_str())
                    .and_then(|package| package.pointer("/metadata/verus/verify"))
                    .and_then(Value::as_bool)
                    == Some(true)
        }) {
            continue;
        }
        let Ok(bytes) =
            serde_json::to_vec(&("sail-dependency-store-v1", family, id, &nodes, &profile))
        else {
            continue;
        };
        out.push(Candidate {
            root: (*id).to_owned(),
            closure,
            key: digest(&bytes),
        });
    }
    out
}

/// The resolve nodes (package id, enabled features, dependency edges) of
/// every package of `closure`, in package-id order, when the whole closure
/// comes from registry or git sources: no workspace member and no local path,
/// whose bytes are per workspace. `None` otherwise, or when a package or its
/// resolve node is missing.
fn foreign_nodes<'a>(
    data: &'a Value,
    members: &BTreeSet<&str>,
    closure: &BTreeSet<String>,
) -> Option<Vec<&'a Value>> {
    let packages = data["packages"].as_array()?;
    let foreign = !closure.is_empty()
        && closure.iter().all(|member| {
            !members.contains(member.as_str())
                && packages.iter().any(|p| {
                    p["id"].as_str() == Some(member.as_str())
                        && p["source"].as_str().is_some_and(|s| !s.is_empty())
                })
        });
    if !foreign {
        return None;
    }
    let resolve = data["resolve"]["nodes"].as_array()?;
    let nodes: Vec<&Value> = closure
        .iter()
        .filter_map(|member| resolve.iter().find(|n| n["id"].as_str() == Some(member)))
        .collect();
    (nodes.len() == closure.len()).then_some(nodes)
}

/// What the workspace decides for a dependency whose whole closure comes
/// from registry or git sources (such as vstd), as a digest: how Cargo
/// resolved the closure (its resolve nodes) and the workspace's `[profile]`
/// table. The workspace manifest and lockfile reach such a closure only
/// through these; Cargo-Verus derives a dependency's verifier arguments from
/// its own arguments and the closure's own metadata, never from the
/// workspace (ADR 0045 decision 3). The closure's bytes are bound separately.
/// So adding an unrelated dependency to the workspace leaves it unchanged
/// (#177), while a different resolution of the closure, a feature it gains
/// or a profile change does not. `None` when the closure is not entirely
/// foreign, or when a node or the workspace manifest cannot be read.
pub(super) fn foreign_closure_binding(data: &Value, closure: &BTreeSet<String>) -> Option<String> {
    let nodes = foreign_nodes(data, &workspace_members(data), closure)?;
    let profile = profile_table(data)?;
    let bytes = serde_json::to_vec(&("sail-foreign-closure-v1", &nodes, &profile)).ok()?;
    Some(digest(&bytes))
}

fn workspace_members(data: &Value) -> BTreeSet<&str> {
    data["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// The workspace manifest's `[profile]` table, which Cargo applies to every
/// package it builds. `None` when the manifest cannot be read or parsed.
fn profile_table(data: &Value) -> Option<Value> {
    let root = Path::new(data["workspace_root"].as_str()?).join("Cargo.toml");
    let text = std::fs::read_to_string(root).ok()?;
    let manifest: toml::Table = toml::from_str(&text).ok()?;
    Some(
        manifest
            .get("profile")
            .and_then(|profile| serde_json::to_value(profile).ok())
            .unwrap_or(Value::Null),
    )
}

/// The content revision of a candidate's closure: every file of every closure
/// package as Cargo resolved it (registry and git sources), independent of the
/// workspace that uses them. `None` if any file cannot be read.
fn revision(data: &Value, candidate: &Candidate) -> Option<String> {
    let packages: Vec<Value> = data["packages"]
        .as_array()?
        .iter()
        .filter(|p| {
            p["id"]
                .as_str()
                .is_some_and(|id| candidate.closure.contains(id))
        })
        .cloned()
        .collect();
    let root = packages
        .iter()
        .find(|p| p["id"].as_str() == Some(candidate.root.as_str()))?;
    let directory = Path::new(root["manifest_path"].as_str()?).parent()?;
    let view = serde_json::json!({
        "workspace_root": directory,
        "packages": packages,
    });
    let snapshot = super::input::snapshot_metadata_home(&view, &[], None);
    snapshot.complete.then_some(snapshot.revision)
}

/// One stored file, relative to the Cargo target directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredFile {
    path: PathBuf,
    sha256: String,
    size: u64,
    /// Modification time (seconds, nanoseconds since the epoch). Cargo
    /// compares output times between units, so seeding restores them.
    modified: (u64, u32),
    executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    key: String,
    root: String,
    revision: String,
    /// The establishing run's Cargo target directory.
    target: PathBuf,
    receipt: Receipt,
    /// Build-script contracts of the closure's packages.
    contracts: Vec<Value>,
    directories: Vec<PathBuf>,
    files: Vec<StoredFile>,
    /// The verifier's own output for the root, as that run captured it.
    recorded: Vec<StoredFile>,
    checksum: String,
}

impl Manifest {
    fn seal(mut self) -> Self {
        self.checksum = String::new();
        self.checksum = digest(&serde_json::to_vec(&self).unwrap_or_default());
        self
    }
    fn intact(&self) -> bool {
        self.clone().seal().checksum == self.checksum
    }
}

/// A store entry placed in this run's new Cargo target.
#[derive(Debug, Clone)]
pub(super) struct Seeded {
    pub key: String,
    /// The entry's receipt, its paths moved into this run's target.
    pub receipt: Receipt,
    /// True when this run copied the entry into its new target, so plain
    /// Cargo-Verus would have verified the dependency in this invocation.
    pub placed: bool,
    /// Exact displayed stdout of a root placed into this run's new target.
    /// Empty for an attached receipt, whose existing target prints nothing.
    pub shown_stdout: Vec<u8>,
}

fn modified(meta: &std::fs::Metadata) -> Option<(u64, u32)> {
    let time = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((time.as_secs(), time.subsec_nanos()))
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
}

/// Copies `from` to a new file `to`, hashing the bytes written, and gives it
/// the recorded permissions and modification time. The copy is written under
/// a temporary name and renamed, so `to` never holds partial content.
fn copy(from: &Path, to: &Path, executable: bool, modified: (u64, u32)) -> Result<(String, u64)> {
    let failed = |e: std::io::Error| Error::new("SAIL3131", format!("{}: {e}", to.display()));
    let temporary = to.with_file_name(format!(
        ".{}.sail-store-{}",
        to.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let mut input = std::fs::File::open(from)
        .map_err(|e| Error::new("SAIL3131", format!("{}: {e}", from.display())))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if executable { 0o755 } else { 0o644 });
    }
    let _ = std::fs::remove_file(&temporary);
    let mut output = options.open(&temporary).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    let outcome = (|| -> std::io::Result<()> {
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            output.write_all(&buffer[..n])?;
            size += n as u64;
        }
        output.set_modified(UNIX_EPOCH + Duration::new(modified.0, modified.1))?;
        drop(output);
        std::fs::rename(&temporary, to)
    })();
    if let Err(e) = outcome {
        let _ = std::fs::remove_file(&temporary);
        return Err(failed(e));
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}

/// Removes what a failed seed placed, newest first.
fn rollback(files: &[PathBuf], directories: &[PathBuf]) {
    for file in files.iter().rev() {
        let _ = std::fs::remove_file(file);
    }
    for directory in directories.iter().rev() {
        let _ = std::fs::remove_dir(directory);
    }
}

fn relocate(path: &Path, from: &Path, to: &Path) -> PathBuf {
    path.strip_prefix(from)
        .map_or_else(|_| path.to_owned(), |relative| to.join(relative))
}

/// Why an entry was not used. Only a damaged entry is reported; everything
/// else is an ordinary miss, as when no entry exists (ADR 0045 decision 9).
#[derive(Debug)]
enum Refusal {
    /// The entry is gone (evicted or deleted), of another layout, or no
    /// longer describes this machine's inputs.
    Miss,
    /// An entry whose key matched fails its own checksums or digests.
    Damaged(String),
}

fn damaged(what: &str) -> Refusal {
    Refusal::Damaged(what.to_owned())
}

/// Leftovers of seeds that never finished (a killed run): hidden siblings of
/// `target`. Seeding runs under the compiler family's exclusive lock, so no
/// other run is writing them.
fn staging_dirs(target: &Path) -> Vec<PathBuf> {
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Vec::new();
    };
    let prefix = format!(".{}.seed-", name.to_string_lossy());
    std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| e.path())
        .collect()
}

/// Places every stored entry of `candidates` in `target`, a Cargo target this
/// run creates. Entries are copied into a hidden sibling directory, which is
/// renamed to `target` only once every entry that could be placed is
/// complete, so a run killed while seeding leaves no partial target (only a
/// hidden leftover, removed by the next seed). Returns the entries placed. A
/// missing or mismatched entry is an ordinary miss; a damaged one prints one
/// warning. Either way verification proceeds as usual.
pub(super) fn seed(
    cache: &Path,
    target: &Path,
    candidates: &[Candidate],
    data: &Value,
    err: &mut dyn Write,
) -> Vec<Seeded> {
    for leftover in staging_dirs(target) {
        let _ = std::fs::remove_dir_all(leftover);
    }
    reclaim(&root(cache), STALE);
    let available: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| root(cache).join(&c.key).join("manifest.json").is_file())
        .collect();
    // The caller decided before its cleanups that the target is new; those
    // cleanups can leave Cargo's bookkeeping files in it, but never a unit.
    if available.is_empty() || holds_units(target) {
        return Vec::new();
    }
    let staging = target.with_file_name(format!(
        ".{}.seed-{}-{}",
        target.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    if std::fs::create_dir_all(&staging).is_err() {
        return Vec::new();
    }
    let mut placed = Vec::new();
    for candidate in available {
        let entry = root(cache).join(&candidate.key);
        let mut files = Vec::new();
        let mut directories = Vec::new();
        let outcome = validated(&entry, candidate, data).and_then(|manifest| {
            let stdout = shown_stdout(&entry, &manifest)?;
            place(&entry, &staging, manifest, &mut files, &mut directories)
                .map(|manifest| (manifest, stdout))
        });
        match outcome {
            Ok((manifest, stdout)) => {
                placed.push((candidate, entry, manifest, files.len(), stdout));
            }
            Err(refusal) => {
                rollback(&files, &directories);
                if let Refusal::Damaged(reason) = refusal {
                    let _ = writeln!(
                        err,
                        "warning[SAIL3131]: ignoring the dependency store entry for {}: {reason}; verifying it normally",
                        candidate.root
                    );
                }
            }
        }
    }
    if placed.is_empty() {
        let _ = std::fs::remove_dir_all(&staging);
        return Vec::new();
    }
    // A target holding only Cargo's bookkeeping is moved aside (to a name the
    // next seed removes) so the staged one can take its place in one rename.
    // A crash in between leaves no target, which is a new target again.
    if target.exists() {
        let mut aside = staging.clone().into_os_string();
        aside.push("-old");
        if std::fs::rename(target, &aside).is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            return Vec::new();
        }
        let _ = std::fs::remove_dir_all(aside);
    }
    if std::fs::rename(&staging, target).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return Vec::new();
    }
    placed
        .into_iter()
        .filter_map(|(candidate, entry, manifest, files, shown_stdout)| {
            // The receipt is checked against the bytes now in the target.
            let receipt = moved_receipt(&manifest, target)?;
            // Recency for eviction; failure to record it changes nothing.
            let _ = std::fs::write(entry.join("used"), []);
            super::files::event(&serde_json::json!({
                "event": "dependency-seeded", "package_id": candidate.root,
                "key": candidate.key, "files": files,
            }));
            Some(Seeded {
                key: candidate.key.clone(),
                receipt,
                placed: true,
                shown_stdout,
            })
        })
        .collect()
}

/// Validates the exact displayed output before a seed can place any files.
/// Older entries lack it and safely miss; output damage warns and verifies.
fn shown_stdout(entry: &Path, manifest: &Manifest) -> std::result::Result<Vec<u8>, Refusal> {
    let read = |name| {
        let file = manifest
            .recorded
            .iter()
            .find(|file| file.path == Path::new(name))
            .ok_or(Refusal::Miss)?;
        let bytes = std::fs::read(entry.join("recorded").join(name))
            .map_err(|_| damaged("stored verifier output cannot be read"))?;
        if bytes.len() as u64 != file.size || digest(&bytes) != file.sha256 {
            return Err(damaged("stored verifier output does not match its digest"));
        }
        Ok(bytes)
    };
    // Never suppress a stored diagnostic. The same narrow stderr policy as
    // publication admits only artifact notices and wall-time progress notes.
    if !quiet_stderr(&read("verus.stderr")?) {
        return Err(Refusal::Miss);
    }
    read("verus.shown-stdout")
}

/// Reads and checks an entry against this machine's current inputs.
fn validated(
    entry: &Path,
    candidate: &Candidate,
    data: &Value,
) -> std::result::Result<Manifest, Refusal> {
    let bytes = std::fs::read(entry.join("manifest.json")).map_err(|_| Refusal::Miss)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| damaged("unreadable manifest"))?;
    if manifest.version != VERSION {
        return Err(Refusal::Miss);
    }
    if !manifest.intact() || manifest.key != candidate.key || manifest.root != candidate.root {
        return Err(damaged("manifest checksum or binding mismatch"));
    }
    let receipt = &manifest.receipt;
    if receipt.partial_args.is_some()
        || receipt.version != RECEIPT_VERSION
        || receipt.clone().seal().checksum != receipt.checksum
        || receipt.package_id != candidate.root
        || !receipt.invocation.success
        || !complete(&receipt.invocation.report)
    {
        return Err(damaged("stored proof receipt is not a complete success"));
    }
    // These describe this machine's current inputs, not the entry's bytes:
    // an edited registry source, a changed declared variable or build-script
    // input is an ordinary miss, as plain Cargo would rebuild.
    if revision(data, candidate).as_deref() != Some(manifest.revision.as_str())
        || !super::environment::unchanged(&receipt.declared_environment)
        || !super::build_inputs::contracts_current(&manifest.contracts, data)
    {
        return Err(Refusal::Miss);
    }
    Ok(manifest)
}

/// A relative path of plain components, which cannot leave the directory it
/// is joined to.
fn contained(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// The entry's receipt with its paths moved into `target`, if the bytes now
/// in `target` are exactly the receipt's artifacts and dependencies.
fn moved_receipt(manifest: &Manifest, target: &Path) -> Option<Receipt> {
    let mut receipt = manifest.receipt.clone();
    receipt.artifact_hashes = receipt
        .artifact_hashes
        .iter()
        .map(|(path, hash)| (relocate(path, &manifest.target, target), hash.clone()))
        .collect();
    receipt.invocation.dependencies = receipt
        .invocation
        .dependencies
        .iter()
        .map(|(path, hash)| (relocate(path, &manifest.target, target), hash.clone()))
        .collect();
    (!receipt.artifact_hashes.is_empty()
        && receipt
            .artifact_hashes
            .keys()
            .chain(receipt.invocation.dependencies.keys())
            .all(|path| path.starts_with(target))
        && digests_match(&receipt.artifact_hashes)
        && digests_match(&receipt.invocation.dependencies))
    .then_some(receipt)
}

/// Copies one entry into `staging`, recording what it created for rollback.
fn place(
    entry: &Path,
    staging: &Path,
    manifest: Manifest,
    files: &mut Vec<PathBuf>,
    directories: &mut Vec<PathBuf>,
) -> std::result::Result<Manifest, Refusal> {
    // A copy that fails because the entry went away (evicted or deleted
    // meanwhile) is a miss; any other failure of an entry still present is
    // damage.
    let failure = |what: &str| {
        if entry.join("manifest.json").is_file() {
            damaged(what)
        } else {
            Refusal::Miss
        }
    };
    for directory in &manifest.directories {
        if !contained(directory) {
            return Err(damaged("stored path escapes the target"));
        }
        let path = staging.join(directory);
        // Every directory this entry creates, outermost first.
        let mut missing: Vec<PathBuf> = path
            .ancestors()
            .take_while(|p| !p.exists())
            .map(Path::to_path_buf)
            .collect();
        missing.reverse();
        for created in missing {
            std::fs::create_dir(&created).map_err(|_| Refusal::Miss)?;
            directories.push(created);
        }
    }
    for file in &manifest.files {
        if !contained(&file.path) {
            return Err(damaged("stored path escapes the target"));
        }
        let destination = staging.join(&file.path);
        if destination.exists() {
            // Another entry placed in this run can share a unit (an
            // overlapping closure); only identical bytes are accepted.
            if super::files::file_digest(&destination).ok().as_deref() == Some(file.sha256.as_str())
            {
                continue;
            }
            return Err(Refusal::Miss);
        }
        let (sha256, size) = copy(
            &entry.join("files").join(&file.path),
            &destination,
            file.executable,
            file.modified,
        )
        .map_err(|_| failure("a stored file cannot be read"))?;
        files.push(destination);
        if sha256 != file.sha256 || size != file.size {
            return Err(failure("a stored file does not match its digest"));
        }
    }
    Ok(manifest)
}

/// For a target that already exists: the receipts of stored entries whose
/// artifacts are exactly the bytes in `target`, for example units an earlier
/// run placed but could not adopt because its inputs changed during the run.
/// Nothing is copied; any failure is a silent miss and Cargo and the verifier
/// proceed as usual.
pub(super) fn attach(
    cache: &Path,
    target: &Path,
    candidates: &[Candidate],
    data: &Value,
) -> Vec<Seeded> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let manifest = validated(&root(cache).join(&candidate.key), candidate, data).ok()?;
            Some(Seeded {
                key: candidate.key.clone(),
                receipt: moved_receipt(&manifest, target)?,
                placed: false,
                shown_stdout: Vec::new(),
            })
        })
        .collect()
}

/// Cargo's unit hash in a target path: the 16 hexadecimal digits after the
/// last `-` of the file stem, or of its directory for `build/<unit>/...`.
fn unit_hash(path: &Path, target: &Path) -> Option<(PathBuf, String)> {
    let relative = path.strip_prefix(target).ok()?;
    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let at = components
        .iter()
        .rposition(|c| *c == "deps" || *c == "build")?;
    let profile: PathBuf = components[..at].iter().collect();
    let name = components.get(at + 1)?;
    let stem = name.split('.').next().unwrap_or(name);
    let (_, hash) = stem.rsplit_once('-')?;
    (hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| (profile, hash.to_owned()))
}

/// Every Cargo unit of `closure` in this run's records: its profile directory
/// (relative to the target) and unit hash.
fn units(
    artifacts: &Path,
    target: &Path,
    closure: &BTreeSet<String>,
) -> Option<BTreeSet<(PathBuf, String)>> {
    let text = std::fs::read_to_string(artifacts).ok()?;
    let mut units = BTreeSet::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).ok()?;
        if !record["package_id"]
            .as_str()
            .is_some_and(|id| closure.contains(id))
        {
            continue;
        }
        if record["reason"] == "compiler-artifact" {
            for name in record["filenames"].as_array()? {
                units.insert(unit_hash(Path::new(name.as_str()?), target)?);
            }
        } else if record["reason"] == "build-script-executed" {
            units.insert(unit_hash(Path::new(record["out_dir"].as_str()?), target)?);
        }
    }
    (!units.is_empty()).then_some(units)
}

/// Adds every file and directory under `base/relative`; `None` on a
/// symbolic link, special file or read error.
fn walk(
    base: &Path,
    relative: &Path,
    files: &mut BTreeSet<PathBuf>,
    directories: &mut BTreeSet<PathBuf>,
) -> Option<()> {
    directories.insert(relative.to_owned());
    for entry in std::fs::read_dir(base.join(relative)).ok()? {
        let entry = entry.ok()?;
        let kind = entry.file_type().ok()?;
        let path = relative.join(entry.file_name());
        if kind.is_dir() {
            walk(base, &path, files, directories)?;
        } else if kind.is_file() {
            files.insert(path);
        } else {
            // Symbolic links and special files are never stored.
            return None;
        }
    }
    Some(())
}

/// The files and directories of `units` in `target`: each unit's
/// `.fingerprint/<name>-<hash>/` and `build/<name>-<hash>/` directories and
/// its `deps/` outputs, relative to `target`.
fn unit_files(
    target: &Path,
    units: &BTreeSet<(PathBuf, String)>,
) -> Option<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let matches = |name: &str, hash: &str| {
        let stem = name.split('.').next().unwrap_or(name);
        stem.rsplit_once('-').is_some_and(|(_, h)| h == hash)
    };
    for (profile, hash) in units {
        for section in [".fingerprint", "build", "deps"] {
            let directory = profile.join(section);
            let Ok(entries) = std::fs::read_dir(target.join(&directory)) else {
                continue;
            };
            directories.insert(directory.clone());
            for entry in entries {
                let entry = entry.ok()?;
                let name = entry.file_name();
                let name = name.to_str()?;
                if !matches(name, hash) {
                    continue;
                }
                let kind = entry.file_type().ok()?;
                let path = directory.join(name);
                if kind.is_dir() {
                    walk(target, &path, &mut files, &mut directories)?;
                } else if kind.is_file() {
                    files.insert(path);
                } else {
                    return None;
                }
            }
        }
    }
    let mut directories: Vec<PathBuf> = directories.into_iter().collect();
    directories.sort_by_key(|d| d.components().count());
    Some((files.into_iter().collect(), directories))
}

/// What publication needs from the run that verified (or reused) the root.
pub(super) struct Publication<'a> {
    pub cache: &'a Path,
    pub target: &'a Path,
    pub artifacts: &'a Path,
    pub build_inputs: &'a Path,
    /// The root's receipt, valid for its own proof context.
    pub receipt: Receipt,
    /// The establishing run's directory, for its recorded verifier output.
    pub sidecar: PathBuf,
    pub data: &'a Value,
}

/// Whether the store already holds `candidate`'s key, so [`publish`] would
/// store nothing.
pub(super) fn holds(cache: &Path, candidate: &Candidate) -> bool {
    root(cache)
        .join(&candidate.key)
        .join("manifest.json")
        .is_file()
}

/// Stores `candidate` from a Current run whose own receipt for it is valid,
/// unless the store already holds its key. Any failure leaves the store
/// unchanged; publication never affects the run's result.
pub(super) fn publish(candidate: &Candidate, publication: &Publication<'_>) {
    let store = root(publication.cache);
    if holds(publication.cache, candidate) {
        return;
    }
    let staging = store.join(format!(
        ".staging-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    publish_at(candidate, publication, &store, &staging);
}

fn publish_at(candidate: &Candidate, publication: &Publication<'_>, store: &Path, staging: &Path) {
    // A colliding name belongs to another publisher. Never write into it or
    // remove it on failure, even when the wall clock returned the same time.
    if let Err(error) = std::fs::create_dir_all(store).and_then(|()| std::fs::create_dir(staging)) {
        super::files::event(&serde_json::json!({
            "event": "dependency-store-skipped", "package_id": candidate.root,
            "key": candidate.key, "reason": error.to_string(),
        }));
        return;
    }
    let staged = stage(candidate, publication, staging);
    let published = staged.and_then(|()| {
        std::fs::rename(staging, store.join(&candidate.key))
            .map_err(|e| Error::new("SAIL3102", e.to_string()))
    });
    if let Err(error) = published {
        // Another run may have published the same key first; either way the
        // staging copy is discarded. The reason is observable for diagnosis.
        let _ = std::fs::remove_dir_all(staging);
        super::files::event(&serde_json::json!({
            "event": "dependency-store-skipped", "package_id": candidate.root,
            "key": candidate.key, "reason": error.message,
        }));
        return;
    }
    super::files::event(&serde_json::json!({
        "event": "dependency-stored", "package_id": candidate.root, "key": candidate.key,
    }));
    evict(store, &candidate.key, capacity());
}

/// The packages of `closure` that have a build script.
fn script_packages(data: &Value, closure: &BTreeSet<String>) -> BTreeSet<String> {
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            p["id"].as_str().is_some_and(|id| closure.contains(id))
                && p["targets"].as_array().is_some_and(|targets| {
                    targets.iter().any(|t| {
                        t["kind"]
                            .as_array()
                            .is_some_and(|k| k.iter().any(|v| v == "custom-build"))
                    })
                })
        })
        .filter_map(|p| p["id"].as_str().map(str::to_owned))
        .collect()
}

/// Whether the root's verifier printed nothing a user would see: its stdout
/// holds only its JSON documents, and its stderr only Cargo artifact
/// notices and Verus's timing-dependent long-running notes. Plain
/// Cargo-Verus shows a registry dependency's diagnostics, if it has any, when
/// it verifies it; such a dependency is never stored, so a seeded run never
/// hides output that plain would print (ADR 0045).
fn quiet(sidecar: &Path) -> bool {
    let (Ok(stdout), Ok(stderr)) = (
        std::fs::read(sidecar.join("verus.stdout")),
        std::fs::read(sidecar.join("verus.stderr")),
    ) else {
        return false;
    };
    let (documents, rest) = crate::attribution::json_documents(&stdout);
    !documents.is_empty() && rest.iter().all(u8::is_ascii_whitespace) && quiet_stderr(&stderr)
}

fn quiet_stderr(stderr: &[u8]) -> bool {
    stderr
        .split(|b| *b == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .all(|line| {
            serde_json::from_slice::<Value>(line)
                .is_ok_and(|v| v["$message_type"] == "artifact" || long_running_note(&v))
        })
}

/// Whether `message` is one of Verus's long-running progress notes:
/// `<check> has been running for <N> seconds` (with its second line) or
/// `<check> finished in <N> seconds`, a note with no code and no children.
/// With one verifier thread (the default on a two-CPU host), Verus prints
/// them for any query that takes more than two seconds of wall time, so
/// plain Cargo-Verus prints them for the same dependency on one run and not
/// on the next. Omitting them reproduces a plain run in which every query
/// finished in time. They carry no verdict, and a diagnostic that Verus
/// flushes early when it prints one is still refused (ADR 0045, decision 8).
fn long_running_note(message: &Value) -> bool {
    const DISCOVERED: &str =
        "\nreporting errors as they are discovered (they may not be in source order)";
    // `<check><infix><N> seconds`: a one-line check name and a whole number.
    let timed = |text: &str, infix: &str| {
        text.strip_suffix(" seconds")
            .and_then(|rest| rest.rsplit_once(infix))
            .is_some_and(|(check, n)| {
                !check.is_empty()
                    && !check.contains('\n')
                    && !n.is_empty()
                    && n.bytes().all(|b| b.is_ascii_digit())
            })
    };
    let text = message["message"].as_str().unwrap_or_default();
    message["$message_type"] == "diagnostic"
        && message["level"] == "note"
        && message["code"].is_null()
        && message["children"].as_array().is_some_and(Vec::is_empty)
        && (text
            .strip_suffix(DISCOVERED)
            .is_some_and(|first| timed(first, " has been running for "))
            || timed(text, " finished in "))
}

fn stage(candidate: &Candidate, publication: &Publication<'_>, staging: &Path) -> Result<()> {
    let refused = |what: &str| Error::new("SAIL3102", what.to_owned());
    let target = publication.target;
    let receipt = &publication.receipt;
    let revision =
        revision(publication.data, candidate).ok_or_else(|| refused("closure unreadable"))?;
    let units = units(publication.artifacts, target, &candidate.closure)
        .ok_or_else(|| refused("closure units unknown"))?;
    let (paths, directories) =
        unit_files(target, &units).ok_or_else(|| refused("closure files unreadable"))?;
    // The receipt's artifacts and the root's compiled dependencies must all
    // be among the stored files, or a seeded target could not satisfy it.
    let stored: BTreeSet<PathBuf> = paths.iter().map(|p| target.join(p)).collect();
    if receipt.artifact_hashes.is_empty()
        || !receipt
            .artifact_hashes
            .keys()
            .chain(receipt.invocation.dependencies.keys())
            .all(|p| stored.contains(p))
    {
        return Err(refused("receipt artifacts are outside the closure"));
    }
    if !quiet(&publication.sidecar) {
        return Err(refused("the dependency's verifier printed output"));
    }
    let contracts = super::build_inputs::contracts_for(
        publication.build_inputs,
        &candidate.closure,
        &script_packages(publication.data, &candidate.closure),
    )
    .ok_or_else(|| refused("build-script contracts incomplete"))?;
    let files_dir = staging.join("files");
    for directory in &directories {
        std::fs::create_dir_all(files_dir.join(directory))
            .map_err(|e| Error::new("SAIL3102", e.to_string()))?;
    }
    let mut files = Vec::new();
    for path in &paths {
        let source = target.join(path);
        let meta = std::fs::metadata(&source).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
        let time = modified(&meta).ok_or_else(|| refused("unreadable modification time"))?;
        let exec = executable(&meta);
        let (sha256, size) = copy(&source, &files_dir.join(path), exec, time)
            .map_err(|e| Error::new("SAIL3102", e.message))?;
        files.push(StoredFile {
            path: path.clone(),
            sha256,
            size,
            modified: time,
            executable: exec,
        });
    }
    // The copies, not the live target, must match the receipt: a concurrent
    // change between the receipt check and the copy refuses publication.
    let copied: BTreeMap<PathBuf, &str> = files
        .iter()
        .map(|f| (target.join(&f.path), f.sha256.as_str()))
        .collect();
    if !receipt
        .artifact_hashes
        .iter()
        .chain(&receipt.invocation.dependencies)
        .all(|(path, hash)| copied.get(path) == Some(&hash.as_str()))
    {
        return Err(refused("artifacts changed while they were stored"));
    }
    let recorded = record_output(&publication.sidecar, &staging.join("recorded"))?;
    let manifest = Manifest {
        version: VERSION,
        key: candidate.key.clone(),
        root: candidate.root.clone(),
        revision,
        target: target.to_owned(),
        receipt: receipt.clone(),
        contracts,
        directories,
        files,
        recorded,
        checksum: String::new(),
    }
    .seal();
    json(&staging.join("manifest.json"), &manifest)
}

/// Copies the verifier's captured output for the root into the entry.
fn record_output(sidecar: &Path, recorded_dir: &Path) -> Result<Vec<StoredFile>> {
    std::fs::create_dir_all(recorded_dir).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
    let mut recorded = Vec::new();
    for name in ["verus.stdout", "verus.stderr", "verus.shown-stdout"] {
        let source = sidecar.join(name);
        let Ok(meta) = std::fs::metadata(&source) else {
            continue;
        };
        let time = modified(&meta).unwrap_or((0, 0));
        let (sha256, size) = copy(&source, &recorded_dir.join(name), false, time)
            .map_err(|e| Error::new("SAIL3102", e.message))?;
        recorded.push(StoredFile {
            path: PathBuf::from(name),
            sha256,
            size,
            modified: time,
            executable: false,
        });
    }
    Ok(recorded)
}

/// Keeps the most recently used entries, up to the configured capacity. An
/// entry is renamed away before it is deleted, so a run that is copying from
/// it fails cleanly and verifies normally.
fn evict(store: &Path, keep: &str, capacity: usize) {
    reclaim(store, STALE);
    let Ok(entries) = std::fs::read_dir(store) else {
        return;
    };
    let mut entries: Vec<(SystemTime, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') || name == keep {
                return None;
            }
            let path = entry.path();
            let used = std::fs::metadata(path.join("used"))
                .or_else(|_| std::fs::metadata(path.join("manifest.json")))
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            Some((used, name))
        })
        .collect();
    entries.sort();
    let excess = (entries.len() + 1).saturating_sub(capacity);
    for (_, name) in entries.into_iter().take(excess) {
        let doomed = store.join(format!(".evicted-{name}-{}", std::process::id()));
        if std::fs::rename(store.join(&name), &doomed).is_ok() {
            let _ = std::fs::remove_dir_all(doomed);
        }
    }
}

/// How old an unfinished publication or eviction must be before it is
/// treated as abandoned by a killed run. Publishing takes seconds.
const STALE: Duration = Duration::from_secs(3600);

/// Deletes staging and eviction directories older than `age`: leftovers of a
/// run killed while publishing or evicting, which nothing else reclaims.
fn reclaim(store: &Path, age: Duration) {
    let Ok(entries) = std::fs::read_dir(store) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with(".staging-") || name.starts_with(".evicted-")) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|elapsed| elapsed >= age);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Whether `target` holds any compiled Cargo unit: a `.fingerprint`
/// directory in it or in a profile directory up to two levels below it.
fn holds_units(target: &Path) -> bool {
    fn search(directory: &Path, depth: u32) -> bool {
        std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && (entry.file_name() == ".fingerprint"
                        || (depth > 0 && search(&entry.path(), depth - 1)))
            })
    }
    search(target, 2)
}

/// Whether `target` is a Cargo target this run creates: it does not exist yet
/// or holds nothing.
pub(super) fn new_target(target: &Path) -> bool {
    match std::fs::read_dir(target) {
        Ok(mut entries) => entries.next().is_none(),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_target_requires_absence_or_an_empty_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-fresh-target-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let target = root.join("target");
        assert!(new_target(&target));
        std::fs::create_dir(&target)?;
        assert!(new_target(&target));
        std::fs::write(target.join("artifact"), "existing output")?;
        assert!(!new_target(&target));
        std::fs::remove_dir_all(&target)?;
        std::fs::write(&target, "not a directory")?;
        assert!(!new_target(&target));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn unit_hashes_come_from_cargo_output_names() {
        let target = Path::new("/t");
        assert_eq!(
            unit_hash(
                Path::new("/t/debug/deps/libvstd-e3b9a244e6e09f32.rmeta"),
                target
            ),
            Some((PathBuf::from("debug"), "e3b9a244e6e09f32".to_owned()))
        );
        assert_eq!(
            unit_hash(
                Path::new("/t/debug/build/proc-macro2-61e4a19fcb6697d2/out"),
                target
            ),
            Some((PathBuf::from("debug"), "61e4a19fcb6697d2".to_owned()))
        );
        assert_eq!(
            unit_hash(
                Path::new(
                    "/t/x86_64-unknown-linux-gnu/debug/build/vstd-7126c789be829cfe/build-script-build"
                ),
                target
            ),
            Some((
                PathBuf::from("x86_64-unknown-linux-gnu/debug"),
                "7126c789be829cfe".to_owned()
            ))
        );
        assert_eq!(
            unit_hash(Path::new("/t/debug/deps/libvstd.rmeta"), target),
            None
        );
        assert_eq!(
            unit_hash(
                Path::new("/elsewhere/debug/deps/a-0123456789abcdef.d"),
                target
            ),
            None
        );
    }

    #[test]
    fn relocation_moves_only_paths_inside_the_old_target() {
        let from = Path::new("/old/target");
        let to = Path::new("/new/target");
        assert_eq!(
            relocate(Path::new("/old/target/debug/deps/a.rmeta"), from, to),
            PathBuf::from("/new/target/debug/deps/a.rmeta")
        );
        assert_eq!(
            relocate(Path::new("/sysroot/lib/x.rlib"), from, to),
            PathBuf::from("/sysroot/lib/x.rlib")
        );
    }

    /// A workspace using a registry `vstd` (with a build script) that depends
    /// on a registry `dep`, and Cargo targets holding their units.
    struct Fixture {
        root: PathBuf,
        data: Value,
        expected: BTreeSet<(String, String, String)>,
    }

    const VSTD: &str = "registry+https://example.invalid/index#vstd@1.0.0";
    const DEP: &str = "registry+https://example.invalid/index#dep@1.0.0";
    const WS: &str = "path+file:///ws#ws@0.1.0";

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            assert!(std::fs::create_dir_all(parent).is_ok());
        }
        assert!(std::fs::write(path, bytes).is_ok());
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "sail-store-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
            write(&root.join("ws/Cargo.toml"), b"[package]\nname = \"ws\"\n");
            write(
                &root.join("reg/vstd/Cargo.toml"),
                b"[package]\nname = \"vstd\"\n",
            );
            write(&root.join("reg/vstd/build.rs"), b"fn main() {}\n");
            write(&root.join("reg/vstd/vstd.rs"), b"// vstd\n");
            write(
                &root.join("reg/dep/Cargo.toml"),
                b"[package]\nname = \"dep\"\n",
            );
            write(&root.join("reg/dep/lib.rs"), b"// dep\n");
            let manifest = |dir: &str| root.join(dir).join("Cargo.toml");
            let data = serde_json::json!({
                "workspace_root": root.join("ws"),
                "workspace_members": [WS],
                "packages": [
                    {"id": WS, "name": "ws", "source": null, "manifest_path": manifest("ws"),
                     "metadata": {"verus": {"verify": true}},
                     "targets": [{"name": "ws", "kind": ["lib"]}]},
                    {"id": VSTD, "name": "vstd", "source": "registry+https://example.invalid/index",
                     "manifest_path": manifest("reg/vstd"),
                     "metadata": {"verus": {"verify": true}},
                     "targets": [{"name": "vstd", "kind": ["lib"]},
                                 {"name": "build-script-build", "kind": ["custom-build"]}]},
                    {"id": DEP, "name": "dep", "source": "registry+https://example.invalid/index",
                     "manifest_path": manifest("reg/dep"),
                     "targets": [{"name": "dep", "kind": ["lib"]}]}
                ],
                "resolve": {"root": WS, "nodes": [
                    {"id": WS, "dependencies": [VSTD], "deps": [], "features": []},
                    {"id": VSTD, "dependencies": [DEP], "deps": [], "features": ["std"]},
                    {"id": DEP, "dependencies": [], "deps": [], "features": []}
                ]}
            });
            Self {
                root,
                data,
                expected: BTreeSet::from([(WS.to_owned(), "ws".to_owned(), "lib".to_owned())]),
            }
        }

        fn cache(&self) -> PathBuf {
            self.root.join("cache")
        }

        /// Builds Cargo target `name` and offers its `vstd` to the store.
        fn build(&self, name: &str) -> (PathBuf, Candidate, Receipt) {
            self.build_with(
                name,
                b"{\"$message_type\":\"artifact\",\"artifact\":\"x.rmeta\",\"emit\":\"metadata\"}\n",
            )
        }

        /// As [`Fixture::build`], with `stderr` as the verifier's captured
        /// standard error for `vstd`.
        #[allow(
            clippy::too_many_lines,
            reason = "One literal fixture of a Cargo target and its receipt."
        )]
        fn build_with(&self, name: &str, stderr: &[u8]) -> (PathBuf, Candidate, Receipt) {
            let target = self.root.join(name);
            let debug = target.join("debug");
            for unit in [
                "deps/libvstd-0123456789abcdef.rmeta",
                "deps/libvstd-0123456789abcdef.vir",
                "deps/vstd-0123456789abcdef.d",
                ".fingerprint/vstd-0123456789abcdef/lib-vstd",
                ".fingerprint/vstd-1111111111111111/run-build-script-build-script-build",
                "deps/libdep-2222222222222222.rmeta",
                ".fingerprint/dep-2222222222222222/lib-dep",
                "deps/libws-3333333333333333.rmeta",
            ] {
                write(&debug.join(unit), unit.as_bytes());
            }
            write(
                &debug.join("build/vstd-1111111111111111/output"),
                b"cargo:rerun-if-changed=build.rs\n",
            );
            assert!(std::fs::create_dir_all(debug.join("build/vstd-1111111111111111/out")).is_ok());
            let artifacts = self.root.join(format!("{name}-artifacts.jsonl"));
            let records = [
                serde_json::json!({"reason": "compiler-artifact", "package_id": VSTD, "fresh": false,
                    "filenames": [debug.join("deps/libvstd-0123456789abcdef.rmeta")]}),
                serde_json::json!({"reason": "build-script-executed", "package_id": VSTD,
                    "out_dir": debug.join("build/vstd-1111111111111111/out")}),
                serde_json::json!({"reason": "compiler-artifact", "package_id": DEP, "fresh": false,
                    "filenames": [debug.join("deps/libdep-2222222222222222.rmeta")]}),
                serde_json::json!({"reason": "compiler-artifact", "package_id": WS, "fresh": false,
                    "filenames": [debug.join("deps/libws-3333333333333333.rmeta")]}),
            ];
            write(
                &artifacts,
                records
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .as_bytes(),
            );
            let build_inputs = self.root.join(format!("{name}-build-inputs.json"));
            assert!(
                super::super::build_inputs::update(&build_inputs, &artifacts, &self.data).is_ok()
            );
            let rmeta = debug.join("deps/libvstd-0123456789abcdef.rmeta");
            let vir = debug.join("deps/libvstd-0123456789abcdef.vir");
            let dep = debug.join("deps/libdep-2222222222222222.rmeta");
            let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
            report.error = None;
            report.exit_code = Some(0);
            report.crates.push(sail_api::output::CrateReport {
                name: Some("vstd".to_owned()),
                verus: sail_api::output::VerusResults {
                    verified: Some(2),
                    errors: Some(0),
                    success: Some(true),
                    encountered_vir_error: Some(false),
                },
                functions: Vec::new(),
            });
            let receipt = Receipt {
                version: RECEIPT_VERSION,
                partial_args: None,
                checksum: String::new(),
                package_id: VSTD.to_owned(),
                name: "vstd".to_owned(),
                kind: vec!["lib".to_owned()],
                context: "establishing".to_owned(),
                input_revision: "workspace-specific".to_owned(),
                invocation: super::super::receipts::Invocation {
                    retained_from: None,
                    identity: super::super::receipts::TargetIdentity {
                        manifest: self.root.join("reg/vstd/Cargo.toml"),
                        name: "vstd".to_owned(),
                        crate_type: "lib".to_owned(),
                        rustc_args_sha256: "args".to_owned(),
                    },
                    run_id: "establishing-run".to_owned(),
                    dependency_build: None,
                    report,
                    success: true,
                    dependencies: super::super::receipts::artifact_hashes(&[dep])
                        .unwrap_or_default(),
                },
                artifact_hashes: super::super::receipts::artifact_hashes(&[rmeta, vir])
                    .unwrap_or_default(),
                committed: PathBuf::new(),
                declared_environment: BTreeMap::new(),
            }
            .seal();
            let candidates = candidates(&self.data, &self.expected, "family");
            assert_eq!(candidates.len(), 1, "{candidates:?}");
            let candidate = candidates
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!());
            let sidecar = self.root.join(format!("{name}-sidecar"));
            write(&sidecar.join("verus.stdout"), b"{\"recorded\":true}\n");
            write(
                &sidecar.join("verus.shown-stdout"),
                b"verification results:: 1 verified, 0 errors\n",
            );
            write(&sidecar.join("verus.stderr"), stderr);
            publish(
                &candidate,
                &Publication {
                    cache: &self.cache(),
                    target: &target,
                    artifacts: &artifacts,
                    build_inputs: &build_inputs,
                    receipt: receipt.clone(),
                    sidecar,
                    data: &self.data,
                },
            );
            (target, candidate, receipt)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn seed_into(
        fixture: &Fixture,
        candidate: &Candidate,
        name: &str,
    ) -> (PathBuf, Vec<Seeded>, String) {
        let target = fixture.root.join(name);
        let mut err = Vec::new();
        let seeded = seed(
            &fixture.cache(),
            &target,
            std::slice::from_ref(candidate),
            &fixture.data,
            &mut err,
        );
        (target, seeded, String::from_utf8_lossy(&err).into_owned())
    }

    #[test]
    fn a_new_target_receives_exactly_the_closure_with_times_and_a_moved_receipt() {
        let fixture = Fixture::new("seed");
        let (old, candidate, receipt) = fixture.build("old");
        let entry = root(&fixture.cache()).join(&candidate.key);
        assert!(entry.join("recorded/verus.stdout").is_file());
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(warnings.is_empty(), "{warnings}");
        assert_eq!(seeded.len(), 1);
        let time = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        for unit in [
            "debug/deps/libvstd-0123456789abcdef.rmeta",
            "debug/deps/libvstd-0123456789abcdef.vir",
            "debug/deps/vstd-0123456789abcdef.d",
            "debug/.fingerprint/vstd-1111111111111111/run-build-script-build-script-build",
            "debug/build/vstd-1111111111111111/output",
            "debug/deps/libdep-2222222222222222.rmeta",
        ] {
            let (a, b) = (old.join(unit), new.join(unit));
            assert_eq!(std::fs::read(&a).ok(), std::fs::read(&b).ok(), "{unit}");
            assert_eq!(time(&a), time(&b), "{unit}");
        }
        assert!(new.join("debug/build/vstd-1111111111111111/out").is_dir());
        // A workspace unit is never stored.
        assert!(!new.join("debug/deps/libws-3333333333333333.rmeta").exists());
        let moved = &seeded[0].receipt;
        assert_eq!(moved.invocation.report, receipt.invocation.report);
        assert!(moved.artifact_hashes.keys().all(|p| p.starts_with(&new)));
        assert!(
            moved
                .invocation
                .dependencies
                .keys()
                .all(|p| p.starts_with(&new))
        );
        assert!(digests_match(&moved.artifact_hashes));
        assert!(seeded[0].placed);
        assert_eq!(
            seeded[0].shown_stdout,
            b"verification results:: 1 verified, 0 errors\n"
        );
        // An existing target holding exactly the stored bytes is attached
        // without copying; one without them is not.
        let attached = attach(
            &fixture.cache(),
            &new,
            std::slice::from_ref(&candidate),
            &fixture.data,
        );
        assert!(attached.len() == 1 && !attached[0].placed);
        assert_eq!(attached[0].shown_stdout, Vec::<u8>::new());
        assert!(
            attach(
                &fixture.cache(),
                &fixture.root.join("empty"),
                std::slice::from_ref(&candidate),
                &fixture.data
            )
            .is_empty()
        );
        // An existing target is never seeded again.
        let (_, again, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(again.is_empty() && warnings.is_empty(), "{warnings}");
    }

    #[test]
    fn corrupt_entries_warn_and_leave_the_new_target_empty() {
        let fixture = Fixture::new("corrupt");
        let (_, candidate, _) = fixture.build("old");
        let entry = root(&fixture.cache()).join(&candidate.key);
        write(
            &entry.join("files/debug/deps/libdep-2222222222222222.rmeta"),
            b"tampered",
        );
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(seeded.is_empty());
        assert!(warnings.starts_with("warning[SAIL3131]:"), "{warnings}");
        assert!(warnings.contains("does not match its digest"), "{warnings}");
        assert!(!new.exists(), "a failed seed left files behind");
        // A tampered manifest is refused before anything is copied.
        let manifest = entry.join("manifest.json");
        let text = std::fs::read_to_string(&manifest).unwrap_or_default();
        assert!(text.contains("\"verified\":2"));
        write(
            &manifest,
            text.replace("\"verified\":2", "\"verified\":3").as_bytes(),
        );
        let (newer, seeded, warnings) = seed_into(&fixture, &candidate, "newer");
        assert!(seeded.is_empty() && !newer.exists());
        assert!(warnings.contains("manifest checksum"), "{warnings}");
    }

    #[test]
    fn changed_closure_inputs_miss_or_refuse() {
        let fixture = Fixture::new("mismatch");
        let (_, candidate, _) = fixture.build("old");
        // Another feature set (or version, edge, profile or compiler family)
        // is another key: an ordinary miss with no warning.
        let mut other = fixture.data.clone();
        other["resolve"]["nodes"][2]["features"] = serde_json::json!(["extra"]);
        let others = candidates(&other, &fixture.expected, "family");
        assert_eq!(others.len(), 1);
        assert_ne!(others[0].key, candidate.key);
        assert_ne!(
            candidates(&fixture.data, &fixture.expected, "another-family")[0].key,
            candidate.key
        );
        write(
            &fixture.root.join("ws/Cargo.toml"),
            b"[package]\nname = \"ws\"\n[profile.dev]\nopt-level = 1\n",
        );
        assert_ne!(
            candidates(&fixture.data, &fixture.expected, "family")[0].key,
            candidate.key
        );
        let (_, seeded, warnings) = seed_into(&fixture, &others[0], "new");
        assert!(seeded.is_empty() && warnings.is_empty(), "{warnings}");
        // Changed bytes in the dependency's own source under the same key
        // are an ordinary miss: plain Cargo would rebuild, and prints nothing
        // about it either.
        write(&fixture.root.join("reg/vstd/vstd.rs"), b"// edited\n");
        let (edited, seeded, warnings) = seed_into(&fixture, &candidate, "edited");
        assert!(seeded.is_empty() && !edited.exists());
        assert!(warnings.is_empty(), "{warnings}");
    }

    #[test]
    fn a_dependency_whose_verifier_printed_output_is_never_stored() {
        let fixture = Fixture::new("noisy");
        let sidecar = fixture.root.join("noisy-sidecar");
        write(&sidecar.join("verus.stdout"), b"{}\n");
        write(
            &sidecar.join("verus.stderr"),
            b"{\"$message_type\":\"artifact\"}\n",
        );
        assert!(quiet(&sidecar));
        write(&sidecar.join("verus.stdout"), b"{}\nnote: something\n");
        assert!(!quiet(&sidecar));
        write(&sidecar.join("verus.stdout"), b"{}\n");
        write(
            &sidecar.join("verus.stderr"),
            b"{\"$message_type\":\"diagnostic\",\"rendered\":\"warning: x\"}\n",
        );
        assert!(!quiet(&sidecar));
        assert!(!quiet(&fixture.root.join("missing")));
    }

    /// Verus's long-running notes as a one-thread verifier prints them for a
    /// vstd query slower than two seconds (CI run 38804596929, attempt 1).
    fn timing_notes(first: &str, last: &str) -> Vec<u8> {
        let note = |message: &str| {
            serde_json::json!({
                "$message_type": "diagnostic", "message": message, "code": null,
                "level": "note", "children": [],
                "spans": [{"file_name": "/cargo/registry/src/vstd/arithmetic/div_mod.rs",
                           "line_start": 665, "is_primary": true}],
                "rendered": format!("note: {message}\n"),
            })
            .to_string()
        };
        format!(
            "{}\n{}\n{}\n",
            note(first),
            note(last),
            serde_json::json!({"$message_type": "artifact", "artifact": "x.rmeta", "emit": "metadata"})
        )
        .into_bytes()
    }

    const RUNNING: &str = "function body check has been running for 2 seconds\nreporting errors as they are discovered (they may not be in source order)";

    #[test]
    fn a_dependency_whose_verifier_was_slow_is_still_stored_and_seeded() {
        // The intermittent CI condition, made deterministic: the run that
        // verified vstd captured timing notes. Publication must not depend
        // on how long a query took, or a later new project verifies vstd.
        let fixture = Fixture::new("slow");
        let (_, candidate, _) = fixture.build_with(
            "old",
            &timing_notes(RUNNING, "function body check finished in 2 seconds"),
        );
        assert!(
            root(&fixture.cache())
                .join(&candidate.key)
                .join("manifest.json")
                .is_file()
        );
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(warnings.is_empty(), "{warnings}");
        assert_eq!(seeded.len(), 1);
        assert!(
            new.join("debug/deps/libvstd-0123456789abcdef.rmeta")
                .is_file()
        );
    }

    #[test]
    fn only_verus_timing_notes_are_ignored() {
        let fixture = Fixture::new("notes");
        let sidecar = fixture.root.join("notes-sidecar");
        write(&sidecar.join("verus.stdout"), b"{}\n");
        let quiet_with = |stderr: &[u8]| {
            write(&sidecar.join("verus.stderr"), stderr);
            quiet(&sidecar)
        };
        let finished = "function termination check finished in 13 seconds";
        assert!(quiet_with(&timing_notes(RUNNING, finished)));
        for (first, last) in [
            // Another note, or a timing note's text without its shape.
            (
                RUNNING,
                "function body check finished in 2 seconds; see above",
            ),
            (RUNNING, "function body check finished in two seconds"),
            (RUNNING, " finished in 2 seconds"),
            (RUNNING, "function body check finished in  seconds"),
            (
                "function body check has been running for 2 seconds",
                finished,
            ),
            ("postcondition not satisfied", finished),
        ] {
            assert!(!quiet_with(&timing_notes(first, last)), "{first} / {last}");
        }
        let note = |change: &dyn Fn(&mut Value)| {
            let mut value = serde_json::json!({"$message_type": "diagnostic",
                "message": finished, "code": null, "level": "note", "children": []});
            change(&mut value);
            format!("{value}\n").into_bytes()
        };
        assert!(quiet_with(&note(&|_| ())));
        assert!(!quiet_with(&note(&|v| v["level"] = "warning".into())));
        assert!(!quiet_with(&note(
            &|v| v["code"] = serde_json::json!({"code": "x"})
        )));
        assert!(!quiet_with(&note(
            &|v| v["children"] = serde_json::json!([{"message": "help"}])
        )));
    }

    #[test]
    fn loader_variables_disable_the_store() {
        let keys = |names: &[&str]| {
            names
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        };
        assert!(loader_variables(keys(&["PATH", "LD_LIBRARY_PATH"])));
        assert!(loader_variables(keys(&["LD_PRELOAD"])));
        assert!(loader_variables(keys(&["DYLD_INSERT_LIBRARIES"])));
        assert!(!loader_variables(keys(&[
            "PATH",
            "HOME",
            "CARGO_HOME",
            "OLD_LD_PATH"
        ])));
    }

    #[test]
    fn a_path_dependency_in_the_closure_is_never_stored() {
        let fixture = Fixture::new("path");
        let mut data = fixture.data.clone();
        data["packages"][2]["source"] = Value::Null;
        assert!(candidates(&data, &fixture.expected, "family").is_empty());
        // Selected targets are verified by the run itself, never stored.
        let selected = BTreeSet::from([(VSTD.to_owned(), "vstd".to_owned(), "lib".to_owned())]);
        assert!(candidates(&fixture.data, &selected, "family").is_empty());
    }

    #[test]
    fn a_root_never_seeds_another_verified_roots_unrecorded_output() {
        let fixture = Fixture::new("nested-proof");
        let mut data = fixture.data.clone();
        data["packages"][2]["metadata"] = serde_json::json!({"verus":{"verify":true}});
        let candidates = candidates(&data, &fixture.expected, "family");
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].root,
            data["packages"][2]["id"].as_str().unwrap_or_default()
        );
    }

    #[test]
    fn publication_never_touches_another_publishers_staging_directory() {
        let fixture = Fixture::new("staging-owner");
        let (target, candidate, receipt) = fixture.build("old");
        let store = root(&fixture.cache());
        assert!(std::fs::remove_dir_all(store.join(&candidate.key)).is_ok());
        let staging = store.join(".staging-other-publisher");
        let marker = staging.join("owned");
        write(&marker, b"another publisher");
        publish_at(
            &candidate,
            &Publication {
                cache: &fixture.cache(),
                target: &target,
                artifacts: &fixture.root.join("old-artifacts.jsonl"),
                build_inputs: &fixture.root.join("old-build-inputs.json"),
                receipt,
                sidecar: fixture.root.join("old-sidecar"),
                data: &fixture.data,
            },
            &store,
            &staging,
        );
        assert_eq!(
            std::fs::read(&marker).unwrap_or_default(),
            b"another publisher"
        );
        assert!(!store.join(&candidate.key).exists());
        assert_eq!(std::fs::read_dir(&staging).map_or(0, Iterator::count), 1);
    }

    #[test]
    fn concurrent_publishers_and_seeders_agree() {
        let fixture = Fixture::new("concurrent");
        let (_, candidate, _) = fixture.build("old");
        let entry = root(&fixture.cache()).join(&candidate.key);
        assert!(std::fs::remove_dir_all(&entry).is_ok());
        std::thread::scope(|scope| {
            let a = scope.spawn(|| fixture.build("a"));
            let b = scope.spawn(|| fixture.build("b"));
            assert!(a.join().is_ok() && b.join().is_ok());
        });
        let names: Vec<String> = std::fs::read_dir(root(&fixture.cache()))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        assert_eq!(
            names,
            std::slice::from_ref(&candidate.key),
            "staging copies left behind"
        );
        std::thread::scope(|scope| {
            let seeders: Vec<_> = (0..4)
                .map(|i| {
                    let fixture = &fixture;
                    let candidate = &candidate;
                    scope.spawn(move || seed_into(fixture, candidate, &format!("seed-{i}")))
                })
                .collect();
            for seeder in seeders {
                let (_, seeded, warnings) = seeder.join().unwrap_or_else(|_| unreachable!());
                assert_eq!(seeded.len(), 1, "{warnings}");
            }
        });
    }

    #[test]
    fn a_seed_killed_midway_leaves_no_target_and_is_cleaned_up() {
        let fixture = Fixture::new("killed");
        let (old, candidate, _) = fixture.build("old");
        // What a run killed while copying leaves: a hidden sibling holding
        // part of the entry. The target itself was never created.
        let target = fixture.root.join("new");
        let leftover = fixture.root.join(".new.seed-1-1");
        write(
            &leftover.join("debug/deps/libdep-2222222222222222.rmeta"),
            b"partial",
        );
        assert!(!target.exists() && new_target(&target));
        let (_, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert_eq!(seeded.len(), 1, "{warnings}");
        assert!(!leftover.exists(), "the leftover was not removed");
        let unit = "debug/deps/libdep-2222222222222222.rmeta";
        assert_eq!(
            std::fs::read(target.join(unit)).ok(),
            std::fs::read(old.join(unit)).ok()
        );
        assert_eq!(staging_dirs(&target), Vec::<PathBuf>::new());
    }

    #[test]
    fn a_target_with_only_cargo_bookkeeping_is_seeded_and_one_with_units_is_not() {
        let fixture = Fixture::new("bookkeeping");
        let (_, candidate, _) = fixture.build("old");
        // What `cargo clean -p` leaves in a target that never built anything.
        write(&fixture.root.join("new/CACHEDIR.TAG"), b"tag");
        write(&fixture.root.join("new/debug/.cargo-lock"), b"");
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert_eq!(seeded.len(), 1, "{warnings}");
        assert!(
            new.join("debug/deps/libdep-2222222222222222.rmeta")
                .is_file()
        );
        assert!(!new.join("CACHEDIR.TAG").exists());
        assert_eq!(staging_dirs(&new), Vec::<PathBuf>::new());
        write(
            &fixture
                .root
                .join("built/debug/.fingerprint/x-0123456789abcdef/lib-x"),
            b"",
        );
        let (built, seeded, _) = seed_into(&fixture, &candidate, "built");
        assert!(seeded.is_empty());
        assert!(!built.join("debug/deps").exists());
    }

    #[test]
    fn a_truncated_stored_file_warns_and_places_nothing() {
        let fixture = Fixture::new("truncated");
        let (_, candidate, _) = fixture.build("old");
        let stored = root(&fixture.cache())
            .join(&candidate.key)
            .join("files/debug/deps/libvstd-0123456789abcdef.rmeta");
        let bytes = std::fs::read(&stored).unwrap_or_default();
        write(&stored, &bytes[..bytes.len() / 2]);
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(seeded.is_empty() && !new.exists());
        assert!(warnings.starts_with("warning[SAIL3131]:"), "{warnings}");
        assert_eq!(staging_dirs(&new), Vec::<PathBuf>::new());
    }

    #[test]
    fn damaged_or_missing_displayed_output_never_seeds_artifacts() {
        for name in ["verus.shown-stdout", "verus.stderr"] {
            for case in ["damaged", "missing"] {
                let fixture = Fixture::new(&format!("{case}-{name}"));
                let (_, candidate, _) = fixture.build("old");
                let output = root(&fixture.cache())
                    .join(&candidate.key)
                    .join("recorded")
                    .join(name);
                if case == "damaged" {
                    write(&output, b"wrong output");
                } else {
                    assert!(std::fs::remove_file(output).is_ok());
                }
                let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
                assert!(seeded.is_empty() && !new.exists());
                assert!(warnings.starts_with("warning[SAIL3131]:"), "{warnings}");
            }
        }
    }

    #[test]
    fn an_entry_evicted_during_a_seed_is_a_silent_miss() {
        let fixture = Fixture::new("evicted");
        let (_, candidate, _) = fixture.build("old");
        let entry = root(&fixture.cache()).join(&candidate.key);
        let manifest = validated(&entry, &candidate, &fixture.data)
            .unwrap_or_else(|_| unreachable!("fixture entry"));
        // Eviction renames the entry away and deletes it after validation.
        assert!(std::fs::remove_dir_all(&entry).is_ok());
        let staging = fixture.root.join("staging");
        let (mut files, mut directories) = (Vec::new(), Vec::new());
        let outcome = place(&entry, &staging, manifest, &mut files, &mut directories);
        assert!(matches!(outcome, Err(Refusal::Miss)), "{outcome:?}");
        rollback(&files, &directories);
        assert!(!staging.exists());
        // seed() itself finds no entry: nothing placed and nothing printed.
        let (new, seeded, warnings) = seed_into(&fixture, &candidate, "new");
        assert!(seeded.is_empty() && warnings.is_empty() && !new.exists());
    }

    #[test]
    fn stored_directories_cannot_escape_the_target() {
        let fixture = Fixture::new("escape");
        let (_, candidate, _) = fixture.build("old");
        let entry = root(&fixture.cache()).join(&candidate.key);
        let mut manifest = validated(&entry, &candidate, &fixture.data)
            .unwrap_or_else(|_| unreachable!("fixture entry"));
        manifest.directories.push(PathBuf::from("../outside"));
        let staging = fixture.root.join("staging");
        let (mut files, mut directories) = (Vec::new(), Vec::new());
        let outcome = place(&entry, &staging, manifest, &mut files, &mut directories);
        assert!(matches!(outcome, Err(Refusal::Damaged(_))), "{outcome:?}");
        assert!(!fixture.root.join("outside").exists());
        assert!(!contained(Path::new("/abs")) && !contained(Path::new("a/../b")));
        assert!(contained(Path::new("debug/deps")));
    }

    #[test]
    fn stale_staging_and_eviction_leftovers_are_reclaimed() {
        let store = std::env::temp_dir().join(format!("sail-store-reclaim-{}", std::process::id()));
        for name in [".staging-1-1", ".evicted-x-1", "entry"] {
            write(&store.join(name).join("manifest.json"), b"{}");
        }
        // Fresh leftovers may belong to a live publisher: kept.
        reclaim(&store, STALE);
        assert!(store.join(".staging-1-1").exists() && store.join(".evicted-x-1").exists());
        reclaim(&store, Duration::ZERO);
        assert!(!store.join(".staging-1-1").exists() && !store.join(".evicted-x-1").exists());
        assert!(store.join("entry").exists(), "entries are never reclaimed");
        assert!(std::fs::remove_dir_all(store).is_ok());
    }

    /// Runs only as the child of `native_artifact_reuse_cannot_be_reenabled`.
    #[test]
    fn enabled_probe() {
        if let Some(expected) = std::env::var_os("SAIL_TEST_STORE_ENABLED") {
            assert_eq!(enabled(), expected == "1");
        }
    }

    #[test]
    fn native_artifact_reuse_cannot_be_reenabled() {
        let executable = std::env::current_exe().unwrap_or_else(|e| unreachable!("fixture: {e}"));
        for (variable, capacity) in [
            (None, None),
            (None, Some("0")),
            (None, Some("4")),
            (Some("LD_SAIL_TEST_PROBE"), Some("4")),
            (Some("DYLD_SAIL_TEST_PROBE"), Some("4")),
        ] {
            let mut command = std::process::Command::new(&executable);
            command
                .args(["--exact", "managed::store::tests::enabled_probe"])
                .env("SAIL_TEST_STORE_ENABLED", "0")
                .env_remove("SAIL_DEPENDENCY_STORE_ENTRIES")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            for (key, _) in std::env::vars_os() {
                let bytes = key.as_encoded_bytes();
                if bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") {
                    command.env_remove(&key);
                }
            }
            if let Some(variable) = variable {
                command.env(variable, "1");
            }
            if let Some(capacity) = capacity {
                command.env("SAIL_DEPENDENCY_STORE_ENTRIES", capacity);
            }
            let status = command
                .status()
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
            assert!(status.success(), "{variable:?}");
        }
    }

    #[test]
    fn eviction_keeps_the_most_recently_used_entries() {
        let store = std::env::temp_dir().join(format!("sail-store-evict-{}", std::process::id()));
        for (seconds, name) in [(1000, "a"), (1001, "b"), (1002, "c"), (1003, "d")] {
            let manifest = store.join(name).join("manifest.json");
            write(&manifest, b"{}");
            let file = std::fs::File::options().write(true).open(&manifest);
            assert!(
                file.and_then(|f| f.set_modified(UNIX_EPOCH + Duration::from_secs(seconds)))
                    .is_ok()
            );
        }
        evict(&store, "d", 2);
        let mut left: Vec<String> = std::fs::read_dir(&store)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        left.sort();
        assert_eq!(left, ["c", "d"]);
        assert!(std::fs::remove_dir_all(store).is_ok());
    }
}
