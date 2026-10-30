//! Opt-in owned-target digest observations under ADR 0091.
//!
//! The complete physical tree and its ancestors are freshly enumerated around
//! every digest phase. Any changed entry or binding keeps the ordinary reads.

use super::{files, noop::Stamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(super) const FILE: &str = "target-digests.json";
const LIMIT: u64 = 8 * 1024 * 1024;
const ENTRIES: usize = 100_000;
const KIND: &str = "sail-current-target-digests";

pub(super) fn enabled() -> bool {
    cfg!(unix)
        && std::env::var_os("SAIL_MANAGED_TARGET_DIGEST_MEMO").is_some_and(|value| value == "1")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    root: PathBuf,
    /// Fresh physical ancestor identities; unrelated sibling writes do not
    /// change this target namespace. The root itself keeps its full stamp.
    ancestors: BTreeMap<PathBuf, (u64, u64, u32)>,
    /// Every directory/file in the complete target, including the root.
    stamps: BTreeMap<PathBuf, Stamp>,
}

impl Tree {
    fn observe(root: &Path) -> Option<Self> {
        if !root.is_absolute() || root.canonicalize().ok()?.as_path() != root {
            return None;
        }
        let ancestors = physical_ancestors(root)?;
        let mut stamps = BTreeMap::from([(
            root.to_owned(),
            Stamp::of(&std::fs::symlink_metadata(root).ok()?),
        )]);
        let mut pending = vec![root.to_owned()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).ok()? {
                let entry = entry.ok()?;
                let metadata = std::fs::symlink_metadata(entry.path()).ok()?;
                // Aliases and special files retain complete ordinary reads.
                if !metadata.is_file() && !metadata.is_dir() {
                    return None;
                }
                let path = entry.path();
                if metadata.is_dir() {
                    pending.push(path.clone());
                }
                if stamps.insert(path, Stamp::of(&metadata)).is_some() || stamps.len() > ENTRIES {
                    return None;
                }
            }
        }
        Some(Self {
            root: root.to_owned(),
            ancestors,
            stamps,
        })
    }

    fn settled(&self, started: SystemTime) -> bool {
        self.stamps.values().all(|stamp| stamp.settled(started))
    }
}

#[cfg(unix)]
fn physical_ancestors(root: &Path) -> Option<BTreeMap<PathBuf, (u64, u64, u32)>> {
    use std::os::unix::fs::MetadataExt as _;
    root.ancestors()
        .map(|path| {
            let metadata = std::fs::symlink_metadata(path).ok()?;
            metadata.is_dir().then(|| {
                (
                    path.to_owned(),
                    (metadata.dev(), metadata.ino(), metadata.mode()),
                )
            })
        })
        .collect()
}

#[cfg(not(unix))]
fn physical_ancestors(_root: &Path) -> Option<BTreeMap<PathBuf, (u64, u64, u32)>> {
    None
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    kind: String,
    schema: u32,
    origin: String,
    run_id: String,
    started: SystemTime,
    binding: String,
    tree: Tree,
    digests: BTreeMap<PathBuf, String>,
    projections: Vec<(String, String, String)>,
    checksum: String,
}

fn is_digest(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

impl Record {
    fn checksum(&self) -> Option<String> {
        serde_json::to_vec(&(
            &self.kind,
            self.schema,
            &self.origin,
            &self.run_id,
            self.started,
            &self.binding,
            &self.tree,
            &self.digests,
            &self.projections,
        ))
        .ok()
        .map(|bytes| files::digest(&bytes))
    }
}

/// Source, metadata/selection, compiler/context and exact receipt bytes are
/// supplied by the caller in `binding`. This is no proof authority by itself.
pub(super) struct Candidate {
    file: PathBuf,
    binding: String,
    tree: Tree,
    started: SystemTime,
}

impl Candidate {
    pub(super) fn begin(
        target: &Path,
        file: PathBuf,
        binding: String,
        started: SystemTime,
    ) -> Option<Self> {
        if !file.is_absolute() {
            return None;
        }
        Some(Self {
            file,
            binding,
            tree: Tree::observe(target)?,
            started,
        })
    }

    pub(super) fn read(&self) -> Option<BTreeMap<PathBuf, String>> {
        self.load().map(|record| record.digests)
    }

    pub(super) fn revisions(&self) -> Option<BTreeMap<(String, String), String>> {
        Some(
            self.load()?
                .projections
                .into_iter()
                .map(|(selection, id, revision)| ((selection, id), revision))
                .collect(),
        )
    }

    fn load(&self) -> Option<Record> {
        let mut bytes = Vec::new();
        super::dependency_snapshot::open(&self.file)?
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > LIMIT {
            return None;
        }
        let record: Record = serde_json::from_slice(&bytes).ok()?;
        if record.kind != KIND
            || record.schema != 1
            || record.origin != "current"
            || record.run_id.is_empty()
            || record.run_id.len() > 256
            || record.started > SystemTime::now()
            || record.binding != self.binding
            || record.tree != self.tree
            || !record.tree.settled(record.started)
            || record.digests.len() > ENTRIES
            || record.projections.len() > ENTRIES
            || !record.projections.iter().all(|(selection, id, revision)| {
                is_digest(selection) && !id.is_empty() && id.len() <= 16_384 && is_digest(revision)
            })
            || record
                .projections
                .windows(2)
                .any(|pair| (&pair[0].0, &pair[0].1) >= (&pair[1].0, &pair[1].1))
            || record.checksum().as_deref() != Some(record.checksum.as_str())
            || !record.digests.iter().all(|(path, hash)| {
                path.starts_with(&self.tree.root)
                    && self.tree.stamps.contains_key(path)
                    && is_digest(hash)
            })
        {
            return None;
        }
        (Tree::observe(&self.tree.root).as_ref() == Some(&self.tree)).then_some(record)
    }

    /// Original read stamps must still match after the phase and at Current
    /// publication. Never stamp old digest bytes afresh at publication time.
    pub(super) fn finish(
        self,
        digests: BTreeMap<PathBuf, String>,
        projections: BTreeMap<(String, String), String>,
    ) -> Option<Pending> {
        (Tree::observe(&self.tree.root)? == self.tree).then_some(Pending {
            candidate: self,
            digests,
            projections,
        })
    }
}

pub(super) struct Pending {
    candidate: Candidate,
    digests: BTreeMap<PathBuf, String>,
    projections: BTreeMap<(String, String), String>,
}

impl Pending {
    /// Only a successful complete Current run may call this, without retry.
    pub(super) fn publish(self, run: &str) {
        let candidate = self.candidate;
        if run.is_empty()
            || run.len() > 256
            || candidate.started > SystemTime::now()
            || !candidate.tree.settled(candidate.started)
            || Tree::observe(&candidate.tree.root).as_ref() != Some(&candidate.tree)
        {
            return;
        }
        let digests = self
            .digests
            .into_iter()
            .filter(|(path, _)| {
                path.starts_with(&candidate.tree.root) && candidate.tree.stamps.contains_key(path)
            })
            .collect();
        let mut record = Record {
            kind: KIND.into(),
            schema: 1,
            origin: "current".into(),
            run_id: run.into(),
            started: candidate.started,
            binding: candidate.binding,
            tree: candidate.tree,
            digests,
            projections: self
                .projections
                .into_iter()
                .map(|((selection, id), revision)| (selection, id, revision))
                .collect(),
            checksum: String::new(),
        };
        let Some(checksum) = record.checksum() else {
            return;
        };
        record.checksum = checksum;
        if let Ok(bytes) = serde_json::to_vec(&record)
            && bytes.len() as u64 <= LIMIT
        {
            let _ = files::bytes_if_changed(&candidate.file, &bytes);
        }
    }
}

/// Exact bounded receipt bytes; aliases, incomplete reads and unknown entries
/// refuse the optimization. No receipt header is treated as content identity.
pub(super) fn receipt_binding(root: &Path) -> Option<BTreeMap<PathBuf, String>> {
    let before = Tree::observe(root)?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(root).ok()? {
        let path = entry.ok()?.path();
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() > LIMIT || paths.len() >= ENTRIES {
            return None;
        }
        paths.push(path);
    }
    paths.sort();
    let descriptors: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    let reads = files::digest_all_tagged(&descriptors);
    if !files::read_outcome_keys_match(&descriptors, &reads) {
        return None;
    }
    if Tree::observe(root)? != before {
        return None;
    }
    reads
        .into_iter()
        .map(|(path, result)| Some((path.to_owned(), result.ok()?)))
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(label: &str) -> std::io::Result<Self> {
            let root = std::env::temp_dir().join(format!(
                "sail-target-digests-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(root.join("target/deps"))?;
            std::fs::create_dir_all(root.join("slot"))?;
            std::fs::write(root.join("target/deps/a.vir"), "before")?;
            Ok(Self(root.canonicalize()?))
        }
        fn candidate(&self, binding: &str, started: SystemTime) -> Option<Candidate> {
            Candidate::begin(
                &self.0.join("target"),
                self.0.join("slot").join(FILE),
                binding.into(),
                started,
            )
        }
        fn digests(&self) -> crate::Result<BTreeMap<PathBuf, String>> {
            let path = self.0.join("target/deps/a.vir");
            Ok(BTreeMap::from([(path.clone(), files::file_digest(&path)?)]))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_target_tree_and_bindings_are_required_for_every_cached_hash() -> TestResult {
        let fixture = Fixture::new("settled")?;
        std::thread::sleep(std::time::Duration::from_millis(2100));
        let started = SystemTime::now();
        let digests = fixture.digests()?;
        fixture
            .candidate("original", started)
            .ok_or("candidate")?
            .finish(digests.clone(), BTreeMap::new())
            .ok_or("original readback")?
            .publish("current-run");
        assert_eq!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read(),
            Some(digests)
        );
        for changed in [
            "source",
            "metadata",
            "selection",
            "compiler",
            "context",
            "receipt-bytes",
        ] {
            assert!(
                fixture
                    .candidate(changed, SystemTime::now())
                    .ok_or("candidate")?
                    .read()
                    .is_none(),
                "{changed}"
            );
        }
        let file = fixture.0.join("target/deps/a.vir");
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&file, "after!")?;
        std::fs::File::options()
            .write(true)
            .open(&file)?
            .set_modified(mtime)?;
        assert!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read()
                .is_none()
        );
        // Fresh same-byte restoration still has a different ctime.
        std::fs::write(&file, "before")?;
        std::fs::File::options()
            .write(true)
            .open(&file)?
            .set_modified(mtime)?;
        assert!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read()
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn pure_revisions_keep_exact_selection_keys_and_checksum_coverage() -> TestResult {
        let fixture = Fixture::new("projections")?;
        std::thread::sleep(std::time::Duration::from_millis(2100));
        let key = ("a".repeat(64), "package::library".to_owned());
        let projections = BTreeMap::from([(key.clone(), "b".repeat(64))]);
        fixture
            .candidate("complete-source-and-receipts", SystemTime::now())
            .ok_or("candidate")?
            .finish(fixture.digests()?, projections.clone())
            .ok_or("readback")?
            .publish("current-run");
        let candidate = fixture
            .candidate("complete-source-and-receipts", SystemTime::now())
            .ok_or("candidate")?;
        assert_eq!(candidate.revisions(), Some(projections));
        let changed = fixture
            .candidate("changed-source-or-receipt", SystemTime::now())
            .ok_or("candidate")?;
        assert!(changed.revisions().is_none());
        let file = fixture.0.join("slot").join(FILE);
        let original = std::fs::read(&file)?;
        for change in ["checksum", "selection", "duplicate"] {
            let mut record: Record = serde_json::from_slice(&original)?;
            match change {
                "checksum" => record.projections[0].2 = "c".repeat(64),
                "selection" => record.projections[0].0 = "bad".into(),
                "duplicate" => record.projections.push(record.projections[0].clone()),
                _ => return Err("unknown control".into()),
            }
            if change != "checksum" {
                record.checksum = record.checksum().ok_or("checksum")?;
            }
            std::fs::write(&file, serde_json::to_vec(&record)?)?;
            assert!(candidate.revisions().is_none(), "{change}");
        }
        Ok(())
    }

    #[test]
    fn target_namespace_changes_and_original_unsettled_reads_refuse() -> TestResult {
        let fixture = Fixture::new("unsettled")?;
        let started = SystemTime::now();
        fixture
            .candidate("original", started)
            .ok_or("candidate")?
            .finish(fixture.digests()?, BTreeMap::new())
            .ok_or("readback")?
            .publish("current-run");
        assert!(!fixture.0.join("slot").join(FILE).exists());
        std::thread::sleep(std::time::Duration::from_millis(2100));
        assert!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read()
                .is_none()
        );
        let candidate = fixture
            .candidate("original", SystemTime::now())
            .ok_or("candidate")?;
        let digests = fixture.digests()?;
        std::fs::write(fixture.0.join("target/new"), "new")?;
        assert!(candidate.finish(digests.clone(), BTreeMap::new()).is_none());
        std::fs::remove_file(fixture.0.join("target/new"))?;
        let candidate = fixture
            .candidate("original", SystemTime::now())
            .ok_or("candidate")?;
        std::fs::remove_file(fixture.0.join("target/deps/a.vir"))?;
        assert!(candidate.finish(digests, BTreeMap::new()).is_none());
        std::os::unix::fs::symlink(fixture.0.join("slot"), fixture.0.join("target/alias"))?;
        assert!(fixture.candidate("original", SystemTime::now()).is_none());
        Ok(())
    }

    #[test]
    fn malformed_origin_and_ledgers_do_not_supply_target_digests() -> TestResult {
        let fixture = Fixture::new("damaged")?;
        std::thread::sleep(std::time::Duration::from_millis(2100));
        fixture
            .candidate("original", SystemTime::now())
            .ok_or("candidate")?
            .finish(fixture.digests()?, BTreeMap::new())
            .ok_or("readback")?
            .publish("current-run");
        let file = fixture.0.join("slot").join(FILE);
        let original = std::fs::read(&file)?;
        for change in ["origin", "schema", "future", "foreign", "hash", "checksum"] {
            let mut record: Record = serde_json::from_slice(&original)?;
            match change {
                "origin" => record.origin = "unknown".into(),
                "schema" => record.schema += 1,
                "future" => record.started = SystemTime::now() + std::time::Duration::from_secs(10),
                "foreign" => {
                    record
                        .digests
                        .insert(fixture.0.join("outside"), "a".repeat(64));
                }
                "hash" => {
                    record
                        .digests
                        .insert(fixture.0.join("target/deps/a.vir"), "bad".into());
                }
                "checksum" => record.binding = "wrong".into(),
                _ => return Err("unknown control".into()),
            }
            if change != "checksum" {
                record.checksum = record.checksum().ok_or("checksum")?;
            }
            std::fs::write(&file, serde_json::to_vec(&record)?)?;
            assert!(
                fixture
                    .candidate("original", SystemTime::now())
                    .ok_or("candidate")?
                    .read()
                    .is_none(),
                "{change}"
            );
        }
        std::fs::write(&file, vec![b' '; usize::try_from(LIMIT)? + 1])?;
        assert!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read()
                .is_none()
        );
        std::fs::remove_file(&file)?;
        std::fs::write(fixture.0.join("slot/held"), original)?;
        std::os::unix::fs::symlink(fixture.0.join("slot/held"), &file)?;
        assert!(
            fixture
                .candidate("original", SystemTime::now())
                .ok_or("candidate")?
                .read()
                .is_none()
        );
        Ok(())
    }
}
