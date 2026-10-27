//! Tiered result cache. Status: the local disk tier (ADR 0011), the
//! in-memory tier ([`memory`]), disk maintenance ([`maint`]: summaries,
//! integrity checks, eviction and clearing) and the shared tier of signed
//! entries ([`shared`], ADR 0017).
//!
//! An entry maps a v2 query key (ADR 0008) to the exact reply bytes the
//! pinned solver produced for that command. Each entry is one file,
//! `<root>/v2/<k0k1>/<key>`, holding a JSON header line followed by the reply
//! bytes. Writes are atomic: a temporary file, then a rename. A read checks
//! that the stored key matches and that the reply hash matches. Any problem
//! is a miss: the entry is removed and the caller falls back to the solver.
#![forbid(unsafe_code)]

pub mod maint;
pub mod memory;
pub mod pack;
mod persist;
pub mod shared;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use sail_key::{QueryKey, sha256};
use serde::{Deserialize, Serialize};

/// Sequence number for temporary file names.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Format version of entry files.
pub const ENTRY_VERSION: u32 = 2;

/// Metadata stored with each reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// Verification namespace; older entries belong to default mode.
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Entry format version.
    pub entry_version: u32,
    /// The key, as hex.
    pub key: String,
    /// SHA-256 of the reply bytes, as hex.
    pub reply_sha256: String,
    /// The command the reply answers, for example `check-sat`.
    pub command: String,
    /// Solver identity summary: `name version sha256 platform`.
    pub solver: String,
    /// Unix time the entry was stored.
    pub stored_unix: u64,
    /// How the result was established (charter rule 7).
    pub provenance: String,
    /// Solver effort for `check-sat` replies (ADR 0009): `limit`, `used`,
    /// and `reason` for `unknown` answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
}

fn default_mode() -> String {
    "default".to_owned()
}

/// Provenance and namespace for an explicitly scoped cache store.
pub struct StoreContext<'a> {
    /// Exact solver identity summary.
    pub solver: &'a str,
    /// How the reply was established.
    pub provenance: &'a str,
    /// Verification namespace.
    pub mode: &'a str,
}

/// Solver effort for a `check-sat` reply (ADR 0009).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effort {
    /// The `:rlimit` in force (0 means unlimited).
    pub limit: Option<u64>,
    /// Resource units used.
    pub used: Option<u64>,
    /// The `reason-unknown` reply, for `unknown` answers.
    pub reason: Option<String>,
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// How long an entry's access time may lag behind its last use. Reads touch
/// an entry's access time at most this often, so `gc` can evict the least
/// recently used entries without a write on every hit.
pub const TOUCH_INTERVAL: Duration = Duration::from_secs(60);

/// The identity of one entry file on disk: device, inode, length and
/// modification time.
///
/// Entries are only ever created by renaming a fresh temporary file into
/// place, so a different entry at the same path has a different inode. An
/// in-place rewrite changes the modification time. The in-memory tier
/// serves a reply only while its file still has the stamp it had when the
/// reply was read from it or written to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileStamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime_ns: i128,
}

impl FileStamp {
    /// The stamp of the file `meta` describes.
    #[must_use]
    pub fn of(meta: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            Self {
                dev: meta.dev(),
                ino: meta.ino(),
                len: meta.len(),
                mtime_ns: i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec()),
            }
        }
        #[cfg(not(unix))]
        {
            let mtime_ns = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i128::try_from(d.as_nanos()).unwrap_or(i128::MAX));
            Self {
                dev: 0,
                ino: 0,
                len: meta.len(),
                mtime_ns,
            }
        }
    }
}

/// A valid entry read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// The entry's header.
    pub meta: Meta,
    /// The exact reply bytes.
    pub reply: Vec<u8>,
    /// The stamp of the file the entry was read from.
    pub stamp: FileStamp,
}

/// What [`DiskCache::put_stamped`] did. This is the same source-shared
/// candidate outcome consumed by original-insertion memory settlement.
pub type PutOutcome = sail_acceptance::cache_candidate::Publication<FileStamp>;

/// Why an entry file is not a valid entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Problem {
    /// The file cannot be read.
    Unreadable,
    /// There is no header line, or it is not valid JSON for a header.
    BadHeader,
    /// The header has another entry format version.
    WrongVersion,
    /// The header names a different key from the file name.
    KeyMismatch,
    /// The reply does not hash to the header's `reply_sha256`.
    HashMismatch,
    /// The reply does not have the shape its command requires.
    BadShape,
    /// A directory where an entry file should be.
    Directory,
    /// A file whose name is not a key, or a directory that is not a key
    /// prefix.
    Stray,
    /// A temporary file older than [`STALE_TEMP`]: left by an interrupted
    /// write.
    StaleTemp,
}

impl Problem {
    /// A short name for reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::BadHeader => "bad-header",
            Self::WrongVersion => "wrong-version",
            Self::KeyMismatch => "key-mismatch",
            Self::HashMismatch => "hash-mismatch",
            Self::BadShape => "bad-shape",
            Self::Directory => "directory",
            Self::Stray => "stray",
            Self::StaleTemp => "stale-temp",
        }
    }
}

/// How old a temporary file must be before maintenance treats it as left
/// over from an interrupted write and removes it. Younger ones may belong
/// to a write in progress.
pub const STALE_TEMP: Duration = Duration::from_secs(3600);

/// Whether `reply` has the shape the solver gives for a command named
/// `command` (the name stored in the header). This is the coarse,
/// name-only form of the shim's shape check (ADR 0011): the header keeps
/// the command's name, not its text, so for example an `echo` reply is only
/// checked to end a line. Every reply must be exactly one complete response
/// frame: two concatenated forms would add a reply to a session's stream.
#[must_use]
pub fn shape_ok(command: &str, reply: &[u8]) -> bool {
    if !sail_smtlib::split::is_single_response(reply) {
        return false;
    }
    let body = reply.trim_ascii();
    match command {
        "check-sat" | "check-sat-assuming" => matches!(body, b"sat" | b"unsat" | b"unknown"),
        "echo" => reply.ends_with(b"\n"),
        "get-info" => body.starts_with(b"(:") && body.ends_with(b")"),
        _ => !body.is_empty() && !body.starts_with(b"(error"),
    }
}

/// Parses and checks entry file contents against the key named `key` (hex).
/// The header must parse, name `key`, carry this format version and hash
/// the reply correctly.
///
/// # Errors
///
/// The first problem found.
pub fn parse_entry(data: &[u8], key: &str) -> Result<(Meta, Vec<u8>), Problem> {
    let (header, reply) = sail_acceptance::query::split_record(data).ok_or(Problem::BadHeader)?;
    let meta: Meta = serde_json::from_slice(header).map_err(|_| Problem::BadHeader)?;
    let reply = reply.to_vec();
    let computed = hex(&sha256(&reply));
    let version = meta.entry_version;
    let recorded_key = meta.key.as_bytes().to_vec();
    let recorded_digest = meta.reply_sha256.as_bytes().to_vec();
    sail_acceptance::query::RecoveryRecord::new(meta, reply, version, recorded_key, recorded_digest)
        .recover(ENTRY_VERSION, key.as_bytes(), computed.as_bytes())
        .map_err(|rejection| match rejection {
            sail_acceptance::query::RecoveryRejection::WrongVersion => Problem::WrongVersion,
            sail_acceptance::query::RecoveryRejection::KeyMismatch => Problem::KeyMismatch,
            sail_acceptance::query::RecoveryRejection::HashMismatch => Problem::HashMismatch,
        })
}

/// Whether `meta`'s access time is missing or older than [`TOUCH_INTERVAL`].
fn access_stale(meta: &std::fs::Metadata) -> bool {
    meta.accessed()
        .ok()
        .and_then(|a| SystemTime::now().duration_since(a).ok())
        .is_none_or(|age| age >= TOUCH_INTERVAL)
}

/// Sets `file`'s access time to now if it is older than [`TOUCH_INTERVAL`].
/// Only the access time changes, so the file's [`FileStamp`] does not.
/// Failures are ignored: a missed touch only makes `gc` think the entry is
/// older than it is.
fn touch_if_stale(file: &std::fs::File, meta: &std::fs::Metadata) {
    if access_stale(meta) {
        let _ = file.set_times(std::fs::FileTimes::new().set_accessed(SystemTime::now()));
    }
}

/// The local disk tier.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// A cache rooted at `root` (created on first write).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The cache root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory holding every entry, `<root>/v2`.
    #[must_use]
    pub fn entries_dir(&self) -> PathBuf {
        self.root.join("v2")
    }

    /// The path of `key`'s entry file.
    #[must_use]
    pub fn entry_path(&self, key: &QueryKey) -> PathBuf {
        let k = key.to_string();
        self.entries_dir().join(k.get(..2).unwrap_or("xx")).join(k)
    }

    /// The reply stored for `key`, if there is a valid entry.
    #[must_use]
    pub fn get(&self, key: &QueryKey) -> Option<(Meta, Vec<u8>)> {
        self.get_stamped(key).map(|s| (s.meta, s.reply))
    }

    /// The entry stored for `key`, if valid, with the stamp of the file it
    /// was read from. A hit refreshes the file's access time (at most once
    /// per [`TOUCH_INTERVAL`]). An invalid entry file is removed.
    #[must_use]
    pub fn get_stamped(&self, key: &QueryKey) -> Option<Stored> {
        use std::io::Read as _;
        let path = self.entry_path(key);
        let mut file = std::fs::File::open(&path).ok()?;
        let meta = file.metadata().ok()?;
        if !meta.is_file() {
            // A directory or other non-file: a miss. `verify` reports it.
            return None;
        }
        let mut data = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
        let parsed = file
            .read_to_end(&mut data)
            .ok()
            .and_then(|_| parse_entry(&data, &key.to_string()).ok());
        let Some((m, reply)) = parsed else {
            // Corrupt or foreign entry: drop it; the caller re-solves.
            let _ = std::fs::remove_file(&path);
            return None;
        };
        touch_if_stale(&file, &meta);
        Some(Stored {
            meta: m,
            reply,
            stamp: FileStamp::of(&meta),
        })
    }

    /// The stamp of `key`'s entry file as it is now, if it is a file. The
    /// caller is about to serve this entry from memory, so the access time
    /// is refreshed if it is older than [`TOUCH_INTERVAL`], for `gc`.
    #[must_use]
    pub fn stamp_for_hit(&self, key: &QueryKey) -> Option<FileStamp> {
        let path = self.entry_path(key);
        let meta = std::fs::metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        if access_stale(&meta)
            && let Ok(f) = std::fs::File::open(&path)
        {
            touch_if_stale(&f, &meta);
        }
        Some(FileStamp::of(&meta))
    }

    /// Whether an entry file exists for `key` (valid or not).
    #[must_use]
    pub fn contains(&self, key: &QueryKey) -> bool {
        self.entry_path(key).is_file()
    }

    /// Stores `reply` for `key`. Existing entries are left alone: content
    /// addressing means they hold the same answer.
    ///
    /// # Errors
    ///
    /// I/O errors creating or writing the entry.
    pub fn put(
        &self,
        key: &QueryKey,
        reply: &[u8],
        command: &str,
        solver: &str,
        provenance: &str,
        effort: Option<&Effort>,
    ) -> std::io::Result<()> {
        self.put_stamped(key, reply, command, solver, provenance, effort)
            .map(|_| ())
    }

    /// [`DiskCache::put`], reporting whether the entry was written and, if
    /// so, the stamp of the file now at the entry's path.
    ///
    /// # Errors
    ///
    /// I/O errors creating or writing the entry.
    pub fn put_stamped(
        &self,
        key: &QueryKey,
        reply: &[u8],
        command: &str,
        solver: &str,
        provenance: &str,
        effort: Option<&Effort>,
    ) -> std::io::Result<PutOutcome> {
        self.put_scoped(
            key,
            reply,
            command,
            &StoreContext {
                solver,
                provenance,
                mode: "default",
            },
            effort,
        )
    }

    /// Stores an entry with explicit provenance and verification namespace.
    ///
    /// # Errors
    ///
    /// I/O errors creating or writing the entry.
    pub fn put_scoped(
        &self,
        key: &QueryKey,
        reply: &[u8],
        command: &str,
        context: &StoreContext<'_>,
        effort: Option<&Effort>,
    ) -> std::io::Result<PutOutcome> {
        let path = self.entry_path(key);
        if path.is_file() {
            return Ok(PutOutcome::Exists);
        }
        let dir = path.parent().unwrap_or(&self.root).to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let meta = Meta {
            mode: context.mode.to_owned(),
            entry_version: ENTRY_VERSION,
            key: key.to_string(),
            reply_sha256: hex(&sha256(reply)),
            command: command.to_owned(),
            solver: context.solver.to_owned(),
            stored_unix: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            provenance: context.provenance.to_owned(),
            effort: effort.cloned(),
        };
        // Unique per write: two writers may store the same key at once, and
        // must not write into the same temporary file.
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".tmp-{}-{seq}-{key}", std::process::id()));
        persist::write_record(&tmp, &path, &meta, reply).map(PutOutcome::Written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sail-cache-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn shape_requires_exactly_one_response_frame() {
        assert!(super::shape_ok("get-info", b"(:name \"Z3\")\n"));
        assert!(super::shape_ok("check-sat", b"unsat\n"));
        assert!(!super::shape_ok(
            "get-info",
            b"(:name \"Z3\")(:evil \"extra\")\n"
        ));
        assert!(!super::shape_ok("check-sat", b"unsat\nsat\n"));
        assert!(!super::shape_ok("get-model", b"(model)\n(model)\n"));
        assert!(!super::shape_ok("echo", b"a\nb\n"));
    }

    #[test]
    fn round_trips_exact_bytes() {
        let c = DiskCache::new(scratch("rt"));
        let k = QueryKey([1; 32]);
        assert!(c.get(&k).is_none());
        let reply = b"(model\n  (define-fun x () Int 1)\n)\n\x00\xff";
        assert!(
            c.put(&k, reply, "get-model", "z3", "pinned-solver", None)
                .is_ok()
        );
        let (meta, got) = c.get(&k).unwrap_or_default_for_test();
        assert_eq!(got, reply);
        assert_eq!(meta.command, "get-model");
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn record_framing_preserves_format_and_rejection_priority() {
        let c = DiskCache::new(scratch("framing"));
        let key = QueryKey([21; 32]);
        let reply = b"unsat\n\x00\xff\n";
        assert!(
            c.put(
                &key,
                reply,
                "check-sat",
                "z3\nidentity",
                "writer\ncontext",
                None
            )
            .is_ok()
        );
        let data = std::fs::read(c.entry_path(&key)).unwrap_or_default();
        let (meta, decoded) = parse_entry(&data, &key.to_string())
            .unwrap_or_else(|problem| unreachable!("unexpected parse failure: {problem:?}"));
        assert_eq!(decoded, reply);
        let wire = |meta: &Meta| {
            // Independent oracle for the existing header-newline-payload format.
            let mut bytes = serde_json::to_vec(meta).unwrap_or_default();
            bytes.push(b'\n');
            bytes.extend_from_slice(reply);
            bytes
        };
        assert_eq!(data, wire(&meta));
        assert_eq!(meta.solver, "z3\nidentity");
        assert_eq!(meta.provenance, "writer\ncontext");
        let mut changed = meta;
        changed.entry_version = 0;
        changed.key = "different-key".to_owned();
        changed.reply_sha256 = "different-digest".to_owned();
        assert!(matches!(
            parse_entry(&wire(&changed), &key.to_string()),
            Err(Problem::WrongVersion)
        ));
        changed.entry_version = ENTRY_VERSION;
        assert!(matches!(
            parse_entry(&wire(&changed), &key.to_string()),
            Err(Problem::KeyMismatch)
        ));
        changed.key = key.to_string();
        assert!(matches!(
            parse_entry(&wire(&changed), &key.to_string()),
            Err(Problem::HashMismatch)
        ));
        assert!(matches!(
            parse_entry(b"{}", &key.to_string()),
            Err(Problem::BadHeader)
        ));
        assert!(matches!(
            parse_entry(b"\nunsat\n", &key.to_string()),
            Err(Problem::BadHeader)
        ));
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn entries_without_session_silence_evidence_are_rejected() {
        let c = DiskCache::new(scratch("legacy-stderr"));
        let k = QueryKey([19; 32]);
        assert!(
            c.put(&k, b"unsat\n", "check-sat", "z3", "pinned-solver", None)
                .is_ok()
        );
        let data = std::fs::read(c.entry_path(&k)).unwrap_or_default();
        let end = data.iter().position(|&b| b == b'\n').unwrap_or(0);
        let mut header: serde_json::Value =
            serde_json::from_slice(&data[..end]).unwrap_or_default();
        header["entry_version"] = 1.into();
        let mut old = serde_json::to_vec(&header).unwrap_or_default();
        old.extend_from_slice(&data[end..]);
        assert!(matches!(
            parse_entry(&old, &k.to_string()),
            Err(Problem::WrongVersion)
        ));
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn corrupt_entries_are_misses_and_removed() {
        let c = DiskCache::new(scratch("corrupt"));
        let k = QueryKey([2; 32]);
        assert!(
            c.put(&k, b"unsat\n", "check-sat", "z3", "pinned-solver", None)
                .is_ok()
        );
        let path = c.entry_path(&k);
        let mut data = std::fs::read(&path).unwrap_or_default();
        if let Some(last) = data.last_mut() {
            *last ^= 1;
        }
        assert!(std::fs::write(&path, &data).is_ok());
        assert!(c.get(&k).is_none());
        assert!(!path.exists());
        // An entry filed under the wrong key is also a miss.
        let other = QueryKey([3; 32]);
        assert!(
            c.put(&k, b"sat\n", "check-sat", "z3", "pinned-solver", None)
                .is_ok()
        );
        assert!(std::fs::create_dir_all(c.entry_path(&other).parent().unwrap_or(c.root())).is_ok());
        assert!(std::fs::copy(c.entry_path(&k), c.entry_path(&other)).is_ok());
        assert!(c.get(&other).is_none());
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn concurrent_puts_of_one_key_leave_a_valid_entry() {
        // Statistics replies for one key differ in their timing figures, so
        // racing writers store different bytes. Each must write its own
        // temporary file, or the renamed entry can mix them.
        let c = DiskCache::new(scratch("race"));
        let k = QueryKey([4; 32]);
        for round in 0..40 {
            let _ = std::fs::remove_file(c.entry_path(&k));
            let barrier = std::sync::Barrier::new(8);
            std::thread::scope(|s| {
                for t in 0..8u8 {
                    let (c, barrier) = (&c, &barrier);
                    s.spawn(move || {
                        let reply = vec![b'a' + t; 4096 * (usize::from(t) + 1)];
                        barrier.wait();
                        let _ = c.put(&k, &reply, "get-info", "z3", "pinned-solver", None);
                    });
                }
            });
            let got = c.get(&k).map(|(_, r)| r);
            assert!(
                got.as_ref()
                    .is_some_and(|r| r.iter().all(|&b| Some(&b) == r.first())),
                "round {round}: no valid entry"
            );
        }
        let _ = std::fs::remove_dir_all(c.root());
    }

    trait OrDefault {
        fn unwrap_or_default_for_test(self) -> (Meta, Vec<u8>);
    }

    impl OrDefault for Option<(Meta, Vec<u8>)> {
        fn unwrap_or_default_for_test(self) -> (Meta, Vec<u8>) {
            match self {
                Some(x) => x,
                None => unreachable!("expected a cache hit"),
            }
        }
    }
}
