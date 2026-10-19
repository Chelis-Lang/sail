//! Memoize only a digest whose complete input bytes have just matched.

use std::io::Read as _;
use std::path::Path;
use std::sync::{Arc, Mutex};

use sail_key::sha256;

/// One retained executable body. Concurrent readers can temporarily retain
/// older immutable snapshots; this is not a process-wide memory bound.
pub(crate) const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

/// Bytes compared per read when checking a file against the retained body.
/// A small reused buffer avoids allocating and faulting in an
/// executable-sized copy on every handshake.
const COMPARE_CHUNK: usize = 256 * 1024;

#[derive(Clone)]
struct Entry {
    bytes: Arc<[u8]>,
    digest: [u8; 32],
}

/// A bounded single-entry digest memo, with no path, stamp or solver identity.
pub(crate) struct DigestMemo {
    latest: Mutex<Option<Entry>>,
    maximum: usize,
}

impl DigestMemo {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            latest: Mutex::new(None),
            maximum,
        }
    }

    /// The retained digest, only if the file at `path` holds exactly the
    /// retained bytes when read now, in full: equal bytes have the same
    /// SHA-256 by construction. A read error, a length difference or any
    /// differing byte returns `None`, and the caller reads and hashes the
    /// file with [`Self::digest`].
    pub(crate) fn current(&self, path: &Path) -> Option<[u8; 32]> {
        let entry = self.latest.lock().ok()?.clone()?;
        let mut file = std::fs::File::open(path).ok()?;
        let mut buf = vec![0u8; COMPARE_CHUNK];
        let mut at = 0usize;
        loop {
            let n = match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            };
            let end = at.checked_add(n)?;
            if entry.bytes.get(at..end)? != buf.get(..n)? {
                return None;
            }
            at = end;
        }
        (at == entry.bytes.len()).then_some(entry.digest)
    }

    /// The caller supplies the whole freshly read body on every invocation.
    /// A matching immutable body has the same SHA-256 by construction. Any
    /// failure to use the optional memo just computes the complete digest.
    pub(crate) fn digest(&self, bytes: Vec<u8>) -> [u8; 32] {
        let previous = match self.latest.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return sha256(&bytes),
        };
        if let Some(previous) = previous
            && previous.bytes.as_ref() == bytes.as_slice()
        {
            return previous.digest;
        }

        let digest = sha256(&bytes);
        if bytes.len() <= self.maximum
            && let Ok(mut guard) = self.latest.lock()
        {
            // Another thread can have installed these exact bytes while this
            // thread hashed. Avoid another retained allocation in that case.
            if guard
                .as_ref()
                .is_none_or(|entry| entry.bytes.as_ref() != bytes.as_slice())
            {
                *guard = Some(Entry {
                    bytes: bytes.into(),
                    digest,
                });
            }
        }
        digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_equal_length_and_returned_bodies_keep_exact_digests() {
        let memo = DigestMemo::new(3);
        for bytes in [b"abc".as_slice(), b"abc", b"abd", b"abc", b""] {
            assert_eq!(memo.digest(bytes.to_vec()), sha256(bytes));
        }
    }

    #[test]
    fn current_reuses_a_digest_only_for_the_complete_retained_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("sail-digest-memo-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("solver");
        // More than one comparison chunk, so a change in a later chunk counts.
        let body: Vec<u8> = (0..=u8::MAX).cycle().take(COMPARE_CHUNK * 2 + 17).collect();
        let memo = DigestMemo::new(body.len());
        std::fs::write(&path, &body)?;
        assert_eq!(memo.current(&path), None, "nothing is retained yet");
        assert_eq!(memo.digest(body.clone()), sha256(&body));
        assert_eq!(memo.current(&path), Some(sha256(&body)));

        let mut late = body.clone();
        if let Some(last) = late.last_mut() {
            *last ^= 1;
        }
        let shorter = body.get(..body.len() - 1).unwrap_or_default().to_vec();
        let longer = [body.as_slice(), b"x"].concat();
        for changed in [&late, &shorter, &longer] {
            std::fs::write(&path, changed)?;
            assert_eq!(memo.current(&path), None);
        }
        std::fs::remove_file(&path)?;
        assert_eq!(memo.current(&path), None);
        std::fs::write(&path, &body)?;
        assert_eq!(memo.current(&path), Some(sha256(&body)));
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    #[test]
    fn oversized_body_is_hashed_without_replacing_the_bounded_entry() {
        let memo = DigestMemo::new(3);
        assert_eq!(memo.digest(b"abc".to_vec()), sha256(b"abc"));
        assert_eq!(memo.digest(b"abcd".to_vec()), sha256(b"abcd"));
        let guard = memo.latest.lock().ok();
        assert_eq!(
            guard
                .as_deref()
                .and_then(Option::as_ref)
                .map(|e| e.bytes.as_ref()),
            Some(b"abc".as_slice())
        );
    }

    #[test]
    fn concurrent_snapshots_cannot_pair_one_body_with_another_digest() {
        let memo = Arc::new(DigestMemo::new(3));
        let mut threads = Vec::new();
        for bytes in [b"abc", b"abd", b"abe", b"abf"] {
            let memo = Arc::clone(&memo);
            threads.push(std::thread::spawn(move || {
                for _ in 0..32 {
                    assert_eq!(memo.digest(bytes.to_vec()), sha256(bytes));
                }
            }));
        }
        for thread in threads {
            assert!(thread.join().is_ok());
        }
    }

    #[test]
    fn poisoned_memo_hashes_fresh_bytes_instead_of_using_retained_state() {
        let memo = DigestMemo::new(3);
        assert_eq!(memo.digest(b"abc".to_vec()), sha256(b"abc"));
        // Fault injection poisons only the optional memo's synchronization.
        let interrupted = std::panic::catch_unwind(|| {
            let _held = memo.latest.lock().ok();
            std::panic::resume_unwind(Box::new("digest memo fault injection"));
        });
        assert!(interrupted.is_err());
        assert!(memo.latest.is_poisoned());
        assert_eq!(memo.digest(b"abc".to_vec()), sha256(b"abc"));
        assert_eq!(memo.digest(b"abd".to_vec()), sha256(b"abd"));
    }
}
