//! PROTOTYPE (not for merge without owner sign-off): file content digests
//! remembered across runs, keyed by the complete stat stamp of the opened
//! file (device, inode, size, mode, mtime and ctime in nanoseconds).
//!
//! A remembered digest is used only when the opened file's stamp equals the
//! recorded one exactly. A digest is recorded only when the file's stamp is
//! the same before and after the complete read and its ctime is at least
//! `SETTLED` older than the read: any later change to the file then moves
//! ctime forward (a user program cannot set ctime), so a same-tick rewrite
//! cannot hide behind a recorded stamp.
//!
//! On by default in this prototype; `SAIL_MANAGED_DIGEST_MEMO=0` disables it.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const SETTLED: std::time::Duration = std::time::Duration::from_secs(2);
const FILE: &str = "digest-memo-v1";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp([i64; 8]);

impl Stamp {
    #[cfg(unix)]
    pub(super) fn of(metadata: &std::fs::Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        Some(Self([
            i64::try_from(metadata.dev()).ok()?,
            i64::try_from(metadata.ino()).ok()?,
            i64::try_from(metadata.size()).ok()?,
            i64::from(metadata.mode()),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ]))
    }
    #[cfg(not(unix))]
    pub(super) fn of(_metadata: &std::fs::Metadata) -> Option<Self> {
        None
    }
    fn settled(self) -> bool {
        let ctime = std::time::UNIX_EPOCH
            + std::time::Duration::new(
                u64::try_from(self.0[6]).unwrap_or(u64::MAX),
                u32::try_from(self.0[7]).unwrap_or(0),
            );
        std::time::SystemTime::now()
            .duration_since(ctime)
            .is_ok_and(|age| age >= SETTLED)
    }
}

struct Memo {
    path: PathBuf,
    entries: HashMap<PathBuf, (Stamp, String)>,
    dirty: bool,
}

fn memo() -> Option<&'static Mutex<Memo>> {
    static MEMO: OnceLock<Option<Mutex<Memo>>> = OnceLock::new();
    MEMO.get_or_init(|| {
        if std::env::var_os("SAIL_MANAGED_DIGEST_MEMO").as_deref() == Some("0".as_ref()) {
            return None;
        }
        let path = crate::cache_dir().ok()?.join(FILE);
        let entries = std::fs::read(&path)
            .ok()
            .map(|bytes| parse(&bytes))
            .unwrap_or_default();
        Some(Mutex::new(Memo {
            path,
            entries,
            dirty: false,
        }))
    })
    .as_ref()
}

fn parse(bytes: &[u8]) -> HashMap<PathBuf, (Stamp, String)> {
    use std::os::unix::ffi::OsStrExt as _;
    let mut entries = HashMap::new();
    for line in bytes.split(|b| *b == b'\n') {
        let mut fields = line.splitn(10, |b| *b == b'\t');
        let Some(sha) = fields.next().and_then(|f| std::str::from_utf8(f).ok()) else {
            continue;
        };
        let mut numbers = [0i64; 8];
        let mut ok = sha.len() == 64;
        for number in &mut numbers {
            match fields
                .next()
                .and_then(|f| std::str::from_utf8(f).ok())
                .and_then(|f| f.parse().ok())
            {
                Some(value) => *number = value,
                None => ok = false,
            }
        }
        match fields.next() {
            Some(path) if ok && !path.is_empty() => {
                entries.insert(
                    PathBuf::from(std::ffi::OsStr::from_bytes(path)),
                    (Stamp(numbers), sha.to_owned()),
                );
            }
            _ => {}
        }
    }
    entries
}

/// The remembered digest of the opened file, when its stamp is unchanged.
pub(super) fn lookup(path: &Path, stamp: Stamp) -> Option<String> {
    let memo = memo()?.lock().ok()?;
    let (recorded, sha) = memo.entries.get(path)?;
    (*recorded == stamp).then(|| sha.clone())
}

/// Remember a digest read completely between two equal stamps.
pub(super) fn record(path: &Path, before: Stamp, after: Stamp, sha: &str) {
    if before != after || !after.settled() {
        return;
    }
    let Some(memo) = memo() else { return };
    if let Ok(mut memo) = memo.lock() {
        memo.entries
            .insert(path.to_owned(), (after, sha.to_owned()));
        memo.dirty = true;
    }
}

/// Persist the memo (best effort; a failed write only costs future reads).
pub(crate) fn save() {
    use std::os::unix::ffi::OsStrExt as _;
    let Some(memo) = memo() else { return };
    let Ok(mut memo) = memo.lock() else { return };
    if !memo.dirty {
        return;
    }
    let mut out = Vec::with_capacity(memo.entries.len() * 160);
    for (path, (stamp, sha)) in &memo.entries {
        let _ = write!(out, "{sha}");
        for number in stamp.0 {
            let _ = write!(out, "\t{number}");
        }
        out.push(b'\t');
        out.extend_from_slice(path.as_os_str().as_bytes());
        out.push(b'\n');
    }
    let temporary = memo
        .path
        .with_extension(format!("tmp-{}", std::process::id()));
    if std::fs::write(&temporary, &out).is_ok() && std::fs::rename(&temporary, &memo.path).is_ok() {
        memo.dirty = false;
    } else {
        let _ = std::fs::remove_file(&temporary);
    }
}
