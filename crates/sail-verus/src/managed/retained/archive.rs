//! Bounded archive/checked placement ported from the unmerged ADR 0050 work.
//!
//! The disabled research driver constructs `Basis` only after fresh closed-input
//! admission; publication additionally requires two-phase compiler reads and
//! the ordinary committed Current result. Matching these serializable fields
//! alone does not establish that admission.

use super::super::dependency_snapshot::{Content, open};
use super::super::{
    files::digest,
    receipts::{RECEIPT_VERSION, Receipt, TargetIdentity, complete},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Version 4: archives before it were established while Sail's private
/// settings and observation options reached the compiler (#385, #393).
const VERSION: u32 = 4;
const FILE_LIMIT: u64 = 256 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
const ENTRY_LIMIT: u64 = 8 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
/// At most this many archived revisions per logical target. The per-target
/// byte limit (`TOTAL_LIMIT`, over every kept entry) and the arena's aggregate
/// admission bound it further: a Vest-class entry is about 38 MB.
const CAPACITY: usize = 16;
/// Eviction hint naming the target's baseline: the first verified revision of
/// its current lineage. It grants nothing; restore never reads it.
const PIN: &str = "baseline.pin";
/// The most recently evicted entry keys, so a later miss can be explained.
const EVICTED: &str = "evicted.log";
const EVICTED_KEPT: usize = 64;

/// Fresh outer-caller values, captured before Cargo applies its managed child
/// overrides. Only digests are serialized; a missing name means actual absence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CallerEnvironment {
    version: u32,
    run: String,
    values: BTreeMap<String, String>,
}

impl CallerEnvironment {
    pub(super) fn capture(run: &str) -> Option<Self> {
        Self::from_values(run, std::env::vars_os())
    }

    fn from_values(
        run: &str,
        values: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
    ) -> Option<Self> {
        if run.is_empty() {
            return None;
        }
        let values = values
            .into_iter()
            .map(|(name, value)| Some((name.into_string().ok()?, digest(value.as_encoded_bytes()))))
            .collect::<Option<_>>()?;
        Some(Self {
            version: 1,
            run: run.to_owned(),
            values,
        })
    }

    pub(super) fn for_run(&self, run: &str) -> bool {
        self.version == 1 && !run.is_empty() && self.run == run
    }

    fn matches(&self, run: &str, recorded: &BTreeMap<String, Option<Vec<u8>>>) -> bool {
        self.for_run(run)
            && recorded.iter().all(|(name, value)| {
                self.values.get(name) == value.as_ref().map(|value| digest(value)).as_ref()
            })
    }
}

/// Exact same-workspace content context; no registry/path normalization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Basis {
    pub(super) identity: TargetIdentity,
    pub(super) context: String,
    pub(super) source_revision: String,
    pub(super) dependency_inventory: String,
    pub(super) directory: PathBuf,
    pub(super) environment: BTreeMap<String, Option<Vec<u8>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Output {
    pub(super) path: PathBuf,
    pub(super) content: Content,
    pub(super) executable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Entry {
    pub(super) version: u32,
    pub(super) basis: Basis,
    pub(super) receipt: Receipt,
    /// Complete accepted GHOST/EXEC wire document, stored as a checked blob.
    pub(super) compiler_reads: Content,
    pub(super) outputs: Vec<Output>,
    pub(super) stdout: Content,
    pub(super) stderr: Content,
    pub(super) checksum: String,
}

impl Entry {
    pub(super) fn new(
        basis: Basis,
        receipt: Receipt,
        compiler_reads: Content,
        outputs: Vec<Output>,
        stdout: Content,
        stderr: Content,
    ) -> Option<Self> {
        Self {
            version: VERSION,
            basis,
            receipt,
            compiler_reads,
            outputs,
            stdout,
            stderr,
            checksum: String::new(),
        }
        .seal()
    }

    pub(super) fn seal(mut self) -> Option<Self> {
        self.checksum.clear();
        self.checksum = digest(&serde_json::to_vec(&self).ok()?);
        Some(self)
    }

    fn intact(&self) -> bool {
        self.version == VERSION
            && self
                .clone()
                .seal()
                .is_some_and(|sealed| sealed.checksum == self.checksum)
            && self.receipt.partial_args.is_none()
            && self.receipt.version == RECEIPT_VERSION
            && self.receipt.clone().seal().checksum == self.receipt.checksum
            && self.receipt.context == self.basis.context
            && self.receipt.invocation.identity == self.basis.identity
            && self.receipt.invocation.success
            && complete(&self.receipt.invocation.report)
            && sha(&self.compiler_reads.sha256)
            && self.compiler_reads.size <= ENTRY_LIMIT
            && !self.outputs.is_empty()
            && self.outputs.len() <= 32
            && within_budget(self)
    }
}

fn within_budget(entry: &Entry) -> bool {
    entry.stdout.size <= OUTPUT_LIMIT
        && entry.stderr.size <= OUTPUT_LIMIT
        && entry.compiler_reads.size <= ENTRY_LIMIT
        && entry
            .outputs
            .iter()
            .all(|output| output.content.size <= FILE_LIMIT)
        && entry
            .outputs
            .iter()
            .map(|output| &output.content)
            .chain([&entry.stdout, &entry.stderr, &entry.compiler_reads])
            .try_fold(0u64, |total, content| total.checked_add(content.size))
            .is_some_and(|total| total <= TOTAL_LIMIT)
}

/// A partial placement could not be removed. The eventual driver caller must
/// invalidate the unit's receipt and refuse this attempt or force a genuinely
/// fresh compilation; this is never an ordinary cache miss.
#[derive(Debug)]
pub(super) struct RollbackFailure {
    pub(super) paths: Vec<(PathBuf, String)>,
}

pub(super) struct Restored {
    pub(super) receipt: Receipt,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

fn rollback(written: Vec<PathBuf>) -> std::result::Result<(), RollbackFailure> {
    let mut failures = Vec::new();
    for path in written {
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push((path, error.to_string()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(RollbackFailure { paths: failures })
    }
}

fn sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn bytes(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let file = open(path)?;
    if file.metadata().ok()?.len() > limit {
        return None;
    }
    let mut value = Vec::new();
    file.take(limit + 1).read_to_end(&mut value).ok()?;
    ((value.len() as u64) <= limit).then_some(value)
}

fn checked_bytes(path: &Path, expected: &Content, limit: u64) -> Option<Vec<u8>> {
    if !sha(&expected.sha256) || expected.size > limit {
        return None;
    }
    let value = bytes(path, expected.size)?;
    (value.len() as u64 == expected.size && digest(&value) == expected.sha256).then_some(value)
}

/// Independent bounded stream copy into a fresh file; never edits in place.
fn fresh_copy(from: &Path, to: &Path, expected: &Content, executable: bool) -> Option<()> {
    if !sha(&expected.sha256) || expected.size > FILE_LIMIT {
        return None;
    }
    let mut input = open(from)?;
    if input.metadata().ok()?.len() != expected.size {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if executable { 0o700 } else { 0o600 });
    }
    let mut output = options.open(to).ok()?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 65536];
    loop {
        let n = input.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        size = size.checked_add(n as u64)?;
        if size > expected.size {
            return None;
        }
        hash.update(&buffer[..n]);
        output.write_all(&buffer[..n]).ok()?;
    }
    (size == expected.size && format!("{:x}", hash.finalize()) == expected.sha256).then_some(())
}

/// Same-directory replacement; a failed hash never places the temporary bytes.
fn place_blob(root: &Path, output: &Output) -> Option<()> {
    let temporary = output.path.with_file_name(format!(
        ".sail-retained-{}-{}",
        std::process::id(),
        output.content.sha256
    ));
    let outcome = fresh_copy(
        &root.join("blobs").join(&output.content.sha256),
        &temporary,
        &output.content,
        output.executable,
    )
    .and_then(|()| std::fs::rename(&temporary, &output.path).ok());
    if outcome.is_none() {
        let _ = std::fs::remove_file(&temporary);
    }
    outcome
}

fn target(root: &Path, identity: &TargetIdentity) -> Option<PathBuf> {
    Some(root.join("targets").join(digest(
        &serde_json::to_vec(&(&identity.manifest, &identity.name, &identity.crate_type)).ok()?,
    )))
}

fn entries(directory: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
        })
        .map(|entry| {
            (
                entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
                entry.path(),
            )
        })
        .collect();
    entries.sort_by(|a, b| b.cmp(a));
    entries.into_iter().map(|(_, path)| path).collect()
}

fn installed(output: &Output, directory: &Path, names: &BTreeSet<std::ffi::OsString>) -> bool {
    output.path.parent() == Some(directory)
        && output
            .path
            .file_name()
            .is_some_and(|name| names.contains(name))
        && output.content.size <= FILE_LIMIT
        && sha(&output.content.sha256)
}

/// Restore only the exact caller-admitted output family and complete basis.
/// The caller must freshly recheck source/dependency/environment state in
/// `after`, before exposing any saved reply. Failure removes every placed file.
pub(super) fn restore(
    root: &Path,
    basis: &Basis,
    output_directory: &Path,
    output_names: &BTreeSet<std::ffi::OsString>,
    caller: &CallerEnvironment,
    run: &str,
    after: impl Fn() -> bool,
) -> std::result::Result<Option<Restored>, RollbackFailure> {
    let Some(directory) = target(root, &basis.identity) else {
        return Ok(None);
    };
    for path in entries(&directory) {
        let Some(raw) = bytes(&path, ENTRY_LIMIT) else {
            continue;
        };
        let Ok(entry) = serde_json::from_slice::<Entry>(&raw) else {
            continue;
        };
        if !entry.intact()
            || entry.basis != *basis
            || !caller.matches(run, &entry.receipt.declared_environment)
        {
            continue;
        }
        let unique: BTreeSet<_> = entry.outputs.iter().map(|output| &output.path).collect();
        if unique.len() != entry.outputs.len()
            || !entry
                .outputs
                .iter()
                .all(|output| installed(output, output_directory, output_names))
            || !entry.receipt.artifact_hashes.iter().all(|(path, sha)| {
                entry
                    .outputs
                    .iter()
                    .any(|output| output.path == *path && output.content.sha256 == *sha)
            })
        {
            continue;
        }
        // The original committed establishing run must still bind this receipt.
        let Some(commit) = bytes(&entry.receipt.committed, ENTRY_LIMIT)
            .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
        else {
            continue;
        };
        if commit["context"].as_str() != Some(&basis.context)
            || commit["run_id"].as_str() != Some(&entry.receipt.invocation.run_id)
            || !commit["receipt_checksums"]
                .as_array()
                .is_some_and(|checksums| {
                    checksums
                        .iter()
                        .any(|sha| sha.as_str() == Some(&entry.receipt.checksum))
                })
        {
            continue;
        }
        let Some(stdout) = checked_bytes(
            &root.join("blobs").join(&entry.stdout.sha256),
            &entry.stdout,
            OUTPUT_LIMIT,
        ) else {
            continue;
        };
        let Some(stderr) = checked_bytes(
            &root.join("blobs").join(&entry.stderr.sha256),
            &entry.stderr,
            OUTPUT_LIMIT,
        ) else {
            continue;
        };
        let Some(reads) = checked_bytes(
            &root.join("blobs").join(&entry.compiler_reads.sha256),
            &entry.compiler_reads,
            ENTRY_LIMIT,
        ) else {
            continue;
        };
        if !super::compiler::stored_header(
            &reads,
            &entry.basis.identity.key(),
            &entry.receipt.invocation.run_id,
        ) {
            continue;
        }
        let mut written = Vec::new();
        let placed = entry.outputs.iter().all(|output| {
            if place_blob(root, output).is_none() {
                return false;
            }
            written.push(output.path.clone());
            true
        });
        if placed && after() {
            let _ = std::fs::File::options()
                .append(true)
                .open(&path)
                .and_then(|file| file.set_modified(SystemTime::now()));
            return Ok(Some(Restored {
                receipt: entry.receipt,
                stdout,
                stderr,
            }));
        }
        rollback(written)?;
        return Ok(None);
    }
    Ok(None)
}

/// Ported retention I/O. Not called until compiler-read admission is connected.
pub(super) fn retain(
    root: &Path,
    entry: &Entry,
    streams: [&Path; 3],
    live_graphs: &BTreeSet<String>,
) -> Option<()> {
    if !entry.intact() || !entry.receipt.valid(&entry.basis.context) {
        return None;
    }
    let reads = checked_bytes(streams[2], &entry.compiler_reads, ENTRY_LIMIT)?;
    if !super::compiler::stored_header(
        &reads,
        &entry.basis.identity.key(),
        &entry.receipt.invocation.run_id,
    ) || entry.stdout.size > OUTPUT_LIMIT
        || entry.stderr.size > OUTPUT_LIMIT
    {
        return None;
    }
    let encoded = serde_json::to_vec(entry).ok()?;
    if encoded.len() as u64 > ENTRY_LIMIT {
        return None;
    }
    let incoming = entry
        .outputs
        .iter()
        .map(|output| &output.content)
        .chain([&entry.stdout, &entry.stderr, &entry.compiler_reads])
        .try_fold(u64::try_from(encoded.len()).ok()?, |total, content| {
            total.checked_add(content.size)
        })?;
    let _admission = super::budget::admit(
        root,
        incoming,
        u64::try_from(entry.outputs.len()).ok()?.checked_add(10)?,
    )?;
    let blobs = root.join("blobs");
    std::fs::create_dir_all(&blobs).ok()?;
    for (source, content) in entry
        .outputs
        .iter()
        .map(|output| (output.path.as_path(), &output.content))
        .chain(
            streams
                .into_iter()
                .zip([&entry.stdout, &entry.stderr, &entry.compiler_reads]),
        )
    {
        let destination = blobs.join(&content.sha256);
        if checked_bytes(&destination, content, FILE_LIMIT).is_some() {
            continue;
        }
        // A corrupt pre-existing blob is never overwritten through its inode.
        let temporary = blobs.join(format!(".{}-{}", std::process::id(), content.sha256));
        let outcome = fresh_copy(source, &temporary, content, false)
            .and_then(|()| std::fs::rename(&temporary, &destination).ok());
        if outcome.is_none() {
            let _ = std::fs::remove_file(temporary);
            return None;
        }
    }
    let directory = target(root, &entry.basis.identity)?;
    std::fs::create_dir_all(&directory).ok()?;
    let key = digest(&serde_json::to_vec(&entry.basis).ok()?);
    super::super::files::json(&directory.join(format!("{key}.json")), &entry).ok()?;
    evict(&directory, &key, &lineage(&entry.basis)?)?;
    collect(root, live_graphs);
    Some(())
}

/// The basis without its source revision: revisions of one lineage differ
/// only in source content.
fn lineage(basis: &Basis) -> Option<String> {
    let mut basis = basis.clone();
    basis.source_revision.clear();
    Some(digest(&serde_json::to_vec(&basis).ok()?))
}

/// The key of the archived entry for `basis`.
fn key(basis: &Basis) -> Option<String> {
    Some(digest(&serde_json::to_vec(basis).ok()?))
}

/// One archived entry as eviction sees it, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Kept {
    key: String,
    bytes: u64,
    lineage: Option<String>,
}

/// Which entries stay. `newest` (just written) always stays. The pinned
/// baseline stays while it fits the byte limit beside `newest`; the others
/// stay newest first while both the count and byte limits allow, and once one
/// does not fit it and every older one go. Returns the kept keys and the
/// baseline to record.
fn plan(
    listed: &[Kept],
    newest: &str,
    newest_lineage: &str,
    pinned: Option<&str>,
    capacity: usize,
    limit: u64,
) -> (BTreeSet<String>, Option<String>) {
    let Some(first) = listed.iter().find(|kept| kept.key == newest) else {
        return (BTreeSet::new(), None);
    };
    let mut kept = BTreeSet::from([first.key.clone()]);
    let mut used = first.bytes;
    // The recorded baseline, while it is of this lineage and still present;
    // otherwise the oldest present revision of the lineage (an arena from
    // before pinning, or after the byte limit evicted the old baseline).
    let baseline = pinned
        .and_then(|pin| {
            listed
                .iter()
                .find(|kept| kept.key == pin && kept.lineage.as_deref() == Some(newest_lineage))
        })
        .or_else(|| {
            listed
                .iter()
                .rev()
                .find(|kept| kept.lineage.as_deref() == Some(newest_lineage))
        });
    let mut recorded = None;
    if let Some(baseline) = baseline {
        if baseline.key == newest {
            recorded = Some(baseline.key.clone());
        } else if kept.len() < capacity
            && let Some(total) = used.checked_add(baseline.bytes)
            && total <= limit
        {
            used = total;
            kept.insert(baseline.key.clone());
            recorded = Some(baseline.key.clone());
        }
    }
    let rest: Vec<&Kept> = listed
        .iter()
        .filter(|entry| !kept.contains(&entry.key))
        .collect();
    for entry in rest {
        match used.checked_add(entry.bytes) {
            Some(total) if total <= limit && kept.len() < capacity => {
                used = total;
                kept.insert(entry.key.clone());
            }
            _ => break,
        }
    }
    (kept, recorded)
}

/// Bounded per-target eviction after `newest` is written. Only the pinned
/// baseline is exempt from count eviction; the byte limit binds every entry
/// but the newest. Evicted keys are logged so a later miss can say why.
fn evict(directory: &Path, newest: &str, newest_lineage: &str) -> Option<()> {
    let paths = entries(directory);
    let listed: Vec<Kept> = paths
        .iter()
        .filter_map(|path| {
            let key = path.file_stem()?.to_str()?.to_owned();
            let raw = bytes(path, ENTRY_LIMIT);
            let parsed = raw
                .as_deref()
                .and_then(|raw| serde_json::from_slice::<Entry>(raw).ok());
            let file = std::fs::metadata(path).map_or(ENTRY_LIMIT, |meta| meta.len());
            let bytes = parsed.as_ref().map_or(Some(file), |entry| {
                entry
                    .outputs
                    .iter()
                    .map(|output| &output.content)
                    .chain([&entry.stdout, &entry.stderr, &entry.compiler_reads])
                    .try_fold(file, |total, content| total.checked_add(content.size))
            });
            Some(Kept {
                key,
                bytes: bytes.unwrap_or(u64::MAX),
                lineage: parsed.and_then(|entry| lineage(&entry.basis)),
            })
        })
        .collect();
    let pinned = std::fs::read_to_string(directory.join(PIN)).ok();
    let (kept, baseline) = plan(
        &listed,
        newest,
        newest_lineage,
        pinned.as_deref().map(str::trim),
        CAPACITY,
        TOTAL_LIMIT,
    );
    let mut evicted = Vec::new();
    for path in &paths {
        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !kept.contains(key) {
            std::fs::remove_file(path).ok()?;
            evicted.push(key.to_owned());
        }
    }
    match baseline {
        Some(baseline) if pinned.as_deref().map(str::trim) != Some(baseline.as_str()) => {
            let _ = std::fs::write(directory.join(PIN), baseline);
        }
        None => {
            let _ = std::fs::remove_file(directory.join(PIN));
        }
        Some(_) => {}
    }
    if !evicted.is_empty() {
        let log = directory.join(EVICTED);
        let previous = std::fs::read_to_string(&log).unwrap_or_default();
        let mut lines: Vec<&str> = previous
            .lines()
            .filter(|line| sha(line) && !kept.contains(*line))
            .chain(evicted.iter().map(String::as_str))
            .collect();
        let excess = lines.len().saturating_sub(EVICTED_KEPT);
        lines.drain(..excess);
        let _ = std::fs::write(&log, lines.join("\n") + "\n");
    }
    Some(())
}

/// Whether the archived revision for `basis` was evicted and is not present.
/// Explanation only: it never affects what is restored.
pub(super) fn evicted(root: &Path, basis: &Basis) -> bool {
    let (Some(directory), Some(key)) = (target(root, &basis.identity), key(basis)) else {
        return false;
    };
    !directory.join(format!("{key}.json")).exists()
        && std::fs::read_to_string(directory.join(EVICTED))
            .is_ok_and(|log| log.lines().any(|line| line == key))
}

/// Eviction also releases unreferenced bodies. Any incomplete index scan keeps
/// blobs rather than guessing that a still-referenced proof is garbage.
fn collect(root: &Path, live_graphs: &BTreeSet<String>) {
    let Some(targets) = std::fs::read_dir(root.join("targets")).ok() else {
        return;
    };
    let mut referenced = BTreeSet::new();
    let mut graphs = live_graphs.clone();
    for target in targets {
        let Ok(target) = target else {
            return;
        };
        if !target.file_type().is_ok_and(|kind| kind.is_dir()) {
            return;
        }
        let Ok(index) = std::fs::read_dir(target.path()) else {
            return;
        };
        for entry in index {
            let Ok(entry) = entry else {
                return;
            };
            // The baseline pin and eviction log are not entries.
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "json")
            {
                continue;
            }
            let Some(record) = bytes(&entry.path(), ENTRY_LIMIT)
                .and_then(|raw| serde_json::from_slice::<Entry>(&raw).ok())
            else {
                return;
            };
            if !record.intact() {
                return;
            }
            graphs.insert(record.basis.dependency_inventory);
            referenced.extend(
                record
                    .outputs
                    .iter()
                    .map(|output| output.content.sha256.clone()),
            );
            referenced.insert(record.stdout.sha256);
            referenced.insert(record.stderr.sha256);
            referenced.insert(record.compiler_reads.sha256);
        }
    }
    let Ok(blobs) = std::fs::read_dir(root.join("blobs")) else {
        return;
    };
    for entry in blobs {
        let Ok(entry) = entry else {
            return;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_file())
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| sha(name) && !referenced.contains(name))
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    // The caller supplies graph references from every live compiler artifact
    // generation, in addition to the retained entries of every logical target.
    let Ok(entries) = std::fs::read_dir(root.join("graphs")) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| sha(name) && !graphs.contains(name))
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    struct Root(PathBuf);
    impl Root {
        fn new(name: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "sail-retained-archive-{}-{name}",
                std::process::id()
            ));
            std::fs::create_dir(&path)?;
            std::fs::create_dir(path.join("blobs"))?;
            Ok(Self(path))
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn caller_environment_keeps_absence_values_and_run_binding_exact()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty = CallerEnvironment::from_values("current", []).ok_or("empty capture")?;
        let named = |value: &[u8]| {
            BTreeMap::from([("SAIL_DECLARED_FIXTURE".to_owned(), Some(value.to_vec()))])
        };
        let absent = BTreeMap::from([("SAIL_DECLARED_FIXTURE".to_owned(), None)]);
        let present = CallerEnvironment::from_values(
            "current",
            [(
                OsString::from("SAIL_DECLARED_FIXTURE"),
                OsString::from("literal A"),
            )],
        )
        .ok_or("present capture")?;
        assert!(empty.matches("current", &absent));
        assert!(!empty.matches("current", &named(b"literal A")));
        assert!(present.matches("current", &named(b"literal A")));
        assert!(!present.matches("current", &named(b"literal B")));
        assert!(!present.matches("current", &absent));
        assert!(!present.matches("stale-run", &named(b"literal A")));
        let mut stale = present.clone();
        stale.version = 0;
        assert!(!stale.matches("current", &named(b"literal A")));
        let wire = serde_json::to_value(&present)?;
        assert_eq!(
            wire["values"]["SAIL_DECLARED_FIXTURE"],
            digest(b"literal A")
        );
        for missing in ["run", "version", "values"] {
            let mut missing_wire = wire.clone();
            missing_wire
                .as_object_mut()
                .ok_or("object")?
                .remove(missing);
            assert!(serde_json::from_value::<CallerEnvironment>(missing_wire).is_err());
        }
        Ok(())
    }

    fn owned_environment_child(
        mode: &str,
        snapshot: Option<&Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut child = std::process::Command::new(std::env::current_exe()?);
        child
            .args([
                "--exact",
                "managed::retained::archive::tests::owned_child_uses_outer_declared_environment",
                "--nocapture",
            ])
            .env("SAIL_RETAINED_CALLER_TEST", mode);
        if let Some(path) = snapshot {
            child
                .env("SAIL_RETAINED_CALLER_TEST_FILE", path)
                .env("RUSTC_BOOTSTRAP", "1")
                .env("SAIL_DECLARED_FIXTURE", "child-only");
        } else {
            child
                .env_remove("RUSTC_BOOTSTRAP")
                .env("SAIL_DECLARED_FIXTURE", "outer-A")
                .env_remove("SAIL_DECLARED_ABSENT_FIXTURE");
        }
        let output = child.output()?;
        assert!(
            output.status.success(),
            "owned environment child: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    fn changed_caller_never_places(
        root: &Path,
        basis: &Basis,
        names: &BTreeSet<OsString>,
        caller: &CallerEnvironment,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for variant in ["changed", "absent", "present", "stale"] {
            let mut other = caller.clone();
            match variant {
                "changed" => {
                    other
                        .values
                        .insert("SAIL_DECLARED_FIXTURE".into(), digest(b"outer-B"));
                }
                "absent" => {
                    other.values.remove("SAIL_DECLARED_FIXTURE");
                }
                "present" => {
                    other
                        .values
                        .insert("SAIL_DECLARED_ABSENT_FIXTURE".into(), digest(b"present"));
                }
                _ => {
                    other.run = "stale-run".into();
                }
            }
            std::fs::write(root.join("libunit-abcd.rmeta"), b"artifact B")?;
            let placed = std::cell::Cell::new(false);
            assert!(
                restore(root, basis, root, names, &other, "actual-owned-run", || {
                    placed.set(true);
                    true
                })
                .is_ok_and(|value| value.is_none())
            );
            assert!(!placed.get());
            assert_eq!(
                std::fs::read(root.join("libunit-abcd.rmeta"))?,
                b"artifact B"
            );
        }
        Ok(())
    }

    #[test]
    fn owned_child_uses_outer_declared_environment() -> Result<(), Box<dyn std::error::Error>> {
        match std::env::var("SAIL_RETAINED_CALLER_TEST").ok().as_deref() {
            None => owned_environment_child("outer", None),
            Some("outer") => {
                assert!(std::env::var_os("RUSTC_BOOTSTRAP").is_none());
                assert_eq!(std::env::var("SAIL_DECLARED_FIXTURE")?, "outer-A");
                assert!(std::env::var_os("SAIL_DECLARED_ABSENT_FIXTURE").is_none());
                let root = Root::new("caller-environment")?;
                let snapshot = root.0.join("caller.json");
                let caller = CallerEnvironment::capture("actual-owned-run").ok_or("capture")?;
                super::super::super::files::json(&snapshot, &caller)?;
                let declared = BTreeMap::from([
                    ("RUSTC_BOOTSTRAP".into(), None),
                    ("SAIL_DECLARED_FIXTURE".into(), Some(b"outer-A".to_vec())),
                    ("SAIL_DECLARED_ABSENT_FIXTURE".into(), None),
                ]);
                let entry = archive_fixture(&root, declared)?;
                retain(
                    &root.0,
                    &entry,
                    [
                        &root.0.join("stdout"),
                        &root.0.join("stderr"),
                        &root.0.join("inputs.json"),
                    ],
                    &BTreeSet::new(),
                )
                .ok_or("outer fixture publication")?;
                super::super::super::files::json(&root.0.join("basis.json"), &entry.basis)?;
                std::fs::write(root.0.join("libunit-abcd.rmeta"), b"artifact B")?;
                owned_environment_child("managed", Some(&snapshot))
            }
            Some("managed") => {
                assert_eq!(std::env::var("RUSTC_BOOTSTRAP")?, "1");
                let snapshot = PathBuf::from(std::env::var("SAIL_RETAINED_CALLER_TEST_FILE")?);
                let root = snapshot.parent().ok_or("snapshot parent")?;
                let caller: CallerEnvironment = serde_json::from_slice(&std::fs::read(&snapshot)?)?;
                let basis: Basis =
                    serde_json::from_slice(&std::fs::read(root.join("basis.json"))?)?;
                let declared = BTreeMap::from([
                    ("RUSTC_BOOTSTRAP".into(), None),
                    ("SAIL_DECLARED_FIXTURE".into(), Some(b"outer-A".to_vec())),
                    ("SAIL_DECLARED_ABSENT_FIXTURE".into(), None),
                ]);
                assert!(!super::super::super::environment::unchanged(&declared));
                let names = BTreeSet::from([OsString::from("libunit-abcd.rmeta")]);
                let restored = restore(
                    root,
                    &basis,
                    root,
                    &names,
                    &caller,
                    "actual-owned-run",
                    || true,
                )
                .map_err(|failure| format!("rollback: {:?}", failure.paths))?
                .ok_or("managed child restore")?;
                assert_eq!(restored.receipt.invocation.run_id, "establishing-A");
                assert_eq!(restored.stdout, b"literal stdout");
                assert_eq!(restored.stderr, b"literal diagnostics");
                assert_eq!(
                    std::fs::read(root.join("libunit-abcd.rmeta"))?,
                    b"artifact A"
                );
                changed_caller_never_places(root, &basis, &names, &caller)?;
                Ok(())
            }
            Some(_) => Err("unknown controlled environment mode".into()),
        }
    }

    #[test]
    fn damaged_blob_never_replaces_output_and_checked_copy_preserves_literal_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("placement")?;
        let expected = b"literal compiler artifact A";
        let output = Output {
            path: root.0.join("libunit-abcd.rmeta"),
            executable: false,
            content: Content {
                sha256: digest(expected),
                size: expected.len() as u64,
            },
        };
        std::fs::write(&output.path, b"current B")?;
        let blob = root.0.join("blobs").join(&output.content.sha256);
        std::fs::write(&blob, b"damaged A")?;
        assert!(place_blob(&root.0, &output).is_none());
        assert_eq!(std::fs::read(&output.path)?, b"current B");
        std::fs::write(&blob, expected)?;
        assert!(place_blob(&root.0, &output).is_some());
        assert_eq!(std::fs::read(&output.path)?, expected);
        let names: Vec<_> = std::fs::read_dir(&root.0)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<_>>()?;
        assert!(
            !names
                .iter()
                .any(|name| name.to_string_lossy().starts_with(".sail-retained-"))
        );
        Ok(())
    }

    #[test]
    fn output_family_admission_and_declared_bounds_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("bounds")?;
        let output = Output {
            path: root.0.join("libunit-abcd.rmeta"),
            executable: false,
            content: Content {
                sha256: digest(b"A"),
                size: 1,
            },
        };
        let names = BTreeSet::from([OsString::from("libunit-abcd.rmeta")]);
        assert!(installed(&output, &root.0, &names));
        assert!(!installed(
            &output,
            &root.0,
            &BTreeSet::from([OsString::from("libunit-abcd-other.rmeta")])
        ));
        let mut oversized = output;
        oversized.content.size = FILE_LIMIT + 1;
        assert!(!installed(&oversized, &root.0, &names));
        assert!(checked_bytes(&root.0.join("missing"), &oversized.content, FILE_LIMIT).is_none());
        Ok(())
    }

    fn receipt_fixture(
        root: &Root,
        artifact: &Path,
        identity: TargetIdentity,
        declared: BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<Receipt, Box<dyn std::error::Error>> {
        let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
        report.error = None;
        report.exit_code = Some(0);
        report.crates.push(sail_api::output::CrateReport {
            name: Some("unit".into()),
            functions: Vec::new(),
            verus: sail_api::output::VerusResults {
                verified: Some(0),
                errors: Some(0),
                success: Some(true),
                encountered_vir_error: Some(false),
            },
        });
        let receipt = Receipt {
            version: RECEIPT_VERSION,
            partial_args: None,
            checksum: String::new(),
            package_id: "unit".into(),
            name: "unit".into(),
            kind: vec!["lib".into()],
            context: "context".into(),
            input_revision: "A".into(),
            invocation: super::super::super::receipts::Invocation {
                dependency_build: None,
                compiler_environment: Some(std::collections::BTreeMap::new()),
                retained_from: None,
                identity,
                run_id: "establishing-A".into(),
                report,
                success: true,
                dependencies: BTreeMap::new(),
            },
            artifact_hashes: BTreeMap::from([(artifact.to_owned(), digest(b"artifact A"))]),
            committed: root.0.join("committed.json"),
            declared_environment: declared,
        }
        .seal();
        super::super::super::files::json(
            &receipt.committed,
            &serde_json::json!({
                "context":"context","run_id":"establishing-A","receipt_checksums":[receipt.checksum]
            }),
        )?;
        Ok(receipt)
    }

    fn aggregate_refusal(entry: &Entry) -> Result<(), Box<dyn std::error::Error>> {
        let mut oversized = entry.clone();
        oversized.outputs = (0..5)
            .map(|index| {
                let mut output = entry.outputs[0].clone();
                output.path = output.path.with_file_name(format!("large-{index}.rmeta"));
                output.content.size = FILE_LIMIT;
                output
            })
            .collect();
        let oversized = oversized.seal().ok_or("oversized seal")?;
        assert!(
            oversized
                .outputs
                .iter()
                .all(|output| output.content.size <= FILE_LIMIT)
        );
        assert!(!within_budget(&oversized));
        assert!(!oversized.intact());
        Ok(())
    }

    fn late_cleanup_refusal(
        root: &Root,
        basis: &Basis,
        names: &BTreeSet<OsString>,
        artifact: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(artifact, b"artifact B")?;
        let result = restore(
            &root.0,
            basis,
            &root.0,
            names,
            &CallerEnvironment::capture("test-current").ok_or("caller snapshot")?,
            "test-current",
            || {
                assert!(std::fs::remove_file(artifact).is_ok());
                assert!(std::fs::create_dir(artifact).is_ok());
                false
            },
        );
        let error = result.err().ok_or("rollback failure was hidden")?;
        assert_eq!(error.paths.len(), 1);
        assert_eq!(error.paths[0].0, artifact);
        assert_ne!(error.paths[0].1, "");
        assert!(artifact.is_dir());
        Ok(())
    }

    fn archive_fixture(
        root: &Root,
        declared: BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<Entry, Box<dyn std::error::Error>> {
        let artifact = root.0.join("libunit-abcd.rmeta");
        std::fs::write(&artifact, b"artifact A")?;
        let stdout = root.0.join("stdout");
        let stderr = root.0.join("stderr");
        let reads = root.0.join("inputs.json");
        std::fs::write(&stdout, b"literal stdout")?;
        std::fs::write(&stderr, b"literal diagnostics")?;
        let identity = TargetIdentity {
            manifest: root.0.join("Cargo.toml"),
            name: "unit".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: digest(b"actual argv fixture"),
            cargo_name: None,
        };
        let receipt = receipt_fixture(root, &artifact, identity.clone(), declared)?;
        let basis = Basis {
            identity,
            context: "context".into(),
            source_revision: "A".into(),
            dependency_inventory: digest(b"exact copied lookup graph"),
            directory: root.0.clone(),
            environment: BTreeMap::new(),
        };
        let phase = serde_json::json!({"sources":{"lib.rs":{"checksum":format!("sha256={}",digest(b"A")),"size":1}},
            "extra_tracked_files":[],"tracked_environment":{}});
        let raw = serde_json::to_vec(&serde_json::json!({"kind":"sail-compiler-read-inputs",
            "schema":1,"complete":true,"pid":123,
            "binding":digest(format!("establishing-A\0{}",basis.identity.key()).as_bytes()),
            "phases":{"verification":phase,"compilation":phase}}))?;
        std::fs::write(&reads, &raw)?;
        let output = Output {
            path: artifact.clone(),
            content: Content {
                sha256: digest(b"artifact A"),
                size: 10,
            },
            executable: false,
        };
        let entry = Entry {
            version: VERSION,
            basis: basis.clone(),
            receipt,
            compiler_reads: Content {
                sha256: digest(&raw),
                size: raw.len() as u64,
            },
            outputs: vec![output],
            stdout: Content {
                sha256: digest(b"literal stdout"),
                size: 14,
            },
            stderr: Content {
                sha256: digest(b"literal diagnostics"),
                size: 19,
            },
            checksum: String::new(),
        }
        .seal()
        .ok_or("seal")?;
        Ok(entry)
    }

    #[test]
    fn actual_archive_restores_establishing_receipt_and_rolls_back_a_late_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("roundtrip")?;
        let entry = archive_fixture(&root, BTreeMap::new())?;
        let basis = entry.basis.clone();
        let artifact = root.0.join("libunit-abcd.rmeta");
        let stdout = root.0.join("stdout");
        let stderr = root.0.join("stderr");
        let reads = root.0.join("inputs.json");
        aggregate_refusal(&entry)?;
        retain(
            &root.0,
            &entry,
            [&stdout, &stderr, &reads],
            &BTreeSet::new(),
        )
        .ok_or("retain")?;
        std::fs::write(&artifact, b"artifact B")?;
        let names = BTreeSet::from([OsString::from("libunit-abcd.rmeta")]);
        let restored = restore(
            &root.0,
            &basis,
            &root.0,
            &names,
            &CallerEnvironment::capture("test-current").ok_or("caller snapshot")?,
            "test-current",
            || true,
        )
        .map_err(|error| format!("rollback: {:?}", error.paths))?
        .ok_or("restore")?;
        assert_eq!(restored.receipt.invocation.run_id, "establishing-A");
        assert_eq!(std::fs::read(&artifact)?, b"artifact A");
        assert_eq!(restored.stdout, b"literal stdout");
        assert_eq!(restored.stderr, b"literal diagnostics");
        std::fs::write(&artifact, b"artifact B")?;
        assert!(
            restore(
                &root.0,
                &basis,
                &root.0,
                &names,
                &CallerEnvironment::capture("test-current").ok_or("caller snapshot")?,
                "test-current",
                || false
            )
            .is_ok_and(|value| value.is_none())
        );
        assert!(!artifact.exists());
        late_cleanup_refusal(&root, &basis, &names, &artifact)?;
        Ok(())
    }

    fn kept(key: &str, bytes: u64, lineage: &str) -> Kept {
        Kept {
            key: key.into(),
            bytes,
            lineage: Some(lineage.into()),
        }
    }

    #[test]
    fn eviction_pins_the_baseline_against_the_count_limit_only() {
        // Newest first: R5 was just written, A is the lineage's first.
        let listed: Vec<_> = ["R5", "R4", "R3", "R2", "R1", "A"]
            .into_iter()
            .map(|key| kept(key, 10, "L"))
            .collect();
        let (keep, pin) = plan(&listed, "R5", "L", Some("A"), 4, 1000);
        assert_eq!(
            keep,
            BTreeSet::from(["R5", "A", "R4", "R3"].map(String::from))
        );
        assert_eq!(pin.as_deref(), Some("A"));
        // Without a recorded pin the oldest revision of the lineage is pinned.
        let (keep, pin) = plan(&listed, "R5", "L", None, 4, 1000);
        assert!(keep.contains("A"));
        assert_eq!(pin.as_deref(), Some("A"));
        // The byte limit binds every entry but the newest, oldest first.
        let (keep, _) = plan(&listed, "R5", "L", Some("A"), 16, 35);
        assert_eq!(keep, BTreeSet::from(["R5", "A", "R4"].map(String::from)));
        // A baseline that cannot fit beside the newest is evicted by bytes.
        let mut large = listed.clone();
        large[5].bytes = 991;
        let (keep, pin) = plan(&large, "R5", "L", Some("A"), 16, 1000);
        assert!(!keep.contains("A"));
        assert_eq!(pin, None);
        assert_eq!(keep.len(), 5);
        // Once one older entry does not fit, it and every older one go.
        let mut gap = listed.clone();
        gap[2].bytes = 500;
        let (keep, _) = plan(&gap, "R5", "L", Some("A"), 16, 100);
        assert_eq!(keep, BTreeSet::from(["R5", "A", "R4"].map(String::from)));
        // A new lineage pins its own first revision; the old one is unpinned.
        let mut switched = listed.clone();
        switched[0].lineage = Some("M".into());
        let (keep, pin) = plan(&switched, "R5", "M", Some("A"), 4, 1000);
        assert_eq!(pin.as_deref(), Some("R5"));
        assert_eq!(
            keep,
            BTreeSet::from(["R5", "R4", "R3", "R2"].map(String::from))
        );
    }

    #[test]
    fn many_passing_edits_still_restore_the_original_revision()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("baseline")?;
        let original = archive_fixture(&root, BTreeMap::new())?;
        let streams = [
            root.0.join("stdout"),
            root.0.join("stderr"),
            root.0.join("inputs.json"),
        ];
        let streams: [&Path; 3] = [&streams[0], &streams[1], &streams[2]];
        let directory = target(&root.0, &original.basis.identity).ok_or("target")?;
        let mut revisions = Vec::new();
        // The original, then more passing edits than the revision limit.
        for index in 0..=CAPACITY + 3 {
            let mut entry = original.clone();
            if index > 0 {
                entry.basis.source_revision = format!("R{index}");
            }
            let entry = entry.seal().ok_or("seal")?;
            retain(&root.0, &entry, streams, &BTreeSet::new()).ok_or("retain")?;
            let path = directory.join(format!("{}.json", key(&entry.basis).ok_or("key")?));
            std::fs::File::options()
                .append(true)
                .open(path)?
                .set_modified(
                    SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(1000 * (index as u64 + 1)),
                )?;
            revisions.push(entry.basis);
        }
        assert_eq!(entries(&directory).len(), CAPACITY);
        let artifact = root.0.join("libunit-abcd.rmeta");
        std::fs::write(&artifact, b"artifact B")?;
        let names = BTreeSet::from([OsString::from("libunit-abcd.rmeta")]);
        let caller = CallerEnvironment::capture("test-current").ok_or("caller snapshot")?;
        let restored = restore(
            &root.0,
            &revisions[0],
            &root.0,
            &names,
            &caller,
            "test-current",
            || true,
        )
        .map_err(|error| format!("rollback: {:?}", error.paths))?
        .ok_or("the original revision was evicted")?;
        assert_eq!(restored.receipt.invocation.run_id, "establishing-A");
        assert_eq!(std::fs::read(&artifact)?, b"artifact A");
        assert!(!evicted(&root.0, &revisions[0]));
        // The oldest unpinned edits went, and the log says so.
        for basis in &revisions[1..=4] {
            assert!(evicted(&root.0, basis));
            assert!(
                restore(
                    &root.0,
                    basis,
                    &root.0,
                    &names,
                    &caller,
                    "test-current",
                    || true
                )
                .is_ok_and(|value| value.is_none())
            );
        }
        assert!(!evicted(&root.0, &revisions[5]));
        // Pin and log files are not entries: collection still releases nothing
        // that a kept entry references.
        assert!(root.0.join("blobs").join(&original.stdout.sha256).exists());
        Ok(())
    }
}
