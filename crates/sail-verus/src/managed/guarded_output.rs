//! Optional continuous ownership of one immutable output digest. This proves
//! output byte identity only; it never certifies input freshness or a verdict.

use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};

/// Maximum requested retained storage admitted for one output owner.
///
/// Callers must reserve this allowance before construction and retain it through
/// every owner reference, including construction and eviction. This is not an
/// allocator RSS or whole-construction peak claim.
pub const GUARDED_OUTPUT_RETAINED_BYTES: usize = 16 * 1024 * 1024;

/// A nonrenewable Linux namespace/read-lease owner of an independently hashed
/// complete output. Unsupported coverage declines rather than trusting metadata.
///
/// Construction belongs outside foreground RPCs. The original complete read and
/// SHA check remains the fallback; every reuse requires fresh native barriers.
/// The daemon result path uses optional background construction and brackets
/// independent input freshness with these barriers before reusing an output seal.
pub struct GuardedOutput {
    #[cfg(target_os = "linux")]
    inner: linux::Owner,
}
impl GuardedOutput {
    /// Acquires all native guards before independently hashing the entire file.
    ///
    /// The exact expected digest is only a comparison value. It provides no
    /// authority without continuous guards and the independent read. A shared
    /// cancellation source and one 120-second construction clock cover every
    /// bounded observer/helper phase; kernel operations are not interruptible.
    /// Files above the existing complete RPC frame limit are refused.
    #[must_use]
    pub fn begin(path: &Path, expected: &str, cancelled: Arc<AtomicBool>) -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            linux::Owner::begin(path, expected, cancelled).map(|inner| Self { inner })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (path, expected, cancelled);
            None
        }
    }

    /// Drains the exact output namespace before a caller's independent input
    /// validation. A later successful [`Self::clean`] must bracket that input
    /// validation before combining its verdict with this output digest.
    #[must_use]
    pub fn namespace_barrier(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.inner.namespace_barrier()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Certifies uninterrupted exact output bytes through fresh namespace,
    /// helper, path, mount-generation and complete kernel read-lease barriers.
    /// Any failed barrier is sticky for this generation, including restored bytes.
    #[must_use]
    pub fn clean(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.inner.clean()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Conservative checked requested-storage charge of the actual retained
    /// owner. Native watches/FDs and the helper process are separate resources.
    #[must_use]
    pub fn retained_bytes(&self) -> Option<usize> {
        #[cfg(target_os = "linux")]
        {
            self.inner.retained_bytes()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Some(0)
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Arc, AtomicBool, GUARDED_OUTPUT_RETAINED_BYTES, Path};
    use crate::managed::{lease::ReadLease, precise::Observer};
    use sha2::{Digest, Sha256};
    use std::io::Read as _;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    pub(super) struct Owner {
        path: PathBuf,
        expected: String,
        observer: Observer,
        lease: ReadLease,
        cancelled: Arc<AtomicBool>,
        dirty: bool,
    }
    impl Owner {
        pub(super) fn begin(
            path: &Path,
            expected: &str,
            cancelled: Arc<AtomicBool>,
        ) -> Option<Self> {
            let started = Instant::now();
            let cancellation = Arc::clone(&cancelled);
            let stopped = || {
                cancellation.load(Ordering::Acquire)
                    || started.elapsed() >= Duration::from_secs(120)
            };
            if stopped()
                || path.as_os_str().as_encoded_bytes().len() > 8192
                || expected.len() != 64
                || !expected
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            let path = std::path::absolute(path).ok()?;
            if path.capacity() > 8192 {
                return None;
            }
            let paths = [path.clone()];
            let observer =
                Observer::begin_files_bounded_with(&paths, Arc::clone(&cancelled), started)?;
            let lease =
                ReadLease::acquire_bounded(&paths, &stopped, Arc::clone(&cancelled), started)?;
            let mut owner = Self {
                path,
                expected: expected.to_owned(),
                observer,
                lease,
                cancelled,
                dirty: false,
            };
            if stopped()
                || owner.retained_bytes()? > GUARDED_OUTPUT_RETAINED_BYTES
                || !owner.clean()
                || hash(&owner.path, &stopped)? != owner.expected
                || stopped()
                || !owner.clean()
            {
                return None;
            }
            Some(owner)
        }

        pub(super) fn namespace_barrier(&mut self) -> bool {
            if self.dirty || self.cancelled.load(Ordering::Acquire) {
                self.dirty = true;
                return false;
            }
            self.dirty |= !self.observer.clean();
            !self.dirty
        }

        pub(super) fn clean(&mut self) -> bool {
            if !self.namespace_barrier() {
                return false;
            }
            self.dirty |= !self.lease.clean();
            !self.dirty && self.namespace_barrier()
        }

        pub(super) fn retained_bytes(&self) -> Option<usize> {
            // Destructure exhaustively so added fields cannot silently escape
            // admission. Shared cancellation is deliberately overcharged.
            let Self {
                path,
                expected,
                observer,
                lease,
                cancelled: _,
                dirty: _,
            } = self;
            std::mem::size_of::<Self>()
                .checked_add(path.capacity())?
                .checked_add(expected.capacity())?
                .checked_add(observer.retained_bytes()?)?
                .checked_add(lease.retained_bytes()?)?
                .checked_add(std::mem::size_of::<AtomicBool>() + 2 * std::mem::size_of::<usize>())
        }
    }

    fn hash(path: &Path, stopped: &dyn Fn() -> bool) -> Option<String> {
        use rustix::fs::{Mode, OFlags, open};
        let descriptor = open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        let mut file = std::fs::File::from(descriptor);
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > sail_api::jobs::MAX_FRAME as u64 {
            return None;
        }
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 16384];
        let mut bytes = 0usize;
        loop {
            if stopped() {
                return None;
            }
            let count = file.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            bytes = bytes.checked_add(count)?;
            if bytes > sail_api::jobs::MAX_FRAME {
                return None;
            }
            digest.update(&buffer[..count]);
        }
        Some(format!("{:x}", digest.finalize()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs::{File, FileTimes};
        use std::sync::atomic::AtomicU64;

        struct Fixture {
            root: PathBuf,
            path: PathBuf,
            expected: String,
        }
        impl Fixture {
            fn new() -> Self {
                static SERIAL: AtomicU64 = AtomicU64::new(0);
                let root = std::env::temp_dir().join(format!(
                    "sail-output-guard-{}-{}",
                    std::process::id(),
                    SERIAL.fetch_add(1, Ordering::Relaxed)
                ));
                assert!(std::fs::create_dir(&root).is_ok());
                let path = root.join("stdout");
                assert!(std::fs::write(&path, b"complete original report").is_ok());
                let expected = crate::managed::digest(b"complete original report");
                Self {
                    root,
                    path,
                    expected,
                }
            }
            fn owner(&self) -> Owner {
                Owner::begin(&self.path, &self.expected, Arc::new(AtomicBool::new(false)))
                    .unwrap_or_else(|| unreachable!("real owned kernel fixture"))
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }

        #[test]
        fn complete_independently_read_digest_is_reusable_only_under_live_coverage() {
            let fixture = Fixture::new();
            assert!(
                Owner::begin(
                    &fixture.path,
                    &"0".repeat(64),
                    Arc::new(AtomicBool::new(false))
                )
                .is_none()
            );
            let mut owner = fixture.owner();
            assert!(
                owner
                    .retained_bytes()
                    .is_some_and(|n| n <= GUARDED_OUTPUT_RETAINED_BYTES)
            );
            for _ in 0..8 {
                assert!(owner.clean());
            }
            assert_eq!(
                std::fs::read(&fixture.path).ok().as_deref(),
                Some(b"complete original report".as_slice())
            );
        }

        #[test]
        fn restored_mtime_and_restored_content_never_revive_an_owner() {
            let fixture = Fixture::new();
            let modified = std::fs::metadata(&fixture.path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
            let mut owner = fixture.owner();
            assert!(owner.namespace_barrier());
            assert!(std::fs::write(&fixture.path, b"changed report").is_ok());
            assert!(std::fs::write(&fixture.path, b"complete original report").is_ok());
            assert!(
                File::open(&fixture.path)
                    .and_then(|f| f.set_times(FileTimes::new().set_modified(modified)))
                    .is_ok()
            );
            assert!(!owner.clean());
            assert!(!owner.clean());
            assert_eq!(
                crate::managed::file_digest(&fixture.path).ok().as_deref(),
                Some(fixture.expected.as_str())
            );
        }

        #[test]
        fn rename_restore_and_identical_replacement_invalidate_exact_bindings() {
            for replace in [false, true] {
                let fixture = Fixture::new();
                let mut owner = fixture.owner();
                let temporary = fixture.root.join("temporary");
                assert!(std::fs::rename(&fixture.path, &temporary).is_ok());
                if replace {
                    assert!(std::fs::write(&fixture.path, b"complete original report").is_ok());
                } else {
                    assert!(std::fs::rename(&temporary, &fixture.path).is_ok());
                }
                assert!(!owner.clean());
            }
        }

        #[test]
        fn outside_hardlink_writer_and_mmap_cannot_hide_from_read_leases() {
            for mmap in [false, true] {
                let fixture = Fixture::new();
                let outside = fixture.root.with_extension("outside");
                assert!(std::fs::hard_link(&fixture.path, &outside).is_ok());
                let mut owner = fixture.owner();
                if mmap {
                    let status = std::process::Command::new("python3").args(["-I", "-c", "import mmap,sys; f=open(sys.argv[1],'r+b'); m=mmap.mmap(f.fileno(),0); m[0:1]=b'X'; m.flush(); m.close(); f.close()"]).arg(&outside).status();
                    assert!(status.is_ok_and(|s| s.success()));
                } else {
                    assert!(std::fs::write(&outside, b"changed via outside inode alias").is_ok());
                }
                assert!(!owner.clean());
                assert!(std::fs::remove_file(outside).is_ok());
            }
        }

        #[test]
        fn helper_loss_and_cancellation_are_sticky_without_any_byte_change() {
            let fixture = Fixture::new();
            let mut owner = fixture.owner();
            assert!(owner.lease.stop_owner_for_test().is_ok());
            assert!(!owner.clean());
            let mut owner = fixture.owner();
            owner.cancelled.store(true, Ordering::Release);
            assert!(!owner.namespace_barrier());
            owner.cancelled.store(false, Ordering::Release);
            assert!(!owner.clean());
            assert!(
                Owner::begin(
                    &fixture.path,
                    &fixture.expected,
                    Arc::new(AtomicBool::new(true))
                )
                .is_none()
            );
        }

        #[test]
        fn oversized_file_is_declined_without_partial_digest_authority() {
            let fixture = Fixture::new();
            assert!(
                File::create(&fixture.path)
                    .and_then(|f| f.set_len(sail_api::jobs::MAX_FRAME as u64 + 1))
                    .is_ok()
            );
            assert!(
                Owner::begin(
                    &fixture.path,
                    &fixture.expected,
                    Arc::new(AtomicBool::new(false))
                )
                .is_none()
            );
        }
    }
}
