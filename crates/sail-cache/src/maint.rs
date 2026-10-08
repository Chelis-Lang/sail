//! Disk-tier maintenance: listing, summaries, integrity checks, eviction
//! and clearing (ADR 0015).
//!
//! Every operation here is safe to run while a daemon serves and writes the
//! same cache:
//! - entries are only ever removed whole, never edited, so a reader sees a
//!   complete entry or no entry;
//! - a removed entry is a miss, which costs a re-solve, never correctness;
//! - temporary files younger than [`STALE_TEMP`] are never touched, because
//!   they may belong to a write in progress.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sail_key::QueryKey;
use serde::{Deserialize, Serialize};

use crate::{DiskCache, Meta, Problem, STALE_TEMP, parse_entry, shape_ok};

/// What a file under `<root>/v2` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Named by a key, in its prefix directory.
    Entry(QueryKey),
    /// A key-named directory where an entry file should be.
    EntryDirectory,
    /// A temporary file of a write (`.tmp-*`).
    Temp,
    /// Anything else.
    Stray,
}

/// One file (or misplaced directory) under `<root>/v2`.
#[derive(Debug, Clone)]
pub struct Listed {
    /// Its path.
    pub path: PathBuf,
    /// What it is.
    pub kind: FileKind,
    /// Its size in bytes.
    pub bytes: u64,
    /// The later of its access and modification times: when it was last
    /// used, as far as the cache knows.
    pub used: SystemTime,
}

/// Parses a 64-character lowercase hex key.
#[must_use]
pub fn parse_key(hex: &str) -> Option<QueryKey> {
    if hex.len() != 64 || hex.bytes().any(|b| !matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(QueryKey(k))
}

fn last_used(meta: &std::fs::Metadata) -> SystemTime {
    let m = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let a = meta.accessed().unwrap_or(SystemTime::UNIX_EPOCH);
    m.max(a)
}

fn listed(path: PathBuf, kind: FileKind, meta: &std::fs::Metadata) -> Listed {
    Listed {
        path,
        kind,
        bytes: meta.len(),
        used: last_used(meta),
    }
}

fn is_prefix_dir(name: &str) -> bool {
    name.len() == 2 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl DiskCache {
    /// Every file under `<root>/v2`, classified. Files that vanish while
    /// listing are skipped.
    #[must_use]
    pub fn list(&self) -> Vec<Listed> {
        let mut out = Vec::new();
        let Ok(dirs) = std::fs::read_dir(self.entries_dir()) else {
            return out;
        };
        for d in dirs.flatten() {
            let path = d.path();
            let name = d.file_name().to_string_lossy().into_owned();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_dir() || !is_prefix_dir(&name) {
                out.push(listed(path, FileKind::Stray, &meta));
                continue;
            }
            let Ok(files) = std::fs::read_dir(&path) else {
                continue;
            };
            for f in files.flatten() {
                let fpath = f.path();
                let fname = f.file_name().to_string_lossy().into_owned();
                let Ok(fmeta) = std::fs::symlink_metadata(&fpath) else {
                    continue;
                };
                let key = parse_key(&fname).filter(|_| fname.starts_with(name.as_str()));
                let kind = match key {
                    Some(k) if fmeta.is_file() => FileKind::Entry(k),
                    Some(_) if fmeta.is_dir() => FileKind::EntryDirectory,
                    _ if fname.starts_with(".tmp-") && fmeta.is_file() => FileKind::Temp,
                    _ => FileKind::Stray,
                };
                out.push(listed(fpath, kind, &fmeta));
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    /// Entry files and their total size, without reading them.
    #[must_use]
    pub fn usage(&self) -> Usage {
        let mut u = Usage::default();
        for l in self.list() {
            if matches!(l.kind, FileKind::Entry(_)) {
                u.entries += 1;
                u.bytes += l.bytes;
            }
        }
        u
    }

    /// Fully checks the entry file at `path` for `key`: integrity (header,
    /// key, hash) and the shape of the reply for its command.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn check_file(path: &Path, key: &QueryKey) -> Result<(Meta, Vec<u8>), Problem> {
        let data = std::fs::read(path).map_err(|_| Problem::Unreadable)?;
        let (meta, reply) = parse_entry(&data, &key.to_string())?;
        if !shape_ok(&meta.command, &reply) {
            return Err(Problem::BadShape);
        }
        Ok((meta, reply))
    }

    /// The problem with a listed file, if any: `None` for a valid entry or
    /// a temporary file that may still be in use.
    fn problem(l: &Listed, now: SystemTime) -> Option<Problem> {
        match l.kind {
            FileKind::Entry(k) => Self::check_file(&l.path, &k).err(),
            FileKind::EntryDirectory => Some(Problem::Directory),
            FileKind::Temp => now
                .duration_since(l.used)
                .is_ok_and(|age| age >= STALE_TEMP)
                .then_some(Problem::StaleTemp),
            FileKind::Stray => Some(Problem::Stray),
        }
    }

    /// Summarizes the cache: entries and bytes by command, `unknown`
    /// answers and their recorded effort, and files that are not valid
    /// entries. Reads every entry; removes nothing.
    #[must_use]
    pub fn summarize(&self) -> Summary {
        let mut s = Summary::default();
        let mut fractions = Vec::new();
        for l in self.list() {
            let FileKind::Entry(k) = l.kind else {
                match l.kind {
                    FileKind::Temp => s.temp_files += 1,
                    _ => s.invalid += 1,
                }
                continue;
            };
            let Ok((meta, reply)) = Self::check_file(&l.path, &k) else {
                // Vanished or invalid: the next read removes it.
                if l.path.exists() {
                    s.invalid += 1;
                }
                continue;
            };
            s.entries += 1;
            s.bytes += l.bytes;
            let by = s.by_command.entry(meta.command.clone()).or_default();
            by.entries += 1;
            by.bytes += l.bytes;
            if meta.command.starts_with("check-sat") && reply.trim_ascii() == b"unknown" {
                let unknown = &mut s.unknown;
                unknown.entries += 1;
                match meta.effort {
                    Some(e) => {
                        unknown.with_effort += 1;
                        let reason = e.reason.clone().unwrap_or_else(|| "(none)".to_owned());
                        *unknown.by_reason.entry(reason).or_default() += 1;
                        if let (Some(used), Some(limit)) = (e.used, e.limit)
                            && limit > 0
                        {
                            #[allow(clippy::cast_precision_loss, reason = "a ratio for display")]
                            fractions.push(used as f64 / limit as f64);
                            if used.saturating_mul(2) <= limit {
                                unknown.within_half_limit += 1;
                            }
                        }
                    }
                    None => {
                        *unknown
                            .by_reason
                            .entry("(no effort)".to_owned())
                            .or_default() += 1;
                    }
                }
            }
        }
        fractions.sort_by(f64::total_cmp);
        if let (Some(&min), Some(&max)) = (fractions.first(), fractions.last()) {
            s.unknown.used_fraction = Some(Spread {
                min,
                median: fractions.get(fractions.len() / 2).copied().unwrap_or(min),
                max,
            });
        }
        s
    }

    /// Checks every file under `<root>/v2`. With `remove`, deletes each bad
    /// one. Temporary files younger than [`STALE_TEMP`] are skipped.
    #[must_use]
    pub fn verify(&self, remove: bool) -> VerifyReport {
        let mut r = VerifyReport::default();
        let now = SystemTime::now();
        for l in self.list() {
            if matches!(l.kind, FileKind::Entry(_)) {
                r.checked += 1;
            }
            match Self::problem(&l, now) {
                None if matches!(l.kind, FileKind::Entry(_)) => r.valid += 1,
                None => {}
                // A file that vanished mid-check was removed or replaced by
                // someone else; it is not a finding.
                Some(Problem::Unreadable) if !l.path.exists() => r.checked -= 1,
                Some(problem) => {
                    let removed = remove && remove_path(&l.path);
                    if removed {
                        r.removed += 1;
                    }
                    *r.by_problem.entry(problem.name().to_owned()).or_default() += 1;
                    r.bad.push(BadFile {
                        path: l.path.display().to_string(),
                        problem,
                        removed,
                    });
                }
            }
        }
        r
    }

    /// Evicts the least recently used entries until entry files take at
    /// most `max_bytes`, and removes temporary files older than
    /// [`STALE_TEMP`]. "Used" is the later of a file's access time (which
    /// the daemon refreshes on hits) and its modification time.
    #[must_use]
    pub fn gc(&self, max_bytes: u64) -> GcReport {
        let now = SystemTime::now();
        let mut entries = Vec::new();
        let mut r = GcReport {
            max_bytes,
            ..GcReport::default()
        };
        for l in self.list() {
            match l.kind {
                FileKind::Entry(_) => {
                    r.before.entries += 1;
                    r.before.bytes += l.bytes;
                    entries.push(l);
                }
                FileKind::Temp => {
                    let stale = now
                        .duration_since(l.used)
                        .is_ok_and(|age| age >= STALE_TEMP);
                    if stale && remove_path(&l.path) {
                        r.temp_removed += 1;
                    }
                }
                FileKind::EntryDirectory | FileKind::Stray => {}
            }
        }
        // Oldest first; ties broken by path for determinism.
        entries.sort_by(|a, b| a.used.cmp(&b.used).then_with(|| a.path.cmp(&b.path)));
        let mut total = r.before.bytes;
        for l in entries {
            if total <= max_bytes {
                break;
            }
            if remove_path(&l.path) {
                r.evicted.entries += 1;
                r.evicted.bytes += l.bytes;
            }
            total = total.saturating_sub(l.bytes);
        }
        r.after = Usage {
            entries: r.before.entries - r.evicted.entries,
            bytes: r.before.bytes - r.evicted.bytes,
        };
        r
    }

    /// Removes every entry. The entries directory is first renamed out of
    /// the way, so entries disappear at once, then deleted. Returns the
    /// number of entries there were.
    ///
    /// # Errors
    ///
    /// I/O errors renaming or deleting the directory.
    pub fn clear(&self) -> std::io::Result<Usage> {
        let usage = self.usage();
        let dir = self.entries_dir();
        if !dir.exists() {
            return Ok(usage);
        }
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let trash = self
            .root()
            .join(format!(".clear-{}-{nanos}", std::process::id()));
        std::fs::rename(&dir, &trash)?;
        std::fs::remove_dir_all(&trash)?;
        Ok(usage)
    }
}

fn remove_path(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path).is_ok(),
        Ok(_) => std::fs::remove_file(path).is_ok(),
        Err(_) => false,
    }
}

/// A count of entries and their bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Entry files.
    pub entries: u64,
    /// Their total size.
    pub bytes: u64,
}

/// Minimum, median and maximum of a set of values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    /// Smallest.
    pub min: f64,
    /// Median.
    pub median: f64,
    /// Largest.
    pub max: f64,
}

/// Cached `check-sat` answers of `unknown` and their recorded effort
/// (ADR 0009).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnknownSummary {
    /// `unknown` entries.
    pub entries: u64,
    /// Those with recorded effort.
    pub with_effort: u64,
    /// Those that used at most half of a non-zero limit.
    pub within_half_limit: u64,
    /// Entries by `reason-unknown` reply.
    pub by_reason: BTreeMap<String, u64>,
    /// Effort used as a fraction of the limit, where both are known.
    pub used_fraction: Option<Spread>,
}

/// What [`DiskCache::summarize`] found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Valid entries.
    pub entries: u64,
    /// Their total size in bytes.
    pub bytes: u64,
    /// Valid entries by the command they answer.
    pub by_command: BTreeMap<String, Usage>,
    /// `unknown` answers.
    pub unknown: UnknownSummary,
    /// Files that are not valid entries (see `sail cache verify`).
    pub invalid: u64,
    /// Temporary files of writes (in progress, or left by a crash).
    pub temp_files: u64,
}

/// One file [`DiskCache::verify`] rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadFile {
    /// Its path.
    pub path: String,
    /// What is wrong with it.
    pub problem: Problem,
    /// Whether it was removed.
    pub removed: bool,
}

/// What [`DiskCache::verify`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// Entry files checked.
    pub checked: u64,
    /// Those that passed.
    pub valid: u64,
    /// Every rejected file.
    pub bad: Vec<BadFile>,
    /// Rejected files by problem.
    pub by_problem: BTreeMap<String, u64>,
    /// Rejected files removed.
    pub removed: u64,
}

/// What [`DiskCache::gc`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcReport {
    /// The budget.
    pub max_bytes: u64,
    /// Entries before.
    pub before: Usage,
    /// Entries evicted.
    pub evicted: Usage,
    /// Entries after.
    pub after: Usage,
    /// Stale temporary files removed.
    pub temp_removed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::time::Duration;

    fn scratch(name: &str) -> DiskCache {
        let d = std::env::temp_dir().join(format!("sail-maint-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        DiskCache::new(d)
    }

    fn put(c: &DiskCache, i: u8, reply: &[u8], command: &str) -> QueryKey {
        let k = QueryKey([i; 32]);
        assert!(c.put(&k, reply, command, "", "pinned-solver", None).is_ok());
        k
    }

    fn age(c: &DiskCache, k: &QueryKey, secs: u64) {
        let t = SystemTime::now() - Duration::from_secs(secs);
        let f = std::fs::File::options().write(true).open(c.entry_path(k));
        assert!(
            f.and_then(|f| f.set_times(FileTimes::new().set_accessed(t).set_modified(t)))
                .is_ok()
        );
    }

    #[test]
    fn gc_evicts_least_recently_used_first() {
        let c = scratch("gc");
        let keys: Vec<QueryKey> = (0..6u8)
            .map(|i| put(&c, i, &vec![b'x'; 1000], "get-model"))
            .collect();
        // Entry i was last used 1000 - 100 i seconds ago; entry 0 is the
        // oldest. Then a hit on entry 1 makes it the newest.
        for (i, k) in keys.iter().enumerate() {
            age(&c, k, 1000 - 100 * i as u64);
        }
        assert!(c.get(&keys[1]).is_some());
        let size = c.usage().bytes / 6;
        let r = c.gc(size * 3);
        assert_eq!(r.evicted.entries, 3);
        assert!(r.after.bytes <= size * 3);
        let left: Vec<bool> = keys.iter().map(|k| c.entry_path(k).exists()).collect();
        assert_eq!(left, [false, true, false, false, true, true]);
        // Everything left is still valid.
        assert_eq!(c.verify(false).bad.len(), 0);
        assert_eq!(c.gc(0).after.entries, 0);
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn verify_reports_and_removes_bad_files() {
        let c = scratch("verify");
        let good = put(&c, 1, b"unsat\n", "check-sat");
        let shape = put(&c, 2, b"(error \"x\")\n", "check-sat");
        let flipped = put(&c, 3, b"sat\n", "check-sat");
        let p = c.entry_path(&flipped);
        let mut data = std::fs::read(&p).unwrap_or_default();
        if let Some(b) = data.last_mut() {
            *b ^= 1;
        }
        assert!(std::fs::write(&p, data).is_ok());
        let dir = c.entry_path(&good).parent().map(Path::to_path_buf);
        let dir = dir.unwrap_or_default();
        assert!(std::fs::write(dir.join("junk"), b"x").is_ok());
        assert!(std::fs::write(dir.join(".tmp-1-2-x"), b"partial").is_ok());
        let r = c.verify(false);
        assert_eq!((r.checked, r.valid, r.bad.len()), (3, 1, 3));
        assert_eq!(r.by_problem.get("bad-shape"), Some(&1));
        assert_eq!(r.by_problem.get("hash-mismatch"), Some(&1));
        assert_eq!(r.by_problem.get("stray"), Some(&1));
        assert!(c.entry_path(&shape).exists());
        let r = c.verify(true);
        assert_eq!(r.removed, 3);
        assert!(!c.entry_path(&shape).exists());
        // The young temporary file is left alone.
        assert!(dir.join(".tmp-1-2-x").exists());
        let r = c.verify(false);
        assert_eq!((r.checked, r.valid, r.bad.len()), (1, 1, 0));
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn summarize_counts_commands_and_unknowns() {
        let c = scratch("summary");
        put(&c, 1, b"unsat\n", "check-sat");
        put(&c, 2, b"hi\n", "echo");
        for (i, used) in [(3u8, 10u64), (4, 90)] {
            let k = QueryKey([i; 32]);
            let effort = crate::Effort {
                limit: Some(100),
                used: Some(used),
                reason: Some("(incomplete quantifiers)".to_owned()),
            };
            assert!(
                c.put(&k, b"unknown\n", "check-sat", "", "p", Some(&effort))
                    .is_ok()
            );
        }
        let s = c.summarize();
        assert_eq!(s.entries, 4);
        assert_eq!(s.by_command.get("check-sat").map(|u| u.entries), Some(3));
        assert_eq!(s.unknown.entries, 2);
        assert_eq!(s.unknown.within_half_limit, 1);
        assert_eq!(
            s.unknown.by_reason.get("(incomplete quantifiers)"),
            Some(&2)
        );
        assert!(s.unknown.used_fraction.is_some_and(|f| f.max > 0.8));
        let cleared = c.clear();
        assert_eq!(cleared.ok().map(|u| u.entries), Some(4));
        assert_eq!(c.usage().entries, 0);
        let _ = std::fs::remove_dir_all(c.root());
    }
}
