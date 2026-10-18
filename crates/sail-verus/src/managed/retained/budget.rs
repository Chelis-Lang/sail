//! Aggregate admission for the retained arena; fullness is a compiler miss.
//!
//! All durable publishers hold the same short filesystem lock through their
//! writes. The bounded scan counts existing and failed/partial publications,
//! never deletes a possibly live compiler generation, and reserves the full
//! additional peak before any durable write. Compiler children run unlocked.

use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

const BYTES: u64 = 8 * 1024 * 1024 * 1024;
const ENTRIES: u64 = 65_536;
const DEPTH: usize = 16;

pub(super) fn admit(root: &Path, bytes: u64, entries: u64) -> Option<File> {
    admit_with(root, bytes, entries, BYTES, ENTRIES)
}

fn admit_with(
    root: &Path,
    bytes: u64,
    entries: u64,
    maximum_bytes: u64,
    maximum_entries: u64,
) -> Option<File> {
    let root = root.canonicalize().ok()?;
    let lock = super::super::projects::lock(
        &root.join("admission.lock"),
        false,
        Instant::now() + Duration::from_secs(2),
    )
    .ok()?;
    let mut used = (bytes, entries);
    scan(&root, 0, &mut used, maximum_bytes, maximum_entries)?;
    Some(lock)
}

fn scan(
    path: &Path,
    depth: usize,
    used: &mut (u64, u64),
    maximum_bytes: u64,
    maximum_entries: u64,
) -> Option<()> {
    if depth > DEPTH {
        return None;
    }
    let metadata = std::fs::symlink_metadata(path).ok()?;
    used.1 = used.1.checked_add(1)?;
    if metadata.is_file() {
        used.0 = used.0.checked_add(metadata.len())?;
    } else if !metadata.is_dir() {
        // Never follow links or open a FIFO/socket/device while measuring.
        return None;
    }
    if used.0 > maximum_bytes || used.1 > maximum_entries {
        return None;
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).ok()? {
            scan(
                &entry.ok()?.path(),
                depth + 1,
                used,
                maximum_bytes,
                maximum_entries,
            )?;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Root(PathBuf);
    impl Root {
        fn new(name: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "sail-retained-budget-{}-{name}",
                std::process::id()
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path.canonicalize()?))
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn peak_bytes_and_partial_publications_refuse_without_deleting_live_graphs()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("bytes")?;
        let guard = admit_with(&root.0, 20, 1, 32, 16).ok_or("first admission")?;
        std::fs::write(root.0.join("live-generation"), [b'A'; 20])?;
        drop(guard);
        assert!(admit_with(&root.0, 20, 1, 32, 16).is_none());
        assert_eq!(
            std::fs::read(root.0.join("live-generation"))?,
            vec![b'A'; 20]
        );
        // An interrupted write still consumes admission budget; nothing guesses
        // that an unknown old generation or partial file can be destroyed.
        std::fs::write(root.0.join("failed.tmp"), [b'B'; 10])?;
        assert!(admit_with(&root.0, 3, 1, 32, 16).is_none());
        assert!(admit_with(&root.0, 2, 1, 32, 16).is_some());
        assert!(admit_with(&root.0, u64::MAX, 0, u64::MAX, 16).is_none());
        assert!(admit_with(&root.0, 0, u64::MAX, 32, u64::MAX).is_none());
        Ok(())
    }

    #[test]
    fn entry_and_depth_limits_apply_even_to_empty_files() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = Root::new("entries")?;
        std::fs::write(root.0.join("a"), [])?;
        std::fs::write(root.0.join("b"), [])?;
        // Root, lock and both empty files already use four entries.
        assert!(admit_with(&root.0, 0, 0, 32, 4).is_some());
        assert!(admit_with(&root.0, 0, 1, 32, 4).is_none());
        let mut nested = root.0.clone();
        for _ in 0..=DEPTH {
            nested.push("nested");
        }
        std::fs::create_dir_all(nested)?;
        assert!(admit_with(&root.0, 0, 0, 32, 100).is_none());
        Ok(())
    }

    #[test]
    fn another_publisher_cannot_spend_the_same_remaining_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("concurrent")?;
        let guard = admit_with(&root.0, 32, 1, 32, 16).ok_or("first publisher")?;
        let path = root.0.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let other = std::thread::spawn(move || -> Result<bool, std::io::Error> {
            let probe = File::options()
                .read(true)
                .write(true)
                .open(path.join("admission.lock"))?;
            let excluded = matches!(probe.try_lock(), Err(std::fs::TryLockError::WouldBlock));
            sender.send(excluded).map_err(std::io::Error::other)?;
            Ok(admit_with(&path, 1, 1, 32, 16).is_none())
        });
        assert!(receiver.recv_timeout(Duration::from_secs(1))?);
        std::fs::write(root.0.join("published"), [b'A'; 32])?;
        drop(guard);
        assert!(other.join().map_err(|_| "publisher fixture panicked")??);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn actual_graph_publication_deduplicates_markers_and_preserves_unknown_live_graphs()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::managed::dependency_snapshot::Snapshot;
        use std::ffi::OsString;
        let root = Root::new("graphs")?;
        let arena = root.0.join("arena");
        std::fs::create_dir_all(arena.join("graphs"))?;
        std::fs::create_dir(arena.join("live-graphs"))?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        std::fs::create_dir_all(&deps)?;
        let input = deps.join("libvstd-abcd.rmeta");
        std::fs::write(&input, b"actual independent bytes A")?;
        let args: Vec<OsString> = vec![
            "rustc".into(),
            "--crate-name".into(),
            "workspace".into(),
            "-C".into(),
            "extra-filename=-abcd".into(),
            "--out-dir".into(),
            deps.clone().into(),
            "-L".into(),
            format!("dependency={}", deps.display()).into(),
            "--extern".into(),
            format!("vstd={}", input.display()).into(),
        ];
        let captured =
            Snapshot::capture(&target, &root.0.join("copy-a"), &args).ok_or("capture A")?;
        let a = super::super::driver::publish_graph(&arena, captured, &args).ok_or("publish A")?;
        let key = a.key().ok_or("graph key")?;
        let again = Snapshot::reuse(&target, &arena.join("graphs"), &args).ok_or("reuse A")?;
        let again = super::super::driver::publish_graph(&arena, again, &args)
            .ok_or("idempotent publish")?;
        assert_eq!(again.key(), Some(key));
        assert_eq!(std::fs::read_dir(arena.join("live-graphs"))?.count(), 1);
        assert_eq!(std::fs::read_dir(arena.join("graphs"))?.count(), 1);
        std::fs::write(&input, b"different public bytes B")?;
        let b = Snapshot::capture(&target, &root.0.join("copy-b"), &args).ok_or("capture B")?;
        std::os::unix::fs::symlink("unknown-generation", arena.join("unknown-live"))?;
        assert!(super::super::driver::publish_graph(&arena, b, &args).is_none());
        assert!(a.intact());
        assert_eq!(std::fs::read_dir(arena.join("graphs"))?.count(), 1);
        assert!(std::fs::symlink_metadata(arena.join("unknown-live"))?.is_symlink());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_real_fifo_are_refused_without_following_or_opening()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("special")?;
        std::os::unix::fs::symlink(root.0.join("missing"), root.0.join("link"))?;
        assert!(admit_with(&root.0, 0, 0, 32, 16).is_none());
        std::fs::remove_file(root.0.join("link"))?;
        assert!(
            std::process::Command::new("mkfifo")
                .arg(root.0.join("fifo"))
                .status()?
                .success()
        );
        assert!(admit_with(&root.0, 0, 0, 32, 16).is_none());
        Ok(())
    }
}
