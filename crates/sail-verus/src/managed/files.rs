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

/// Avoids replacing an unchanged private input-declaration sidecar on Linux.
/// Byte equality only avoids a write; independent native freshness and every
/// proof/input barrier remain responsible for authority. Uncertain identities,
/// aliases and all other platforms retain the original atomic publication.
pub(crate) fn json_metadata<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let bytes = serde_json::to_vec(value).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
        if metadata_bytes_equal(path, &bytes) {
            return Ok(());
        }
        json_bytes(path, &bytes)
    }
    #[cfg(not(target_os = "linux"))]
    json(path, value)
}

#[cfg(target_os = "linux")]
fn metadata_bytes_equal(path: &Path, bytes: &[u8]) -> bool {
    metadata_bytes_equal_after_read(path, bytes, || {})
}

#[cfg(target_os = "linux")]
fn metadata_bytes_equal_after_read(path: &Path, bytes: &[u8], after_read: impl FnOnce()) -> bool {
    use rustix::fs::{Mode, OFlags, open};
    use std::os::unix::fs::MetadataExt as _;

    let identity = |metadata: &std::fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.uid(),
            metadata.gid(),
            metadata.nlink(),
            metadata.mode(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    let matches = || -> std::io::Result<bool> {
        let before = std::fs::symlink_metadata(path)?;
        if !before.is_file()
            || before.nlink() != 1
            || before.mode() & 0o7777 != 0o600
            || u64::try_from(bytes.len()).ok() != Some(before.len())
        {
            return Ok(false);
        }
        let descriptor = open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?;
        let mut file = std::fs::File::from(descriptor);
        if identity(&before) != identity(&file.metadata()?) {
            return Ok(false);
        }
        let mut buffer = [0u8; 16384];
        let mut offset = 0usize;
        while offset < bytes.len() {
            let count = file.read(&mut buffer[..(bytes.len() - offset).min(16384)])?;
            if count == 0 || bytes[offset..offset + count] != buffer[..count] {
                return Ok(false);
            }
            offset += count;
        }
        // An independently checked EOF refuses growth; before/after descriptor
        // and no-follow name identities refuse replacement during comparison.
        if file.read(&mut buffer[..1])? != 0 {
            return Ok(false);
        }
        after_read();
        Ok(identity(&before) == identity(&file.metadata()?)
            && identity(&before) == identity(&std::fs::symlink_metadata(path)?))
    };
    matches().unwrap_or(false)
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

#[cfg(all(test, target_os = "linux"))]
mod metadata_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> std::io::Result<Self> {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "sail-metadata-write-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root)?;
            Ok(Self(root))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unchanged_metadata_keeps_inode_and_exact_bytes_across_chunk_boundaries() -> TestResult {
        let root = Fixture::new()?;
        let path = root.0.join("state.json");
        let value = serde_json::json!({"exact": "x".repeat(32769)});
        json_metadata(&path, &value)?;
        let before = std::fs::metadata(&path)?;
        let bytes = std::fs::read(&path)?;
        json_metadata(&path, &value)?;
        let after = std::fs::metadata(&path)?;
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
        assert_eq!(after.modified()?, before.modified()?);
        assert_eq!(std::fs::read(&path)?, bytes);
        assert!(metadata_bytes_equal(&path, &serde_json::to_vec(&value)?));
        Ok(())
    }

    #[test]
    fn changed_or_equivalent_but_nonidentical_metadata_uses_original_atomic_write() -> TestResult {
        let root = Fixture::new()?;
        let path = root.0.join("state.json");
        let value = serde_json::json!({"x": 1});
        json(&path, &value)?;
        for old in [b"{\"x\": 1}".as_slice(), b"{\"x\":2}", b"{\"x\":1}\n"] {
            std::fs::write(&path, old)?;
            let before = std::fs::metadata(&path)?;
            assert!(!metadata_bytes_equal(&path, &serde_json::to_vec(&value)?));
            json_metadata(&path, &value)?;
            assert_ne!(std::fs::metadata(&path)?.ino(), before.ino());
            assert_eq!(std::fs::read(&path)?, serde_json::to_vec(&value)?);
        }
        Ok(())
    }

    #[test]
    fn symlink_hardlink_and_nonprivate_mode_never_take_noop_path() -> TestResult {
        let root = Fixture::new()?;
        let path = root.0.join("state.json");
        let other = root.0.join("external.json");
        let value = serde_json::json!({"exact": true});
        let bytes = serde_json::to_vec(&value)?;
        json(&other, &value)?;
        symlink(&other, &path)?;
        assert!(!metadata_bytes_equal(&path, &bytes));
        json_metadata(&path, &value)?;
        assert!(!std::fs::symlink_metadata(&path)?.is_symlink());
        assert_eq!(std::fs::read(&other)?, bytes);
        std::fs::remove_file(&path)?;
        std::fs::hard_link(&other, &path)?;
        assert!(!metadata_bytes_equal(&path, &bytes));
        json_metadata(&path, &value)?;
        assert_ne!(
            std::fs::metadata(&path)?.ino(),
            std::fs::metadata(&other)?.ino()
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
        assert!(!metadata_bytes_equal(&path, &bytes));
        json_metadata(&path, &value)?;
        assert_eq!(std::fs::metadata(&path)?.mode() & 0o777, 0o600);
        Ok(())
    }

    #[test]
    fn same_bytes_name_replacement_during_comparison_refuses_noop() -> TestResult {
        let root = Fixture::new()?;
        let path = root.0.join("state.json");
        let value = serde_json::json!({"exact": true});
        json(&path, &value)?;
        let before = std::fs::metadata(&path)?;
        let mut replacement = Ok(());
        assert!(!metadata_bytes_equal_after_read(
            &path,
            &serde_json::to_vec(&value)?,
            || {
                replacement = json(&path, &value);
            }
        ));
        replacement?;
        assert_ne!(std::fs::metadata(&path)?.ino(), before.ino());
        assert_eq!(std::fs::read(&path)?, serde_json::to_vec(&value)?);
        Ok(())
    }

    #[test]
    fn absent_or_unavailable_metadata_preserves_original_write_and_error() -> TestResult {
        let root = Fixture::new()?;
        let path = root.0.join("state.json");
        let value = serde_json::json!([]);
        assert!(!metadata_bytes_equal(&path, &serde_json::to_vec(&value)?));
        json_metadata(&path, &value)?;
        assert_eq!(std::fs::read(&path)?, serde_json::to_vec(&value)?);
        let missing = root.0.join("absent/state.json");
        let original = json(&missing, &value).err();
        let candidate = json_metadata(&missing, &value).err();
        assert!(
            original
                .as_ref()
                .is_some_and(|error| error.code == "SAIL3102")
        );
        assert_eq!(candidate, original);
        Ok(())
    }
}
