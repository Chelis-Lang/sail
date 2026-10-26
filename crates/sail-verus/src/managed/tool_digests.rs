//! Content identity of installed tools, memoized by complete stat stamp
//! (ADR 0041, note of 2026-10-26).
//!
//! The proof context and compiler family identify every installed tool
//! (Sail, Cargo, Cargo-Verus, Verus and its engine, the solver, rustc,
//! rustup, sysroot libraries) by the SHA-256 of its bytes, so a
//! reinstallation or a restored cache with identical bytes at the same path
//! keeps the same context. Byte equality is the identity relation
//! (`docs/design/evidence-acceptance-core.md`, assumption 4).
//!
//! Reading every tool on every run would cost about a gigabyte of reads, so
//! a per-machine memo maps each absolute path to the complete stat stamp
//! (device, inode, size, mode, mtime and ctime) it had while it was read and
//! the digest read. A later observation whose stamp equals the recorded one
//! exactly takes the recorded digest without reading. A digest is recorded
//! only when the stamps before and after the read are equal and the ctime is
//! at least 2 s older than the start of the observation, the settle rule of
//! unchanged-call replay. A restored memo never matches on another machine:
//! restoring gives new inodes and change times.

use super::files::file_digest;
use crate::{Error, Result};
use sail_api::stamp::Stamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The memo file, under the cache's `runtime-identities/`.
const FILE: &str = "tool-digests.json";
const VERSION: u32 = 1;
/// Above this many entries a save keeps only the paths this process saw.
const LIMIT: usize = 4096;
/// The prefix of a content identity, as recorded in runtime identities.
pub(super) const PREFIX: &str = "sha256:";

#[derive(Default, Serialize, Deserialize)]
struct Memo {
    version: u32,
    entries: BTreeMap<PathBuf, (Stamp, String)>,
}

/// One observation's view of the memo: the recorded entries and those this
/// observation added.
pub(super) struct ToolDigests {
    file: PathBuf,
    start: SystemTime,
    known: BTreeMap<PathBuf, (Stamp, String)>,
    seen: Mutex<BTreeMap<PathBuf, (Stamp, String)>>,
    recorded: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    reads: std::sync::atomic::AtomicUsize,
}

impl ToolDigests {
    /// The memo of `cache`; an absent or unreadable memo is empty.
    pub(super) fn open(cache: &Path) -> Self {
        let start = SystemTime::now();
        let file = cache.join("runtime-identities").join(FILE);
        let known = read(&file).map(|memo| memo.entries).unwrap_or_default();
        Self {
            file,
            start,
            known,
            seen: Mutex::default(),
            recorded: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            reads: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// The content identity of `path`: `sha256:` and the digest of its bytes,
    /// following links.
    ///
    /// # Errors
    /// `SAIL3101` when the file cannot be read.
    pub(super) fn identity(&self, path: &Path) -> Result<String> {
        let unreadable = |e: std::io::Error| {
            Error::new("SAIL3101", format!("cannot read {}: {e}", path.display()))
        };
        let before = Stamp::of(&std::fs::metadata(path).map_err(unreadable)?);
        let memoizable = cfg!(unix) && path.is_absolute();
        if memoizable && let Some(digest) = self.recorded_digest(path, before) {
            return Ok(format!("{PREFIX}{digest}"));
        }
        #[cfg(test)]
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let digest = file_digest(path)?;
        if memoizable
            && before.settled(self.start)
            && std::fs::metadata(path).is_ok_and(|m| Stamp::of(&m) == before)
            && let Ok(mut seen) = self.seen.lock()
        {
            seen.insert(path.to_owned(), (before, digest.clone()));
            self.recorded
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(format!("{PREFIX}{digest}"))
    }

    /// [`Self::identity`] of each path, read concurrently, in order.
    pub(super) fn identities(&self, paths: &[PathBuf]) -> Vec<Result<String>> {
        crate::parallel::in_order(paths, |path| self.identity(path))
    }

    /// Whether `path` still has the content identity `expected`.
    pub(super) fn matches(&self, path: &Path, expected: &str) -> bool {
        self.identity(path).is_ok_and(|actual| actual == expected)
    }

    fn recorded_digest(&self, path: &Path, stamp: Stamp) -> Option<String> {
        let hit = |entry: Option<&(Stamp, String)>| {
            entry
                .filter(|(recorded, _)| *recorded == stamp)
                .map(|(_, digest)| digest.clone())
        };
        if let Some(digest) = self.seen.lock().ok().and_then(|seen| hit(seen.get(path))) {
            return Some(digest);
        }
        let digest = hit(self.known.get(path))?;
        // A hit stays in the memo when this observation saves it.
        if let Ok(mut seen) = self.seen.lock() {
            seen.insert(path.to_owned(), (stamp, digest.clone()));
        }
        Some(digest)
    }

    /// Records the digests this observation read, merged into the memo as
    /// it is now. Best effort: without the record the next run reads again.
    pub(super) fn save(&self) {
        if !self.recorded.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let Ok(seen) = self.seen.lock() else {
            return;
        };
        let mut entries = read(&self.file)
            .map(|memo| memo.entries)
            .unwrap_or_default();
        if entries.len() + seen.len() > LIMIT {
            entries.clear();
        }
        entries.extend(
            seen.iter()
                .map(|(path, entry)| (path.clone(), entry.clone())),
        );
        let memo = Memo {
            version: VERSION,
            entries,
        };
        let Some(parent) = self.file.parent() else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec(&memo) else {
            return;
        };
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let temp = parent.join(format!(".{FILE}.{}-{nanos}", std::process::id()));
        if std::fs::create_dir_all(parent).is_ok()
            && std::fs::write(&temp, bytes)
                .and_then(|()| std::fs::rename(&temp, &self.file))
                .is_err()
        {
            let _ = std::fs::remove_file(&temp);
        }
    }

    #[cfg(test)]
    pub(super) fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn read(file: &Path) -> Option<Memo> {
    let memo = serde_json::from_slice::<Memo>(&std::fs::read(file).ok()?).ok()?;
    (memo.version == VERSION).then_some(memo)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn fixture(name: &str) -> std::io::Result<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("sail-tool-digests-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cache"))?;
        root.canonicalize()
    }

    /// A memo whose observation starts `later` after now, so files written
    /// now are settled for it.
    fn open_later(cache: &Path, later: Duration) -> ToolDigests {
        let mut memo = ToolDigests::open(cache);
        memo.start += later;
        memo
    }

    #[test]
    fn identical_bytes_reinstalled_keep_their_identity() -> TestResult {
        let root = fixture("reinstall")?;
        let tool = root.join("rustc");
        std::fs::write(&tool, b"toolchain bytes")?;
        let first = ToolDigests::open(&root.join("cache")).identity(&tool)?;
        // A new inode and mtime with the same bytes at the same path.
        let staged = root.join("rustc.new");
        std::fs::write(&staged, b"toolchain bytes")?;
        std::fs::rename(&staged, &tool)?;
        let second = ToolDigests::open(&root.join("cache")).identity(&tool)?;
        assert_eq!(first, second);
        assert_eq!(first, format!("{PREFIX}{}", file_digest(&tool)?));
        std::fs::write(&tool, b"another toolchain")?;
        assert_ne!(
            ToolDigests::open(&root.join("cache")).identity(&tool)?,
            first
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn a_settled_stamp_is_reused_without_reading() -> TestResult {
        let root = fixture("hit")?;
        let cache = root.join("cache");
        let tool = root.join("cargo");
        std::fs::write(&tool, b"cargo bytes")?;
        let memo = open_later(&cache, Duration::from_secs(3));
        let identity = memo.identity(&tool)?;
        assert_eq!(memo.reads(), 1);
        memo.save();
        let again = open_later(&cache, Duration::from_secs(3));
        assert_eq!(again.identity(&tool)?, identity);
        assert!(again.matches(&tool, &identity));
        assert_eq!(
            again.reads(),
            0,
            "a recorded stamp takes the recorded digest"
        );
        // A rewrite changes the stamp, even with the same length.
        std::fs::write(&tool, b"cargo BYTES")?;
        let changed = open_later(&cache, Duration::from_secs(3));
        assert!(!changed.matches(&tool, &identity));
        assert_eq!(changed.reads(), 1);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn a_recent_status_change_is_not_recorded() -> TestResult {
        let root = fixture("recent")?;
        let cache = root.join("cache");
        let tool = root.join("z3");
        std::fs::write(&tool, b"solver")?;
        let memo = ToolDigests::open(&cache);
        let identity = memo.identity(&tool)?;
        memo.save();
        assert!(!cache.join("runtime-identities").join(FILE).exists());
        let again = ToolDigests::open(&cache);
        assert_eq!(again.identity(&tool)?, identity);
        assert_eq!(again.reads(), 1, "an unsettled stamp is read again");
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn a_restored_memo_never_matches_a_new_inode() -> TestResult {
        let root = fixture("restored")?;
        let cache = root.join("cache");
        let tool = root.join("sail");
        std::fs::write(&tool, b"sail one")?;
        let memo = open_later(&cache, Duration::from_secs(3));
        let identity = memo.identity(&tool)?;
        memo.save();
        // Same size, new bytes, new inode: the memo is not consulted.
        let staged = root.join("sail.new");
        std::fs::write(&staged, b"sail two")?;
        std::fs::rename(&staged, &tool)?;
        let fresh = open_later(&cache, Duration::from_secs(3));
        assert_ne!(fresh.identity(&tool)?, identity);
        assert_eq!(fresh.reads(), 1);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn relative_paths_and_missing_files() -> TestResult {
        let root = fixture("relative")?;
        let memo = open_later(&root.join("cache"), Duration::from_secs(3));
        assert_eq!(
            memo.identity(&root.join("missing")).err().map(|e| e.code),
            Some("SAIL3101")
        );
        assert!(!memo.matches(&root.join("missing"), "sha256:00"));
        memo.save();
        assert!(!root.join("cache/runtime-identities").join(FILE).exists());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
