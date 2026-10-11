//! Sticky Linux mount-generation coverage. A proc mountinfo descriptor retains
//! its namespace event generation; poll reports changes even after restoration.
//! No worker, sleep, unsafe code or mutable filesystem operation is used here.

use std::fs::File;
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;

use rustix::event::{PollFd, PollFlags, Timespec, poll};

const MAX_MOUNT_BYTES: usize = 1_048_576;

fn identity(path: &str) -> Option<(u64, u64)> {
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

pub(super) struct Guard {
    descriptor: File,
    namespace: (u64, u64),
    root: (u64, u64),
    snapshot: String,
    failed: bool,
}

impl Guard {
    pub(super) fn begin() -> Option<Self> {
        let namespace = identity("/proc/self/ns/mnt")?;
        let root = identity("/")?;
        let descriptor = File::open("/proc/self/mountinfo").ok()?;
        if rustix::fs::fstatfs(&descriptor).ok()?.f_type != rustix::fs::PROC_SUPER_MAGIC {
            return None;
        }
        let mut snapshot = String::new();
        (&descriptor)
            .take(u64::try_from(MAX_MOUNT_BYTES).ok()?.checked_add(1)?)
            .read_to_string(&mut snapshot)
            .ok()?;
        if snapshot.is_empty() || snapshot.len() > MAX_MOUNT_BYTES {
            return None;
        }
        let mut guard = Self {
            descriptor,
            namespace,
            root,
            snapshot,
            failed: false,
        };
        if guard.retained_bytes()? > 2 * MAX_MOUNT_BYTES + std::mem::size_of::<Self>() {
            return None;
        }
        // The first poll is already a barrier; discarding a pending generation
        // would lose a change between opening the descriptor and reading it.
        guard.clean().then_some(guard)
    }

    pub(super) fn snapshot(&self) -> &str {
        &self.snapshot
    }

    pub(super) fn clean(&mut self) -> bool {
        if self.failed {
            return false;
        }
        if identity("/proc/self/ns/mnt") != Some(self.namespace) || identity("/") != Some(self.root)
        {
            self.failed = true;
            return false;
        }
        let mut descriptors = [PollFd::new(&self.descriptor, PollFlags::PRI)];
        let zero = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // mounts_poll consumes its descriptor's generation notification.
        // Latch every event/error permanently before any subsequent barrier.
        self.failed =
            poll(&mut descriptors, Some(&zero)).is_err() || !descriptors[0].revents().is_empty();
        !self.failed
    }

    pub(super) fn retained_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>().checked_add(self.snapshot.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn unchanged_namespace_keeps_its_live_generation() -> TestResult {
        let mut guard = Guard::begin().ok_or("mount generation unavailable")?;
        assert_ne!(guard.snapshot(), "");
        assert!(guard.clean());
        assert!(guard.clean());
        assert!(guard.retained_bytes().is_some());
        Ok(())
    }

    #[test]
    fn failed_generation_never_becomes_clean_again() -> TestResult {
        let mut guard = Guard::begin().ok_or("mount generation unavailable")?;
        guard.failed = true;
        assert!(!guard.clean());
        assert!(!guard.clean());
        Ok(())
    }

    #[test]
    #[ignore = "requires isolated mount privileges; explicitly run by the solver step"]
    fn restored_bind_mount_is_sticky_without_a_worker() -> TestResult {
        use std::process::Command;
        if std::env::var_os("SAIL_TEST_MOUNT_NAMESPACE").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            let status = Command::new("unshare")
                .args(["--mount", "--"])
                .arg(std::env::current_exe()?)
                .args([
                    "--exact",
                    "managed::kernel_mounts::tests::restored_bind_mount_is_sticky_without_a_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env("SAIL_TEST_MOUNT_NAMESPACE", "1")
                .status()?;
            assert!(status.success(), "isolated real mount control failed");
            return Ok(());
        }
        assert!(
            Command::new("mount")
                .args(["--make-rprivate", "/"])
                .status()?
                .success()
        );
        let root =
            std::env::temp_dir().join(format!("sail-mount-generation-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let source = root.join("source");
        let target = root.join("target");
        std::fs::create_dir(&source)?;
        std::fs::create_dir(&target)?;
        std::fs::write(source.join("data"), b"replacement")?;
        std::fs::write(target.join("data"), b"original")?;
        let before = std::fs::read_to_string("/proc/self/mountinfo")?;
        let mut guard = Guard::begin().ok_or("mount generation unavailable")?;
        let mut observer =
            super::super::precise::Observer::begin(std::slice::from_ref(&target), &[])
                .ok_or("namespace coverage unavailable")?;
        let mut lease = super::super::lease::ReadLease::acquire(&[target.join("data")])
            .ok_or("lease coverage unavailable")?;
        assert!(guard.clean() && observer.clean() && lease.clean());
        assert!(
            Command::new("mount")
                .arg("--bind")
                .arg(&source)
                .arg(&target)
                .status()?
                .success()
        );
        assert_eq!(std::fs::read(target.join("data"))?, b"replacement");
        assert!(Command::new("umount").arg(&target).status()?.success());
        assert_eq!(std::fs::read(target.join("data"))?, b"original");
        assert_eq!(std::fs::read_to_string("/proc/self/mountinfo")?, before);
        // The whole mutation and restoration precedes every consumer check;
        // no callback, worker scheduling or differing final snapshot helps.
        assert!(!guard.clean());
        assert!(!guard.clean(), "poll notification must remain sticky");
        assert!(!observer.clean());
        assert!(!lease.clean());
        drop(lease);
        drop(observer);
        drop(guard);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
