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
        // Capture once when input coverage changes, never every native poll.
        self.baseline = fingerprint(roots).ok();
        self.scanned = Instant::now();
    }
    pub(super) fn changed(&mut self, roots: &[PathBuf], err: &mut dyn Write) -> bool {
        let mut edited = false;
        while let Ok(event) = self.changes.try_recv() {
            match event {
                Ok(event) if !event.need_rescan() => {
                    let (sender, receiver) = channel();
                    let _ = sender.send(Ok(event));
                    edited |= super::changed(&receiver, roots);
                }
                _ => self.degrade(err),
            }
        }
        if self.fallback && self.scanned.elapsed() >= SCAN_INTERVAL {
            self.scanned = Instant::now();
            match fingerprint(roots) {
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

fn fingerprint(roots: &[PathBuf]) -> std::io::Result<[u8; 32]> {
    let mut files = Vec::new();
    for root in roots {
        collect(root, &mut files)?;
    }
    files.sort();
    files.dedup();
    let mut hash = Sha256::new();
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
    Ok(hash.finalize().into())
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !super::relevant(path) {
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
            collect(&entry.path(), files)?;
        }
    } else {
        files.push(path.to_path_buf());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let roots = vec![root.clone()];
        monitoring.subscribe(&roots, &mut BTreeMap::new(), &mut Vec::new());
        monitoring.scanned -= SCAN_INTERVAL;
        assert!(!monitoring.changed(&roots, &mut Vec::new()));
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
