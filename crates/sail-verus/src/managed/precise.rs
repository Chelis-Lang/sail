//! Linux binding and transient-change observation with a synchronous queue
//! barrier. This is not a content oracle: existing mmap writes are invisible.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod platform {
    use super::{Path, PathBuf};
    use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::ops::Bound::{Included, Unbounded};
    use std::os::unix::fs::MetadataExt as _;
    use std::time::{Duration, Instant};

    const MAX_WATCHES: usize = 8192;
    const MAX_EVENTS: usize = 32768;

    struct Construction {
        left: usize,
        visits: usize,
        stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        started: Instant,
    }
    impl Construction {
        fn charge(&mut self, bytes: usize) -> Option<()> {
            if self
                .stop
                .as_ref()
                .is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire))
                || self.started.elapsed() >= Duration::from_secs(120)
            {
                return None;
            }
            self.left = self.left.checked_sub(bytes)?;
            Some(())
        }
        fn path(&mut self, path: &PathBuf, inline: usize) -> Option<()> {
            (path.capacity() <= 8192).then_some(())?;
            self.charge(inline.checked_add(path.capacity())?)
        }
    }

    #[cfg(test)]
    mod construction_tests {
        use super::*;
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };

        #[test]
        fn original_clock_and_shared_stop_apply_during_helper_observation() {
            let stop = Arc::new(AtomicBool::new(false));
            let mut control = Construction {
                left: 16 * 1024 * 1024,
                visits: 0,
                stop: Some(Arc::clone(&stop)),
                started: Instant::now(),
            };
            assert!(control.charge(1024).is_some());
            stop.store(true, Ordering::Release);
            assert!(control.charge(0).is_none());
            assert!(
                Observer::begin_bounded_with(
                    &[PathBuf::from("/tmp")],
                    &[],
                    Some(stop),
                    control.started,
                )
                .is_none()
            );
            let start = Instant::now()
                .checked_sub(Duration::from_secs(121))
                .unwrap_or_else(|| unreachable!("monotonic fixture"));
            assert!(
                Observer::begin_bounded_with(
                    &[PathBuf::from("/tmp")],
                    &[],
                    Some(Arc::new(AtomicBool::new(false))),
                    start,
                )
                .is_none(),
                "a nested helper phase cannot renew the construction clock"
            );
        }
    }

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

    fn tracked_ancestors<'a>(
        tracked: &'a BTreeSet<PathBuf>,
        path: &'a Path,
    ) -> impl Iterator<Item = &'a Path> {
        // starts_with(empty) is true even for absolute paths, whose ancestors
        // omit the empty path. Retain that original predicate explicitly.
        path.ancestors()
            .chain(std::iter::once(Path::new("")))
            .filter(|ancestor| tracked.contains(*ancestor))
    }
    fn related(tracked: &BTreeSet<PathBuf>, path: &Path) -> bool {
        // Path Ord and starts_with both compare components. Descendants share
        // a component prefix and therefore form a contiguous sorted range
        // beginning at path; the first lower-bound result is sufficient.
        tracked_ancestors(tracked, path).next().is_some()
            || tracked
                .range::<Path, _>((Included(path), Unbounded))
                .next()
                .is_some_and(|root| root.starts_with(path))
    }
    fn ignored_path(tracked: &BTreeSet<PathBuf>, excluded: &[PathBuf], path: &Path) -> bool {
        excluded.iter().any(|p| path.starts_with(p))
            && !tracked_ancestors(tracked, path)
                .any(|root| excluded.iter().any(|p| root.starts_with(p)))
    }
    pub(super) struct Observer {
        native: Inotify,
        directories: HashMap<WatchDescriptor, PathBuf>,
        canonical_watches: BTreeMap<PathBuf, usize>,
        tracked: BTreeSet<PathBuf>,
        excluded: Vec<PathBuf>,
        complete: bool,
        changed: bool,
        failure: Option<&'static str>,
        mounts: super::super::kernel_mounts::Guard,
        bindings: Vec<(PathBuf, Option<(u64, u64)>)>,
        construction: Option<Construction>,
    }
    impl Observer {
        pub(super) fn retained_bytes(&self) -> Option<usize> {
            let Self {
                native: _,
                directories,
                canonical_watches,
                tracked,
                excluded,
                complete: _,
                changed: _,
                failure: _,
                mounts,
                bindings,
                construction,
            } = self;
            // inotify0.11.5 owns one Arc<FdGuard> (two atomic counters, fd and
            // close flag). WatchDescriptor only holds a Weak to that same Arc.
            let used = std::mem::size_of::<Self>()
                .checked_add(64)?
                .checked_add(if construction.as_ref().is_some_and(|c| c.stop.is_some()) {
                    64
                } else {
                    0
                })?
                .checked_add(super::super::retained::hash::<(WatchDescriptor, PathBuf)>(
                    directories.capacity(),
                )?)?
                .checked_add(super::super::retained::tree(canonical_watches.len())?)?
                .checked_add(super::super::retained::tree(tracked.len())?)?
                .checked_add(super::super::retained::vector(excluded)?)?
                .checked_add(super::super::retained::vector(bindings)?)?
                .checked_add(mounts.retained_bytes()?)?;
            directories
                .values()
                .chain(canonical_watches.keys())
                .chain(tracked)
                .chain(excluded)
                .chain(bindings.iter().map(|(path, _)| path))
                .try_fold(used, |used, path| used.checked_add(path.capacity()))
        }

        pub(super) fn begin(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
            Self::begin_with(roots, excluded, None, false)
        }
        pub(super) fn begin_bounded(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
            Self::begin_bounded_with(roots, excluded, None, Instant::now())
        }
        pub(super) fn begin_bounded_with(
            roots: &[PathBuf],
            excluded: &[PathBuf],
            stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
            started: Instant,
        ) -> Option<Self> {
            Self::begin_with(
                roots,
                excluded,
                Some(Construction {
                    left: 16 * 1024 * 1024,
                    visits: 0,
                    stop,
                    started,
                }),
                false,
            )
        }
        pub(super) fn begin_files_bounded_with(
            roots: &[PathBuf],
            stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
            started: Instant,
        ) -> Option<Self> {
            Self::begin_with(
                roots,
                &[],
                Some(Construction {
                    left: 16 * 1024 * 1024,
                    visits: 0,
                    stop: Some(stop),
                    started,
                }),
                true,
            )
        }
        fn begin_with(
            roots: &[PathBuf],
            excluded: &[PathBuf],
            mut construction: Option<Construction>,
            files_only: bool,
        ) -> Option<Self> {
            let mounts = super::super::kernel_mounts::Guard::begin()?;
            if let Some(budget) = construction.as_mut() {
                (roots.len() <= 4096 && excluded.len() <= 4096).then_some(())?;
                budget.charge(1024 * 1024 + mounts.retained_bytes()?)?;
                for path in roots.iter().chain(excluded) {
                    budget.path(path, 1024)?;
                }
            }
            let mut observer = Self {
                native: Inotify::init().ok()?,
                directories: HashMap::new(),
                canonical_watches: BTreeMap::new(),
                tracked: roots.iter().cloned().collect(),
                excluded: excluded.to_vec(),
                complete: true,
                changed: false,
                failure: None,
                mounts,
                bindings: Vec::new(),
                construction,
            };
            let mut visited = BTreeSet::new();
            for root in roots {
                observer.binding(root)?;
                if files_only {
                    // Exact output/helper files need their bindings and parent
                    // watches, never a recursive directory snapshot. A raced
                    // directory replacement declines instead of entering walk.
                    match root.canonicalize() {
                        Ok(path) => {
                            path.is_file().then_some(())?;
                            observer.watch(&path)?;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => return None,
                    }
                } else {
                    observer.walk(root, &mut visited, true)?;
                }
            }
            for path in roots {
                if let Some(budget) = observer.construction.as_mut() {
                    budget.path(path, 128)?;
                }
                observer.bindings.push((path.clone(), identity(path).ok()?));
            }
            Some(observer)
        }
        fn watch(&mut self, directory: &Path) -> Option<()> {
            let canonical = directory.canonicalize().ok()?;
            if !local_filesystem(&canonical, self.mounts.snapshot()) {
                return None;
            }
            if self.canonical_watches.contains_key(&canonical) {
                return Some(());
            }
            if self.directories.len() >= MAX_WATCHES {
                return None;
            }
            if let Some(budget) = self.construction.as_mut() {
                budget.path(&canonical, 2048)?;
                budget.path(&canonical, 0)?;
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
            // Inotify may reuse a descriptor for a different hardlink spelling.
            // Match the descriptor map's current values, rather than retaining
            // every canonical path ever supplied to add(). Counts preserve the
            // exact membership invariant even if multiple descriptors share one.
            if let Some(previous) = self.directories.insert(descriptor, canonical.clone()) {
                let count = self.canonical_watches.get_mut(&previous)?;
                *count = count.checked_sub(1)?;
                if *count == 0 {
                    self.canonical_watches.remove(&previous);
                }
            }
            let count = self.canonical_watches.entry(canonical).or_default();
            *count = count.checked_add(1)?;
            Some(())
        }
        fn track(&mut self, path: PathBuf) -> Option<()> {
            if !self.tracked.contains(&path)
                && let Some(budget) = self.construction.as_mut()
            {
                (self.tracked.len() < 8192).then_some(())?;
                budget.path(&path, 1024)?;
            }
            self.tracked.insert(path);
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
                    self.track(target.clone())?;
                    self.binding_depth(&target, depth + 1)?;
                }
                Ok(_) => {
                    if let Ok(canonical) = path.canonicalize() {
                        if !local_filesystem(&canonical, self.mounts.snapshot()) {
                            return None;
                        }
                        self.track(canonical)?;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let parent = path.ancestors().skip(1).find(|p| p.is_dir())?;
                    self.track(
                        parent
                            .canonicalize()
                            .ok()?
                            .join(path.strip_prefix(parent).ok()?),
                    )?;
                }
                Err(_) => return None,
            }
            Some(())
        }
        fn ignored(&self, path: &Path) -> bool {
            ignored_path(&self.tracked, &self.excluded, path)
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
            if let Some(budget) = self.construction.as_mut() {
                budget.charge(0)?;
                budget.visits = budget.visits.checked_add(1)?;
                (budget.visits <= 16384).then_some(())?;
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
            if !visited.contains(&canonical)
                && let Some(budget) = self.construction.as_mut()
            {
                budget.path(&canonical, 1024)?;
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
            if !self.mounts.clean() {
                self.complete = false;
                self.failure
                    .get_or_insert("namespace-mount-inventory-changed");
                return false;
            }
            if self
                .bindings
                .iter()
                .any(|(p, old)| identity(p).as_ref().ok() != Some(old))
            {
                self.complete = false;
                self.failure.get_or_insert("namespace-root-binding-changed");
                return false;
            }
            let deadline = Instant::now() + Duration::from_millis(20);
            let mut buffer = vec![0; 65536];
            let mut count = 0;
            loop {
                if count >= MAX_EVENTS {
                    self.failure
                        .get_or_insert("namespace-event-count-exhausted");
                    self.complete = false;
                    return false;
                }
                match self.native.read_events(&mut buffer) {
                    Ok(events) => {
                        // Always attempt the nonblocking read: scheduler delay
                        // cannot turn an already empty native queue into lost
                        // coverage. Bound actual nonempty drain work instead.
                        if Instant::now() >= deadline {
                            self.failure
                                .get_or_insert("namespace-event-drain-budget-exhausted");
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
                                self.failure
                                    .get_or_insert("namespace-native-watch-coverage-lost");
                                self.complete = false;
                            }
                            let Some(parent) = self.directories.get(&event.wd) else {
                                self.failure.get_or_insert("namespace-event-watch-unbound");
                                self.complete = false;
                                continue;
                            };
                            let path = event
                                .name
                                .map_or_else(|| parent.clone(), |name| parent.join(name));
                            if !self.ignored(&path) && related(&self.tracked, &path) {
                                self.failure.get_or_insert("namespace-tracked-input-event");
                                self.changed = true;
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return self.complete && !self.changed;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        self.failure.get_or_insert("namespace-event-read-failed");
                        self.complete = false;
                        return false;
                    }
                }
            }
        }
        pub(super) fn failure_reason(&self) -> &'static str {
            self.failure.unwrap_or("namespace-coverage-unavailable")
        }
    }
    #[cfg(test)]
    mod predicate_tests {
        use super::*;
        fn linear_related(tracked: &BTreeSet<PathBuf>, path: &Path) -> bool {
            tracked
                .iter()
                .any(|root| path.starts_with(root) || root.starts_with(path))
        }
        fn linear_ignored(tracked: &BTreeSet<PathBuf>, excluded: &[PathBuf], path: &Path) -> bool {
            excluded.iter().any(|p| path.starts_with(p))
                && !tracked.iter().any(|root| {
                    excluded.iter().any(|p| root.starts_with(p)) && path.starts_with(root)
                })
        }
        #[test]
        fn hardlink_descriptor_replacement_preserves_current_canonical_membership() {
            let root = std::env::temp_dir().join(format!(
                "sail-precise-canonical-index-{}",
                std::process::id()
            ));
            assert!(std::fs::create_dir_all(root.join("source")).is_ok());
            let first = root.join("source/first");
            let second = root.join("source/second");
            assert!(std::fs::write(&first, b"input").is_ok());
            assert!(std::fs::hard_link(&first, &second).is_ok());
            let source = root.join("source");
            let mut observer = Observer::begin(std::slice::from_ref(&source), &[])
                .unwrap_or_else(|| unreachable!("fixture"));
            for path in [&first, &second, &first, &second, &second] {
                assert!(observer.watch(path).is_some());
                let mut original = BTreeMap::<PathBuf, usize>::new();
                for current in observer.directories.values() {
                    *original.entry(current.clone()).or_default() += 1;
                }
                assert_eq!(observer.canonical_watches, original);
                assert!(observer.canonical_watches.contains_key(path));
                let other = if path == &first { &second } else { &first };
                assert!(!observer.canonical_watches.contains_key(other));
            }
            assert!(observer.clean());
            assert!(std::fs::write(&first, b"changed").is_ok());
            assert!(!observer.clean());
            drop(observer);
            assert!(std::fs::remove_dir_all(root).is_ok());
        }
        #[test]
        fn all_component_boundaries_and_missing_ancestor_relations_are_equal() {
            use std::os::unix::ffi::OsStringExt;
            let mut paths: Vec<PathBuf> = [
                "/",
                "///",
                "/tmp",
                "/tmp/source",
                "/tmp/source/",
                "/tmp/./source",
                "/tmp/source/a",
                "//tmp//source//a",
                "/tmp/source/a-b",
                "/tmp/source/ab",
                "/tmp/source/../sibling",
                "/tmp/source/target/include.rs",
                "/tmp/source/.cargo/config",
                "/tmp/source2",
                "",
                ".",
                "./",
                "..",
                "../source",
                "source",
                "source/a",
                "source/a-b",
                "source/ab",
            ]
            .into_iter()
            .map(PathBuf::from)
            .collect();
            paths.push(PathBuf::from(std::ffi::OsString::from_vec(
                b"/tmp/source/\xff/file".to_vec(),
            )));
            let mut state = 21_u64;
            for trial in 0..3000 {
                let mut tracked = BTreeSet::new();
                let mut excluded = Vec::new();
                for path in &paths {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1);
                    if state >> 63 != 0 {
                        tracked.insert(path.clone());
                    }
                    if state.is_multiple_of(11) {
                        excluded.push(path.clone());
                    }
                }
                for path in &paths {
                    assert_eq!(
                        linear_related(&tracked, path),
                        related(&tracked, path),
                        "relation trial={trial} path={path:?} tracked={tracked:?}"
                    );
                    assert_eq!(
                        linear_ignored(&tracked, &excluded, path),
                        ignored_path(&tracked, &excluded, path),
                        "exclusion trial={trial} path={path:?} tracked={tracked:?} excluded={excluded:?}"
                    );
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
    /// Checked requested-storage charge; excludes native/kernel resources.
    #[allow(
        dead_code,
        reason = "Used by forthcoming bounded daemon sysroot admission"
    )]
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        self.0.retained_bytes()
    }
    pub(super) fn begin(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
        platform::Observer::begin(roots, excluded).map(Self)
    }
    #[allow(
        dead_code,
        reason = "Bounded construction is consumed by forthcoming sysroot daemon owners"
    )]
    pub(super) fn begin_bounded(roots: &[PathBuf], excluded: &[PathBuf]) -> Option<Self> {
        platform::Observer::begin_bounded(roots, excluded).map(Self)
    }
    pub(super) fn begin_bounded_with(
        roots: &[PathBuf],
        excluded: &[PathBuf],
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        started: std::time::Instant,
    ) -> Option<Self> {
        platform::Observer::begin_bounded_with(roots, excluded, Some(stop), started).map(Self)
    }
    pub(super) fn begin_files_bounded_with(
        roots: &[PathBuf],
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        started: std::time::Instant,
    ) -> Option<Self> {
        platform::Observer::begin_files_bounded_with(roots, stop, started).map(Self)
    }
    pub(super) fn clean(&mut self) -> bool {
        self.0.clean()
    }
    pub(super) fn files(&self) -> Vec<PathBuf> {
        self.0.files()
    }
    pub(super) fn failure_reason(&self) -> &'static str {
        self.0.failure_reason()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_file_construction_rejects_directory_instead_of_recursing() {
        let root =
            std::env::temp_dir().join(format!("sail-observe-file-only-{}", std::process::id()));
        assert!(std::fs::create_dir(&root).is_ok());
        let outcome = Observer::begin_files_bounded_with(
            std::slice::from_ref(&root),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            std::time::Instant::now(),
        );
        assert!(std::fs::remove_dir(root).is_ok());
        assert!(outcome.is_none());
    }
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

    fn generated_source_fixture(name: &str) -> (PathBuf, serde_json::Value, PathBuf) {
        let root = fixture(name);
        let workspace = root.join("source");
        let target = root.join("family-target");
        let generated = target.join("debug/build/serde_core-unit/out/private.rs");
        assert!(
            std::fs::write(
                workspace.join("Cargo.toml"),
                "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\n[lib]\npath='lib.rs'\n"
            )
            .is_ok()
        );
        assert!(
            std::fs::write(
                workspace.join("lib.rs"),
                "include!(\"../family-target/debug/build/serde_core-unit/out/private.rs\");\n"
            )
            .is_ok()
        );
        assert!(std::fs::create_dir_all(generated.parent().unwrap_or(&target)).is_ok());
        assert!(std::fs::write(&generated, "pub const GENERATED: bool = true;\n").is_ok());
        let data = serde_json::json!({
            "workspace_root": workspace,
            "target_directory": target,
            "packages": [{ "manifest_path": workspace.join("Cargo.toml") }],
            "sail_source_inputs": [&generated],
        });
        (root, data, generated)
    }

    #[test]
    fn unchanged_generated_source_remains_guarded_inside_excluded_target() {
        let (root, data, generated) = generated_source_fixture("generated-unchanged");
        let before = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(before.complete);
        assert!(before.roots.contains(&generated));
        assert!(
            before
                .excluded
                .iter()
                .any(|excluded| generated.starts_with(excluded))
        );
        let mut observer = Observer::begin(&before.roots, &before.excluded)
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.files().contains(&generated));
        assert!(observer.clean());
        let monitored = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(monitored.complete);
        assert_eq!(before.revision, monitored.revision);
        assert!(observer.clean());
        drop(observer);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn generated_source_clean_before_watch_then_recreation_refuses_root_binding() {
        let (root, data, generated) = generated_source_fixture("generated-preclean");
        let before = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(before.complete);
        assert!(before.roots.contains(&generated));
        // Model the runner's initial snapshot, stale-script clean, then original
        // guard acquisition. This is an owned fixture mutation, not Cargo/proof.
        assert!(std::fs::remove_file(&generated).is_ok());
        let mut observer = Observer::begin(&before.roots, &before.excluded)
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        let monitored = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(!monitored.complete);
        assert!(monitored.roots.contains(&generated));
        assert!(std::fs::write(&generated, "pub const GENERATED: bool = true;\n").is_ok());
        assert!(!observer.clean());
        assert_eq!(observer.failure_reason(), "namespace-root-binding-changed");
        assert!(!observer.clean());
        drop(observer);
        // A new original acquisition can observe the new output. This does not
        // authorize the earlier proof or bypass family-generation admission.
        let after = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(after.complete);
        assert_eq!(before.revision, after.revision);
        let mut fresh = Observer::begin(&after.roots, &after.excluded)
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(fresh.clean());
        drop(fresh);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn generated_source_replaced_after_watch_refuses_even_with_equal_content() {
        use std::os::unix::fs::MetadataExt as _;

        let (root, data, generated) = generated_source_fixture("generated-replaced");
        let before = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(before.complete);
        assert!(before.roots.contains(&generated));
        let mut observer = Observer::begin(&before.roots, &before.excluded)
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(observer.clean());
        let original =
            std::fs::metadata(&generated).unwrap_or_else(|error| unreachable!("fixture: {error}"));
        // Retain the old inode under another name so the replacement identity
        // cannot accidentally equal it through immediate inode reuse.
        let retired = generated.with_extension("retired");
        assert!(std::fs::rename(&generated, &retired).is_ok());
        assert!(std::fs::write(&generated, "pub const GENERATED: bool = true;\n").is_ok());
        let replacement =
            std::fs::metadata(&generated).unwrap_or_else(|error| unreachable!("fixture: {error}"));
        assert_ne!(
            (original.dev(), original.ino()),
            (replacement.dev(), replacement.ino())
        );
        assert!(!observer.clean());
        assert_eq!(observer.failure_reason(), "namespace-root-binding-changed");
        drop(observer);
        assert!(std::fs::remove_file(retired).is_ok());
        let after = super::super::input::snapshot_metadata_home(&data, &[], None);
        assert!(after.complete);
        assert_eq!(before.revision, after.revision);
        assert!(std::fs::remove_dir_all(root).is_ok());
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
