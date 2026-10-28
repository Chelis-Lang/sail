//! Content digests and atomic private sidecar files.

use crate::{Error, Result};
use sail_acceptance::inputs::{InputFact, InputValue, reads_complete, selection_covered};
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

/// Whether two files hold the same bytes, read completely from both. A
/// length difference answers without reading; equal lengths are never taken
/// as equal content. Comparing directly is several times cheaper than two
/// digests of a large executable.
pub(crate) fn same_content(left: &Path, right: &Path) -> Result<bool> {
    let open = |path: &Path| {
        std::fs::File::open(path)
            .map_err(|e| Error::new("SAIL3101", format!("cannot read {}: {e}", path.display())))
    };
    let (mut left, mut right) = (open(left)?, open(right)?);
    let length = |file: &std::fs::File| {
        file.metadata()
            .map(|m| m.len())
            .map_err(|e| Error::new("SAIL3101", e.to_string()))
    };
    if length(&left)? != length(&right)? {
        return Ok(false);
    }
    let (mut a, mut b) = (vec![0u8; 256 * 1024], vec![0u8; 256 * 1024]);
    loop {
        let n = fill(&mut left, &mut a)?;
        if n != fill(&mut right, &mut b)? || a[..n] != b[..n] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

/// Reads until `buf` is full or the file ends; returns the bytes read.
fn fill(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(Error::new("SAIL3101", e.to_string())),
        }
    }
    Ok(filled)
}

/// Deduplicates alias reads within one hash phase, never across observations.
#[derive(Default)]
pub(super) struct FileDigests(BTreeMap<PathBuf, String>);

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

impl FileDigests {
    pub(super) fn hash(&mut self, path: &Path) -> Result<String> {
        self.hash_with(path, || {})
    }

    /// As [`Self::hash`]; `discovered` runs between the canonical lookup and
    /// its confirmation, where an alias can be retargeted.
    fn hash_with(&mut self, path: &Path, discovered: impl FnOnce()) -> Result<String> {
        let key = canonical(path);
        let hit = self.0.get(&key).cloned();
        discovered();
        // The requested path must still name the object the digest is of: an
        // alias retargeted after the canonical lookup would otherwise receive
        // its old destination's digest (#381). A changed binding reads the
        // requested path itself.
        if let Some(hash) = hit
            && canonical(path) == key
        {
            return Ok(hash);
        }
        let hash = file_digest(path)?;
        // A read whose alias moved during the read is still a read of the
        // requested path, but it is not cached under either destination.
        if canonical(path) == key {
            self.0.insert(key, hash.clone());
        }
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
        let descriptors: Vec<_> = pending
            .iter()
            .map(|(canonical, path)| (canonical.as_path(), *path))
            .collect();
        // A prefetched digest is kept only if its path still resolves to the
        // canonical key after the read; otherwise `hash` reads it again.
        let reads = crate::parallel::in_order(&descriptors, |&(key, path)| {
            let read = file_digest(path).and_then(|hash| {
                if canonical(path) == key {
                    Ok(hash)
                } else {
                    Err(Error::new("SAIL3101", "the path was retargeted while read"))
                }
            });
            (key, path, read)
        });
        self.retain_prefetched(&pending, reads);
    }

    fn retain_prefetched(
        &mut self,
        pending: &[(PathBuf, &Path)],
        reads: Vec<(&Path, &Path, Result<String>)>,
    ) {
        let required: Vec<_> = pending
            .iter()
            .map(|(canonical, path)| InputFact {
                namespace: 0,
                name: canonical.as_os_str().as_encoded_bytes(),
                value: InputValue::Present(path.as_os_str().as_encoded_bytes()),
            })
            .collect();
        let observed: Vec<_> = reads
            .iter()
            .filter(|(_, _, read)| read.is_ok())
            .map(|(canonical, path, _)| InputFact {
                namespace: 0,
                name: canonical.as_os_str().as_encoded_bytes(),
                value: InputValue::Present(path.as_os_str().as_encoded_bytes()),
            })
            .collect();
        let (source_order, source_inverse) = super::input::proposed_order(&required);
        let (selected_order, selected_inverse) = super::input::proposed_order(&observed);
        if !selection_covered(
            &required,
            &observed,
            &source_order,
            &source_inverse,
            &selected_order,
            &selected_inverse,
        ) {
            return;
        }
        // Successful records retain their own canonical/lexical descriptor and
        // digest. Failed or missing prefetches remain uncached for hash().
        for (canonical, _, read) in reads {
            if let Ok(hash) = read {
                self.0.insert(canonical.to_owned(), hash);
            }
        }
    }
}

/// Complete reads retain the actual requested path inside each worker result;
/// callers validate these keys rather than relying on executor result order.
pub(super) fn digest_all_tagged<'a>(paths: &[&'a Path]) -> Vec<(&'a Path, Result<String>)> {
    crate::parallel::in_order(paths, |&path| (path, file_digest(path)))
}

/// Checks the complete ledger of returned outcome records, including failures.
/// Present markers mean a returned record exists, not that its file was read
/// successfully. Callers retain and check the actual Result independently.
pub(super) fn read_outcome_keys_match<T, E>(
    paths: &[&Path],
    reads: &[(&Path, std::result::Result<T, E>)],
) -> bool {
    let required: Vec<_> = paths
        .iter()
        .map(|path| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(&[]),
        })
        .collect();
    let observed: Vec<_> = reads
        .iter()
        .map(|(path, _)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(&[]),
        })
        .collect();
    let (required_order, required_inverse) = super::input::proposed_order(&required);
    let (observed_order, observed_inverse) = super::input::proposed_order(&observed);
    reads_complete(
        &required,
        &observed,
        &required_order,
        &required_inverse,
        &observed_order,
        &observed_inverse,
    )
}

pub(crate) fn json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::new("SAIL3102", e.to_string()))?;
    json_bytes(path, &bytes)
}

/// Identity of a file without reading its bytes: device, inode, size and
/// modification time. The engine runtime's revalidation record uses it; the
/// proof context identifies installed tools by content instead
/// ([`super::tool_digests`]). Replacement, reinstallation and ordinary
/// writes change it, and any change repeats the full content check. Elsewhere
/// this falls back to the complete content digest.
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

/// Whether `path` still has the recorded identity: a `sha256:` identity of
/// an installed tool through the stamp memo, a `stat:` identity (recorded
/// before tools were identified by content) by stat, anything else as a
/// complete content digest.
pub(super) fn identity_matches(
    path: &Path,
    expected: &str,
    tools: &super::tool_digests::ToolDigests,
) -> bool {
    if expected.starts_with(super::tool_digests::PREFIX) {
        tools.matches(path, expected)
    } else if expected.starts_with("stat:") {
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

/// Whether consumers read `value` from a job's event stream: the daemon's
/// running-revision projection and proof journal read `context-ready`, and
/// reused targets name their establishing run. An unchanged-call replay emits
/// these again (ADR 0082, amendment of 2026-10-28); every other event is
/// diagnostic and belongs to the run that wrote it.
fn replayed(value: &serde_json::Value) -> bool {
    value["kind"] == "context-ready" || value["event"] == "target-reused"
}

/// The most events one run's capture holds; a run that emits more is not
/// recorded.
const CAPTURE_LIMIT: usize = 4096;

/// Events captured on this thread, or `None` with no capture active. The
/// flag records an overflow.
type Captured = Option<(Vec<serde_json::Value>, bool)>;

thread_local! {
    static CAPTURED: std::cell::RefCell<Captured> = const { std::cell::RefCell::new(None) };
}

/// The replayable job events of one managed run, captured while the run
/// lasts. A managed run emits them from the thread that runs it, and one
/// thread runs one managed run at a time; the capture ends when it is dropped.
pub(crate) struct Capture(());

impl Capture {
    /// Start capturing this thread's replayable events, discarding any
    /// earlier capture.
    pub(crate) fn start() -> Self {
        CAPTURED.with(|captured| *captured.borrow_mut() = Some((Vec::new(), false)));
        Self(())
    }

    /// The events captured so far, in order; `None` after an overflow.
    #[allow(
        clippy::unused_self,
        reason = "only a live capture can be read; dropping it ends the capture"
    )]
    pub(crate) fn events(&self) -> Option<Vec<serde_json::Value>> {
        CAPTURED.with(|captured| match &*captured.borrow() {
            Some((events, false)) => Some(events.clone()),
            _ => None,
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        CAPTURED.with(|captured| *captured.borrow_mut() = None);
    }
}

pub(crate) fn event(value: &serde_json::Value) {
    if replayed(value) {
        CAPTURED.with(|captured| {
            if let Some((events, overflowed)) = &mut *captured.borrow_mut() {
                if events.len() < CAPTURE_LIMIT {
                    events.push(value.clone());
                } else {
                    *overflowed = true;
                }
            }
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_holds_only_replayable_events_of_its_own_run() {
        event(&serde_json::json!({"kind":"context-ready","run_id":"before"}));
        let capture = Capture::start();
        event(&serde_json::json!({"kind":"context-ready","run_id":"run"}));
        event(&serde_json::json!({"kind":"managed-cleanup-decision","forced":false}));
        event(&serde_json::json!({"event":"target-reused","target":"t","establishing_run":"e"}));
        event(&serde_json::json!({"event":"dependency-stored","key":"k"}));
        // Another thread's run never contributes.
        let _ = std::thread::spawn(|| {
            event(&serde_json::json!({"kind":"context-ready","run_id":"other"}));
        })
        .join();
        assert_eq!(
            capture.events(),
            Some(vec![
                serde_json::json!({"kind":"context-ready","run_id":"run"}),
                serde_json::json!({"event":"target-reused","target":"t","establishing_run":"e"}),
            ])
        );
        drop(capture);
        event(&serde_json::json!({"kind":"context-ready","run_id":"after"}));
        assert_eq!(Capture::start().events(), Some(Vec::new()));
    }

    #[test]
    fn an_overflowing_capture_records_nothing() {
        let capture = Capture::start();
        for _ in 0..=CAPTURE_LIMIT {
            event(&serde_json::json!({"event":"target-reused"}));
        }
        assert_eq!(capture.events(), None);
    }

    #[test]
    fn prefetched_records_preserve_their_read_descriptors()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-tagged-prefetch-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        let extra = root.join("extra.rs");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        std::fs::write(&extra, "extra")?;
        let pending = [(a.clone(), a.as_path()), (b.clone(), b.as_path())];
        let descriptors = [(a.as_path(), a.as_path()), (b.as_path(), b.as_path())];
        let read = || {
            crate::parallel::in_order(&descriptors, |&(canonical, path)| {
                (canonical, path, file_digest(path))
            })
        };
        let mut reads = read();
        reads.reverse();
        let mut phase = FileDigests::default();
        phase.retain_prefetched(&pending, reads);
        assert_eq!(phase.hash(&a)?, file_digest(&a)?);
        assert_eq!(phase.hash(&b)?, file_digest(&b)?);
        for invalid in [
            vec![
                (a.as_path(), a.as_path(), file_digest(&a)),
                (a.as_path(), a.as_path(), file_digest(&a)),
            ],
            vec![(extra.as_path(), extra.as_path(), file_digest(&extra))],
            vec![(a.as_path(), b.as_path(), file_digest(&b))],
        ] {
            let mut phase = FileDigests::default();
            phase.retain_prefetched(&pending, invalid);
            assert!(phase.0.is_empty());
        }
        let mut partial = FileDigests::default();
        partial.retain_prefetched(&pending, vec![(a.as_path(), a.as_path(), file_digest(&a))]);
        assert!(!partial.0.contains_key(&b));
        assert_eq!(partial.hash(&b)?, file_digest(&b)?);
        std::fs::write(&a, "second")?;
        std::fs::write(&b, "first")?;
        let mut swapped = read();
        swapped.reverse();
        let mut fresh = FileDigests::default();
        fresh.retain_prefetched(&pending, swapped);
        assert_eq!(fresh.hash(&a)?, file_digest(&a)?);
        assert_eq!(fresh.hash(&b)?, file_digest(&b)?);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cache_hit_checks_requested_path_after_canonical_discovery()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-alias-digest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let (a, b, alias) = (root.join("a"), root.join("b"), root.join("alias"));
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        std::os::unix::fs::symlink(&a, &alias)?;
        let mut phase = FileDigests::default();
        assert_eq!(phase.hash(&alias)?, file_digest(&a)?);
        // The alias is retargeted after its canonical key (A) is found in the
        // phase map; A itself is unchanged.
        let retarget = || {
            let next = root.join("next");
            let _ = std::os::unix::fs::symlink(&b, &next);
            let _ = std::fs::rename(&next, &alias);
        };
        assert_eq!(phase.hash_with(&alias, retarget)?, file_digest(&b)?);
        assert_eq!(phase.hash(&a)?, file_digest(&a)?);
        assert_eq!(phase.hash(&alias)?, file_digest(&b)?);
        // Prefetch keeps the same binding.
        let mut prefetched = FileDigests::default();
        prefetched.prefetch([alias.as_path()]);
        assert_eq!(prefetched.0.get(&b), Some(&file_digest(&b)?));
        assert!(!prefetched.0.contains_key(&a));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn same_content_reads_every_byte() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-same-content-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let large: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
        let write = |name: &str, bytes: &[u8]| -> std::io::Result<PathBuf> {
            let path = root.join(name);
            std::fs::write(&path, bytes)?;
            Ok(path)
        };
        let a = write("a", &large)?;
        let b = write("b", &large)?;
        assert!(same_content(&a, &b)?);
        // Same length, one byte different past the first chunk.
        let mut late = large.clone();
        late[600_000] ^= 1;
        let c = write("c", &late)?;
        assert!(!same_content(&a, &c)?);
        // A prefix differs in length; empty files are equal.
        let d = write("d", &large[..large.len() - 1])?;
        assert!(!same_content(&a, &d)?);
        assert!(same_content(&write("e", b"")?, &write("f", b"")?)?);
        assert_eq!(
            same_content(&a, &root.join("missing"))
                .err()
                .map(|e| e.code),
            Some("SAIL3101")
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
