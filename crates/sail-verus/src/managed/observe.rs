//! Input mutation observation catches edits restored before final hashing.

use super::input::InputSnapshot;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Observation {
    _watcher: Option<RecommendedWatcher>,
    changed: Arc<AtomicBool>,
    complete: bool,
    lost: Arc<AtomicBool>,
}

impl Observation {
    pub(super) fn begin(snapshot: &InputSnapshot, extra: &[PathBuf], cache: &Path) -> Self {
        let changed = Arc::new(AtomicBool::new(false));
        let lost = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&changed);
        let failed = Arc::clone(&lost);
        let excluded = cache.canonicalize().unwrap_or_else(|_| cache.to_owned());
        let callback = move |event: notify::Result<notify::Event>| match event {
            Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                if event.paths.iter().any(|p| {
                    !p.starts_with(&excluded)
                        && !p
                            .components()
                            .any(|c| c.as_os_str() == "target" || c.as_os_str() == ".git")
                }) {
                    signal.store(true, Ordering::Release);
                }
            }
            Ok(_) => {}
            Err(_) => failed.store(true, Ordering::Release),
        };
        let Ok(mut watcher) = notify::recommended_watcher(callback) else {
            return Self {
                _watcher: None,
                changed,
                complete: false,
                lost,
            };
        };
        let mut complete = true;
        for root in snapshot.roots.iter().chain(extra) {
            let path = if root.is_file() {
                root.parent().unwrap_or(root)
            } else {
                root.as_path()
            };
            complete &= watcher.watch(path, RecursiveMode::Recursive).is_ok();
        }
        // Native observation errors prevent reuse even if all final bytes match.
        if lost.load(Ordering::Acquire) {
            complete = false;
        }
        Self {
            _watcher: Some(watcher),
            changed,
            complete,
            lost,
        }
    }
    pub(super) fn changed(&self) -> bool {
        self.changed.load(Ordering::Acquire)
    }
    pub(super) fn complete(&self) -> bool {
        self.complete && !self.lost.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_restored_edit_is_observed_even_when_final_contents_match() {
        let root =
            std::env::temp_dir().join(format!("sail-observation-test-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let source = root.join("lib.rs");
        assert!(std::fs::write(&source, "original").is_ok());
        let snapshot = InputSnapshot {
            workspace: root.clone(),
            revision: String::new(),
            roots: vec![root.clone()],
            complete: true,
        };
        let observation = Observation::begin(&snapshot, &[], &root.join("cache"));
        assert!(observation.complete());
        assert!(std::fs::write(&source, "temporary").is_ok());
        assert!(std::fs::write(&source, "original").is_ok());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        while !observation.changed() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            observation.changed(),
            "restored input change must invalidate establishing work"
        );
        drop(observation);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
