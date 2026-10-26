//! Native notifications, an always-on metadata backstop for dropped events,
//! and a content-based fallback for unavailable events.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};

const SCAN_INTERVAL: Duration = Duration::from_secs(5);
/// Interval of the metadata backstop that runs beside native notifications.
/// macOS can drop a watcher's events and report the drop late or never, so
/// native delivery alone cannot bound how long an edit goes unseen.
const BACKSTOP_INTERVAL: Duration = Duration::from_secs(2);

/// The inputs as a run observed them, or as last reported changed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Snapshot {
    /// Path list and file metadata only; cheap enough to poll.
    stamp: [u8; 32],
    /// Path list and file bytes.
    contents: [u8; 32],
    /// Bytes plus file identity, including change times on Unix.
    epoch: [u8; 32],
}
impl Snapshot {
    fn take(roots: &[PathBuf], excluded: &[PathBuf]) -> std::io::Result<Self> {
        // Stamp first: an edit between the two walks then differs from the
        // stamp later, so the backstop cannot adopt it unseen.
        let stamp = stamp(roots, excluded)?;
        let (contents, epoch) = capture(roots, excluded)?;
        Ok(Self {
            stamp,
            contents,
            epoch,
        })
    }
    /// Whether two snapshots are the same input state. Without Unix change
    /// times file identity is no evidence, so only bytes count there.
    fn same(&self, other: &Self) -> bool {
        if cfg!(unix) {
            self.epoch == other.epoch
        } else {
            self.contents == other.contents
        }
    }
}

pub(super) struct Monitoring {
    native: Option<RecommendedWatcher>,
    changes: Receiver<notify::Result<Event>>,
    fallback: bool,
    scanned: Instant,
    polled: Instant,
    /// The input state the current run observed, captured just before it was
    /// submitted. A native event whose inputs still match it is a late or
    /// repeated notification for a change that run already includes.
    observed: Option<Snapshot>,
    /// `observed`, or the state last reported as an edit since then. Polls
    /// and scans compare against it, so they never report a change twice.
    seen: Option<Snapshot>,
    /// The coverage `observed` and `seen` were captured over.
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    /// An edit found while replacing the observed coverage, not yet reported.
    missed: bool,
    /// The backstop could not stamp the inputs; reported once per streak.
    stamp_failed: bool,
}
impl Monitoring {
    pub(super) fn new(err: &mut dyn Write) -> Self {
        let (sender, changes) = channel();
        let native = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        });
        let mut monitoring = Self::detached(native.ok(), changes, false);
        if monitoring.native.is_none() {
            monitoring.degrade(err);
        }
        monitoring
    }
    fn detached(
        native: Option<RecommendedWatcher>,
        changes: Receiver<notify::Result<Event>>,
        fallback: bool,
    ) -> Self {
        Self {
            native,
            changes,
            fallback,
            scanned: Instant::now(),
            polled: Instant::now(),
            observed: None,
            seen: None,
            roots: vec![],
            excluded: vec![],
            missed: false,
            stamp_failed: false,
        }
    }
    fn degrade(&mut self, err: &mut dyn Write) {
        if !self.fallback {
            let _ = writeln!(
                err,
                "sail watch: filesystem events unavailable or incomplete; scanning known input contents every 5 seconds."
            );
        }
        self.fallback = true;
        self.native = None;
    }
    pub(super) fn subscribe(
        &mut self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        subscriptions: &mut BTreeMap<PathBuf, bool>,
        err: &mut dyn Write,
    ) {
        let desired = super::desired_subscriptions(roots);
        let mut failed = false;
        if let Some(native) = &mut self.native {
            for (path, recursive) in subscriptions.iter() {
                if desired.get(path) != Some(recursive) {
                    let _ = native.unwatch(path);
                }
            }
            for (path, recursive) in &desired {
                if subscriptions.get(path) != Some(recursive)
                    && native
                        .watch(
                            path,
                            if *recursive {
                                RecursiveMode::Recursive
                            } else {
                                RecursiveMode::NonRecursive
                            },
                        )
                        .is_err()
                {
                    failed = true;
                }
            }
        }
        if failed {
            self.degrade(err);
        }
        *subscriptions = desired;
        // Capture only when input coverage changes, never every refresh: a
        // recapture would adopt an edit that no notification, poll or scan
        // has reported yet as already observed. Before replacing the
        // coverage, report any change since the observation.
        if self.roots == roots && self.excluded == excluded {
            return;
        }
        if !self.roots.is_empty() {
            let current = Snapshot::take(&self.roots, &self.excluded).ok();
            self.missed |=
                !matches!((current, self.seen), (Some(now), Some(seen)) if now.same(&seen));
        }
        self.roots = roots.to_vec();
        self.excluded = excluded.to_vec();
        self.record();
    }
    /// Records the complete input state a run is about to observe. Called
    /// immediately before submission, after the debounce and input refresh.
    pub(super) fn observe(&mut self) {
        self.missed = false;
        self.record();
    }
    fn record(&mut self) {
        self.observed = Snapshot::take(&self.roots, &self.excluded).ok();
        self.seen = self.observed;
        self.scanned = Instant::now();
        self.polled = Instant::now();
    }
    /// Takes a fresh snapshot and reports whether it differs from
    /// `baseline`, adopting it as `seen` so polls never repeat the change.
    /// Unreadable state counts as a change.
    fn refresh(&mut self, baseline: Option<Snapshot>) -> bool {
        match Snapshot::take(&self.roots, &self.excluded) {
            Ok(now) => {
                self.seen = Some(now);
                baseline.is_none_or(|baseline| !now.same(&baseline))
            }
            Err(_) => true,
        }
    }
    pub(super) fn changed(&mut self, err: &mut dyn Write) -> bool {
        let mut edited = std::mem::take(&mut self.missed);
        let mut native = false;
        let mut replayable = true;
        while let Ok(event) = self.changes.try_recv() {
            match event {
                Ok(_) if self.fallback => {}
                Ok(event) if !event.need_rescan() => {
                    let removal_or_rename = matches!(
                        event.kind,
                        notify::EventKind::Remove(_)
                            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                    );
                    let (sender, receiver) = channel();
                    let _ = sender.send(Ok(event));
                    let relevant = super::changed(&receiver, &self.roots, &self.excluded);
                    // Ancestor subscriptions also deliver sibling events. Only
                    // an input removal/rename is always treated as an edit:
                    // moving a directory away and back keeps every identity
                    // the snapshot records.
                    replayable &= !relevant || !removal_or_rename;
                    native |= relevant;
                }
                _ => self.degrade(err),
            }
        }
        // Once coverage is incomplete, use one observation channel. Mixing
        // delayed native events with scans can invalidate the same edit twice.
        // FSEvents may deliver notifications late or repeat them (including
        // creation events from before subscription). A batch restarts only
        // if the complete input state, including inode and change times so
        // that edit-and-restore counts, differs from what the current run
        // observed. Unreadable state remains conservative, and without Unix
        // change times native events stay unconditional.
        if native && !self.fallback {
            let differs = self.refresh(self.observed);
            edited |= differs || !replayable || !cfg!(unix);
        }
        // The backstop runs in every mode. It compares metadata stamps and
        // rehashes only when one changes, so an edit whose notification is
        // dropped is still seen within one interval.
        if self.polled.elapsed() >= BACKSTOP_INTERVAL {
            self.polled = Instant::now();
            match stamp(&self.roots, &self.excluded) {
                Ok(now) => {
                    self.stamp_failed = false;
                    if self.seen.is_none_or(|seen| seen.stamp != now) {
                        edited |= self.refresh(self.seen);
                    }
                }
                Err(error) => {
                    if !std::mem::replace(&mut self.stamp_failed, true) {
                        let _ = writeln!(err, "sail watch: input check incomplete: {error}");
                    }
                }
            }
        }
        if self.fallback && self.scanned.elapsed() >= SCAN_INTERVAL {
            self.scanned = Instant::now();
            match Snapshot::take(&self.roots, &self.excluded) {
                Ok(now) => {
                    edited |= self.seen.is_none_or(|seen| seen.contents != now.contents);
                    self.seen = Some(now);
                }
                Err(error) => {
                    let _ = writeln!(err, "sail watch: content scan incomplete: {error}");
                }
            }
        }
        edited
    }
}

#[cfg(test)]
fn fingerprint(roots: &[PathBuf], excluded: &[PathBuf]) -> std::io::Result<[u8; 32]> {
    capture(roots, excluded).map(|(contents, _)| contents)
}

/// Hashes the input path list and each entry's metadata without reading any
/// file. On Unix every write changes the change time, so an edit always
/// changes the stamp; elsewhere size and modification time stand in, and the
/// content scan remains the check for scan mode.
fn stamp(roots: &[PathBuf], excluded: &[PathBuf]) -> std::io::Result<[u8; 32]> {
    let mut entries = Vec::new();
    for root in roots {
        let declared_output = excluded.iter().any(|output| root.starts_with(output));
        let metadata = std::fs::symlink_metadata(root).ok();
        stamp_entries(root, metadata, excluded, declared_output, &mut entries)?;
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries.dedup_by(|left, right| left.0 == right.0);
    let mut hash = Sha256::new();
    for (path, link) in entries {
        let bytes = path.as_os_str().as_encoded_bytes();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        let Some(link) = link else {
            hash.update(b"missing");
            continue;
        };
        let target = if link.file_type().is_symlink() {
            let bytes = std::fs::read_link(&path)?;
            let bytes = bytes.as_os_str().as_encoded_bytes();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
            std::fs::metadata(&path).ok()
        } else {
            None
        };
        for metadata in std::iter::once(&link).chain(target.as_ref()) {
            hash.update([u8::from(metadata.is_dir())]);
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                hash.update(metadata.dev().to_le_bytes());
                hash.update(metadata.ino().to_le_bytes());
                hash.update(metadata.mode().to_le_bytes());
            }
            // Directory times also change for writes inside excluded output
            // subtrees, so only file times count.
            if metadata.is_dir() {
                continue;
            }
            hash.update(metadata.len().to_le_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                hash.update(metadata.mtime().to_le_bytes());
                hash.update(metadata.mtime_nsec().to_le_bytes());
                hash.update(metadata.ctime().to_le_bytes());
                hash.update(metadata.ctime_nsec().to_le_bytes());
            }
            #[cfg(not(unix))]
            if let Ok(since) = metadata.modified().and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)
            }) {
                hash.update(since.as_nanos().to_le_bytes());
            }
        }
    }
    Ok(hash.finalize().into())
}

fn capture(roots: &[PathBuf], excluded: &[PathBuf]) -> std::io::Result<([u8; 32], [u8; 32])> {
    let mut files = Vec::new();
    for root in roots {
        let declared_output = excluded.iter().any(|output| root.starts_with(output));
        collect(root, excluded, declared_output, &mut files)?;
    }
    files.sort();
    files.dedup();
    let mut hash = Sha256::new();
    let mut epoch = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    for path in files {
        let bytes = path.as_os_str().as_encoded_bytes();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        if !path.exists() {
            hash.update(b"missing");
            continue;
        }
        if path.is_dir() {
            // Include empty input directories and their identity. Directory
            // times also change for writes inside excluded output subtrees.
            hash.update(b"directory");
            if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
                let target = std::fs::read_link(&path)?;
                let bytes = target.as_os_str().as_encoded_bytes();
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                let metadata = std::fs::symlink_metadata(&path)?;
                epoch.update(metadata.dev().to_le_bytes());
                epoch.update(metadata.ino().to_le_bytes());
                epoch.update(metadata.mode().to_le_bytes());
            }
            continue;
        }
        let mut file = std::fs::File::open(&path)?;
        #[cfg(unix)]
        for metadata in [std::fs::symlink_metadata(&path)?, file.metadata()?] {
            use std::os::unix::fs::MetadataExt as _;
            // Access times change when we read; modification/change identity
            // must change for a real edit even when bytes and mtime return.
            epoch.update(metadata.dev().to_le_bytes());
            epoch.update(metadata.ino().to_le_bytes());
            epoch.update(metadata.mode().to_le_bytes());
            epoch.update(metadata.mtime().to_le_bytes());
            epoch.update(metadata.mtime_nsec().to_le_bytes());
            epoch.update(metadata.ctime().to_le_bytes());
            epoch.update(metadata.ctime_nsec().to_le_bytes());
        }
        hash.update(file.metadata()?.len().to_le_bytes());
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        hash.update(b"end");
    }
    let contents: [u8; 32] = hash.finalize().into();
    epoch.update(contents);
    Ok((contents, epoch.finalize().into()))
}

/// The entries `collect` lists, each with its own (unfollowed) metadata,
/// from one `lstat` per entry so polling stays cheap on large workspaces.
fn stamp_entries(
    path: &Path,
    metadata: Option<std::fs::Metadata>,
    excluded: &[PathBuf],
    declared_output: bool,
    entries: &mut Vec<(PathBuf, Option<std::fs::Metadata>)>,
) -> std::io::Result<()> {
    if !super::relevant(path)
        || (!declared_output && excluded.iter().any(|output| path.starts_with(output)))
    {
        return Ok(());
    }
    // Directory symlinks are listed, never followed, as in `collect`.
    let directory = metadata.as_ref().is_some_and(std::fs::Metadata::is_dir);
    entries.push((path.to_path_buf(), metadata));
    if directory {
        let cargo = path.file_name().is_some_and(|name| name == ".cargo");
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if cargo
                && !["config", "config.toml"]
                    .contains(&entry.file_name().to_str().unwrap_or_default())
            {
                continue;
            }
            let metadata = entry.metadata().ok();
            stamp_entries(&entry.path(), metadata, excluded, declared_output, entries)?;
        }
    }
    Ok(())
}

fn collect(
    path: &Path,
    excluded: &[PathBuf],
    declared_output: bool,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if !super::relevant(path)
        || (!declared_output && excluded.iter().any(|output| path.starts_with(output)))
    {
        return Ok(());
    }
    if path.is_dir() {
        // Directory symlinks are supplied as resolved metadata roots, never followed recursively.
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            files.push(path.to_path_buf());
            return Ok(());
        }
        files.push(path.to_path_buf());
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if path.file_name().is_some_and(|name| name == ".cargo")
                && !["config", "config.toml"]
                    .contains(&entry.file_name().to_str().unwrap_or_default())
            {
                continue;
            }
            collect(&entry.path(), excluded, declared_output, files)?;
        }
    } else {
        files.push(path.to_path_buf());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn directory_links_are_observed_without_following_their_contents()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-watch-links-{}", std::process::id()));
        let inputs = root.join("inputs");
        let a = root.join("a");
        let b = root.join("b");
        for path in [&inputs, &a, &b] {
            std::fs::create_dir_all(path)?;
        }
        let roots = vec![inputs.clone()];
        let before = fingerprint(&roots, &[])?;
        let link = inputs.join("linked");
        std::os::unix::fs::symlink(&a, &link)?;
        let linked = fingerprint(&roots, &[])?;
        assert_ne!(linked, before);
        std::fs::write(a.join("unobserved.rs"), "outside the input roots")?;
        assert_eq!(fingerprint(&roots, &[])?, linked);
        std::fs::remove_file(&link)?;
        std::os::unix::fs::symlink(&b, &link)?;
        assert_ne!(fingerprint(&roots, &[])?, linked);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn fallback_does_not_repeat_an_already_scanned_native_edit()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-watch-mixed-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let source = root.join("lib.rs");
        std::fs::write(&source, "pub fn a() {}")?;
        let roots = vec![root.clone()];
        let (sender, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, true);
        monitoring.subscribe(&roots, &[], &mut BTreeMap::new(), &mut Vec::new());
        monitoring.scanned = Instant::now()
            .checked_sub(SCAN_INTERVAL)
            .ok_or("scan clock is too young")?;
        std::fs::write(&source, "pub fn b() {}")?;
        let event = || Event::new(notify::EventKind::Any).add_path(source.clone());
        sender.send(Ok(event()))?;
        assert!(monitoring.changed(&mut Vec::new()));
        sender.send(Ok(event()))?;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn new_empty_input_directories_invalidate_startup() -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-watch-empty-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let initial = capture(std::slice::from_ref(&root), &[])?.1;
        std::fs::create_dir(root.join("new-input"))?;
        assert_ne!(capture(std::slice::from_ref(&root), &[])?.1, initial);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_removals_do_not_turn_startup_replays_into_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        use notify::EventKind;
        use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
        let root =
            std::env::temp_dir().join(format!("sail-watch-unrelated-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let source = root.join("lib.rs");
        std::fs::write(&source, "pub fn a() {}")?;
        let roots = vec![root.clone()];
        for kind in [
            EventKind::Remove(RemoveKind::File),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        ] {
            let (sender, changes) = channel();
            let mut monitoring = Monitoring::detached(None, changes, false);
            monitoring.subscribe(&roots, &[], &mut BTreeMap::new(), &mut Vec::new());
            sender.send(Ok(
                Event::new(kind).add_path(root.with_file_name("unrelated-output"))
            ))?;
            sender.send(Ok(
                Event::new(EventKind::Create(CreateKind::File)).add_path(source.clone())
            ))?;
            assert!(!monitoring.changed(&mut Vec::new()));
            // Even unchanged bytes cannot suppress removal/rename of an input.
            sender.send(Ok(Event::new(kind).add_path(source.clone())))?;
            assert!(monitoring.changed(&mut Vec::new()));
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn startup_replays_are_ignored_but_edit_and_restore_still_cancels()
    -> Result<(), Box<dyn std::error::Error>> {
        use notify::EventKind;
        use notify::event::{CreateKind, DataChange, ModifyKind};
        let root = std::env::temp_dir().join(format!("sail-watch-epoch-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let source = root.join("lib.rs");
        std::fs::write(&source, "pub fn a() {}")?;
        let modified = std::fs::metadata(&source)?.modified()?;
        let output = root.join("output");
        let roots = vec![root.clone()];
        let (sender, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, false);
        monitoring.subscribe(
            &roots,
            std::slice::from_ref(&output),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::File)).add_path(source.clone())
        ))?;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::create_dir_all(&output)?;
        std::fs::write(output.join("generated.rs"), "output")?;
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.clone())
        ))?;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::write(&source, "pub fn b() {}")?;
        std::fs::write(&source, "pub fn a() {}")?;
        std::fs::File::open(&source)?.set_modified(modified)?;
        sender.send(Ok(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Content,
        )))
        .add_path(source.clone())))?;
        assert!(monitoring.changed(&mut Vec::new()));
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::File)).add_path(source)
        ))?;
        assert!(monitoring.changed(&mut Vec::new()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn fallback_detects_restored_mtime_edits_without_reverifying_unchanged_inputs() {
        let root = std::env::temp_dir().join(format!("sail-watch-poll-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let source = root.join("lib.rs");
        assert!(std::fs::write(&source, "pub fn a() {}\n").is_ok());
        let modified = std::fs::metadata(&source)
            .and_then(|metadata| metadata.modified())
            .ok();
        let (_, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, true);
        let roots = vec![root.clone()];
        let output = root.join("output");
        assert!(std::fs::create_dir_all(&output).is_ok());
        monitoring.subscribe(
            &roots,
            std::slice::from_ref(&output),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        assert!(std::fs::write(output.join("generated.rs"), "generated").is_ok());
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        assert!(std::fs::create_dir_all(root.join("src/target")).is_ok());
        assert!(std::fs::write(root.join("src/target/mod.rs"), "source").is_ok());
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(monitoring.changed(&mut Vec::new()));
        assert!(std::fs::write(&source, "pub fn b() {}\n").is_ok());
        if let Some(modified) = modified {
            assert!(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&source)
                    .and_then(
                        |file| file.set_times(std::fs::FileTimes::new().set_modified(modified))
                    )
                    .is_ok()
            );
        }
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(monitoring.changed(&mut Vec::new()));
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        let _ = std::fs::remove_dir_all(root);
    }

    fn fixture(name: &str) -> std::io::Result<(PathBuf, PathBuf)> {
        let root = std::env::temp_dir().join(format!("sail-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let source = root.join("lib.rs");
        std::fs::write(&source, "pub fn a() {}")?;
        Ok((root, source))
    }

    fn content(path: &Path) -> Event {
        use notify::event::{DataChange, ModifyKind};
        Event::new(notify::EventKind::Modify(ModifyKind::Data(
            DataChange::Content,
        )))
        .add_path(path.to_path_buf())
    }

    // #403: FSEvents can deliver an edit's notifications after the debounce
    // has submitted the run that already includes the edit.
    #[cfg(unix)]
    #[test]
    fn delayed_notifications_for_an_observed_edit_do_not_restart_the_run()
    -> Result<(), Box<dyn std::error::Error>> {
        use notify::EventKind;
        use notify::event::{CreateKind, RemoveKind};
        let (root, source) = fixture("delayed")?;
        let roots = vec![root.clone()];
        let (sender, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, false);
        monitoring.subscribe(&roots, &[], &mut BTreeMap::new(), &mut Vec::new());
        monitoring.observe();
        std::fs::write(&source, "pub fn b() {}")?;
        sender.send(Ok(content(&source)))?;
        assert!(monitoring.changed(&mut Vec::new()));
        // The debounce settles and the restarted run observes the edit.
        monitoring.observe();
        for event in [
            content(&source),
            Event::new(EventKind::Create(CreateKind::File)).add_path(source.clone()),
            Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.clone()),
        ] {
            sender.send(Ok(event))?;
            assert!(!monitoring.changed(&mut Vec::new()));
        }
        // A later edit, even one that restores bytes and mtime, still cancels.
        let modified = std::fs::metadata(&source)?.modified()?;
        std::fs::write(&source, "pub fn c() {}")?;
        std::fs::write(&source, "pub fn b() {}")?;
        std::fs::File::open(&source)?.set_modified(modified)?;
        sender.send(Ok(content(&source)))?;
        assert!(monitoring.changed(&mut Vec::new()));
        // Input removals and renames always restart, as before.
        monitoring.observe();
        sender.send(Ok(
            Event::new(EventKind::Remove(RemoveKind::File)).add_path(source.clone())
        ))?;
        assert!(monitoring.changed(&mut Vec::new()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn cargo_output_writes_never_restart_with_events_or_scans()
    -> Result<(), Box<dyn std::error::Error>> {
        use notify::EventKind;
        use notify::event::{CreateKind, MetadataKind, ModifyKind};
        let (root, _) = fixture("output")?;
        let output = root.join("target");
        let nested = root.join("src/target/mod.rs");
        std::fs::create_dir_all(root.join("src/target"))?;
        std::fs::write(&nested, "pub fn a() {}")?;
        std::fs::create_dir_all(&output)?;
        let roots = vec![root.clone()];
        for fallback in [false, true] {
            let (sender, changes) = channel();
            let mut monitoring = Monitoring::detached(None, changes, fallback);
            monitoring.subscribe(
                &roots,
                std::slice::from_ref(&output),
                &mut BTreeMap::new(),
                &mut Vec::new(),
            );
            monitoring.observe();
            let generated = output.join(format!("generated-{fallback}.rs"));
            std::fs::write(&generated, "generated output")?;
            for event in [
                Event::new(EventKind::Create(CreateKind::File)).add_path(generated.clone()),
                content(&generated),
                Event::new(EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)))
                    .add_path(output.clone()),
                // A coalesced directory-level notification for the workspace.
                Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.clone()),
            ] {
                sender.send(Ok(event))?;
                monitoring.scanned -= SCAN_INTERVAL;
                assert!(!monitoring.changed(&mut Vec::new()));
            }
            // A source directory named target is still an input.
            std::fs::write(&nested, format!("pub fn edited_{fallback}() {{}}"))?;
            sender.send(Ok(content(&nested)))?;
            monitoring.scanned -= SCAN_INTERVAL;
            assert!(monitoring.changed(&mut Vec::new()));
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    // fseventsd can drop a client's events mid-run; the scans that replace
    // native events start from what the run observed.
    #[cfg(unix)]
    #[test]
    fn degrading_to_scans_mid_run_neither_repeats_nor_loses_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        use notify::event::Flag;
        let (root, source) = fixture("degrade")?;
        let roots = vec![root.clone()];
        let (sender, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, false);
        monitoring.subscribe(&roots, &[], &mut BTreeMap::new(), &mut Vec::new());
        monitoring.observe();
        std::fs::write(&source, "pub fn b() {}")?;
        sender.send(Ok(content(&source)))?;
        assert!(monitoring.changed(&mut Vec::new()));
        monitoring.observe();
        // A late repeat of the edit arrives with the dropped-events rescan.
        sender.send(Ok(content(&source)))?;
        sender.send(Ok(
            Event::new(notify::EventKind::Other).set_flag(Flag::Rescan)
        ))?;
        let mut err = Vec::new();
        assert!(!monitoring.changed(&mut err));
        assert!(String::from_utf8_lossy(&err).contains("scanning known input contents"));
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::write(&source, "pub fn c() {}")?;
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(monitoring.changed(&mut Vec::new()));
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    // A completed run refreshes the watched roots from its input record.
    // That refresh must not adopt an edit nobody has reported yet.
    #[test]
    fn refreshing_roots_after_a_run_keeps_unreported_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        let (root, source) = fixture("refresh")?;
        let declared = root.with_file_name(format!(
            "sail-watch-refresh-declared-{}.rs",
            std::process::id()
        ));
        std::fs::write(&declared, "declared")?;
        let roots = vec![root.clone()];
        let wider = vec![root.clone(), declared.clone()];
        for fallback in [true, false] {
            let (_sender, changes) = channel();
            let mut monitoring = Monitoring::detached(None, changes, fallback);
            let mut subscriptions = BTreeMap::new();
            monitoring.subscribe(&roots, &[], &mut subscriptions, &mut Vec::new());
            monitoring.observe();
            // Unchanged inputs: neither refresh restarts verification.
            monitoring.subscribe(&roots, &[], &mut subscriptions, &mut Vec::new());
            monitoring.subscribe(&wider, &[], &mut subscriptions, &mut Vec::new());
            monitoring.scanned -= SCAN_INTERVAL;
            assert!(!monitoring.changed(&mut Vec::new()));
            monitoring.subscribe(&roots, &[], &mut subscriptions, &mut Vec::new());
            monitoring.observe();
            std::fs::write(&source, format!("pub fn edited_{fallback}() {{}}"))?;
            // The same coverage, then wider coverage, before any scan.
            monitoring.subscribe(&roots, &[], &mut subscriptions, &mut Vec::new());
            monitoring.subscribe(&wider, &[], &mut subscriptions, &mut Vec::new());
            monitoring.scanned -= SCAN_INTERVAL;
            assert!(monitoring.changed(&mut Vec::new()));
            monitoring.scanned -= SCAN_INTERVAL;
            assert!(!monitoring.changed(&mut Vec::new()));
        }
        std::fs::remove_file(declared)?;
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn the_stamp_walk_lists_exactly_the_captured_inputs() -> Result<(), Box<dyn std::error::Error>>
    {
        let (root, _) = fixture("stamp-walk")?;
        let outside = root.with_file_name(format!("sail-watch-stamp-out-{}", std::process::id()));
        std::fs::create_dir_all(outside.join("inner"))?;
        for directory in ["src/nested", ".cargo", ".git", "target/debug", "empty"] {
            std::fs::create_dir_all(root.join(directory))?;
        }
        for file in [
            "src/nested/m.rs",
            ".cargo/config.toml",
            ".cargo/credentials",
            ".git/HEAD",
            "target/debug/out.rs",
        ] {
            std::fs::write(root.join(file), file)?;
        }
        std::os::unix::fs::symlink(&outside, root.join("linked"))?;
        std::os::unix::fs::symlink(root.join("absent"), root.join("dangling"))?;
        let roots = vec![
            root.clone(),
            root.join(".cargo"),
            root.join("missing.rs"),
            root.join("target/debug/out.rs"),
        ];
        let excluded = vec![root.join("target")];
        let mut collected = Vec::new();
        let mut walked = Vec::new();
        for path in &roots {
            let declared = excluded.iter().any(|output| path.starts_with(output));
            collect(path, &excluded, declared, &mut collected)?;
            let metadata = std::fs::symlink_metadata(path).ok();
            stamp_entries(path, metadata, &excluded, declared, &mut walked)?;
        }
        collected.sort();
        collected.dedup();
        let mut walked: Vec<_> = walked.into_iter().map(|(path, _)| path).collect();
        walked.sort();
        walked.dedup();
        assert_eq!(walked, collected);
        assert!(walked.contains(&root.join("target/debug/out.rs")));
        assert!(!walked.contains(&outside.join("inner")));
        std::fs::remove_dir_all(root)?;
        std::fs::remove_dir_all(outside)?;
        Ok(())
    }

    // #416: fseventsd can drop a watcher's events without a timely notice.
    // The backstop must see such an edit within one interval, while native
    // events still look healthy, and never report it twice.
    #[test]
    fn backstop_catches_an_edit_whose_notification_was_dropped()
    -> Result<(), Box<dyn std::error::Error>> {
        let (root, source) = fixture("backstop")?;
        let output = root.join("target");
        std::fs::create_dir_all(&output)?;
        let roots = vec![root.clone()];
        let (sender, changes) = channel();
        let mut monitoring = Monitoring::detached(None, changes, false);
        monitoring.subscribe(
            &roots,
            std::slice::from_ref(&output),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        monitoring.observe();
        // Polls of unchanged inputs, and of Cargo output writes, are quiet.
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::write(output.join("generated.rs"), "generated output")?;
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        // The edit's notification never arrives.
        let modified = std::fs::metadata(&source)?.modified()?;
        std::fs::write(&source, "pub fn b() {}")?;
        std::fs::File::options()
            .write(true)
            .open(&source)?
            .set_modified(modified)?;
        assert!(!monitoring.changed(&mut Vec::new()), "before the interval");
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(monitoring.changed(&mut Vec::new()), "within the interval");
        assert!(!monitoring.fallback, "native events remain the fast path");
        // The next poll does not repeat it, nor does the late notification
        // once the debounce has submitted the run that includes the edit.
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        monitoring.observe();
        sender.send(Ok(content(&source)))?;
        assert!(!monitoring.changed(&mut Vec::new()));
        // A new input file is seen too, and a notified edit is not repeated
        // by the following poll.
        std::fs::write(root.join("new.rs"), "pub fn n() {}")?;
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(monitoring.changed(&mut Vec::new()));
        std::fs::write(&source, "pub fn c() {}")?;
        sender.send(Ok(content(&source)))?;
        assert!(monitoring.changed(&mut Vec::new()));
        monitoring.polled -= BACKSTOP_INTERVAL;
        assert!(!monitoring.changed(&mut Vec::new()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
