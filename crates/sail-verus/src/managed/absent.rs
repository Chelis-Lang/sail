//! Precise native observation of absent configuration paths.
//!
//! A directory timestamp cannot identify which child changed: Cargo's SQLite
//! journals would then invalidate every proof. Native queues retain the exact
//! child names, including configurations created and removed during a proof.

use super::input::InputSnapshot;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub(super) struct Outcome {
    pub changed: bool,
    pub complete: bool,
}

#[cfg(target_os = "linux")]
fn relevant(path: &Path, tracked: &[PathBuf]) -> bool {
    tracked
        .iter()
        .any(|root| path.starts_with(root) || root.starts_with(path))
}

#[cfg(target_os = "linux")]
fn missing(snapshot: &InputSnapshot) -> Vec<PathBuf> {
    snapshot
        .roots
        .iter()
        .filter(|p| !p.exists())
        .cloned()
        .collect()
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Duration, InputSnapshot, Instant, Outcome, Path, PathBuf, missing, relevant};
    use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
    use std::collections::HashMap;

    pub(in crate::managed) struct Observer {
        native: Option<Inotify>,
        directories: HashMap<WatchDescriptor, PathBuf>,
        tracked: Vec<PathBuf>,
    }
    impl Observer {
        pub(in crate::managed) fn begin(snapshot: &InputSnapshot, _: &Path) -> Self {
            let mut tracked = missing(snapshot);
            tracked.extend(tracked.clone().into_iter().filter_map(|path| {
                let ancestor = path.ancestors().find(|p| p.exists())?;
                let canonical = ancestor.canonicalize().ok()?;
                Some(canonical.join(path.strip_prefix(ancestor).ok()?))
            }));
            let mut directories = HashMap::new();
            let native = Inotify::init().ok().and_then(|native| {
                for root in &tracked {
                    let parent = root.ancestors().skip(1).find(|p| p.is_dir())?;
                    let canonical = parent.canonicalize().ok()?;
                    let descriptor = native
                        .watches()
                        .add(
                            &canonical,
                            WatchMask::CREATE
                                | WatchMask::DELETE
                                | WatchMask::MOVED_FROM
                                | WatchMask::MOVED_TO
                                | WatchMask::DELETE_SELF
                                | WatchMask::MOVE_SELF,
                        )
                        .ok()?;
                    directories.insert(descriptor, canonical);
                }
                Some(native)
            });
            Self {
                native,
                directories,
                tracked,
            }
        }
        pub(in crate::managed) fn ready(&self) -> bool {
            self.native.is_some()
        }
        pub(in crate::managed) fn finish(mut self) -> Outcome {
            let Some(mut native) = self.native.take() else {
                return Outcome {
                    changed: false,
                    complete: false,
                };
            };
            let mut outcome = Outcome {
                changed: false,
                complete: true,
            };
            let mut buffer = vec![0; 65536];
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                if Instant::now() >= deadline {
                    return Outcome {
                        changed: outcome.changed,
                        complete: false,
                    };
                }
                match native.read_events(&mut buffer) {
                    Ok(events) => {
                        let mut count = 0;
                        for event in events {
                            count += 1;
                            if event.mask.intersects(
                                EventMask::Q_OVERFLOW
                                    | EventMask::IGNORED
                                    | EventMask::UNMOUNT
                                    | EventMask::DELETE_SELF
                                    | EventMask::MOVE_SELF,
                            ) {
                                outcome.complete = false;
                            }
                            if let Some(parent) = self.directories.get(&event.wd) {
                                let path = event
                                    .name
                                    .map_or_else(|| parent.clone(), |name| parent.join(name));
                                outcome.changed |= relevant(&path, &self.tracked);
                            }
                        }
                        if count == 0 {
                            return outcome;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return outcome,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        return Outcome {
                            changed: outcome.changed,
                            complete: false,
                        };
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{InputSnapshot, Outcome, Path};
    pub(in crate::managed) struct Observer {
        ready: bool,
    }
    impl Observer {
        pub(in crate::managed) fn begin(_: &InputSnapshot, _: &Path) -> Self {
            Self { ready: true }
        }
        pub(in crate::managed) fn ready(&self) -> bool {
            self.ready
        }
        pub(in crate::managed) fn finish(self) -> Outcome {
            Outcome {
                changed: false,
                complete: self.ready,
            }
        }
    }
}

pub(super) use platform::Observer;

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    fn fixture(name: &str) -> (PathBuf, InputSnapshot) {
        let dir = std::env::temp_dir().join(format!("sail-absent-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(dir.join("cargo-home")).is_ok());
        let root = dir.join("cargo-home/config.toml");
        (
            dir.clone(),
            InputSnapshot {
                workspace: dir,
                revision: String::new(),
                roots: vec![root],
                excluded: vec![],
                complete: true,
                observation_paths: std::collections::BTreeSet::new(),
            },
        )
    }
    #[test]
    fn transient_config_creation_is_drained_without_a_sleep() {
        let (dir, snapshot) = fixture("transient");
        let observer = Observer::begin(&snapshot, &dir.join("markers"));
        assert!(observer.ready());
        assert!(std::fs::write(&snapshot.roots[0], "[build]\njobs=1\n").is_ok());
        assert!(std::fs::remove_file(&snapshot.roots[0]).is_ok());
        let result = observer.finish();
        assert!(result.complete);
        assert!(result.changed);
        assert!(std::fs::remove_dir_all(dir).is_ok());
    }
    #[test]
    fn cargo_cache_journal_does_not_change_absent_config() {
        let (dir, snapshot) = fixture("journal");
        let observer = Observer::begin(&snapshot, &dir.join("markers"));
        assert!(observer.ready());
        let journal = dir.join("cargo-home/.global-cache-journal");
        assert!(std::fs::write(&journal, "journal").is_ok());
        assert!(std::fs::remove_file(journal).is_ok());
        let result = observer.finish();
        assert!(result.complete);
        assert!(!result.changed);
        assert!(std::fs::remove_dir_all(dir).is_ok());
    }
    #[test]
    fn kernel_queue_overflow_prevents_complete_observation() {
        let (dir, snapshot) = fixture("overflow");
        let observer = Observer::begin(&snapshot, &dir.join("markers"));
        assert!(observer.ready());
        let capacity = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(16384);
        // Each distinct creation occupies a queue entry; reading only afterwards
        // injects a genuine kernel overflow rather than mirroring a bitmask check.
        assert!(
            capacity <= 65536,
            "test requires bounded default kernel queue"
        );
        for n in 0..=capacity {
            assert!(std::fs::write(dir.join(format!("cargo-home/journal-{n}")), []).is_ok());
        }
        assert!(!observer.finish().complete);
        assert!(std::fs::remove_dir_all(dir).is_ok());
    }
    #[test]
    fn symlinked_missing_config_is_matched_by_canonical_event_path() {
        let (dir, snapshot) = fixture("alias");
        let alias = dir.join("alias");
        assert!(std::os::unix::fs::symlink(dir.join("cargo-home"), &alias).is_ok());
        let mut snapshot = snapshot;
        snapshot.roots = vec![alias.join("config.toml")];
        let observer = Observer::begin(&snapshot, &dir.join("markers"));
        assert!(observer.ready());
        assert!(std::fs::write(&snapshot.roots[0], "config").is_ok());
        assert!(std::fs::remove_file(&snapshot.roots[0]).is_ok());
        let outcome = observer.finish();
        assert!(outcome.complete && outcome.changed);
        assert!(std::fs::remove_dir_all(dir).is_ok());
    }
    #[test]
    fn missing_ancestor_creation_and_removal_is_tracked() {
        let (dir, mut snapshot) = fixture("ancestor");
        snapshot.roots = vec![dir.join("cargo-home/missing/config.toml")];
        let observer = Observer::begin(&snapshot, &dir.join("markers"));
        assert!(observer.ready());
        assert!(std::fs::create_dir_all(dir.join("cargo-home/missing")).is_ok());
        assert!(std::fs::write(&snapshot.roots[0], "x").is_ok());
        assert!(std::fs::remove_dir_all(dir.join("cargo-home/missing")).is_ok());
        let result = observer.finish();
        assert!(result.complete);
        assert!(result.changed);
        assert!(std::fs::remove_dir_all(dir).is_ok());
    }
}
