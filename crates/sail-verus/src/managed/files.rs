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

struct ObservedDigest {
    hash: String,
    seal: sail_acceptance::inputs::ReadSeal,
}

pub(crate) fn file_digest(path: &Path) -> Result<String> {
    file_digest_observed(path).map(|observed| observed.hash)
}

fn file_digest_observed(path: &Path) -> Result<ObservedDigest> {
    let original =
        object_stamp(&std::fs::metadata(path).map_err(|e| {
            Error::new("SAIL3101", format!("cannot read {}: {e}", path.display()))
        })?)?;
    let input = std::fs::File::open(path)
        .map_err(|e| Error::new("SAIL3101", format!("cannot read {}: {e}", path.display())))?;
    digest_opened(path, input, original)
}

/// Read, hash and validate through one owned handle. A pathname observation
/// cannot label bytes read from a different object, even with equal content.
fn digest_opened(
    path: &Path,
    mut input: std::fs::File,
    original: sail_acceptance::inputs::ObjectStamp,
) -> Result<ObservedDigest> {
    use sail_acceptance::inputs::OpenedRead;
    // One read buffer per thread; the checked chunk borrows exactly its actual
    // successful prefix, and the hasher receives only that returned slice.
    thread_local! {
        static BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 65536]);
    }
    let changed = || {
        Error::new(
            "SAIL3101",
            format!(
                "opened input changed or could not be observed completely: {}",
                path.display()
            ),
        )
    };
    let opened = object_stamp(
        &input
            .metadata()
            .map_err(|e| Error::new("SAIL3101", e.to_string()))?,
    )?;
    let read = OpenedRead::begin(original, &opened).ok_or_else(changed)?;
    let mut hasher = Sha256::new();
    let read = BUFFER.with_borrow_mut(|buffer| -> Result<OpenedRead> {
        let mut read = read;
        loop {
            let count = match input.read(buffer) {
                Ok(count) => count,
                Err(error) => {
                    let _ = read.fail();
                    return Err(Error::new("SAIL3101", error.to_string()));
                }
            };
            if count == 0 {
                return Ok(read.eof());
            }
            let (next, chunk) = read.chunk(&buffer[..count]);
            read = next;
            let chunk = chunk.ok_or_else(changed)?;
            hasher.update(chunk.bytes());
        }
    })?;
    let handle_after = object_stamp(
        &input
            .metadata()
            .map_err(|e| Error::new("SAIL3101", e.to_string()))?,
    )?;
    let path_after =
        object_stamp(&std::fs::metadata(path).map_err(|e| {
            Error::new("SAIL3101", format!("cannot read {}: {e}", path.display()))
        })?)?;
    let seal = read
        .finish(&handle_after, &path_after)
        .ok_or_else(changed)?;
    Ok(ObservedDigest {
        hash: format!("{:x}", hasher.finalize()),
        seal,
    })
}

#[cfg(unix)]
fn object_stamp(metadata: &std::fs::Metadata) -> Result<sail_acceptance::inputs::ObjectStamp> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(sail_acceptance::inputs::ObjectStamp {
        device: metadata.dev(),
        object: metadata.ino(),
        length: usize::try_from(metadata.size())
            .map_err(|e| Error::new("SAIL3101", e.to_string()))?,
        mode: metadata.mode(),
        modified_seconds: metadata.mtime(),
        modified_nanos: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanos: metadata.ctime_nsec(),
        kind: u8::from(metadata.is_file()),
    })
}

#[cfg(not(unix))]
fn object_stamp(_metadata: &std::fs::Metadata) -> Result<sail_acceptance::inputs::ObjectStamp> {
    Err(Error::new(
        "SAIL3101",
        "opened object identity is unavailable on this platform",
    ))
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
pub(super) struct FileDigests(BTreeMap<PathBuf, ObservedDigest>);

impl FileDigests {
    pub(super) fn hash(&mut self, path: &Path) -> Result<String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(hash) = self.cached_hash(path, &canonical) {
            return Ok(hash);
        }
        self.0.remove(&canonical);
        let observed = file_digest_observed(path)?;
        if !matches_cache_binding(&observed, &canonical, path) {
            return Err(Error::new(
                "SAIL3101",
                "canonical input changed before cache admission",
            ));
        }
        let hash = observed.hash.clone();
        self.0.insert(canonical, observed);
        Ok(hash)
    }

    /// A separately testable actual hit stage: canonical discovery is only a
    /// proposal, and cannot replace the requested pathname's own observation.
    fn cached_hash(&self, requested: &Path, canonical: &Path) -> Option<String> {
        let observed = self.0.get(canonical)?;
        matches_cache_binding(observed, canonical, requested).then(|| observed.hash.clone())
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
        let reads = crate::parallel::in_order(&descriptors, |&(canonical, path)| {
            (canonical, path, file_digest_observed(path))
        });
        self.retain_prefetched(&pending, reads);
    }

    fn retain_prefetched(
        &mut self,
        pending: &[(PathBuf, &Path)],
        reads: Vec<(&Path, &Path, Result<ObservedDigest>)>,
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
        for (canonical, requested, read) in reads {
            if let Ok(observed) = read
                && matches_cache_binding(&observed, canonical, requested)
            {
                self.0.insert(canonical.to_owned(), observed);
            }
        }
    }
}

fn matches_cache_binding(observed: &ObservedDigest, canonical: &Path, requested: &Path) -> bool {
    let (Ok(canonical), Ok(requested)) = (canonical_stamp(canonical), canonical_stamp(requested))
    else {
        return false;
    };
    observed.seal.matches_binding(&canonical, &requested)
}

fn canonical_stamp(path: &Path) -> Result<sail_acceptance::inputs::ObjectStamp> {
    object_stamp(
        &std::fs::metadata(path).map_err(|error| Error::new("SAIL3101", error.to_string()))?,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opened_digest_accounts_exact_chunks_and_rejects_partial_or_unreadable_handles()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-opened-digest-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let path = root.join("source.rs");
        let bytes: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes)?;
        assert_eq!(file_digest(&path)?, digest(&bytes));
        let original = object_stamp(&std::fs::metadata(&path)?)?;
        let mut partial = std::fs::File::open(&path)?;
        partial.read_exact(&mut [0u8; 1])?;
        assert_eq!(
            digest_opened(&path, partial, original)
                .err()
                .map(|e| e.code),
            Some("SAIL3101")
        );
        let original = object_stamp(&std::fs::metadata(&path)?)?;
        let unreadable = std::fs::OpenOptions::new().write(true).open(&path)?;
        assert_eq!(
            digest_opened(&path, unreadable, original)
                .err()
                .map(|e| e.code),
            Some("SAIL3101")
        );
        let empty = root.join("empty.rs");
        std::fs::write(&empty, [])?;
        assert_eq!(file_digest(&empty)?, digest(&[]));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn opened_digest_does_not_relabel_a_retained_handle_after_path_retarget()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-opened-retarget-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let first = root.join("first.rs");
        let second = root.join("second.rs");
        let alias = root.join("alias.rs");
        std::fs::write(&first, "same bytes")?;
        std::fs::write(&second, "same bytes")?;
        std::os::unix::fs::symlink(&first, &alias)?;
        let original = object_stamp(&std::fs::metadata(&alias)?)?;
        let input = std::fs::File::open(&alias)?;
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(&second, &alias)?;
        assert_eq!(
            digest_opened(&alias, input, original).err().map(|e| e.code),
            Some("SAIL3101")
        );
        // A genuinely fresh open of the newly selected object remains usable.
        assert_eq!(file_digest(&alias)?, digest(b"same bytes"));
        std::fs::remove_dir_all(root)?;
        Ok(())
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
                (canonical, path, file_digest_observed(path))
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
                (a.as_path(), a.as_path(), file_digest_observed(&a)),
                (a.as_path(), a.as_path(), file_digest_observed(&a)),
            ],
            vec![(
                extra.as_path(),
                extra.as_path(),
                file_digest_observed(&extra),
            )],
            vec![(a.as_path(), b.as_path(), file_digest_observed(&b))],
        ] {
            let mut phase = FileDigests::default();
            phase.retain_prefetched(&pending, invalid);
            assert!(phase.0.is_empty());
        }
        let mut partial = FileDigests::default();
        partial.retain_prefetched(
            &pending,
            vec![(a.as_path(), a.as_path(), file_digest_observed(&a))],
        );
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
    fn phase_cache_validates_canonical_objects_before_admission_and_reuse()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-opened-cache-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        let alias = root.join("alias.rs");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "other")?;
        std::os::unix::fs::symlink(&a, &alias)?;
        let canonical = alias.canonicalize()?;
        let pending = [(canonical.clone(), alias.as_path())];
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(&b, &alias)?;
        let observed = file_digest_observed(&alias)?;
        let mut phase = FileDigests::default();
        phase.retain_prefetched(
            &pending,
            vec![(canonical.as_path(), alias.as_path(), Ok(observed))],
        );
        assert!(
            !phase.0.contains_key(&canonical),
            "a successful read of B cannot fill A's old canonical slot"
        );
        assert_eq!(phase.hash(&a)?, digest(b"first"));
        let replacement = root.join("replacement.rs");
        std::fs::write(&replacement, "later")?;
        std::fs::rename(&replacement, &a)?;
        assert_eq!(
            phase.hash(&a)?,
            digest(b"later"),
            "cached old object must be reread after replacement"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cache_hit_checks_requested_path_after_canonical_discovery()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-cache-hit-retarget-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        let alias = root.join("alias.rs");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "other")?;
        std::os::unix::fs::symlink(&a, &alias)?;
        let mut phase = FileDigests::default();
        assert_eq!(phase.hash(&alias)?, digest(b"first"));
        let earlier_canonical = alias.canonicalize()?;
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(&b, &alias)?;
        // Execute the actual hit stage after a deterministic filesystem change
        // at the canonical-lookup/cache-validation seam. A itself is unchanged.
        assert_eq!(phase.cached_hash(&alias, &earlier_canonical), None);
        assert_eq!(phase.hash(&alias)?, digest(b"other"));
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
