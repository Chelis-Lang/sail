//! Linux binding and transient-change observation with a synchronous queue
//! barrier. This is not a content oracle: existing mmap writes are invisible.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod platform {
    use super::{Path, PathBuf};
    use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
    use std::collections::{BTreeSet, HashMap};
    use std::os::unix::fs::MetadataExt as _;
    use std::time::{Duration, Instant};

    const MAX_WATCHES: usize = 8192;
    const MAX_EVENTS: usize = 32768;

    fn identity(path: &Path) -> std::io::Result<Option<(u64, u64)>> {
        match std::fs::metadata(path) {
            Ok(m) => Ok(Some((m.dev(), m.ino()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    fn local_filesystem(path: &Path, mounts: &str) -> bool {
        let mut longest = 0;
        let mut trusted = false;
        for line in mounts.lines() {
            let Some((before, after)) = line.split_once(" - ") else {
                return false;
            };
            let Some(mount) = before.split_whitespace().nth(4) else {
                return false;
            };
            // Unsupported path escapes fail closed, rather than guessing which
            // mount supplies a declared input. Ordinary UTF-8 paths are exact.
            if mount.contains('\\') {
                return false;
            }
            let mount = Path::new(mount);
            if path.starts_with(mount) && mount.as_os_str().len() >= longest {
                longest = mount.as_os_str().len();
                trusted = after.split_whitespace().next().is_some_and(|fs| {
                    [
                        "ext2", "ext3", "ext4", "xfs", "btrfs", "tmpfs", "overlay", "zfs",
                    ]
                    .contains(&fs)
                });
            }
        }
        trusted
    }

    pub(super) struct Observer {
        native: Inotify,
        directories: HashMap<WatchDescriptor, PathBuf>,
        tracked: BTreeSet<PathBuf>,
        excluded: Vec<PathBuf>,
        complete: bool,
        changed: bool,
        mounts: String,
        bindings: Vec<(PathBuf, Option<(u64, u64)>)>,
    }
    impl Observer {
        pub(super) fn begin(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
            let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
            let mut observer = Self {
                native: Inotify::init().ok()?,
                directories: HashMap::new(),
                tracked: roots.iter().cloned().collect(),
                excluded: excluded.to_vec(),
                complete: true,
                changed: false,
                mounts,
                bindings: Vec::new(),
            };
            let mut visited = BTreeSet::new();
            for root in roots {
                observer.binding(root)?;
                observer.walk(root, &mut visited, true)?;
            }
            for path in roots {
                observer.bindings.push((path.clone(), identity(path).ok()?));
            }
            Some(observer)
        }
        fn watch(&mut self, directory: &Path) -> Option<()> {
            let canonical = directory.canonicalize().ok()?;
            if !local_filesystem(&canonical, &self.mounts) {
                return None;
            }
            if self.directories.values().any(|p| *p == canonical) {
                return Some(());
            }
            if self.directories.len() >= MAX_WATCHES {
                return None;
            }
            let descriptor = self
                .native
                .watches()
                .add(
                    &canonical,
                    WatchMask::MODIFY
                        | WatchMask::ATTRIB
                        | WatchMask::CREATE
                        | WatchMask::DELETE
                        | WatchMask::MOVED_FROM
                        | WatchMask::MOVED_TO
                        | WatchMask::DELETE_SELF
                        | WatchMask::MOVE_SELF,
                )
                .ok()?;
            self.directories.insert(descriptor, canonical);
            Some(())
        }
        fn binding(&mut self, path: &Path) -> Option<()> {
            self.binding_depth(path, 0)
        }
        fn binding_depth(&mut self, path: &Path, depth: usize) -> Option<()> {
            if depth > 64 || self.tracked.len() > 65536 {
                return None;
            }
            // Observe every path component, not just the final file. Renaming a
            // grandparent must not silently redirect a previously hashed input.
            for ancestor in path.ancestors().skip(1).filter(|p| p.is_dir()) {
                self.watch(ancestor)?;
            }
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    let target = std::fs::read_link(path).ok()?;
                    let target = path.parent()?.join(target);
                    self.tracked.insert(target.clone());
                    self.binding_depth(&target, depth + 1)?;
                }
                Ok(_) => {
                    if let Ok(canonical) = path.canonicalize() {
                        if !local_filesystem(&canonical, &self.mounts) {
                            return None;
                        }
                        self.tracked.insert(canonical);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let parent = path.ancestors().skip(1).find(|p| p.is_dir())?;
                    self.tracked.insert(
                        parent
                            .canonicalize()
                            .ok()?
                            .join(path.strip_prefix(parent).ok()?),
                    );
                }
                Err(_) => return None,
            }
            Some(())
        }
        fn ignored(&self, path: &Path) -> bool {
            self.excluded.iter().any(|p| path.starts_with(p))
                && !self.tracked.iter().any(|root| {
                    self.excluded.iter().any(|p| root.starts_with(p)) && path.starts_with(root)
                })
        }
        fn walk(
            &mut self,
            path: &Path,
            visited: &mut BTreeSet<PathBuf>,
            explicit: bool,
        ) -> Option<()> {
            if !explicit
                && (self.ignored(path)
                    || path.file_name().is_some_and(|name| {
                        [".git", ".claude", ".codex"].iter().any(|s| name == *s)
                    }))
            {
                return Some(());
            }
            self.binding(path)?;
            let canonical = match path.canonicalize() {
                Ok(path) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(()),
                Err(_) => return None,
            };
            if !canonical.is_dir() {
                self.watch(&canonical)?;
                return Some(());
            }
            if !visited.insert(canonical.clone()) {
                return Some(());
            }
            self.watch(&canonical)?;
            for entry in std::fs::read_dir(canonical).ok()? {
                self.walk(&entry.ok()?.path(), visited, false)?;
            }
            Some(())
        }
        pub(super) fn files(&self) -> Vec<PathBuf> {
            let mut files: Vec<_> = self
                .directories
                .values()
                .filter(|p| p.is_file())
                .cloned()
                .collect();
            files.sort();
            files.dedup();
            files
        }
        pub(super) fn clean(&mut self) -> bool {
            // Inotify::init uses a nonblocking FD. A drain ending in WouldBlock
            // is a synchronous absence barrier, with no callback delivery race.
            // Mount operations can change path resolution without ordinary file
            // events. Bindings supplement continuous native observation; they
            // never replace it with metadata-only content trust.
            if std::fs::read_to_string("/proc/self/mountinfo")
                .as_ref()
                .ok()
                != Some(&self.mounts)
                || self
                    .bindings
                    .iter()
                    .any(|(p, old)| identity(p).as_ref().ok() != Some(old))
            {
                self.complete = false;
                return false;
            }
            let deadline = Instant::now() + Duration::from_millis(20);
            let mut buffer = vec![0; 65536];
            let mut count = 0;
            loop {
                if count >= MAX_EVENTS {
                    self.complete = false;
                    return false;
                }
                match self.native.read_events(&mut buffer) {
                    Ok(events) => {
                        // Always attempt the nonblocking read: scheduler delay
                        // cannot turn an already empty native queue into lost
                        // coverage. Bound actual nonempty drain work instead.
                        if Instant::now() >= deadline {
                            self.complete = false;
                            return false;
                        }
                        for event in events {
                            count += 1;
                            if event.mask.intersects(
                                EventMask::Q_OVERFLOW
                                    | EventMask::IGNORED
                                    | EventMask::UNMOUNT
                                    | EventMask::DELETE_SELF
                                    | EventMask::MOVE_SELF,
                            ) {
                                self.complete = false;
                            }
                            let Some(parent) = self.directories.get(&event.wd) else {
                                self.complete = false;
                                continue;
                            };
                            let path = event
                                .name
                                .map_or_else(|| parent.clone(), |name| parent.join(name));
                            if !self.ignored(&path)
                                && self
                                    .tracked
                                    .iter()
                                    .any(|root| path.starts_with(root) || root.starts_with(&path))
                            {
                                self.changed = true;
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return self.complete && !self.changed;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        self.complete = false;
                        return false;
                    }
                }
            }
        }
    }
}

/// No cached projection is permitted without a precise native queue.
#[cfg(not(target_os = "linux"))]
mod platform {
    use super::PathBuf;
    pub(super) struct Observer;
    impl Observer {
        pub(super) fn begin(_: &[PathBuf], _: &[PathBuf]) -> Option<Self> {
            None
        }
        pub(super) fn clean(&mut self) -> bool {
            false
        }
    }
}

pub(super) struct Observer(platform::Observer);
impl Observer {
    pub(super) fn begin(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
        platform::Observer::begin(roots, excluded).map(Self)
    }
    pub(super) fn clean(&mut self) -> bool {
        self.0.clean()
    }
    pub(super) fn files(&self) -> Vec<PathBuf> {
        self.0.files()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn fixture(name: &str) -> PathBuf {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-precise-{name}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir_all(root.join("source")).is_ok());
        root
    }
    #[test]
    fn restored_content_and_mtime_are_invalidated_without_callback_waits() {
        let root = fixture("restored");
        let source = root.join("source/lib.rs");
        assert!(std::fs::write(&source, "good").is_ok());
        let file = std::fs::File::open(&source).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let mut observer =
            Observer::begin(&[root.join("source")], &[]).unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::write(&source, "fail").is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        assert!(!observer.clean());
        assert!(std::fs::write(&source, "good").is_ok());
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn absent_configuration_transient_creation_and_new_paths_invalidate() {
        let root = fixture("absent");
        let config = root.join("missing/config.toml");
        let mut observer = Observer::begin(std::slice::from_ref(&config), &[])
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::create_dir_all(root.join("missing")).is_ok());
        assert!(std::fs::write(&config, "temporary").is_ok());
        assert!(std::fs::remove_dir_all(root.join("missing")).is_ok());
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn directory_binding_and_symlink_replacement_invalidate() {
        let root = fixture("alias");
        let real = root.join("source/lib.rs");
        let alias = root.join("alias");
        assert!(std::fs::write(&real, "good").is_ok());
        assert!(std::os::unix::fs::symlink(&real, &alias).is_ok());
        let mut observer = Observer::begin(std::slice::from_ref(&alias), &[])
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::remove_file(&alias).is_ok());
        assert!(std::os::unix::fs::symlink(&real, &alias).is_ok());
        assert!(!observer.clean());
        drop(observer);
        let mut observer = Observer::begin(&[real], &[]).unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::rename(root.join("source"), root.join("moved")).is_ok());
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn unrelated_parent_events_and_output_writes_do_not_invalidate() {
        let root = fixture("outputs");
        let source = root.join("source");
        let target = source.join("target");
        assert!(std::fs::create_dir_all(&target).is_ok());
        let mut observer =
            Observer::begin(std::slice::from_ref(&source), std::slice::from_ref(&target))
                .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::write(target.join("output"), "compiled").is_ok());
        assert!(std::fs::write(root.join("unrelated"), "journal").is_ok());
        assert!(observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn preexisting_outside_hardlink_write_is_observed_on_inode_watch() {
        let root = fixture("hardlink");
        let file = root.join("source/lib.rs");
        let outside = root.join("outside");
        assert!(std::fs::write(&file, b"good").is_ok());
        assert!(std::fs::hard_link(&file, &outside).is_ok());
        let mut observer =
            Observer::begin(&[root.join("source")], &[]).unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::write(outside, b"fail").is_ok());
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn real_kernel_overflow_and_lost_watch_never_allow_reuse() {
        let root = fixture("overflow");
        let source = root.join("source");
        let mut observer = Observer::begin(std::slice::from_ref(&source), &[])
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        let capacity = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(16384);
        assert!(capacity <= 65536, "bounded kernel queue fixture");
        for n in 0..=capacity {
            assert!(std::fs::write(source.join(format!("input-{n}")), []).is_ok());
        }
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(&source).is_ok());
        assert!(std::fs::create_dir(&source).is_ok());
        let mut observer = Observer::begin(std::slice::from_ref(&source), &[])
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        assert!(std::fs::remove_dir_all(source).is_ok());
        assert!(!observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
