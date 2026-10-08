//! The daemon's log file.
//!
//! The daemon never writes to stderr while serving: `sail verify` starts it
//! in the background, and sail must not change what Verus prints. Lifecycle
//! events and errors go to a log file instead (by default
//! `<cache root>/daemon.log`).

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A log larger than this at start is moved to `<log>.1` first.
const ROTATE_BYTES: u64 = 1024 * 1024;

/// An append-only log. Every write failure is ignored: logging must never
/// affect serving.
#[derive(Debug, Default)]
pub(crate) struct Log {
    file: Option<Mutex<File>>,
    path: Option<PathBuf>,
}

impl Log {
    /// Opens (creating) the log at `path`; `None` logs nothing.
    pub fn open(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
            let mut old = path.as_os_str().to_owned();
            old.push(".1");
            let _ = std::fs::rename(path, old);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok();
        Self {
            path: file.is_some().then(|| path.to_path_buf()),
            file: file.map(Mutex::new),
        }
    }

    /// The log's path, if it is open.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Appends one line, prefixed with the Unix time and the process id.
    pub fn line(&self, msg: &str) {
        let Some(file) = &self.file else { return };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let text = format!("{now} sail-daemon[{}] {msg}\n", std::process::id());
        if let Ok(mut f) = file.lock() {
            let _ = f.write_all(text.as_bytes());
        }
    }
}
