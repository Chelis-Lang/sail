//! Reply packs: the signed replies a release ships for its pinned vstd
//! (ADR 0062).
//!
//! A pack is one file holding ordinary signed shared entries (ADR 0017),
//! each exactly as [`sign_entry`] builds it. A
//! pack adds no trust of its own. A reader looks an entry up by its key and
//! then applies every check of
//! [`verify_entry`](crate::shared::verify_entry) against the keys it trusts
//! for that pack. One file rather than a directory keeps a release archive
//! small, and lets the release index bind the whole store with one digest.
//!
//! **Format.** The line [`MAGIC`], then one record per entry, in ascending
//! key order: the 32 key bytes, the entry length as a little-endian `u32`,
//! and the entry bytes. A key appears once. Nothing may follow the last
//! record.

use std::collections::{BTreeMap, HashMap};

use sail_key::QueryKey;

use crate::DiskCache;
use crate::Effort;
use crate::shared::{
    MAX_ENTRY_BYTES, PublishOptions, PublishReport, SecretKey, eligible, now_unix, sign_entry,
};

/// The first line of every pack.
pub const MAGIC: &[u8] = b"sail-reply-pack-v1\n";

/// The largest pack a reader accepts: 256 MiB.
pub const MAX_PACK_BYTES: u64 = 256 * 1024 * 1024;

/// A parsed pack: its bytes and where each entry lies in them.
#[derive(Debug)]
pub struct Pack {
    data: Vec<u8>,
    entries: HashMap<[u8; 32], (usize, usize)>,
}

impl Pack {
    /// Parses `data` as a pack.
    ///
    /// Returns `None` for anything malformed: a wrong first line, a
    /// truncated record, an entry over [`MAX_ENTRY_BYTES`], keys out of
    /// order or repeated, trailing bytes, or a pack over
    /// [`MAX_PACK_BYTES`].
    #[must_use]
    pub fn parse(data: Vec<u8>) -> Option<Self> {
        if data.len() as u64 > MAX_PACK_BYTES || !data.starts_with(MAGIC) {
            return None;
        }
        let mut entries = HashMap::new();
        let mut at = MAGIC.len();
        let mut previous: Option<[u8; 32]> = None;
        while at < data.len() {
            let key: [u8; 32] = data.get(at..at.checked_add(32)?)?.try_into().ok()?;
            at += 32;
            let length: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
            at += 4;
            let length = u32::from_le_bytes(length);
            if u64::from(length) > MAX_ENTRY_BYTES {
                return None;
            }
            let length = usize::try_from(length).ok()?;
            let end = at.checked_add(length)?;
            data.get(at..end)?;
            if previous.is_some_and(|p| p >= key) {
                return None;
            }
            previous = Some(key);
            entries.insert(key, (at, end));
            at = end;
        }
        Some(Self { data, entries })
    }

    /// The entry bytes stored for `key`, unchecked: the caller verifies them.
    #[must_use]
    pub fn get(&self, key: &QueryKey) -> Option<&[u8]> {
        let &(start, end) = self.entries.get(&key.0)?;
        self.data.get(start..end)
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the pack holds no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Builds a pack of `disk`'s exportable entries, each signed with `secret`.
///
/// It selects exactly what `publish` would export (the same eligibility
/// and serving policy). Entries are written in ascending key order. The
/// report's `published` counts the entries in the pack, and `failed`
/// counts entries left out because they are too large for a record.
#[must_use]
pub fn build(
    disk: &DiskCache,
    secret: &SecretKey,
    options: &PublishOptions,
    unknown_ok: &dyn Fn(Option<&Effort>) -> bool,
) -> (Vec<u8>, PublishReport) {
    let now = now_unix();
    let mut signed = BTreeMap::new();
    let mut report = PublishReport::default();
    eligible(disk, options, unknown_ok, &mut report, |e, r| {
        let data = sign_entry(
            secret,
            &e.key,
            &e.reply,
            &e.meta.command,
            &e.solver,
            e.meta.effort.as_ref(),
            now,
        );
        if data.len() as u64 > MAX_ENTRY_BYTES || u32::try_from(data.len()).is_err() {
            r.failed += 1;
            return;
        }
        signed.insert(e.key.0, data);
    });
    let mut out = MAGIC.to_vec();
    for (key, data) in &signed {
        out.extend_from_slice(key);
        out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(data);
    }
    report.published = signed.len() as u64;
    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(out: &mut Vec<u8>, key: [u8; 32], entry: &[u8]) {
        out.extend_from_slice(&key);
        out.extend_from_slice(&u32::try_from(entry.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(entry);
    }

    #[test]
    fn parses_records_and_finds_each_key() {
        let mut data = MAGIC.to_vec();
        record(&mut data, [1; 32], b"one");
        record(&mut data, [2; 32], b"");
        record(&mut data, [3; 32], b"three");
        let pack = Pack::parse(data);
        let Some(pack) = pack else {
            unreachable!("a well-formed pack parses")
        };
        assert_eq!(pack.len(), 3);
        assert_eq!(pack.get(&QueryKey([1; 32])), Some(&b"one"[..]));
        assert_eq!(pack.get(&QueryKey([2; 32])), Some(&b""[..]));
        assert_eq!(pack.get(&QueryKey([3; 32])), Some(&b"three"[..]));
        assert_eq!(pack.get(&QueryKey([4; 32])), None);
        assert!(Pack::parse(MAGIC.to_vec()).is_some_and(|p| p.is_empty()));
    }

    #[test]
    fn refuses_every_malformed_pack() {
        let mut good = MAGIC.to_vec();
        record(&mut good, [1; 32], b"one");
        record(&mut good, [2; 32], b"two");
        assert!(Pack::parse(good.clone()).is_some());
        // Wrong first line.
        let mut bad = good.clone();
        bad[0] = b'x';
        assert!(Pack::parse(bad).is_none());
        // Truncated anywhere inside a record (a cut between records is a
        // shorter, well-formed pack).
        let boundary = MAGIC.len() + 32 + 4 + 3;
        for cut in (MAGIC.len() + 1..good.len()).filter(|&cut| cut != boundary) {
            assert!(Pack::parse(good[..cut].to_vec()).is_none(), "cut at {cut}");
        }
        // Trailing bytes.
        let mut bad = good.clone();
        bad.push(0);
        assert!(Pack::parse(bad).is_none());
        // Repeated and out-of-order keys.
        let mut bad = MAGIC.to_vec();
        record(&mut bad, [2; 32], b"two");
        record(&mut bad, [2; 32], b"two");
        assert!(Pack::parse(bad).is_none());
        let mut bad = MAGIC.to_vec();
        record(&mut bad, [2; 32], b"two");
        record(&mut bad, [1; 32], b"one");
        assert!(Pack::parse(bad).is_none());
        // An entry length over the entry limit.
        let mut bad = MAGIC.to_vec();
        bad.extend_from_slice(&[1; 32]);
        bad.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(Pack::parse(bad).is_none());
    }
}
