//! Tiered result cache. Status: the local disk tier (ADR 0011).
//!
//! An entry maps a v2 query key (ADR 0008) to the exact reply bytes the
//! pinned solver produced for that command. Each entry is one file,
//! `<root>/v2/<k0k1>/<key>`, holding a JSON header line followed by the reply
//! bytes. Writes are atomic: a temporary file, then a rename. A read checks
//! that the stored key matches and that the reply hash matches. Any problem
//! is a miss: the entry is removed and the caller falls back to the solver.
#![forbid(unsafe_code)]

use std::io::Write as _;
use std::path::{Path, PathBuf};

use sail_key::{QueryKey, sha256};
use serde::{Deserialize, Serialize};

/// Format version of entry files.
pub const ENTRY_VERSION: u32 = 1;

/// Metadata stored with each reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// Entry format version.
    pub entry_version: u32,
    /// The key, as hex.
    pub key: String,
    /// SHA-256 of the reply bytes, as hex.
    pub reply_sha256: String,
    /// The command the reply answers, for example `check-sat`.
    pub command: String,
    /// Solver identity summary: `name version sha256 platform`.
    pub solver: String,
    /// Unix time the entry was stored.
    pub stored_unix: u64,
    /// How the result was established (charter rule 7).
    pub provenance: String,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The local disk tier.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// A cache rooted at `root` (created on first write).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The cache root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &QueryKey) -> PathBuf {
        let k = key.to_string();
        self.root
            .join("v2")
            .join(k.get(..2).unwrap_or("xx"))
            .join(k)
    }

    /// The reply stored for `key`, if there is a valid entry.
    #[must_use]
    pub fn get(&self, key: &QueryKey) -> Option<(Meta, Vec<u8>)> {
        let path = self.path(key);
        let data = std::fs::read(&path).ok()?;
        let parsed = (|| {
            let nl = data.iter().position(|&b| b == b'\n')?;
            let meta: Meta = serde_json::from_slice(data.get(..nl)?).ok()?;
            let reply = data.get(nl + 1..)?.to_vec();
            (meta.entry_version == ENTRY_VERSION
                && meta.key == key.to_string()
                && meta.reply_sha256 == hex(&sha256(&reply)))
            .then_some((meta, reply))
        })();
        if parsed.is_none() {
            // Corrupt or foreign entry: drop it; the caller re-solves.
            let _ = std::fs::remove_file(&path);
        }
        parsed
    }

    /// Stores `reply` for `key`. Existing entries are left alone: content
    /// addressing means they hold the same answer.
    ///
    /// # Errors
    ///
    /// I/O errors creating or writing the entry.
    pub fn put(
        &self,
        key: &QueryKey,
        reply: &[u8],
        command: &str,
        solver: &str,
        provenance: &str,
    ) -> std::io::Result<()> {
        let path = self.path(key);
        if path.is_file() {
            return Ok(());
        }
        let dir = path.parent().unwrap_or(&self.root).to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let meta = Meta {
            entry_version: ENTRY_VERSION,
            key: key.to_string(),
            reply_sha256: hex(&sha256(reply)),
            command: command.to_owned(),
            solver: solver.to_owned(),
            stored_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            provenance: provenance.to_owned(),
        };
        let tmp = dir.join(format!(".tmp-{}-{}", std::process::id(), key));
        let mut f = std::fs::File::create(&tmp)?;
        serde_json::to_writer(&mut f, &meta).map_err(std::io::Error::other)?;
        f.write_all(b"\n")?;
        f.write_all(reply)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sail-cache-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trips_exact_bytes() {
        let c = DiskCache::new(scratch("rt"));
        let k = QueryKey([1; 32]);
        assert!(c.get(&k).is_none());
        let reply = b"(model\n  (define-fun x () Int 1)\n)\n\x00\xff";
        assert!(c.put(&k, reply, "get-model", "z3", "pinned-solver").is_ok());
        let (meta, got) = c.get(&k).unwrap_or_default_for_test();
        assert_eq!(got, reply);
        assert_eq!(meta.command, "get-model");
        let _ = std::fs::remove_dir_all(c.root());
    }

    #[test]
    fn corrupt_entries_are_misses_and_removed() {
        let c = DiskCache::new(scratch("corrupt"));
        let k = QueryKey([2; 32]);
        assert!(
            c.put(&k, b"unsat\n", "check-sat", "z3", "pinned-solver")
                .is_ok()
        );
        let path = c.path(&k);
        let mut data = std::fs::read(&path).unwrap_or_default();
        if let Some(last) = data.last_mut() {
            *last ^= 1;
        }
        assert!(std::fs::write(&path, &data).is_ok());
        assert!(c.get(&k).is_none());
        assert!(!path.exists());
        // An entry filed under the wrong key is also a miss.
        let other = QueryKey([3; 32]);
        assert!(
            c.put(&k, b"sat\n", "check-sat", "z3", "pinned-solver")
                .is_ok()
        );
        assert!(std::fs::create_dir_all(c.path(&other).parent().unwrap_or(c.root())).is_ok());
        assert!(std::fs::copy(c.path(&k), c.path(&other)).is_ok());
        assert!(c.get(&other).is_none());
        let _ = std::fs::remove_dir_all(c.root());
    }

    trait OrDefault {
        fn unwrap_or_default_for_test(self) -> (Meta, Vec<u8>);
    }

    impl OrDefault for Option<(Meta, Vec<u8>)> {
        fn unwrap_or_default_for_test(self) -> (Meta, Vec<u8>) {
            match self {
                Some(x) => x,
                None => unreachable!("expected a cache hit"),
            }
        }
    }
}
