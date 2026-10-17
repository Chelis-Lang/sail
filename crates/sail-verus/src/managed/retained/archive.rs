//! Bounded archive/checked placement ported from the unmerged ADR 0050 work.
//!
//! Admission is intentionally not wired to a compiler or driver yet. `Basis`
//! must eventually be constructed only after the two-phase source-read and
//! closed-input policy pass; matching these serializable fields alone is not
//! such a proof.

use super::super::{
    files::digest,
    receipts::{RECEIPT_VERSION, Receipt, TargetIdentity, complete},
};
use super::dependencies::{Content, open};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const VERSION: u32 = 3;
const FILE_LIMIT: u64 = 256 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
const ENTRY_LIMIT: u64 = 8 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
const CAPACITY: usize = 4;

/// Exact same-workspace content context; no registry/path normalization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Basis {
    identity: TargetIdentity,
    context: String,
    source_revision: String,
    dependency_inventory: String,
    directory: PathBuf,
    environment: BTreeMap<String, Option<Vec<u8>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Output {
    path: PathBuf,
    content: Content,
    executable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    version: u32,
    basis: Basis,
    receipt: Receipt,
    /// Complete accepted GHOST/EXEC wire document, stored as a checked blob.
    compiler_reads: Content,
    outputs: Vec<Output>,
    stdout: Content,
    stderr: Content,
    checksum: String,
}

impl Entry {
    fn seal(mut self) -> Option<Self> {
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
fn restore(
    root: &Path,
    basis: &Basis,
    output_directory: &Path,
    output_names: &BTreeSet<std::ffi::OsString>,
    after: impl Fn() -> bool,
) -> Option<(Receipt, Vec<u8>, Vec<u8>)> {
    for path in entries(&target(root, &basis.identity)?) {
        let Some(raw) = bytes(&path, ENTRY_LIMIT) else {
            continue;
        };
        let Ok(entry) = serde_json::from_slice::<Entry>(&raw) else {
            continue;
        };
        if !entry.intact() || entry.basis != *basis {
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
            return Some((entry.receipt, stdout, stderr));
        }
        for path in written {
            let _ = std::fs::remove_file(path);
        }
        return None;
    }
    None
}

/// Ported retention I/O. Not called until compiler-read admission is connected.
fn retain(
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
    let mut size = 0u64;
    for content in entry.outputs.iter().map(|output| &output.content).chain([
        &entry.stdout,
        &entry.stderr,
        &entry.compiler_reads,
    ]) {
        size = size.checked_add(content.size)?;
        if size > TOTAL_LIMIT {
            return None;
        }
    }
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
    for path in entries(&directory).into_iter().skip(CAPACITY) {
        std::fs::remove_file(path).ok()?;
    }
    collect(root, live_graphs);
    Some(())
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
    // generation, in addition to the four retained entries per logical target.
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
            checksum: String::new(),
            package_id: "unit".into(),
            name: "unit".into(),
            kind: vec!["lib".into()],
            context: "context".into(),
            input_revision: "A".into(),
            invocation: super::super::super::receipts::Invocation {
                identity,
                run_id: "establishing-A".into(),
                report,
                success: true,
                dependencies: BTreeMap::new(),
            },
            artifact_hashes: BTreeMap::from([(artifact.to_owned(), digest(b"artifact A"))]),
            committed: root.0.join("committed.json"),
            declared_environment: BTreeMap::new(),
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

    #[test]
    fn actual_archive_restores_establishing_receipt_and_rolls_back_a_late_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("roundtrip")?;
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
        };
        let receipt = receipt_fixture(&root, &artifact, identity.clone())?;
        let basis = Basis {
            identity,
            context: "context".into(),
            source_revision: "A".into(),
            dependency_inventory: digest(b"exact copied lookup graph"),
            directory: root.0.clone(),
            environment: BTreeMap::new(),
        };
        let phase = serde_json::json!({"sources":{"lib.rs":{"checksum":digest(b"A"),"size":1}},
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
        retain(
            &root.0,
            &entry,
            [&stdout, &stderr, &reads],
            &BTreeSet::new(),
        )
        .ok_or("retain")?;
        std::fs::write(&artifact, b"artifact B")?;
        let names = BTreeSet::from([OsString::from("libunit-abcd.rmeta")]);
        let (restored, out, err) =
            restore(&root.0, &basis, &root.0, &names, || true).ok_or("restore")?;
        assert_eq!(restored.invocation.run_id, "establishing-A");
        assert_eq!(std::fs::read(&artifact)?, b"artifact A");
        assert_eq!(out, b"literal stdout");
        assert_eq!(err, b"literal diagnostics");
        std::fs::write(&artifact, b"artifact B")?;
        assert!(restore(&root.0, &basis, &root.0, &names, || false).is_none());
        assert!(!artifact.exists());
        Ok(())
    }
}
