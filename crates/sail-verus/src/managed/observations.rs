//! Run-local decoded records. Fresh bytes remain the authority on every read.
//!
//! Sharing a decoded value never shares a filesystem freshness decision. Each
//! consumer must still validate its own source/target observation and barriers.

use serde::de::DeserializeOwned;
use std::io::Read as _;
use std::path::Path;
use std::sync::{Arc, Mutex};

struct Decoded<T> {
    bytes: Vec<u8>,
    value: Arc<T>,
}

/// One bounded record per run. Admission, namespace and full-stamp checks stay
/// with the consumer; only decoding and the immutable checksum are shared.
pub(super) struct JsonRecord<T>(Mutex<Option<Decoded<T>>>);

impl<T> Default for JsonRecord<T> {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl<T: DeserializeOwned> JsonRecord<T> {
    pub(super) fn read(
        &self,
        path: &Path,
        limit: u64,
        intact: impl FnOnce(&T) -> bool,
    ) -> Option<Arc<T>> {
        let mut bytes = Vec::new();
        super::dependency_snapshot::open(path)?
            .take(limit.checked_add(1)?)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > limit {
            return None;
        }
        let mut previous = self.0.lock().ok()?;
        if let Some(record) = previous.as_ref()
            && record.bytes == bytes
        {
            return Some(Arc::clone(&record.value));
        }
        let decoded: T = serde_json::from_slice(&bytes).ok()?;
        if !intact(&decoded) {
            return None;
        }
        let value = Arc::new(decoded);
        *previous = Some(Decoded {
            bytes,
            value: Arc::clone(&value),
        });
        Some(value)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn sharing_decoding_never_hides_replacement_corruption_or_removal()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "sail-decoded-record-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let record = JsonRecord::<serde_json::Value>::default();
        let checks = AtomicUsize::new(0);
        let read = || {
            record.read(&path, 64, |value| {
                checks.fetch_add(1, Ordering::Relaxed);
                value.get("valid").and_then(serde_json::Value::as_bool) == Some(true)
            })
        };
        std::fs::write(&path, br#"{"valid":true,"value":1}"#)?;
        let first = read().ok_or("first read")?;
        assert!(Arc::ptr_eq(&first, &read().ok_or("second read")?));
        assert_eq!(checks.load(Ordering::Relaxed), 1);
        std::fs::write(&path, br#"{"valid":true,"value":2}"#)?;
        let changed = read().ok_or("replacement read")?;
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(changed["value"], 2);
        std::fs::write(&path, br#"{"valid":false}"#)?;
        assert!(read().is_none());
        std::fs::write(&path, b"not json")?;
        assert!(read().is_none());
        std::fs::write(&path, [b' '; 65])?;
        assert!(read().is_none());
        std::fs::remove_file(&path)?;
        assert!(read().is_none());
        std::os::unix::fs::symlink("absent", &path)?;
        assert!(read().is_none());
        std::fs::remove_file(path)?;
        Ok(())
    }
}
