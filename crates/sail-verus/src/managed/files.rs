//! Content digests and atomic private sidecar files.

use crate::{Error, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn file_digest(path: &Path) -> Result<String> {
    // One read buffer per thread: a snapshot reads thousands of small files,
    // and zeroing a new 64 KiB buffer for each cost more than reading it.
    thread_local! {
        static BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 65536]);
    }
    let mut input = std::fs::File::open(path)
        .map_err(|e| Error::new("SAIL3101", format!("cannot read {}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    BUFFER.with_borrow_mut(|buf| -> Result<()> {
        loop {
            let n = input
                .read(buf)
                .map_err(|e| Error::new("SAIL3101", e.to_string()))?;
            if n == 0 {
                return Ok(());
            }
            hasher.update(&buf[..n]);
        }
    })?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Deduplicates alias reads within one hash phase, never across observations.
#[derive(Default)]
pub(super) struct FileDigests(BTreeMap<PathBuf, String>);

impl FileDigests {
    pub(super) fn hash(&mut self, path: &Path) -> Result<String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(hash) = self.0.get(&canonical) {
            return Ok(hash.clone());
        }
        let hash = file_digest(path)?;
        self.0.insert(canonical, hash.clone());
        Ok(hash)
    }

    /// Reads files concurrently into this phase-local map, exactly as `hash`
    /// would read them one by one. Only overlap changes: every digest is still
    /// the complete content read in this phase. A failed read is not recorded,
    /// so the later `hash` call reads the file again and reports its original
    /// error in the original order.
    pub(super) fn prefetch<'a>(&mut self, paths: impl IntoIterator<Item = &'a Path>) {
        let mut pending: Vec<(PathBuf, &Path)> = Vec::new();
        for path in paths {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
            if !self.0.contains_key(&canonical) && !pending.iter().any(|(c, _)| *c == canonical) {
                pending.push((canonical, path));
            }
        }
        for ((canonical, _), digest) in pending.iter().zip(digest_all(
            &pending.iter().map(|(_, path)| *path).collect::<Vec<_>>(),
        )) {
            if let Ok(digest) = digest {
                self.0.insert(canonical.clone(), digest);
            }
        }
    }
}

/// Complete content digests of `paths`, in order, read by at most eight
/// concurrent workers. Each result is exactly `file_digest` of that path.
pub(super) fn digest_all(paths: &[&Path]) -> Vec<Result<String>> {
    crate::parallel::in_order(paths, |path| file_digest(path))
}

pub(crate) fn json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
    json_bytes(path, &bytes)
}

/// Identity of an installed toolchain file without reading its bytes: device,
/// inode, size and modification time. Replacement, reinstallation and ordinary
/// writes change it, and any change makes the next run fresh; an in-place
/// rewrite that restores the modification time is not detected. The change time
/// is left out: Sail's own hard-linked aliases of `sail` change it on every new
/// compiler family. Elsewhere this falls back to the complete content digest.
pub(crate) fn stat_identity(path: &Path) -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let m = std::fs::metadata(path)
            .map_err(|e| Error::new("SAIL3101", format!("cannot read {}: {e}", path.display())))?;
        Ok(format!(
            "stat:{}:{}:{}:{}.{}",
            m.dev(),
            m.ino(),
            m.size(),
            m.mtime(),
            m.mtime_nsec()
        ))
    }
    #[cfg(not(unix))]
    {
        file_digest(path)
    }
}

/// Whether `path` still has the recorded identity: a `stat:` identity is
/// compared by stat, anything else as a complete content digest.
pub(super) fn identity_matches(path: &Path, expected: &str) -> bool {
    if expected.starts_with("stat:") {
        stat_identity(path).is_ok_and(|actual| actual == expected)
    } else {
        file_digest(path).is_ok_and(|actual| actual == expected)
    }
}

fn json_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|e| Error::new("SAIL3102", e.to_string()))?;
    file.write_all(bytes)
        .and_then(|()| std::fs::rename(&temp, path))
        .map_err(|e| Error::new("SAIL3102", e.to_string()))
}

pub(crate) fn event(value: &serde_json::Value) {
    if let Some(path) = std::env::var_os("SAIL_JOB_EVENTS")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        && let Ok(bytes) = serde_json::to_vec(value)
    {
        let _ = file.write_all(&bytes).and_then(|()| file.write_all(b"\n"));
    }
}
