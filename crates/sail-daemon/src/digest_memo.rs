//! Memoize only a digest whose complete input bytes have just matched.

use std::sync::{Arc, Mutex};

use sail_key::sha256;

/// One retained executable body. Concurrent readers can temporarily retain
/// older immutable snapshots; this is not a process-wide memory bound.
pub(crate) const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

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
