//! Bounded local source digest observations under ADR 0091.
//!
//! Namespace discovery and original before/after readback belong to the caller.
//! This module supplies individual digests only under equal full file stamps.
//! Only a successful complete Current caller may publish its original reads.

use super::{files, noop::Stamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(super) const FIELD: &str = "sail_source_digest_memo";
pub(super) const FILE: &str = "source-digests.json";
const LIMIT: u64 = 8 * 1024 * 1024;
const ENTRIES: usize = 100_000;
const KIND: &str = "sail-current-source-digests";

/// The prototype stays opt-in until platform before/after qualification.
pub(super) fn enabled() -> bool {
    cfg!(unix)
        && std::env::var_os("SAIL_MANAGED_SOURCE_DIGEST_MEMO").is_some_and(|value| value == "1")
        && std::env::var_os("SAIL_MANAGED_FORCE").is_none_or(|value| value != "1")
}

#[derive(Debug)]
pub(super) struct Source {
    file: PathBuf,
    namespace: String,
    regular: BTreeMap<PathBuf, Stamp>,
    /// A successfully admitted previous Current ledger, used only to avoid
    /// rewriting identical observations. Every read still validates disk data.
    previous: std::sync::OnceLock<Vec<Entry>>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: PathBuf,
    stamp: Stamp,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    kind: String,
    schema: u32,
    namespace: String,
    origin: String,
    run_id: String,
    started: SystemTime,
    entries: Vec<Entry>,
    checksum: String,
}

impl Record {
    fn checksum(&self) -> Option<String> {
        serde_json::to_vec(&(
            &self.kind,
            self.schema,
            &self.namespace,
            &self.origin,
            &self.run_id,
            self.started,
            &self.entries,
        ))
        .ok()
        .map(|bytes| files::digest(&bytes))
    }
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

impl Source {
    /// A fresh complete namespace and regular-file stamps from the original
    /// before-content readback. Neither a ledger nor a projection selects it.
    pub(super) fn new(
        file: PathBuf,
        paths: &BTreeSet<PathBuf>,
        bindings: &BTreeMap<PathBuf, PathBuf>,
        mut regular: BTreeMap<PathBuf, Stamp>,
    ) -> Option<Self> {
        if !file.is_absolute()
            || paths.len() > ENTRIES
            || !paths.iter().all(|path| path.is_absolute())
        {
            return None;
        }
        regular.retain(|path, _| paths.contains(path));
        let namespace = files::digest(&serde_json::to_vec(&(KIND, paths, bindings)).ok()?);
        Some(Self {
            file,
            namespace,
            regular,
            previous: std::sync::OnceLock::new(),
        })
    }

    /// Changed entries are read afresh; a changed namespace or alias binding
    /// refuses the whole ledger. Unsettled original observations never age in.
    pub(super) fn read(&self) -> Option<BTreeMap<PathBuf, String>> {
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
            || record.namespace != self.namespace
            || record.origin != "current"
            || record.run_id.is_empty()
            || record.run_id.len() > 256
            || record.started > SystemTime::now()
            || record.entries.len() > ENTRIES
            || record.checksum().as_deref() != Some(record.checksum.as_str())
        {
            return None;
        }
        let mut previous: Option<&Path> = None;
        let mut matched = BTreeMap::new();
        for entry in &record.entries {
            if !entry.path.is_absolute()
                || !self.regular.contains_key(&entry.path)
                || !hash(&entry.sha256)
                || !entry.stamp.settled(record.started)
                || previous.is_some_and(|path| path >= entry.path.as_path())
            {
                return None;
            }
            previous = Some(&entry.path);
            if self.regular.get(&entry.path) == Some(&entry.stamp) {
                matched.insert(entry.path.clone(), entry.sha256.clone());
            }
        }
        let _ = self.previous.set(record.entries);
        Some(matched)
    }

    /// Called only after successful complete Current publication, without a
    /// retry. These are the original bracketed bytes and stamps, never a later
    /// stat paired with earlier bytes. Failure costs only the next optimization.
    pub(super) fn publish(
        &self,
        digests: &BTreeMap<PathBuf, String>,
        run: &str,
        started: SystemTime,
    ) {
        let entries: Vec<_> = digests
            .iter()
            .filter_map(|(path, sha256)| {
                let stamp = *self.regular.get(path)?;
                (stamp.settled(started) && hash(sha256)).then(|| Entry {
                    path: path.clone(),
                    stamp,
                    sha256: sha256.clone(),
                })
            })
            .collect();
        // Keep the actual original Current provenance when every path,
        // original full stamp and digest is already identical. An originally
        // omitted unsettled entry remains omitted until a new fresh read can
        // independently qualify it; entries with changed stamps never match.
        if self
            .previous
            .get()
            .is_some_and(|previous| *previous == entries)
        {
            return;
        }
        if entries.len() > ENTRIES
            || run.is_empty()
            || run.len() > 256
            || started > SystemTime::now()
        {
            return;
        }
        let mut record = Record {
            kind: KIND.into(),
            schema: 1,
            namespace: self.namespace.clone(),
            origin: "current".into(),
            run_id: run.into(),
            started,
            entries,
            checksum: String::new(),
        };
        let Some(checksum) = record.checksum() else {
            return;
        };
        record.checksum = checksum;
        let Ok(bytes) = serde_json::to_vec(&record) else {
            return;
        };
        if bytes.len() as u64 <= LIMIT {
            let _ = files::bytes_if_changed(&self.file, &bytes);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(label: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "sail-source-digests-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path)?;
            for name in ["a.rs", "b.rs"] {
                std::fs::write(path.join(name), "before")?;
            }
            Ok(Self(path.canonicalize()?))
        }
        fn source(&self, bindings: &BTreeMap<PathBuf, PathBuf>) -> Option<Source> {
            let paths: BTreeSet<_> = [self.0.join("a.rs"), self.0.join("b.rs")]
                .into_iter()
                .collect();
            let regular = paths
                .iter()
                .filter_map(|path| {
                    Some((
                        path.clone(),
                        Stamp::of(&std::fs::symlink_metadata(path).ok()?),
                    ))
                })
                .collect();
            Source::new(self.0.join(FILE), &paths, bindings, regular)
        }
        fn digests(&self) -> crate::Result<BTreeMap<PathBuf, String>> {
            ["a.rs", "b.rs"]
                .into_iter()
                .map(|name| {
                    let path = self.0.join(name);
                    files::file_digest(&path).map(|hash| (path, hash))
                })
                .collect()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn settled_source_reuses_exact_digests_and_refuses_changed_entries_and_namespaces() -> TestResult
    {
        let fixture = Fixture::new("settled")?;
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        let started = SystemTime::now();
        let source = fixture.source(&BTreeMap::new()).ok_or("source")?;
        let hashes = fixture.digests()?;
        source.publish(&hashes, "original-current-run", started);
        assert_eq!(source.read(), Some(hashes.clone()));
        let path = fixture.0.join("a.rs");
        let original = path.metadata()?.modified()?;
        std::fs::write(&path, "after!")?;
        std::fs::File::options()
            .write(true)
            .open(&path)?
            .set_modified(original)?;
        let fresh = fixture.source(&BTreeMap::new()).ok_or("fresh source")?;
        let retained = fresh.read().ok_or("ledger")?;
        assert!(!retained.contains_key(&path));
        assert_eq!(
            retained.get(&fixture.0.join("b.rs")),
            hashes.get(&fixture.0.join("b.rs"))
        );
        assert_ne!(fixture.digests()?, hashes);
        let bindings = BTreeMap::from([(fixture.0.join("alias"), fixture.0.join("b.rs"))]);
        assert!(
            fixture
                .source(&bindings)
                .ok_or("alias source")?
                .read()
                .is_none()
        );
        let names = BTreeSet::from([fixture.0.join("b.rs")]);
        assert!(
            Source::new(
                fixture.0.join(FILE),
                &names,
                &BTreeMap::new(),
                fresh.regular.clone()
            )
            .ok_or("namespace")?
            .read()
            .is_none()
        );
        // The checksum is not a substitute for origin, policy or sorted-key validation.
        let original = std::fs::read(&source.file)?;
        for kind in ["origin", "schema", "future", "unknown-entry", "duplicate"] {
            let mut record: Record = serde_json::from_slice(&original)?;
            match kind {
                "origin" => record.origin = "historical".into(),
                "schema" => record.schema += 1,
                "future" => record.started = SystemTime::now() + std::time::Duration::from_secs(60),
                "unknown-entry" => record.entries[0].path = fixture.0.join("unknown"),
                "duplicate" => record.entries.push(Entry {
                    path: record.entries[0].path.clone(),
                    stamp: record.entries[0].stamp,
                    sha256: record.entries[0].sha256.clone(),
                }),
                _ => return Err("unknown control".into()),
            }
            record.checksum = record.checksum().ok_or("checksum")?;
            std::fs::write(&source.file, serde_json::to_vec(&record)?)?;
            assert!(fresh.read().is_none(), "{kind}");
        }
        std::fs::write(&source.file, &original)?;
        std::fs::rename(&source.file, fixture.0.join("held"))?;
        std::os::unix::fs::symlink(fixture.0.join("held"), &source.file)?;
        assert!(fresh.read().is_none());
        Ok(())
    }

    #[test]
    fn source_memo_switch_process_probe() {
        if let Ok(expected) = std::env::var("SAIL_SOURCE_MEMO_PROBE") {
            assert_eq!(enabled(), expected == "enabled");
        }
    }

    #[test]
    fn environment_force_refuses_source_memo_before_any_snapshot() -> TestResult {
        for (force, expected) in [
            (None, "enabled"),
            (Some("0"), "enabled"),
            (Some("1"), "disabled"),
        ] {
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .args([
                    "--exact",
                    "managed::digest_memo::tests::source_memo_switch_process_probe",
                    "--nocapture",
                ])
                .env("SAIL_MANAGED_SOURCE_DIGEST_MEMO", "1")
                .env("SAIL_SOURCE_MEMO_PROBE", expected)
                .env_remove("SAIL_MANAGED_FORCE");
            if let Some(force) = force {
                command.env("SAIL_MANAGED_FORCE", force);
            }
            let output = command.output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        }
        Ok(())
    }

    #[test]
    fn unchanged_publication_retains_the_original_current_provenance() -> TestResult {
        let fixture = Fixture::new("stable-current")?;
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        let original = fixture.source(&BTreeMap::new()).ok_or("source")?;
        let hashes = fixture.digests()?;
        let started = SystemTime::now();
        original.publish(&hashes, "original-current-run", started);
        let bytes = std::fs::read(&original.file)?;
        let stamp = Stamp::of(&original.file.metadata()?);
        let next = fixture.source(&BTreeMap::new()).ok_or("next source")?;
        assert_eq!(next.read(), Some(hashes.clone()));
        next.publish(&hashes, "later-current-run", SystemTime::now());
        assert_eq!(std::fs::read(&next.file)?, bytes);
        assert_eq!(Stamp::of(&next.file.metadata()?), stamp);
        let retained: Record = serde_json::from_slice(&bytes)?;
        assert_eq!(retained.run_id, "original-current-run");
        assert_eq!(retained.started, started);
        // A changed stamp cannot preserve that original observation, even
        // after a same-byte rewrite and restoration of the old mtime.
        let file = fixture.0.join("a.rs");
        let modified = file.metadata()?.modified()?;
        std::fs::write(&file, "before")?;
        std::fs::File::options()
            .write(true)
            .open(&file)?
            .set_modified(modified)?;
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        let changed = fixture.source(&BTreeMap::new()).ok_or("changed source")?;
        assert_ne!(changed.read(), Some(hashes.clone()));
        changed.publish(&fixture.digests()?, "fresh-current-run", SystemTime::now());
        let updated: Record = serde_json::from_slice(&std::fs::read(&changed.file)?)?;
        assert_eq!(updated.run_id, "fresh-current-run");
        assert_ne!(std::fs::read(&changed.file)?, bytes);
        Ok(())
    }

    #[test]
    fn an_unsettled_original_observation_never_ages_into_trust() -> TestResult {
        let fixture = Fixture::new("unsettled")?;
        let source = fixture.source(&BTreeMap::new()).ok_or("source")?;
        source.publish(
            &fixture.digests()?,
            "original-current-run",
            SystemTime::now(),
        );
        assert_eq!(source.read(), Some(BTreeMap::new()));
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        assert_eq!(
            fixture
                .source(&BTreeMap::new())
                .ok_or("later source")?
                .read(),
            Some(BTreeMap::new())
        );
        std::fs::write(&source.file, "damaged")?;
        assert!(source.read().is_none());
        std::fs::write(&source.file, vec![b' '; usize::try_from(LIMIT)? + 1])?;
        assert!(source.read().is_none());
        Ok(())
    }
}
