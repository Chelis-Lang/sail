//! Native notifications with a content-based fallback for unavailable events.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};

const SCAN_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct Monitoring {
    native: Option<RecommendedWatcher>,
    changes: Receiver<notify::Result<Event>>,
    fallback: bool,
    scanned: Instant,
    baseline: Option<[u8; 32]>,
    startup_epoch: Option<[u8; 32]>,
    excluded: Vec<PathBuf>,
}
impl Monitoring {
    pub(super) fn new(err: &mut dyn Write) -> Self {
        let (sender, changes) = channel();
        let native = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        });
        let mut monitoring = Self {
            native: native.ok(),
            changes,
            fallback: false,
            scanned: Instant::now(),
            baseline: None,
            startup_epoch: None,
            excluded: vec![],
        };
        if monitoring.native.is_none() {
            monitoring.degrade(err);
        }
        monitoring
    }
    fn degrade(&mut self, err: &mut dyn Write) {
        if !self.fallback {
            let _ = writeln!(
                err,
                "sail watch: filesystem events unavailable or incomplete; scanning known input contents every 5 seconds."
            );
        }
        self.fallback = true;
    }
    pub(super) fn subscribe(
        &mut self,
        roots: &[PathBuf],
        excluded: &[PathBuf],
        subscriptions: &mut BTreeMap<PathBuf, bool>,
        err: &mut dyn Write,
    ) {
        let initial = subscriptions.is_empty();
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
        // Capture once when input coverage changes, never every native poll.
        self.excluded = excluded.to_vec();
        let captured = capture(roots, excluded).ok();
        self.baseline = captured.map(|(contents, _)| contents);
        if initial && cfg!(unix) {
            self.startup_epoch = captured.map(|(_, epoch)| epoch);
        }
        self.scanned = Instant::now();
    }
    pub(super) fn changed(&mut self, roots: &[PathBuf], err: &mut dyn Write) -> bool {
        let mut edited = false;
        while let Ok(event) = self.changes.try_recv() {
            match event {
                Ok(event) if !event.need_rescan() => {
                    let (sender, receiver) = channel();
                    let _ = sender.send(Ok(event));
                    edited |= super::changed(&receiver, roots, &self.excluded);
                }
                _ => self.degrade(err),
            }
        }
        // FSEvents may replay creation notifications from before subscription.
        // Ignore those only while the complete initial input state is still
        // unchanged, including inode/change times so edit-and-restore counts.
        // After the first actual edit, native invalidation stays unconditional.
        if edited && let Some(initial) = self.startup_epoch {
            match capture(roots, &self.excluded) {
                Ok((_, current)) if current == initial => edited = false,
                _ => self.startup_epoch = None,
            }
        }
        if self.fallback && self.scanned.elapsed() >= SCAN_INTERVAL {
            self.scanned = Instant::now();
            match fingerprint(roots, &self.excluded) {
                Ok(next) => {
                    edited |= self.baseline.is_none_or(|previous| previous != next);
                    self.baseline = Some(next);
                }
                Err(error) => {
                    let _ = writeln!(err, "sail watch: content scan incomplete: {error}");
                }
            }
        }
        edited
    }
}

fn fingerprint(roots: &[PathBuf], excluded: &[PathBuf]) -> std::io::Result<[u8; 32]> {
    capture(roots, excluded).map(|(contents, _)| contents)
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
            return Ok(());
        }
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
        let mut monitoring = Monitoring {
            native: None,
            changes,
            fallback: false,
            scanned: Instant::now(),
            baseline: None,
            startup_epoch: None,
            excluded: vec![],
        };
        monitoring.subscribe(
            &roots,
            std::slice::from_ref(&output),
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::File)).add_path(source.clone())
        ))?;
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
        std::fs::create_dir_all(&output)?;
        std::fs::write(output.join("generated.rs"), "output")?;
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.clone())
        ))?;
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
        std::fs::write(&source, "pub fn b() {}")?;
        std::fs::write(&source, "pub fn a() {}")?;
        std::fs::File::open(&source)?.set_modified(modified)?;
        sender.send(Ok(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Content,
        )))
        .add_path(source.clone())))?;
        assert!(monitoring.changed(&roots, &mut Vec::new()));
        sender.send(Ok(
            Event::new(EventKind::Create(CreateKind::File)).add_path(source)
        ))?;
        assert!(monitoring.changed(&roots, &mut Vec::new()));
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
        let mut monitoring = Monitoring {
            native: None,
            changes,
            fallback: true,
            scanned: Instant::now(),
            baseline: None,
            startup_epoch: None,
            excluded: vec![],
        };
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
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
        assert!(std::fs::write(output.join("generated.rs"), "generated").is_ok());
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
        assert!(std::fs::create_dir_all(root.join("src/target")).is_ok());
        assert!(std::fs::write(root.join("src/target/mod.rs"), "source").is_ok());
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(monitoring.changed(&roots, &mut Vec::new()));
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
        assert!(monitoring.changed(&roots, &mut Vec::new()));
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
        let _ = std::fs::remove_dir_all(root);
    }
}
