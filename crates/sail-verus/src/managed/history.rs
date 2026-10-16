//! Retained proofs: reverting to verified content reuses its proof (ADR 0050).
//!
//! A proof context keeps one receipt per target, for the latest content. When
//! a workspace target is edited and the edit is undone (an agent's try, fail,
//! undo; `git checkout`; switching branches), Cargo rebuilds the target and
//! the verifier would verify content Sail has already verified.
//!
//! After a Current run publishes a workspace target's receipt, the runner also
//! keeps a retained proof of it under `<context>/retained/`: the receipt, the
//! files rustc wrote for the unit (`.rmeta`, `.vir`, rustc's dep-info) as
//! content-addressed blobs, the verifier's captured stdout and stderr, and
//! what the driver observed when it verified the unit: the content of every
//! file rustc's dep-info names, the environment the driver received, and its
//! working directory, and the complete source file/directory inventory. The
//! most recently used few per logical target are kept across compiler variants.
//!
//! When Cargo invokes the driver for such a target again, the driver looks
//! among the retained proofs of exactly this target identity (its complete
//! compiler arguments) for one whose recorded inputs all match. It restores
//! the unit's files and replays the recorded output instead of running the
//! verifier only when the receipt and the entry are intact, the proof context
//! is the same, the dependencies' compiled bytes are the same, every file
//! rustc's dep-info names has the same content, the complete source inventory
//! matches and is unchanged across restoration, and the environment the
//! driver received is the same. Recording is bracketed in the same way;
//! uncovered inputs refuse retention. Positive dep-info inputs alone miss
//! newly appearing module candidates. Anything else, including a damaged entry or blob,
//! is an ordinary miss and the driver verifies as usual. Cargo itself has
//! decided that the unit must be rebuilt and prints its usual status line, and
//! records its own fingerprint from the restored dep-info, so the output is
//! what plain Cargo-Verus prints when it verifies the same content.

use super::environment::input_environment_key;
use super::files::{event, file_digest, json};
use super::receipts::{RECEIPT_VERSION, Receipt, Runtime, TargetIdentity, complete, read};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Layout version of retained proofs.
const VERSION: u32 = 2;
/// Retained proofs kept per target by default; `SAIL_RETAINED_PROOFS`
/// overrides it, and `0` turns retention off.
const DEFAULT_PROOFS: usize = 4;
/// What the driver observed for a target it verified, in its sidecar.
const OBSERVATION: &str = "retained.json";
/// Unfinished blob copies older than this were left by a killed run.
const STALE: Duration = Duration::from_secs(3600);

/// How many retained proofs each target keeps.
pub(crate) fn capacity() -> usize {
    std::env::var("SAIL_RETAINED_PROOFS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PROOFS)
}

/// A proof context's retained proofs, beside its receipts.
pub(crate) fn root(receipts: &Path) -> PathBuf {
    receipts.with_file_name("retained")
}

/// Blob names are SHA-256 digests; anything else never names a file.
fn plain_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn target_dir(root: &Path, identity: &TargetIdentity) -> PathBuf {
    let logical = super::files::digest(
        &serde_json::to_vec(&(&identity.manifest, &identity.name, &identity.crate_type))
            .unwrap_or_default(),
    );
    root.join("targets").join(logical)
}

fn blob(root: &Path, sha256: &str) -> PathBuf {
    root.join("blobs").join(sha256)
}

/// One file rustc wrote for the unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Output {
    path: PathBuf,
    sha256: String,
    size: u64,
    executable: bool,
}

/// What the driver observed for a unit it verified successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Observation {
    version: u32,
    identity: TargetIdentity,
    /// The driver's working directory, against which dep-info paths resolve.
    directory: PathBuf,
    /// Variables rustc's dep-info declares (`# env-dep:`).
    declared: BTreeSet<String>,
    /// The value or absence of every proof-context variable, `OUT_DIR` and
    /// every declared variable, as the driver received them from Cargo.
    environment: BTreeMap<String, Option<Vec<u8>>>,
    /// Content of every file rustc's dep-info names.
    inputs: BTreeMap<PathBuf, String>,
    /// Complete source inventory, including the presence of files not named
    /// by dep-info. Positive inputs alone cannot detect new module candidates.
    source_revision: String,
    outputs: Vec<Output>,
}

/// A complete source inventory held around verification or restoration.
/// Reuses managed snapshots, including directory and symlink observations.
pub(crate) struct SourceGuard {
    view: serde_json::Value,
    extras: Vec<PathBuf>,
    before: super::input::InputSnapshot,
    revision: String,
    stamp: String,
    resolution: super::dependency_resolution::Resolution,
}

impl SourceGuard {
    fn begin(runtime: &Runtime, identity: &TargetIdentity, process: &Process) -> Option<Self> {
        let mut view = runtime.retained_sources.get(&identity.manifest)?.clone();
        let mut extras: Vec<PathBuf> = view["sail_source_inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(Path::new)
            .filter_map(Path::parent)
            .map(Path::to_owned)
            .collect();
        // Cargo has finished this crate's script before invoking its driver.
        // Capture all generated inputs and their directory inventory now,
        // rather than binding the prior run's generated bytes.
        if let Some((_, out)) = process.variables.iter().find(|(name, _)| name == "OUT_DIR") {
            extras.push(process.directory.join(out));
        }
        extras.sort();
        extras.dedup();
        // The full directories above replace possibly absent old generated
        // paths; absence or unreadability of a directory refuses retention.
        view["sail_source_inputs"] = serde_json::json!([]);
        let before = source_snapshot(&view, &extras);
        if !before.complete {
            return None;
        }
        let (revision, paths) = source_revision(&before)?;
        let stamp = super::input::mutation_stamp(&before)?;
        let resolution = super::dependency_resolution::Resolution::capture(&paths)?;
        let observed = Self {
            view,
            extras,
            before,
            revision,
            stamp,
            resolution,
        };
        // The first snapshot discovers all roots and aliases. Observation is
        // activated before a second full snapshot proves those preceding
        // bytes/topology/stamps still match the guarded compiler epoch.
        observed.unchanged().then_some(observed)
    }

    fn unchanged(&self) -> bool {
        if !self.resolution.unchanged() {
            return false;
        }
        let after = source_snapshot(&self.view, &self.extras);
        after.complete
            && source_revision(&after)
                .map(|(revision, _)| revision)
                .as_ref()
                == Some(&self.revision)
            && super::input::mutation_stamp(&after).as_ref() == Some(&self.stamp)
            && self.resolution.unchanged()
    }

    fn covers(&self, inputs: &BTreeMap<PathBuf, String>) -> bool {
        self.resolution.unchanged()
            && inputs.iter().all(|(path, digest)| {
                self.resolution
                    .canonical(path)
                    .map(Path::to_owned)
                    .or_else(|| path.canonicalize().ok())
                    .and_then(|canonical| self.before.digests.get(&canonical))
                    == Some(digest)
            })
            && self.resolution.unchanged()
    }
}

/// Bind the snapshot's file bytes and the entries of its already observed
/// directories. Canonical file deduplication alone misses new symlink aliases
/// and empty directories, either of which a compiler or macro can inspect.
fn source_revision(snapshot: &super::input::InputSnapshot) -> Option<(String, Vec<PathBuf>)> {
    let mut resolution_paths = BTreeSet::new();
    for root in &snapshot.roots {
        // Cargo configuration roots may be absent. Keep the existing absence
        // checks, and observe resolution through their nearest existing parent.
        let mut path = std::path::absolute(root).ok()?;
        loop {
            match std::fs::symlink_metadata(&path) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !path.pop() {
                        return None;
                    }
                }
                Err(_) => return None,
            }
        }
        resolution_paths.insert(path);
    }
    let excluded: BTreeSet<_> = snapshot
        .excluded
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect();
    let mut hash = Sha256::new();
    let mut bytes = |value: &[u8]| {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    };
    bytes(snapshot.revision.as_bytes());
    for directory in &snapshot.observation_paths {
        if !directory.is_dir() {
            continue;
        }
        resolution_paths.insert(directory.to_owned());
        bytes(b"directory-root");
        bytes(directory.as_os_str().as_encoded_bytes());
        let mut entries = std::fs::read_dir(directory)
            .ok()?
            .collect::<std::io::Result<Vec<_>>>()
            .ok()?;
        entries.sort_by_cached_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name();
            if [".git", ".claude", ".codex"]
                .iter()
                .any(|skip| name == *skip)
            {
                continue;
            }
            let kind = entry.file_type().ok()?;
            if excluded.contains(&entry.path())
                || (kind.is_symlink()
                    && entry
                        .path()
                        .canonicalize()
                        .is_ok_and(|p| excluded.contains(&p)))
            {
                continue;
            }
            bytes(b"entry");
            bytes(name.as_encoded_bytes());
            if kind.is_symlink() {
                // Canonical file deduplication can omit this alias from the
                // snapshot's roots. Its full resolution chain still matters.
                resolution_paths.insert(entry.path());
                bytes(b"symlink");
                bytes(
                    std::fs::read_link(entry.path())
                        .ok()?
                        .as_os_str()
                        .as_encoded_bytes(),
                );
            } else if kind.is_dir() {
                bytes(b"directory");
            } else if kind.is_file() {
                bytes(b"file");
            } else {
                return None;
            }
        }
        bytes(b"end-directory");
    }
    Some((
        format!("{:x}", hash.finalize()),
        resolution_paths.into_iter().collect(),
    ))
}

fn source_snapshot(view: &serde_json::Value, extras: &[PathBuf]) -> super::input::InputSnapshot {
    let mut snapshot = super::input::snapshot_metadata(view, extras);
    let invocation = Path::new(
        view["sail_invocation_directory"]
            .as_str()
            .unwrap_or_default(),
    );
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    snapshot.roots.extend(super::input::configuration_paths(
        invocation,
        home.as_deref(),
    ));
    snapshot
}

/// Starts an eligible target's complete source observation before the
/// verifier or retained-proof lookup. Missing coverage uses fresh verification.
pub(crate) fn begin(runtime: &Runtime, identity: &TargetIdentity) -> Option<SourceGuard> {
    SourceGuard::begin(runtime, identity, &Process::current()?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    version: u32,
    observation: Observation,
    receipt: Receipt,
    /// Blob digests of the verifier's captured stdout and stderr.
    stdout: String,
    stderr: String,
    checksum: String,
}

impl Entry {
    fn seal(mut self) -> Self {
        self.checksum = String::new();
        self.checksum = super::files::digest(&serde_json::to_vec(&self).unwrap_or_default());
        self
    }
    fn intact(&self) -> bool {
        self.clone().seal().checksum == self.checksum
    }
}

/// A retained proof the driver restored: the receipt it carries and the
/// verifier output to replay.
pub(crate) struct Restored {
    pub receipt: Receipt,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// The driver process as far as a retained proof depends on it.
struct Process {
    directory: PathBuf,
    variables: Vec<(OsString, OsString)>,
}

impl Process {
    fn current() -> Option<Self> {
        Some(Self {
            directory: std::env::current_dir().ok()?,
            variables: std::env::vars_os().collect(),
        })
    }

    /// The environment as far as it can reach a proof: the proof-context
    /// variables (which include Cargo's per-unit `CARGO_*` and the Verus
    /// driver's `__VERUS_*`), `OUT_DIR`, and every declared variable. Cargo's
    /// jobserver handle names this invocation's pipes, not an input.
    fn environment(&self, declared: &BTreeSet<String>) -> BTreeMap<String, Option<Vec<u8>>> {
        let mut values = BTreeMap::new();
        for (key, value) in &self.variables {
            if key == "CARGO_MAKEFLAGS" {
                continue;
            }
            let name = key.to_string_lossy();
            if input_environment_key(key) || key == "OUT_DIR" || declared.contains(&*name) {
                values.insert(name.into_owned(), Some(value.as_encoded_bytes().to_vec()));
            }
        }
        for name in declared {
            values.entry(name.clone()).or_insert(None);
        }
        values
    }
}

fn argument(args: &[OsString], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .and_then(|w| w[1].to_str().map(str::to_owned))
}

/// The unit's output directory and file stem (`<crate><extra-filename>`).
/// Without Cargo's unit hash in `-C extra-filename` the names are not unique
/// to the unit, and nothing is retained.
fn unit(args: &[OsString]) -> Option<(PathBuf, String)> {
    let directory = argument(args, "--out-dir")?;
    let name = argument(args, "--crate-name")?;
    let extra = args
        .windows(2)
        .filter(|w| w[0] == "-C")
        .find_map(|w| {
            w[1].to_str()?
                .strip_prefix("extra-filename=")
                .map(str::to_owned)
        })
        .or_else(|| {
            args.iter().find_map(|a| {
                a.to_str()?
                    .strip_prefix("-Cextra-filename=")
                    .map(str::to_owned)
            })
        })?;
    if extra.is_empty() || name.is_empty() {
        return None;
    }
    Some((PathBuf::from(directory), format!("{name}{extra}")))
}

fn named(file: &str, stem: &str) -> bool {
    file == stem
        || file
            .strip_prefix(stem)
            .is_some_and(|rest| rest.starts_with('.'))
        || file
            .strip_prefix("lib")
            .and_then(|rest| rest.strip_prefix(stem))
            .is_some_and(|rest| rest.starts_with('.'))
}

/// Every file of the unit in its output directory. A directory of the unit
/// (a debug-symbol bundle) is not retained, so the unit is not either.
fn unit_files(directory: &Path, stem: &str) -> Option<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !named(name, stem) {
            continue;
        }
        if !entry.file_type().ok()?.is_file() {
            return None;
        }
        files.push(entry.path());
    }
    files.sort();
    Some(files)
}

/// Splits a Makefile rule at its first unescaped `:` that ends the target.
fn rule(line: &str) -> Option<(String, &str)> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b':' if bytes.get(index + 1).is_none_or(|b| *b == b' ') => {
                return Some((
                    line[..index].replace("\\ ", " "),
                    line.get(index + 1..).unwrap_or_default(),
                ));
            }
            _ => index += 1,
        }
    }
    None
}

/// Space-separated paths, where rustc escapes a space in a path as `\ `.
fn paths(list: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut current = String::new();
    let mut chars = list.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&' ') => {
                current.push(' ');
                chars.next();
            }
            ' ' => {
                if !current.is_empty() {
                    paths.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        paths.push(current);
    }
    paths
}

/// The files and variables rustc's dep-info names, or `None` when a line is
/// not understood.
fn dep_info(text: &str, directory: &Path) -> Option<(BTreeSet<PathBuf>, BTreeSet<String>)> {
    let mut inputs = BTreeSet::new();
    let mut declared = BTreeSet::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# env-dep:") {
            let name = rest.split_once('=').map_or(rest, |(name, _)| name);
            declared.insert(name.to_owned());
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let (target, dependencies) = rule(line)?;
        let dependencies = paths(dependencies);
        if dependencies.is_empty() {
            inputs.insert(directory.join(target));
        }
        inputs.extend(dependencies.into_iter().map(|path| directory.join(path)));
    }
    (!inputs.is_empty()).then_some((inputs, declared))
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

/// Records what a successful verification of `identity` read and wrote, in
/// `sidecar`, for the runner to retain if the run ends Current. Any doubt
/// records nothing.
pub(crate) fn observe(
    args: &[OsString],
    identity: &TargetIdentity,
    sidecar: &Path,
    source: &SourceGuard,
) {
    if let Some(observation) =
        Process::current().and_then(|process| observation(args, identity, &process, source))
    {
        let _ = json(&sidecar.join(OBSERVATION), &observation);
    }
}

fn observation(
    args: &[OsString],
    identity: &TargetIdentity,
    process: &Process,
    source: &SourceGuard,
) -> Option<Observation> {
    let (out_dir, stem) = unit(args)?;
    let files = unit_files(&out_dir, &stem)?;
    let info_path = out_dir.join(format!("{stem}.d"));
    if !files.contains(&info_path) {
        return None;
    }
    let info = std::fs::read_to_string(&info_path).ok()?;
    let (inputs, declared) = dep_info(&info, &process.directory)?;
    let input_paths: Vec<&Path> = inputs.iter().map(PathBuf::as_path).collect();
    let mut recorded_inputs = BTreeMap::new();
    for (path, digest) in inputs.iter().zip(super::files::digest_all(&input_paths)) {
        recorded_inputs.insert(path.clone(), digest.ok()?);
    }
    if !source.covers(&recorded_inputs) || !source.unchanged() {
        return None;
    }
    let file_paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    let mut outputs = Vec::new();
    for (path, digest) in files.iter().zip(super::files::digest_all(&file_paths)) {
        let meta = std::fs::symlink_metadata(path).ok()?;
        outputs.push(Output {
            path: path.clone(),
            sha256: digest.ok()?,
            size: meta.len(),
            executable: executable(&meta),
        });
    }
    Some(Observation {
        version: VERSION,
        identity: identity.clone(),
        directory: process.directory.clone(),
        environment: process.environment(&declared),
        declared,
        inputs: recorded_inputs,
        source_revision: source.revision.clone(),
        outputs,
    })
}

/// Copies `from` to `to` through a temporary sibling, hashing the bytes
/// written; `to` is replaced only when they have the `expected` digest.
fn copy_checked(from: &Path, to: &Path, expected: &str, executable: bool) -> Result<()> {
    let failed = |e: std::io::Error| Error::new("SAIL3133", format!("{}: {e}", to.display()));
    let temporary = to.with_file_name(format!(
        ".{}.sail-retained-{}",
        to.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let mut input = std::fs::File::open(from).map_err(failed)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if executable { 0o755 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    let mut output = options.open(&temporary).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let outcome = (|| -> std::io::Result<bool> {
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            output.write_all(&buffer[..n])?;
        }
        drop(output);
        if format!("{:x}", hasher.finalize()) != expected {
            return Ok(false);
        }
        std::fs::rename(&temporary, to)?;
        Ok(true)
    })();
    match outcome {
        Ok(true) => Ok(()),
        Ok(false) => {
            let _ = std::fs::remove_file(&temporary);
            Err(Error::new(
                "SAIL3133",
                format!("{} does not have its recorded digest", from.display()),
            ))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(failed(e))
        }
    }
}

fn read_blob(root: &Path, sha256: &str) -> Option<Vec<u8>> {
    if !plain_digest(sha256) {
        return None;
    }
    let bytes = std::fs::read(blob(root, sha256)).ok()?;
    (super::files::digest(&bytes) == sha256).then_some(bytes)
}

/// Puts `path`'s bytes in the blob store under `sha256`, unless an intact
/// blob is already there.
fn store_blob(root: &Path, path: &Path, sha256: &str) -> Result<()> {
    if !plain_digest(sha256) {
        return Err(Error::new("SAIL3133", "malformed digest"));
    }
    let destination = blob(root, sha256);
    if file_digest(&destination).is_ok_and(|actual| actual == sha256) {
        return Ok(());
    }
    copy_checked(path, &destination, sha256, false)
}

/// Restores a retained proof of `identity` whose recorded inputs all match,
/// most recently used first; otherwise `None`, and nothing of an entry is
/// left in the target.
pub(crate) fn restore(
    runtime: &Runtime,
    identity: &TargetIdentity,
    args: &[OsString],
    dependencies: &BTreeMap<PathBuf, String>,
    source: &SourceGuard,
) -> Option<Restored> {
    restore_in(
        runtime,
        identity,
        args,
        dependencies,
        &Process::current()?,
        source,
    )
}

fn restore_in(
    runtime: &Runtime,
    identity: &TargetIdentity,
    args: &[OsString],
    dependencies: &BTreeMap<PathBuf, String>,
    process: &Process,
    source: &SourceGuard,
) -> Option<Restored> {
    if runtime.engine.is_some() || !runtime.retained.contains(&identity.manifest) {
        return None;
    }
    let root = root(&runtime.receipts);
    let request = Request {
        context: &runtime.context_sha256,
        identity,
        args,
        dependencies,
        process,
        source,
    };
    let mut digests = Digests::default();
    for path in entries(&target_dir(&root, identity)) {
        let Some(entry) = read::<Entry>(&path) else {
            damaged(&path, &identity.name, "unreadable entry");
            continue;
        };
        match usable(&request, &entry, &mut digests) {
            Ok(()) => {}
            Err(Refusal::Miss(reason)) => {
                event(
                    &serde_json::json!({"event":"retained-miss","target":identity.name,"reason":reason}),
                );
                continue;
            }
            Err(Refusal::Damaged(reason)) => {
                damaged(&path, &identity.name, reason);
                continue;
            }
        }
        if let Some(restored) = place(&root, &path, &identity.name, &entry) {
            if source.unchanged() {
                return Some(restored);
            }
            for output in &entry.observation.outputs {
                let _ = std::fs::remove_file(&output.path);
            }
            return None;
        }
    }
    None
}

/// A damaged entry is removed, so it is not checked again.
fn damaged(path: &Path, target: &str, reason: &str) {
    event(&serde_json::json!({"event":"retained-damaged","target":target,"reason":reason}));
    let _ = std::fs::remove_file(path);
}

/// Copies an entry's files into the target and reads its recorded output.
fn place(root: &Path, path: &Path, target: &str, entry: &Entry) -> Option<Restored> {
    let (Some(stdout), Some(stderr)) = (
        read_blob(root, &entry.stdout),
        read_blob(root, &entry.stderr),
    ) else {
        damaged(path, target, "recorded output");
        return None;
    };
    let mut written = Vec::new();
    for output in &entry.observation.outputs {
        if copy_checked(
            &blob(root, &output.sha256),
            &output.path,
            &output.sha256,
            output.executable,
        )
        .is_err()
        {
            // The unit's files are either all the retained ones or Cargo's
            // own: remove what was placed and verify as usual.
            for placed in &written {
                let _ = std::fs::remove_file(placed);
            }
            damaged(path, target, "stored file");
            return None;
        }
        written.push(output.path.clone());
    }
    // Most recently used first, in lookup and eviction.
    let _ = std::fs::File::options()
        .append(true)
        .open(path)
        .and_then(|file| file.set_modified(SystemTime::now()));
    Some(Restored {
        receipt: entry.receipt.clone(),
        stdout,
        stderr,
    })
}

/// A target's retained proofs, most recently used first.
fn entries(target: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(target) else {
        return Vec::new();
    };
    let mut entries: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| {
            let used = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (used, e.path())
        })
        .collect();
    entries.sort_by(|a, b| b.cmp(a));
    entries.into_iter().map(|(_, path)| path).collect()
}

/// Current content digests, each file read at most once per lookup.
#[derive(Default)]
struct Digests(BTreeMap<PathBuf, Option<String>>);

impl Digests {
    fn all_match(&mut self, expected: &BTreeMap<PathBuf, String>) -> bool {
        let missing: Vec<&Path> = expected
            .keys()
            .filter(|path| !self.0.contains_key(*path))
            .map(PathBuf::as_path)
            .collect();
        for (path, digest) in missing.iter().zip(super::files::digest_all(&missing)) {
            self.0.insert(path.to_path_buf(), digest.ok());
        }
        expected
            .iter()
            .all(|(path, sha)| self.0.get(path).and_then(Option::as_ref) == Some(sha))
    }
}

/// What the driver is asked to verify now.
struct Request<'a> {
    context: &'a str,
    identity: &'a TargetIdentity,
    args: &'a [OsString],
    dependencies: &'a BTreeMap<PathBuf, String>,
    process: &'a Process,
    source: &'a SourceGuard,
}

enum Refusal {
    /// The entry describes other inputs.
    Miss(&'static str),
    /// The entry fails its own checks.
    Damaged(&'static str),
}

fn usable(
    request: &Request<'_>,
    entry: &Entry,
    digests: &mut Digests,
) -> std::result::Result<(), Refusal> {
    let receipt = &entry.receipt;
    let observation = &entry.observation;
    if entry.version != VERSION || !entry.intact() {
        return Err(Refusal::Damaged("entry checksum"));
    }
    if receipt.version != RECEIPT_VERSION || receipt.clone().seal().checksum != receipt.checksum {
        return Err(Refusal::Damaged("receipt checksum"));
    }
    if observation.identity != *request.identity || receipt.invocation.identity != *request.identity
    {
        return Err(Refusal::Miss("compiler identity"));
    }
    if receipt.context != request.context {
        return Err(Refusal::Miss("proof context"));
    }
    if !receipt.invocation.success || !complete(&receipt.invocation.report) {
        return Err(Refusal::Damaged("incomplete receipt"));
    }
    let (directory, stem) = unit(request.args).ok_or(Refusal::Miss("unit"))?;
    let outputs: BTreeMap<&Path, &str> = observation
        .outputs
        .iter()
        .map(|o| (o.path.as_path(), o.sha256.as_str()))
        .collect();
    let contained = observation.outputs.iter().all(|o| {
        o.path.parent() == Some(directory.as_path())
            && o.path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| named(name, &stem))
            && plain_digest(&o.sha256)
    });
    if !contained
        || outputs.len() != observation.outputs.len()
        || !receipt
            .artifact_hashes
            .iter()
            .all(|(path, sha)| outputs.get(path.as_path()) == Some(&sha.as_str()))
    {
        return Err(Refusal::Damaged("outputs"));
    }
    if receipt.invocation.dependencies != *request.dependencies {
        return Err(Refusal::Miss("dependencies"));
    }
    if observation.source_revision != request.source.revision
        || !request.source.covers(&observation.inputs)
    {
        return Err(Refusal::Miss("source inventory"));
    }
    if request.process.directory != observation.directory {
        return Err(Refusal::Miss("directory"));
    }
    if request.process.environment(&observation.declared) != observation.environment {
        return Err(Refusal::Miss("environment"));
    }
    if !digests.all_match(&observation.inputs) {
        return Err(Refusal::Miss("inputs"));
    }
    Ok(())
}

/// Retains a receipt this Current run published, with the files and output
/// its driver observed, and keeps the `capacity` most recently used proofs of
/// the target. A receipt the driver restored is already retained.
pub(super) fn retain(root: &Path, run: &Path, receipt: &Receipt, capacity: usize) -> Result<()> {
    if capacity == 0 || receipt.invocation.restored.is_some() {
        return Ok(());
    }
    let identity = &receipt.invocation.identity;
    let sidecar = run.join("sidecars").join(identity.key());
    let Some(observation) = read::<Observation>(&sidecar.join(OBSERVATION)) else {
        return Ok(());
    };
    let outputs: BTreeMap<&Path, &str> = observation
        .outputs
        .iter()
        .map(|o| (o.path.as_path(), o.sha256.as_str()))
        .collect();
    if observation.version != VERSION
        || observation.identity != *identity
        || !receipt
            .artifact_hashes
            .iter()
            .all(|(path, sha)| outputs.get(path.as_path()) == Some(&sha.as_str()))
    {
        return Ok(());
    }
    std::fs::create_dir_all(root.join("blobs"))
        .map_err(|e| Error::new("SAIL3133", e.to_string()))?;
    reclaim(&root.join("blobs"), STALE);
    // A blob's bytes are checked against what the driver observed, so an
    // output rewritten since then is never retained.
    for output in &observation.outputs {
        store_blob(root, &output.path, &output.sha256)?;
    }
    let mut recorded = Vec::new();
    for name in ["verus.stdout", "verus.stderr"] {
        let path = sidecar.join(name);
        let sha256 = file_digest(&path)?;
        store_blob(root, &path, &sha256)?;
        recorded.push(sha256);
    }
    let [stdout, stderr]: [String; 2] = recorded
        .try_into()
        .map_err(|_| Error::new("SAIL3133", "recorded output"))?;
    let entry = Entry {
        version: VERSION,
        observation,
        receipt: receipt.clone(),
        stdout,
        stderr,
        checksum: String::new(),
    }
    .seal();
    // One entry per set of inputs: verifying the same inputs again replaces
    // the older proof of them.
    let name = super::files::digest(
        &serde_json::to_vec(&(
            &entry.observation.identity,
            &entry.observation.source_revision,
            &entry.observation.inputs,
            &entry.observation.environment,
            &entry.receipt.invocation.dependencies,
        ))
        .unwrap_or_default(),
    );
    let target = target_dir(root, identity);
    std::fs::create_dir_all(&target).map_err(|e| Error::new("SAIL3133", e.to_string()))?;
    json(&target.join(format!("{name}.json")), &entry)?;
    evict(&target, capacity);
    collect(root);
    Ok(())
}

/// Keeps the `capacity` most recently used proofs of one target.
fn evict(target: &Path, capacity: usize) {
    for path in entries(target).into_iter().skip(capacity) {
        let _ = std::fs::remove_file(path);
    }
}

#[derive(Deserialize)]
struct References {
    version: u32,
    observation: OutputReferences,
    stdout: String,
    stderr: String,
}

#[derive(Deserialize)]
struct OutputReferences {
    outputs: Vec<BlobReference>,
}

#[derive(Deserialize)]
struct BlobReference {
    sha256: String,
}

/// Deletes blobs no retained proof refers to, and unreadable entries.
fn collect(root: &Path) {
    let mut referenced = BTreeSet::new();
    for target in std::fs::read_dir(root.join("targets"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for entry in std::fs::read_dir(target.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            if let Some(references) =
                read::<References>(&path).filter(|entry| entry.version == VERSION)
            {
                referenced.extend(references.observation.outputs.into_iter().map(|o| o.sha256));
                referenced.insert(references.stdout);
                referenced.insert(references.stderr);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    for entry in std::fs::read_dir(root.join("blobs"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if plain_digest(&name) && !referenced.contains(&name) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Removes temporary blob copies left by a killed run.
fn reclaim(blobs: &Path, age: Duration) {
    for entry in std::fs::read_dir(blobs).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|elapsed| elapsed >= age);
        if name.starts_with('.') && old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::receipts::{Invocation, artifact_hashes};
    use sail_api::output::CrateReport;

    /// A verified unit in a scratch workspace, as Cargo and the driver see it.
    struct Fixture {
        root: PathBuf,
        args: Vec<OsString>,
        identity: TargetIdentity,
        source: PathBuf,
        dependency: PathBuf,
        outputs: Vec<PathBuf>,
        process: Process,
        runtime: Runtime,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        assert!(std::fs::create_dir_all(path.parent().unwrap_or(Path::new("/"))).is_ok());
        assert!(std::fs::write(path, bytes).is_ok());
    }

    fn fixture(label: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "sail-retained-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let workspace = root.join("ws");
        let deps = root.join("target/debug/deps");
        let source = workspace.join("src/lib.rs");
        let dependency = deps.join("libdep-0123.rmeta");
        write(&source, b"pub fn a() {}\n");
        write(&workspace.join("src/extra file.rs"), b"// with a space\n");
        write(&dependency, b"dependency");
        let args: Vec<OsString> = [
            "--crate-name",
            "demo",
            "--edition=2021",
            "src/lib.rs",
            "--crate-type",
            "lib",
            "-C",
            "extra-filename=-abcd",
            "--out-dir",
            &deps.to_string_lossy(),
            "--extern",
            &format!("dep={}", dependency.display()),
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let identity = TargetIdentity {
            manifest: workspace.join("Cargo.toml"),
            name: "demo".to_owned(),
            crate_type: "lib".to_owned(),
            rustc_args_sha256: "args".to_owned(),
        };
        let outputs = vec![
            deps.join("demo-abcd.d"),
            deps.join("libdemo-abcd.rmeta"),
            deps.join("libdemo-abcd.vir"),
        ];
        let process = Process {
            directory: workspace.clone(),
            variables: vec![
                (OsString::from("CARGO_PKG_NAME"), OsString::from("demo")),
                (
                    OsString::from("CARGO_MAKEFLAGS"),
                    OsString::from("-j --jobserver-auth=3,4"),
                ),
                (
                    OsString::from(" __VERUS_DRIVER_ARGS__"),
                    OsString::from("x"),
                ),
                (OsString::from("TERM"), OsString::from("dumb")),
            ],
        };
        let runtime = Runtime {
            run_id: "run-b".to_owned(),
            args: Vec::new(),
            z3: PathBuf::new(),
            z3_sha256: String::new(),
            daemon: false,
            sidecars: root.join("ctx/runs/run-b/sidecars"),
            invocations: root.join("ctx/runs/run-b/invocations"),
            original_wrapper: PathBuf::new(),
            receipts: root.join("ctx/receipts"),
            context_sha256: "ctx".to_owned(),
            input_revision: String::new(),
            packages: BTreeMap::new(),
            target_kinds: BTreeMap::new(),
            engine: None,
            attribution_logs: false,
            retained: BTreeSet::from([workspace.join("Cargo.toml")]),
            retained_sources: BTreeMap::from([(
                workspace.join("Cargo.toml"),
                serde_json::json!({
                    "workspace_root": workspace,
                    "sail_invocation_directory": workspace,
                    "target_directory": root.join("target"),
                    "packages": [{"manifest_path": workspace.join("Cargo.toml")}],
                }),
            )]),
        };
        Fixture {
            root,
            args,
            identity,
            source,
            dependency,
            outputs,
            process,
            runtime,
        }
    }

    impl Fixture {
        /// The driver verified the source as it is now: its outputs and its
        /// observation in `run`'s sidecar, and the receipt the run publishes.
        fn verify(&self, run: &str, content: &[u8]) -> Receipt {
            write(&self.source, content);
            let deps = self.outputs[0]
                .parent()
                .unwrap_or(Path::new("/"))
                .to_owned();
            write(
                &self.outputs[0],
                format!(
                    "{}: src/lib.rs src/extra\\ file.rs\n\nsrc/lib.rs:\nsrc/extra\\ file.rs:\n\n# env-dep:CARGO_PKG_NAME=demo\n# env-dep: __VERUS_DRIVER_ARGS__=x\n# env-dep:UNSET_DECLARED\n",
                    deps.join("libdemo-abcd.rmeta").display()
                )
                .as_bytes(),
            );
            write(&self.outputs[1], &[b"rmeta:".as_slice(), content].concat());
            write(&self.outputs[2], &[b"vir:".as_slice(), content].concat());
            let sidecar = self
                .root
                .join("ctx/runs")
                .join(run)
                .join("sidecars")
                .join(self.identity.key());
            write(
                &sidecar.join("verus.stdout"),
                b"{\"verification-results\":{}}\n",
            );
            write(&sidecar.join("verus.stderr"), b"");
            let source = SourceGuard::begin(&self.runtime, &self.identity, &self.process);
            assert!(source.is_some(), "complete source inventory");
            let observed = source
                .and_then(|source| observation(&self.args, &self.identity, &self.process, &source));
            assert!(observed.is_some(), "a verified unit is observed");
            assert!(json(&sidecar.join(OBSERVATION), &observed).is_ok());
            self.receipt(run)
        }

        fn receipt(&self, run: &str) -> Receipt {
            let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
            report.error = None;
            report.exit_code = Some(0);
            report.crates.push(CrateReport {
                name: Some("demo".to_owned()),
                verus: sail_api::output::VerusResults {
                    verified: Some(1),
                    errors: Some(0),
                    success: Some(true),
                    encountered_vir_error: Some(false),
                },
                functions: Vec::new(),
            });
            Receipt {
                version: RECEIPT_VERSION,
                checksum: String::new(),
                package_id: "demo".to_owned(),
                name: "demo".to_owned(),
                kind: vec!["lib".to_owned()],
                context: "ctx".to_owned(),
                input_revision: "revision".to_owned(),
                invocation: Invocation {
                    identity: self.identity.clone(),
                    run_id: run.to_owned(),
                    report,
                    success: true,
                    dependencies: artifact_hashes(std::slice::from_ref(&self.dependency))
                        .unwrap_or_default(),
                    restored: None,
                },
                artifact_hashes: artifact_hashes(&self.outputs[1..]).unwrap_or_default(),
                committed: PathBuf::new(),
                declared_environment: BTreeMap::new(),
            }
            .seal()
        }

        fn retain(&self, run: &str, receipt: &Receipt, capacity: usize) {
            assert!(
                retain(
                    &root(&self.runtime.receipts),
                    &self.root.join("ctx/runs").join(run),
                    receipt,
                    capacity
                )
                .is_ok()
            );
        }

        fn dependencies(&self) -> BTreeMap<PathBuf, String> {
            artifact_hashes(std::slice::from_ref(&self.dependency)).unwrap_or_default()
        }

        fn restore_with(&self, runtime: &Runtime, process: &Process) -> Option<Restored> {
            let source = SourceGuard::begin(runtime, &self.identity, process)?;
            restore_in(
                runtime,
                &self.identity,
                &self.args,
                &self.dependencies(),
                process,
                &source,
            )
        }

        fn restore(&self) -> Option<Restored> {
            self.restore_with(&self.runtime, &self.process)
        }

        fn output_bytes(&self) -> Vec<Option<Vec<u8>>> {
            self.outputs.iter().map(|p| std::fs::read(p).ok()).collect()
        }

        fn entries(&self) -> Vec<PathBuf> {
            entries(&target_dir(&root(&self.runtime.receipts), &self.identity))
        }

        fn blobs(&self) -> BTreeSet<String> {
            std::fs::read_dir(root(&self.runtime.receipts).join("blobs"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        }

        fn with_variables(&self, variables: Vec<(OsString, OsString)>) -> Process {
            Process {
                directory: self.process.directory.clone(),
                variables,
            }
        }
    }

    fn sha(bytes: &str) -> String {
        super::super::files::digest(bytes.as_bytes())
    }

    #[test]
    fn dep_info_names_inputs_and_declared_variables() {
        let text = "/t/deps/x-1.d: src/lib.rs src/a\\ b.rs /abs/gen.rs\n\nsrc/lib.rs:\nsrc/a\\ b.rs:\n/abs/gen.rs:\n\n# env-dep:CARGO_PKG_NAME=x\n# env-dep: __VERUS_DRIVER_ARGS__=a=b\n# env-dep:UNSET\n# checksum:ignored\n";
        let parsed = dep_info(text, Path::new("/ws"));
        assert_eq!(
            parsed,
            Some((
                BTreeSet::from([
                    PathBuf::from("/ws/src/lib.rs"),
                    PathBuf::from("/ws/src/a b.rs"),
                    PathBuf::from("/abs/gen.rs"),
                ]),
                BTreeSet::from([
                    "CARGO_PKG_NAME".to_owned(),
                    " __VERUS_DRIVER_ARGS__".to_owned(),
                    "UNSET".to_owned()
                ])
            ))
        );
        assert!(dep_info("no rule here\n", Path::new("/ws")).is_none());
        assert!(named("libdemo-ab.rmeta", "demo-ab") && named("demo-ab.d", "demo-ab"));
        assert!(named("demo-ab", "demo-ab") && !named("demo-abc.d", "demo-ab"));
        assert!(!named("libdemo-ab", "demo-ab"));
        let args: Vec<OsString> = ["--crate-name", "x", "--out-dir", "/o"]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(unit(&args).is_none(), "no unit hash, no retained proof");
    }

    #[test]
    fn reverting_restores_the_retained_unit_and_its_output() {
        let f = fixture("revert");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        f.retain("run-a", &a, 4);
        let verified_a = f.output_bytes();
        let b = f.verify("run-b", b"pub fn b() {}\n");
        f.retain("run-b", &b, 4);
        let verified_b = f.output_bytes();
        // B is current: B's own proof matches, not A's.
        let restored = f.restore().map(|r| r.receipt.invocation.run_id);
        assert_eq!(restored.as_deref(), Some("run-b"));
        assert_eq!(f.output_bytes(), verified_b);
        // Undo the edit: A's proof, outputs and recorded output come back.
        write(&f.source, b"pub fn a() {}\n");
        let restored = f.restore();
        assert!(restored.is_some(), "A is retained");
        if let Some(restored) = restored {
            assert_eq!(restored.receipt.invocation.run_id, "run-a");
            assert_eq!(restored.receipt.checksum, a.checksum);
            assert_eq!(restored.stdout, b"{\"verification-results\":{}}\n");
            assert_eq!(restored.stderr, b"");
        }
        assert_eq!(f.output_bytes(), verified_a);
    }

    #[test]
    fn any_difference_in_a_recorded_input_verifies_again() {
        let f = fixture("differs");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        f.retain("run-a", &a, 4);
        let _ = f.verify("run-b", b"pub fn b() {}\n");
        let verified_b = f.output_bytes();
        // One byte differs from A.
        write(&f.source, b"pub fn a() {} \n");
        assert!(f.restore().is_none());
        write(&f.source, b"pub fn a() {}\n");
        // Another dep-info input changed.
        write(&f.root.join("ws/src/extra file.rs"), b"// changed\n");
        assert!(f.restore().is_none());
        write(&f.root.join("ws/src/extra file.rs"), b"// with a space\n");
        // A dependency's compiled bytes changed.
        write(&f.dependency, b"other dependency");
        assert!(f.restore().is_none());
        write(&f.dependency, b"dependency");
        // A variable Cargo gives the driver changed, or a declared variable
        // was set.
        let mut changed = f.process.variables.clone();
        changed[0].1 = OsString::from("other");
        assert!(
            f.restore_with(&f.runtime, &f.with_variables(changed))
                .is_none()
        );
        let mut declared = f.process.variables.clone();
        declared.push((OsString::from("UNSET_DECLARED"), OsString::from("1")));
        assert!(
            f.restore_with(&f.runtime, &f.with_variables(declared))
                .is_none()
        );
        // Another working directory, another proof context, or a run that
        // may not restore.
        let elsewhere = Process {
            directory: f.root.clone(),
            variables: f.process.variables.clone(),
        };
        assert!(f.restore_with(&f.runtime, &elsewhere).is_none());
        let mut runtime = f.runtime.clone();
        runtime.context_sha256 = "other".to_owned();
        assert!(f.restore_with(&runtime, &f.process).is_none());
        runtime = f.runtime.clone();
        runtime.retained.clear();
        assert!(f.restore_with(&runtime, &f.process).is_none());
        // No refusal placed anything.
        assert_eq!(f.output_bytes(), verified_b);
        // Cargo's jobserver handle and variables outside the proof context
        // are not inputs: A is restored.
        let mut unbound = f.process.variables.clone();
        unbound[1].1 = OsString::from("-j --jobserver-auth=7,8");
        unbound[3].1 = OsString::from("xterm");
        assert!(
            f.restore_with(&f.runtime, &f.with_variables(unbound))
                .is_some()
        );
    }

    #[test]
    fn newly_appearing_source_inputs_refuse_restoration() {
        let f = fixture("new-module");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        f.retain("run-a", &a, 4);
        let old = f.output_bytes();
        // The compiler's earlier dep-info says nothing about this new
        // module candidate. Its presence alone can turn a valid module
        // resolution into E0761; every earlier positive input still matches.
        write(&f.root.join("ws/src/leaf/mod.rs"), b"pub fn extra() {}\n");
        assert!(f.restore().is_none());
        assert_eq!(f.output_bytes(), old);
    }

    #[test]
    fn source_inventory_brackets_writes_and_refuses_uncovered_inputs() {
        let f = fixture("source-guard");
        let _ = f.verify("run-a", b"pub fn a() {}\n");
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process);
        assert!(source.is_some());
        if let Some(source) = source {
            assert!(source.unchanged());
            write(&f.source, b"pub fn a() {}\n");
            assert!(!source.unchanged(), "same bytes rewritten during a run");
        }
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process);
        let outside = f.root.join("unobserved/input.rs");
        write(&outside, b"outside\n");
        if let Some(source) = source {
            let extra = BTreeMap::from([(outside.clone(), sha("outside\n"))]);
            assert!(!source.covers(&extra));
        }
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process);
        if let Some(source) = source {
            write(&f.root.join("ws/src/new.rs"), b"new\n");
            assert!(!source.unchanged(), "a newly present input is observed");
        }
    }

    #[test]
    fn source_inventory_observes_ordinary_ancestor_replacement_and_restoration() {
        let f = fixture("ancestor-aba");
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process)
            .unwrap_or_else(|| unreachable!("complete initial inventory"));
        let parked = f.root.with_extension("parked");
        assert!(std::fs::rename(&f.root, &parked).is_ok());
        write(&f.source, b"pub fn a() { this does not compile }\n");
        assert_ne!(
            std::fs::read(&f.source).ok(),
            Some(b"pub fn a() {}\n".to_vec())
        );
        assert!(std::fs::remove_dir_all(&f.root).is_ok());
        assert!(std::fs::rename(&parked, &f.root).is_ok());
        assert_eq!(
            std::fs::read(&f.source).ok(),
            Some(b"pub fn a() {}\n".to_vec())
        );
        assert!(
            !source.unchanged(),
            "a different source tree was observable during the guarded epoch"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_inventory_observes_deduplicated_alias_target_chain_aba() {
        use std::os::unix::fs::symlink;
        let f = fixture("indirect-alias-aba");
        let workspace = f.root.join("ws");
        let hop = f.root.join("hop");
        let alias = workspace.join("src/alias");
        let bad = f.root.join("bad");
        write(&bad.join("src/lib.rs"), b"this does not compile\n");
        assert!(symlink(&workspace, &hop).is_ok());
        assert!(symlink(&hop, &alias).is_ok());
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process)
            .unwrap_or_else(|| unreachable!("complete initial inventory"));
        assert!(
            !source.before.roots.contains(&alias),
            "the alias target is already visited"
        );
        assert!(std::fs::remove_file(&hop).is_ok());
        assert!(symlink(&bad, &hop).is_ok());
        assert_ne!(
            std::fs::read(alias.join("src/lib.rs")).ok(),
            Some(b"pub fn a() {}\n".to_vec())
        );
        assert!(std::fs::remove_file(&hop).is_ok());
        assert!(symlink(&workspace, &hop).is_ok());
        assert_eq!(
            std::fs::read(alias.join("src/lib.rs")).ok(),
            Some(b"pub fn a() {}\n".to_vec())
        );
        assert!(
            !source.unchanged(),
            "an intermediate alias changed within the guarded epoch"
        );
        assert!(!source.unchanged(), "event refusal remains latched");
    }

    #[test]
    fn generated_inputs_are_bound_after_cargos_script_and_can_be_reverted() {
        let mut f = fixture("generated-source");
        let out = f.root.join("target/build/demo/out");
        let generated = out.join("generated.rs");
        f.process
            .variables
            .push(("OUT_DIR".into(), out.as_os_str().to_owned()));
        write(&generated, b"first\n");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        let info = format!(
            "{}: src/lib.rs {}\n\nsrc/lib.rs:\n{}:\n",
            f.outputs[0].display(),
            generated.display(),
            generated.display(),
        );
        write(&f.outputs[0], info.as_bytes());
        let guard = SourceGuard::begin(&f.runtime, &f.identity, &f.process);
        let observation =
            guard.and_then(|guard| observation(&f.args, &f.identity, &f.process, &guard));
        assert!(
            observation.is_some(),
            "the current OUT_DIR is covered before compilation"
        );
        let sidecar = f
            .root
            .join("ctx/runs/run-a/sidecars")
            .join(f.identity.key());
        assert!(json(&sidecar.join(OBSERVATION), &observation).is_ok());
        f.retain("run-a", &a, 4);
        write(&generated, b"second\n");
        assert!(
            f.restore().is_none(),
            "changed generated content verifies again"
        );
        write(&generated, b"first\n");
        assert!(
            f.restore().is_some(),
            "a completed script can regenerate earlier inputs"
        );
        write(&out.join("new-module.rs"), b"new\n");
        assert!(
            f.restore().is_none(),
            "generated input absence is bound too"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_inventory_observes_symlink_retargeting() {
        use std::os::unix::fs::symlink;
        let f = fixture("source-alias");
        let first = f.root.join("first.rs");
        let second = f.root.join("second.rs");
        write(&first, b"same\n");
        write(&second, b"same\n");
        let alias = f.root.join("ws/src/alias.rs");
        assert!(symlink(&first, &alias).is_ok());
        let source = SourceGuard::begin(&f.runtime, &f.identity, &f.process);
        assert!(source.is_some());
        assert!(std::fs::remove_file(&alias).is_ok());
        assert!(symlink(&second, &alias).is_ok());
        if let Some(source) = source {
            assert!(!source.unchanged());
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_inventory_binds_new_aliases_and_empty_directories_between_runs() {
        use std::os::unix::fs::symlink;
        let f = fixture("source-topology");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        f.retain("run-a", &a, 4);
        let empty = f.root.join("ws/src/empty");
        assert!(std::fs::create_dir(&empty).is_ok());
        assert!(f.restore().is_none(), "directory presence is an input");
        assert!(std::fs::remove_dir(&empty).is_ok());
        assert!(
            f.restore().is_some(),
            "returning to the earlier inventory matches"
        );
        let alias = f.root.join("ws/src/alias.rs");
        assert!(symlink(&f.source, &alias).is_ok());
        assert!(
            f.restore().is_none(),
            "an alias is bound even when its canonical file was already observed"
        );
    }

    #[test]
    fn history_capacity_counts_compiler_identity_variants_together() {
        let mut f = fixture("identity-capacity");
        for index in 1..=3 {
            f.identity.rustc_args_sha256 = format!("compiler-variant-{index}");
            let receipt = f.verify(&format!("run-{index}"), b"pub fn same() {}\n");
            f.retain(&format!("run-{index}"), &receipt, 2);
            std::thread::sleep(Duration::from_millis(20));
        }
        let root = root(&f.runtime.receipts);
        let total: usize = std::fs::read_dir(root.join("targets"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|target| entries(&target.path()).len())
            .sum();
        assert_eq!(
            total, 2,
            "the bound is per logical target, across unit hashes"
        );
        f.identity.rustc_args_sha256 = "compiler-variant-1".to_owned();
        assert!(f.restore().is_none(), "old identity variant was evicted");
        f.identity.rustc_args_sha256 = "compiler-variant-2".to_owned();
        assert!(f.restore().is_some());
        f.identity.rustc_args_sha256 = "compiler-variant-3".to_owned();
        assert!(
            f.restore().is_some(),
            "other identity variants are misses, not damage"
        );
        assert_eq!(f.entries().len(), 2);
    }

    #[test]
    fn a_damaged_entry_or_blob_is_ignored_and_removed() {
        let f = fixture("damaged");
        let a = f.verify("run-a", b"pub fn a() {}\n");
        f.retain("run-a", &a, 4);
        let _ = f.verify("run-b", b"pub fn b() {}\n");
        let verified_b = f.output_bytes();
        write(&f.source, b"pub fn a() {}\n");
        // A stored file's bytes changed: nothing of the entry stays in the
        // target, and the entry is removed.
        let root = root(&f.runtime.receipts);
        write(&blob(&root, &sha("vir:pub fn a() {}\n")), b"tampered");
        assert!(f.restore().is_none());
        for (index, bytes) in f.output_bytes().iter().enumerate() {
            assert!(
                bytes.is_none() || *bytes == verified_b[index],
                "no file of the damaged entry stays in the target"
            );
        }
        assert_eq!(f.entries(), Vec::<PathBuf>::new());
        // A tampered or unreadable entry document is removed too.
        let a = f.verify("run-a2", b"pub fn a() {}\n");
        f.retain("run-a2", &a, 4);
        let entry = f.entries().into_iter().next().unwrap_or_default();
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        write(&entry, text.replace("\"run-a2\"", "\"run-x\"").as_bytes());
        assert!(f.restore().is_none());
        assert!(!entry.exists());
        f.retain("run-a2", &a, 4);
        write(&entry, b"{ not json");
        assert!(f.restore().is_none());
        assert!(!entry.exists());
        // A missing recorded output is a damaged entry as well.
        f.retain("run-a2", &a, 4);
        let _ = std::fs::remove_file(blob(&root, &sha("")));
        assert!(f.restore().is_none());
        assert_eq!(f.entries(), Vec::<PathBuf>::new());
    }

    #[test]
    fn retention_keeps_the_most_recent_proofs_and_their_blobs_only() {
        let f = fixture("evict");
        for (run, content) in [
            ("run-1", "pub fn one() {}\n"),
            ("run-2", "pub fn two() {}\n"),
            ("run-3", "pub fn three() {}\n"),
        ] {
            let receipt = f.verify(run, content.as_bytes());
            f.retain(run, &receipt, 2);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(f.entries().len(), 2);
        // Each kept proof's outputs, plus the dep-info and recorded output
        // they share by content; the evicted proof's own blobs are gone.
        let blobs = f.blobs();
        for name in ["two", "three"] {
            let content = format!("pub fn {name}() {{}}\n");
            assert!(blobs.contains(&sha(&format!("rmeta:{content}"))));
            assert!(blobs.contains(&sha(&format!("vir:{content}"))));
        }
        assert!(!blobs.contains(&sha("vir:pub fn one() {}\n")));
        assert!(!blobs.contains(&sha("rmeta:pub fn one() {}\n")));
        assert_eq!(blobs.len(), 4 + 3);
        write(&f.source, b"pub fn one() {}\n");
        assert!(f.restore().is_none(), "the evicted proof is gone");
        write(&f.source, b"pub fn two() {}\n");
        assert!(f.restore().is_some());
        // Retention off retains nothing.
        let g = fixture("off");
        let receipt = g.verify("run-1", b"pub fn one() {}\n");
        g.retain("run-1", &receipt, 0);
        assert!(g.entries().is_empty() && g.blobs().is_empty());
        // A restored receipt is not retained again.
        let mut restored = g.receipt("run-1");
        restored.invocation.restored = Some("run-0".to_owned());
        g.retain("run-1", &restored.seal(), 4);
        assert_eq!(g.entries(), Vec::<PathBuf>::new());
        // Outputs rewritten after the driver observed them are never retained.
        let receipt = g.verify("run-3", b"pub fn three() {}\n");
        write(&g.outputs[2], b"rewritten");
        assert!(
            retain(
                &root(&g.runtime.receipts),
                &g.root.join("ctx/runs/run-3"),
                &receipt,
                4
            )
            .is_err()
        );
        assert_eq!(g.entries(), Vec::<PathBuf>::new());
    }
}
