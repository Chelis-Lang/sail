//! Opt-in per-file owned-target observations under ADR 0092.
//!
//! These supply content digests only. Every consumer retains its source,
//! receipt and proof checks, and complete current namespace barriers.

use super::noop::Stamp;
use super::target_digest_memo::{DirectoryBindings, Tree};
use super::{files, observations};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

pub(super) const FILE: &str = "file-target-digests.json";
const KIND: &str = "sail-current-file-target-digests";
const LIMIT: u64 = 8 * 1024 * 1024;
const ENTRIES: usize = 100_000;

pub(super) fn enabled() -> bool {
    enabled_for(std::env::var_os("SAIL_MANAGED_FILE_TARGET_DIGEST_MEMO").as_deref())
}

fn enabled_for(value: Option<&std::ffi::OsStr>) -> bool {
    cfg!(unix) && value == Some(std::ffi::OsStr::new("1"))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: PathBuf,
    origin: String,
    run_id: String,
    started: SystemTime,
    stamp: Stamp,
    digest: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    kind: String,
    schema: u32,
    root: PathBuf,
    binding: String,
    // Arrays retain duplicate/order evidence instead of silently overwriting
    // repeated map keys during decoding.
    directories: Vec<(PathBuf, (u64, u64, u32))>,
    entries: Vec<Entry>,
    checksum: String,
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
}

impl Record {
    fn checksum(&self) -> Option<String> {
        files::json_digest(&(
            &self.kind,
            self.schema,
            &self.root,
            &self.binding,
            &self.directories,
            &self.entries,
        ))
        .ok()
    }

    fn valid(&self, tree: &Tree, binding: &str, started: SystemTime) -> bool {
        let valid = self.kind == KIND
            && self.schema == 1
            && self.root.as_os_str() == tree.root.as_os_str()
            && self.binding == binding
            && self.entries.len().saturating_add(self.directories.len()) <= ENTRIES
            && !self.entries.is_empty()
            && self
                .directories
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
            && self
                .entries
                .windows(2)
                .all(|pair| pair[0].path < pair[1].path)
            && self.directories.iter().all(|(path, _)| {
                absolute(path) && (path.starts_with(&self.root) || self.root.starts_with(path))
            });
        if !valid {
            return false;
        }
        let parents: BTreeSet<_> = self
            .entries
            .iter()
            .flat_map(|entry| entry.path.ancestors().skip(1))
            .collect();
        self.directories.len() == parents.len()
            && self
                .directories
                .iter()
                .zip(parents)
                .all(|((recorded, _), parent)| recorded.as_os_str() == parent.as_os_str())
            && self.entries.iter().all(|entry| {
                absolute(&entry.path)
                    && entry.path.starts_with(&self.root)
                    && entry.path != self.root
                    && entry.origin == "current"
                    && !entry.run_id.is_empty()
                    && entry.run_id.len() <= 256
                    && entry.started <= started
                    && entry.stamp.settled(entry.started)
                    && digest_valid(&entry.digest)
            })
    }
}

/// One complete pre-phase target observation, after the caller's independent
/// complete source/context/receipt checks. Nothing survives a retry.
pub(super) struct Candidate {
    file: PathBuf,
    binding: String,
    started: SystemTime,
    tree: Tree,
    directories: DirectoryBindings,
    inherited: BTreeMap<PathBuf, Entry>,
    records: Arc<observations::JsonRecord<Record>>,
}

impl Candidate {
    pub(super) fn begin(
        root: &Path,
        file: PathBuf,
        binding: String,
        started: SystemTime,
        records: Arc<observations::JsonRecord<Record>>,
    ) -> Option<Self> {
        if !cfg!(unix) || !file.is_absolute() || started > SystemTime::now() {
            return None;
        }
        let (tree, directories) = Tree::observe_bound(root)?;
        Some(Self {
            file,
            binding,
            started,
            tree,
            directories,
            inherited: BTreeMap::new(),
            records,
        })
    }

    /// Fresh bounded bytes, original per-file provenance and every original
    /// physical ancestor identity, followed by a complete current readback.
    pub(super) fn read(&mut self) -> Option<BTreeMap<PathBuf, String>> {
        self.inherited.clear();
        let record = self.records.read(&self.file, LIMIT, |record| {
            record.checksum().as_deref() == Some(record.checksum.as_str())
        })?;
        if !record.valid(&self.tree, &self.binding, self.started) {
            return None;
        }
        let original: BTreeMap<_, _> = record.directories.iter().cloned().collect();
        let inherited: BTreeMap<_, _> = record
            .entries
            .iter()
            .filter(|entry| {
                self.tree
                    .stamps
                    .get_key_value(&entry.path)
                    .is_some_and(|(path, stamp)| {
                        path.as_os_str() == entry.path.as_os_str() && *stamp == entry.stamp
                    })
                    && !self.directories.contains_key(&entry.path)
                    && entry.path.ancestors().skip(1).all(|parent| {
                        original
                            .get_key_value(parent)
                            .is_some_and(|(path, identity)| {
                                path.as_os_str() == parent.as_os_str()
                                    && self.directories.get(parent) == Some(identity)
                            })
                    })
            })
            .map(|entry| (entry.path.clone(), entry.clone()))
            .collect();
        if Tree::observe(&self.tree.root).as_ref() != Some(&self.tree) {
            return None;
        }
        let digests = inherited
            .iter()
            .map(|(path, entry)| (path.clone(), entry.digest.clone()))
            .collect();
        self.inherited = inherited;
        Some(digests)
    }

    /// The caller supplies only this phase's successful reads or the exact
    /// inherited entries returned above. No other memo may seed fresh entries.
    pub(super) fn finish(self, digests: &BTreeMap<PathBuf, String>) -> Option<Pending> {
        if Tree::observe(&self.tree.root).as_ref() != Some(&self.tree) {
            return None;
        }
        let entries: Vec<_> = digests
            .iter()
            .filter_map(|(path, digest)| {
                let (observed, stamp) = self.tree.stamps.get_key_value(path)?;
                if observed.as_os_str() != path.as_os_str()
                    || self.directories.contains_key(path)
                    || !digest_valid(digest)
                {
                    return None;
                }
                if let Some(entry) = self.inherited.get(path)
                    && entry.digest == *digest
                    && entry.stamp == *stamp
                {
                    return Some(PendingEntry::Inherited(entry.clone()));
                }
                stamp.settled(self.started).then(|| PendingEntry::Fresh {
                    path: path.clone(),
                    stamp: *stamp,
                    digest: digest.clone(),
                })
            })
            .collect();
        (!entries.is_empty()).then_some(Pending {
            candidate: self,
            entries,
        })
    }
}

enum PendingEntry {
    Inherited(Entry),
    Fresh {
        path: PathBuf,
        stamp: Stamp,
        digest: String,
    },
}

pub(super) struct Pending {
    candidate: Candidate,
    entries: Vec<PendingEntry>,
}

impl Pending {
    /// Only a successful complete Current run, without force/retry, calls this.
    /// Publication cannot replace the original read stamp or settlement start.
    pub(super) fn publish(self, run: &str) {
        let candidate = self.candidate;
        if run.is_empty()
            || run.len() > 256
            || candidate.started > SystemTime::now()
            || Tree::observe(&candidate.tree.root).as_ref() != Some(&candidate.tree)
        {
            return;
        }
        let entries: Vec<_> = self
            .entries
            .into_iter()
            .map(|entry| match entry {
                PendingEntry::Inherited(entry) => entry,
                PendingEntry::Fresh {
                    path,
                    stamp,
                    digest,
                } => Entry {
                    path,
                    stamp,
                    digest,
                    origin: "current".into(),
                    run_id: run.into(),
                    started: candidate.started,
                },
            })
            .collect();
        let parents: BTreeSet<_> = entries
            .iter()
            .flat_map(|entry| entry.path.ancestors().skip(1))
            .collect();
        let directories: Option<Vec<_>> = parents
            .into_iter()
            .map(|path| Some((path.to_owned(), *candidate.directories.get(path)?)))
            .collect();
        let Some(directories) = directories else {
            return;
        };
        let mut record = Record {
            kind: KIND.into(),
            schema: 1,
            root: candidate.tree.root.clone(),
            binding: candidate.binding,
            directories,
            entries,
            checksum: String::new(),
        };
        if !record.valid(&candidate.tree, &record.binding, candidate.started) {
            return;
        }
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

#[cfg(test)]
mod selection_tests {
    use super::enabled_for;
    use std::ffi::OsStr;

    #[test]
    fn every_platform_defaults_off_and_only_exact_one_opts_in() {
        assert!(!enabled_for(None));
        assert_eq!(enabled_for(Some(OsStr::new("1"))), cfg!(unix));
        for value in ["", "0", "true", "2", "01"] {
            assert!(!enabled_for(Some(OsStr::new(value))));
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> std::io::Result<Self> {
            let root = std::env::temp_dir()
                .join(format!("sail-file-target-{label}-{}", std::process::id()));
            std::fs::create_dir_all(root.join("target/deps"))?;
            std::fs::create_dir_all(root.join("slot"))?;
            std::fs::write(root.join("target/deps/a.vir"), "before")?;
            std::fs::write(root.join("target/deps/b.vir"), "second")?;
            Ok(Self(root.canonicalize()?))
        }

        fn ledger(&self) -> PathBuf {
            self.0.join("slot").join(FILE)
        }

        fn candidate(&self, binding: &str, started: SystemTime) -> Option<Candidate> {
            Candidate::begin(
                &self.0.join("target"),
                self.ledger(),
                binding.into(),
                started,
                Arc::default(),
            )
        }

        fn digests(&self) -> crate::Result<BTreeMap<PathBuf, String>> {
            ["a.vir", "b.vir"]
                .into_iter()
                .map(|name| {
                    let path = self.0.join("target/deps").join(name);
                    let digest = files::file_digest(&path)?;
                    Ok((path, digest))
                })
                .collect()
        }

        fn seed(&self) -> Result<Record, Box<dyn std::error::Error>> {
            let candidate = self
                .candidate("binding", SystemTime::now())
                .ok_or("candidate")?;
            candidate
                .finish(&self.digests()?)
                .ok_or("original reads")?
                .publish("original-current-run");
            Ok(serde_json::from_slice(&std::fs::read(self.ledger())?)?)
        }

        fn write_record(&self, mut record: Record) -> TestResult {
            record.checksum = record.checksum().ok_or("checksum")?;
            std::fs::write(self.ledger(), serde_json::to_vec(&record)?)?;
            Ok(())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unrelated_churn_preserves_original_entries_but_phase_changes_refuse() -> TestResult {
        let f = Fixture::new("churn")?;
        std::thread::sleep(Duration::from_millis(2100));
        let original = f.seed()?;
        let bytes = std::fs::read(f.ledger())?;
        let stamp = Stamp::of(&std::fs::metadata(f.ledger())?);
        std::fs::write(f.0.join("target/unrelated"), "new name")?;
        let mut candidate = f.candidate("binding", SystemTime::now()).ok_or("churn")?;
        let cached = candidate.read().ok_or("unchanged files")?;
        assert_eq!(cached, f.digests()?);
        candidate
            .finish(&cached)
            .ok_or("readback")?
            .publish("later-run");
        assert_eq!(std::fs::read(f.ledger())?, bytes);
        assert_eq!(Stamp::of(&std::fs::metadata(f.ledger())?), stamp);
        let retained: Record = serde_json::from_slice(&bytes)?;
        assert_eq!(retained.entries[0].started, original.entries[0].started);
        assert_eq!(retained.entries[0].run_id, "original-current-run");
        std::fs::remove_file(f.0.join("target/unrelated"))?;
        let mut candidate = f.candidate("binding", SystemTime::now()).ok_or("deleted")?;
        assert_eq!(candidate.read(), Some(cached.clone()));
        std::fs::write(f.0.join("target/during-phase"), "mutation")?;
        assert!(candidate.finish(&cached).is_none());
        let mut candidate = f
            .candidate("binding", SystemTime::now())
            .ok_or("before read")?;
        std::fs::write(f.0.join("target/during-read"), "mutation")?;
        assert!(candidate.read().is_none());
        let candidate = f
            .candidate("binding", SystemTime::now())
            .ok_or("publication")?;
        let pending = candidate.finish(&f.digests()?).ok_or("pending")?;
        std::fs::write(f.0.join("target/late"), "late mutation")?;
        pending.publish("must-not-publish");
        assert_eq!(std::fs::read(f.ledger())?, bytes);
        Ok(())
    }

    #[test]
    fn same_size_restored_mtime_and_directory_modes_do_not_hide_changes() -> TestResult {
        let f = Fixture::new("restored-time")?;
        std::thread::sleep(Duration::from_millis(2100));
        f.seed()?;
        let path = f.0.join("target/deps/a.vir");
        let modified = std::fs::metadata(&path)?.modified()?;
        std::fs::write(&path, "after!")?;
        std::fs::File::options()
            .write(true)
            .open(&path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
        let mut candidate = f.candidate("binding", SystemTime::now()).ok_or("changed")?;
        let cached = candidate.read().ok_or("record")?;
        assert!(!cached.contains_key(&path));
        assert_eq!(cached.len(), 1);
        let directory = f.0.join("target/deps");
        let mode = std::fs::metadata(&directory)?.permissions().mode() ^ 0o010;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(mode))?;
        let mut candidate = f.candidate("binding", SystemTime::now()).ok_or("mode")?;
        assert!(candidate.read().ok_or("record")?.is_empty());
        Ok(())
    }

    #[test]
    fn replacing_a_parent_refuses_even_an_identical_hardlinked_file_stamp() -> TestResult {
        let f = Fixture::new("parent-replacement")?;
        std::fs::create_dir(f.0.join("target/spare"))?;
        for name in ["a.vir", "b.vir"] {
            std::fs::hard_link(
                f.0.join("target/deps").join(name),
                f.0.join("target/spare").join(name),
            )?;
        }
        std::thread::sleep(Duration::from_millis(2100));
        f.seed()?;
        let path = f.0.join("target/deps/a.vir");
        let stamp = Stamp::of(&std::fs::metadata(&path)?);
        std::fs::rename(f.0.join("target/deps"), f.0.join("target/retired"))?;
        std::fs::rename(f.0.join("target/spare"), f.0.join("target/deps"))?;
        assert_eq!(Stamp::of(&std::fs::metadata(&path)?), stamp);
        let mut candidate = f
            .candidate("binding", SystemTime::now())
            .ok_or("replacement")?;
        assert!(candidate.read().ok_or("record")?.is_empty());
        std::fs::rename(f.0.join("target/deps"), f.0.join("target/moved"))?;
        symlink("moved", f.0.join("target/deps"))?;
        assert!(f.candidate("binding", SystemTime::now()).is_none());
        Ok(())
    }

    #[test]
    fn an_originally_recent_read_never_ages_into_an_inherited_entry() -> TestResult {
        let f = Fixture::new("recent")?;
        std::thread::sleep(Duration::from_millis(2100));
        std::fs::write(f.0.join("target/deps/b.vir"), "recent")?;
        let original = f.seed()?;
        assert_eq!(original.entries.len(), 1);
        std::thread::sleep(Duration::from_millis(2100));
        let mut candidate = f.candidate("binding", SystemTime::now()).ok_or("aged")?;
        let inherited = candidate.read().ok_or("record")?;
        assert_eq!(inherited.len(), 1);
        assert!(!inherited.contains_key(&f.0.join("target/deps/b.vir")));
        let fresh = f.digests()?;
        candidate
            .finish(&fresh)
            .ok_or("fresh reads")?
            .publish("fresh-current-run");
        let updated: Record = serde_json::from_slice(&std::fs::read(f.ledger())?)?;
        assert_eq!(updated.entries.len(), 2);
        assert_eq!(updated.entries[0].run_id, "original-current-run");
        assert_eq!(updated.entries[0].started, original.entries[0].started);
        assert_eq!(updated.entries[1].run_id, "fresh-current-run");
        assert!(updated.entries[1].started > original.entries[0].started);
        Ok(())
    }

    #[test]
    fn source_context_and_receipt_binding_changes_refuse_the_whole_record() -> TestResult {
        let f = Fixture::new("binding")?;
        std::thread::sleep(Duration::from_millis(2100));
        f.seed()?;
        for binding in ["source", "context", "selection", "compiler", "receipt"] {
            let mut candidate = f.candidate(binding, SystemTime::now()).ok_or("candidate")?;
            assert!(candidate.read().is_none());
        }
        Ok(())
    }

    #[test]
    fn malformed_provenance_and_incomplete_or_duplicate_entries_refuse() -> TestResult {
        let f = Fixture::new("damaged")?;
        std::thread::sleep(Duration::from_millis(2100));
        let original = f.seed()?;
        for case in 0..9 {
            let mut record = original.clone();
            match case {
                0 => record.kind = "other".into(),
                1 => record.schema = 2,
                2 => record.entries[0].origin = "historical".into(),
                3 => record.entries[0].run_id.clear(),
                4 => record.entries[0].started = SystemTime::now() + Duration::from_secs(60),
                5 => record.entries[0].started = SystemTime::UNIX_EPOCH,
                6 => record.entries.push(record.entries[0].clone()),
                7 => record.entries.reverse(),
                _ => {
                    record.directories.pop();
                }
            }
            f.write_record(record)?;
            let mut candidate = f
                .candidate("binding", SystemTime::now())
                .ok_or("candidate")?;
            assert!(candidate.read().is_none(), "case {case}");
        }
        for bytes in [
            b"not json".to_vec(),
            vec![b' '; usize::try_from(LIMIT)? + 1],
        ] {
            std::fs::write(f.ledger(), bytes)?;
            let mut candidate = f
                .candidate("binding", SystemTime::now())
                .ok_or("candidate")?;
            assert!(candidate.read().is_none());
        }
        f.write_record(original)?;
        std::fs::remove_file(f.ledger())?;
        symlink(f.0.join("target/deps/a.vir"), f.ledger())?;
        let mut candidate = f
            .candidate("binding", SystemTime::now())
            .ok_or("candidate")?;
        assert!(candidate.read().is_none());
        Ok(())
    }
}
