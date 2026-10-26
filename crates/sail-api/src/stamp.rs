//! Complete stat stamps and the settle rule for cross-run stat trust
//! (ADR 0082).
//!
//! A [`Stamp`] is a file's device, inode, size, mode, mtime and ctime, both
//! times with nanoseconds. A user program can set mtime but not ctime, and
//! every write, rename onto the path, `chmod` or `utimes` advances ctime.
//! A stamp may be trusted across runs only once its ctime is at least
//! [`SETTLED`] older than the moment observation began
//! ([`Stamp::settled`]): a later change then moves ctime past the recorded
//! one even on a filesystem with a coarse clock. Unchanged-call replay and
//! the daemon's solver identity both use it.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How much older than the start of an observation a trusted ctime must be.
pub const SETTLED: Duration = Duration::from_secs(2);

/// A complete stat stamp: device, inode, size, mode, mtime and ctime with
/// nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stamp([i64; 8]);

impl Stamp {
    /// The stamp of the file `metadata` describes.
    #[cfg(unix)]
    #[must_use]
    pub fn of(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        // Bit-for-bit, so distinct unsigned values never share a field.
        let wide = |value: u64| i64::from_ne_bytes(value.to_ne_bytes());
        Self([
            wide(metadata.dev()),
            wide(metadata.ino()),
            wide(metadata.size()),
            i64::from(metadata.mode()),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ])
    }

    /// Without a ctime there is no cross-run stat trust: the stamp is never
    /// [settled](Self::settled).
    #[cfg(not(unix))]
    #[must_use]
    pub fn of(_metadata: &std::fs::Metadata) -> Self {
        Self([0, 0, 0, 0, 0, 0, -1, -1])
    }

    /// Whether the last status change happened at least [`SETTLED`] before
    /// `start`. A ctime before the epoch or after `start` is never settled.
    #[must_use]
    pub fn settled(self, start: SystemTime) -> bool {
        let (Ok(seconds), Ok(nanos)) = (u64::try_from(self.0[6]), u64::try_from(self.0[7])) else {
            return false;
        };
        let Some(changed) = UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds))
            .and_then(|t| t.checked_add(Duration::from_nanos(nanos)))
        else {
            return false;
        };
        start
            .duration_since(changed)
            .is_ok_and(|age| age >= SETTLED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ctime: Duration) -> Stamp {
        let mut fields = [0; 8];
        fields[6] = i64::try_from(ctime.as_secs()).unwrap_or(i64::MAX);
        fields[7] = i64::from(ctime.subsec_nanos());
        Stamp(fields)
    }

    #[test]
    fn only_status_changes_two_seconds_before_the_start_are_settled() {
        let start = UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(at(Duration::from_secs(998)).settled(start));
        assert!(at(Duration::from_secs(997)).settled(start));
        assert!(!at(Duration::from_millis(998_001)).settled(start));
        assert!(!at(Duration::from_millis(998_500)).settled(start));
        assert!(!at(Duration::from_secs(1_000)).settled(start));
        assert!(!at(Duration::from_secs(1_001)).settled(start));
        let mut negative = at(Duration::ZERO);
        negative.0[6] = -1;
        assert!(!negative.settled(start));
    }

    #[test]
    fn the_serialized_form_is_the_field_array() -> serde_json::Result<()> {
        let stamp = Stamp([1, 2, 3, 4, 5, 6, 7, 8]);
        let text = serde_json::to_string(&stamp)?;
        assert_eq!(text, "[1,2,3,4,5,6,7,8]");
        assert_eq!(serde_json::from_str::<Stamp>(&text)?, stamp);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_fresh_file_is_stamped_but_not_settled() -> std::io::Result<()> {
        let path = std::env::temp_dir().join(format!("sail-stamp-{}", std::process::id()));
        std::fs::write(&path, b"x")?;
        let stamp = Stamp::of(&std::fs::metadata(&path)?);
        std::fs::remove_file(&path)?;
        assert!(!stamp.settled(SystemTime::now()));
        Ok(())
    }
}
