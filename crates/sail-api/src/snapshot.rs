//! Client side of exact solver snapshots (prototype, ADR 0048 draft).
//!
//! A snapshot is a parked copy of a pinned Z3 process, forked by the
//! `sail-snap` preload library (`native/sail-snap/sail_snap.c`) at the moment
//! Z3 had consumed exactly a known input prefix. It is named after that
//! prefix's v2 key ([`name`]), so a session whose input starts with the same
//! bytes can resume from it and get exactly the replies a fresh process fed
//! the whole input would give.
//!
//! The wire protocol is the library's: three connections to the snapshot's
//! Unix socket, each starting with a fixed 29-byte header
//! `SAILSNAP1 <role> <16 hex nonce>\n` (roles `i` for stdin and stdout, `e`
//! for stderr, `s` for the exit status).
//!
//! Status: prototype on branch `perf/solver-snapshots`; off unless
//! `SAIL_SNAPSHOT_DIR` and `SAIL_SNAPSHOT_LIB` are both set.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sail_key::QueryKey;

/// Environment variable naming the directory for snapshot sockets. Keep it
/// short: a Unix socket path is limited to about 100 bytes.
pub const ENV_SNAPSHOT_DIR: &str = "SAIL_SNAPSHOT_DIR";
/// Environment variable naming the `sail-snap` preload library.
pub const ENV_SNAPSHOT_LIB: &str = "SAIL_SNAPSHOT_LIB";
/// Environment variable: minimum solver time between snapshots, in
/// milliseconds (passed to the library as `SAIL_SNAP_GAP_MS`).
pub const ENV_SNAPSHOT_GAP_MS: &str = "SAIL_SNAPSHOT_GAP_MS";
/// Environment variable: idle lifetime of a parked snapshot, in seconds
/// (passed to the library as `SAIL_SNAP_TTL_S`).
pub const ENV_SNAPSHOT_TTL_S: &str = "SAIL_SNAPSHOT_TTL_S";
/// Environment variable: most parked snapshots kept in the directory; the
/// least recently used are evicted beyond it.
pub const ENV_SNAPSHOT_MAX_PARKED: &str = "SAIL_SNAPSHOT_MAX_PARKED";

/// Version of the snapshot naming and protocol, bound into every name.
const VERSION: &[u8] = b"sail-snapshot/v1";

/// Snapshot configuration from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    /// Socket directory.
    pub dir: PathBuf,
    /// The preload library.
    pub lib: PathBuf,
    /// `SAIL_SNAP_GAP_MS` for the library.
    pub gap_ms: Option<String>,
    /// `SAIL_SNAP_TTL_S` for the library.
    pub ttl_s: Option<String>,
    /// Parked snapshots kept.
    pub max_parked: usize,
}

impl Config {
    /// The configuration, if snapshots are enabled and the library exists.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os(ENV_SNAPSHOT_DIR).filter(|v| !v.is_empty())?;
        let lib = std::env::var_os(ENV_SNAPSHOT_LIB).filter(|v| !v.is_empty())?;
        let lib = PathBuf::from(lib);
        if !lib.is_file() {
            return None;
        }
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).ok()?;
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Some(Self {
            dir,
            lib,
            gap_ms: var(ENV_SNAPSHOT_GAP_MS),
            ttl_s: var(ENV_SNAPSHOT_TTL_S),
            max_parked: var(ENV_SNAPSHOT_MAX_PARKED)
                .and_then(|v| v.parse().ok())
                .unwrap_or(256),
        })
    }

    /// Environment for a solver process that takes snapshots. `ready` is
    /// created by the library when it loads.
    #[must_use]
    pub fn solver_env(&self, ready: &Path) -> Vec<(String, std::ffi::OsString)> {
        let preload = if cfg!(target_os = "macos") {
            "DYLD_INSERT_LIBRARIES"
        } else {
            "LD_PRELOAD"
        };
        let mut env = vec![
            (preload.to_owned(), self.lib.clone().into_os_string()),
            (
                "SAIL_SNAP_DIR".to_owned(),
                self.dir.clone().into_os_string(),
            ),
            ("SAIL_SNAP_READY".to_owned(), ready.as_os_str().to_owned()),
        ];
        if let Some(g) = &self.gap_ms {
            env.push(("SAIL_SNAP_GAP_MS".to_owned(), g.into()));
        }
        if let Some(t) = &self.ttl_s {
            env.push(("SAIL_SNAP_TTL_S".to_owned(), t.into()));
        }
        env
    }

    /// Evicts the least recently used snapshots beyond `max_parked`, by
    /// removing their sockets (a parked snapshot exits when its socket file
    /// is gone).
    pub fn evict(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut socks: Vec<(std::time::SystemTime, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with('s') && !n.ends_with(".pid")
            })
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        if socks.len() <= self.max_parked {
            return;
        }
        socks.sort();
        let excess = socks.len() - self.max_parked;
        for (_, path) in socks.into_iter().take(excess) {
            let _ = std::fs::remove_file(&path);
            let mut pid = path.into_os_string();
            pid.push(".pid");
            let _ = std::fs::remove_file(pid);
        }
    }
}

/// The snapshot name for a process that has consumed exactly the input
/// whose v2 prefix key (over those bytes) is `key`.
#[must_use]
pub fn name(key: &QueryKey) -> String {
    let mut bytes = VERSION.to_vec();
    bytes.extend_from_slice(&key.0);
    let digest = sail_key::sha256(&bytes);
    let mut s = String::from("s");
    for b in digest.iter().take(16) {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// The marker that asks the library to take snapshot `name` at this point
/// of the input. The library strips it: the solver never sees it.
#[must_use]
pub fn marker(name: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(name.len() + 2);
    m.push(0);
    m.extend_from_slice(name.as_bytes());
    m.push(0);
    m
}

/// A resumed solver: its input/output, stderr and exit-status connections.
#[derive(Debug)]
pub struct Resumed {
    /// Write the remaining input here; read the solver's stdout here.
    pub io: UnixStream,
    /// The solver's stderr.
    pub err: UnixStream,
    /// One line, `exit <code>`, when the solver exits.
    pub status: UnixStream,
}

fn nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seed = format!(
        "{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let digest = sail_key::sha256(seed.as_bytes());
    digest.iter().take(8).fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Resumes snapshot `name` in `dir`, if it is parked there.
///
/// # Errors
///
/// Any connection error: the snapshot does not exist, or is gone.
pub fn resume(dir: &Path, name: &str) -> std::io::Result<Resumed> {
    let path = dir.join(name);
    let n = nonce();
    let open = |role: char| -> std::io::Result<UnixStream> {
        let mut s = UnixStream::connect(&path)?;
        s.write_all(format!("SAILSNAP1 {role} {n}\n").as_bytes())?;
        Ok(s)
    };
    let io = open('i')?;
    let err = open('e')?;
    let status = open('s')?;
    Ok(Resumed { io, err, status })
}

/// Reads a resumed solver's exit status: `Some(code)` once it has exited,
/// `None` if the snapshot went away without saying.
#[must_use]
pub fn read_status(mut status: UnixStream) -> Option<i32> {
    let mut s = String::new();
    status.read_to_string(&mut s).ok()?;
    s.trim().strip_prefix("exit ")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{marker, name};
    use sail_key::QueryKey;

    #[test]
    fn names_are_short_stable_and_distinct() {
        let a = name(&QueryKey([1; 32]));
        let b = name(&QueryKey([2; 32]));
        assert_eq!(a.len(), 33);
        assert_ne!(a, b);
        assert_eq!(a, name(&QueryKey([1; 32])));
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn markers_are_nul_delimited() {
        assert_eq!(marker("sab"), b"\0sab\0");
    }
}
