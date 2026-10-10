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
    let mut input = std::fs::File::open(path)
        .map_err(|e| Error::new("SAIL3101", format!("cannot read {}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = input
            .read(&mut buf)
            .map_err(|e| Error::new("SAIL3101", e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Deduplicates alias reads within one hash phase, never across observations.
#[derive(Default)]
pub(super) struct FileDigests(BTreeMap<PathBuf, String>);

impl FileDigests {
    pub(super) fn contains(&self, path: &Path) -> bool {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        self.0.contains_key(&canonical)
    }

    pub(super) fn hash(&mut self, path: &Path) -> Result<String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(hash) = self.0.get(&canonical) {
            return Ok(hash.clone());
        }
        let hash = file_digest(path)?;
        self.0.insert(canonical, hash.clone());
        Ok(hash)
    }
}

pub(crate) fn json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
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
    file.write_all(&bytes)
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
