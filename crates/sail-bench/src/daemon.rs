//! A cache daemon for one benchmark unit, run as a separate `sail serve`
//! process (`sail-bench` does not link the daemon; see the layering table).
//!
//! Each [`Daemon`] owns a fresh cache root and a socket, and is stopped (and
//! its socket removed) when dropped. [`Daemon::stats`] reads the daemon's
//! counters over the `STATS` operation (ADR 0012).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(unix)]
use sail_api::client::Client;
use sail_api::protocol::StatsSnapshot;
use serde::{Deserialize, Serialize};

/// Longest socket path accepted. Unix socket paths are limited to about
/// 104 bytes (macOS) or 108 (Linux), including the terminator.
pub const MAX_SOCKET_PATH: usize = 100;

/// How long to wait for `sail serve` to accept connections.
const START_TIMEOUT: Duration = Duration::from_secs(20);

/// A running `sail serve`.
#[derive(Debug)]
pub struct Daemon {
    child: Child,
    socket: PathBuf,
}

impl Daemon {
    /// Starts `sail serve` with the cache rooted at `cache_root` (emptied
    /// first) on `socket`, and waits until it accepts connections.
    ///
    /// # Errors
    ///
    /// A message if the socket path is too long, the process cannot be
    /// started, or it does not accept connections in time.
    pub fn start(sail: &Path, socket: &Path, cache_root: &Path) -> Result<Self, String> {
        let len = socket.as_os_str().len();
        if len >= MAX_SOCKET_PATH {
            return Err(format!(
                "socket path {} is {len} bytes, too long for a Unix socket; set SAIL_CACHE_DIR to a shorter directory",
                socket.display()
            ));
        }
        let _ = std::fs::remove_dir_all(cache_root);
        let _ = std::fs::remove_file(socket);
        let child = Command::new(sail)
            .arg("serve")
            .arg("--socket")
            .arg(socket)
            .arg("--cache")
            .arg(cache_root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start {} serve: {e}", sail.display()))?;
        let mut daemon = Self {
            child,
            socket: socket.to_path_buf(),
        };
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            if daemon.stats().is_some() {
                return Ok(daemon);
            }
            if let Ok(Some(status)) = daemon.child.try_wait() {
                return Err(format!("`sail serve` exited early ({status})"));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "`sail serve` did not accept connections on {} within {}s",
                    socket.display(),
                    START_TIMEOUT.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The socket to hand to `sail-z3` (`SAIL_DAEMON_SOCKET`).
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The daemon's counters, or `None` if it cannot be asked.
    #[must_use]
    pub fn stats(&self) -> Option<StatsSnapshot> {
        #[cfg(unix)]
        {
            Client::connect(&self.socket).ok()?.stats().ok()
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// What the cache did during one sail run, from the daemon's counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRun {
    /// `fill`, `warm` or `edit`.
    pub phase: String,
    /// Cached shim sessions (`HELLO` requests).
    pub sessions: u64,
    /// `GET` requests. A session stops asking at its first miss, so
    /// `gets - hits` is the number of sessions that started the solver.
    pub gets: u64,
    /// Replies served from the cache.
    pub hits: u64,
    /// Replies recorded from the solver (`PUT`).
    pub puts: u64,
}

impl CacheRun {
    /// The counters accumulated between `before` and `after`, labelled
    /// `phase`.
    #[must_use]
    pub fn between(phase: &str, before: &StatsSnapshot, after: &StatsSnapshot) -> Self {
        let d = after.since(before);
        Self {
            phase: phase.to_owned(),
            sessions: d.hellos,
            gets: d.gets,
            hits: d.hits,
            puts: d.puts,
        }
    }

    /// Sessions that started the solver (their one `GET` that missed).
    #[must_use]
    pub fn solver_starts(&self) -> u64 {
        self.gets.saturating_sub(self.hits)
    }
}

/// Total size of the files under `dir`, in bytes (0 if unreadable).
#[must_use]
pub fn dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_bytes(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

/// The `sail` binary next to the running executable, as `sail-z3` is found.
///
/// # Errors
///
/// A message if it cannot be found.
pub fn locate_sail(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    let sail = match explicit {
        Some(p) => p,
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("sail"),
    };
    if sail.is_file() {
        Ok(sail)
    } else {
        Err(format!(
            "sail not found at {} (build it with `cargo build --release -p sail-cli`, or pass --sail)",
            sail.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_run_deltas() {
        let before = StatsSnapshot {
            hellos: 2,
            gets: 10,
            hits: 9,
            puts: 4,
        };
        let after = StatsSnapshot {
            hellos: 5,
            gets: 30,
            hits: 27,
            puts: 4,
        };
        let r = CacheRun::between("warm", &before, &after);
        assert_eq!((r.sessions, r.gets, r.hits, r.puts), (3, 20, 18, 0));
        assert_eq!(r.solver_starts(), 2);
        assert_eq!(r.phase, "warm");
    }

    #[test]
    fn rejects_long_socket_paths() {
        let long = PathBuf::from(format!("/{}", "s".repeat(MAX_SOCKET_PATH)));
        let r = Daemon::start(
            Path::new("/nonexistent/sail"),
            &long,
            Path::new("/nonexistent"),
        );
        assert!(matches!(r, Err(m) if m.contains("too long")));
    }
}
