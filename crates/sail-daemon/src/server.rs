//! The Unix-socket server.

use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use sail_api::protocol::{
    Identity, Request, StatsSnapshot, write_get_response, write_hello_response, write_put_response,
    write_stats_response,
};
use sail_cache::DiskCache;
use sail_key::{QueryKey, SolverIdentity, sha256};

/// Counters since the daemon started.
#[derive(Debug, Default)]
pub struct Stats {
    /// `HELLO` requests (one per cached shim session).
    pub hellos: AtomicU64,
    /// `GET` requests.
    pub gets: AtomicU64,
    /// `GET` hits.
    pub hits: AtomicU64,
    /// `PUT` requests.
    pub puts: AtomicU64,
}

impl Stats {
    /// The current values.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            hellos: self.hellos.load(Ordering::Relaxed),
            gets: self.gets.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            puts: self.puts.load(Ordering::Relaxed),
        }
    }
}

type IdentityCache = Mutex<HashMap<(PathBuf, u64, Option<SystemTime>), Identity>>;

struct State {
    cache: DiskCache,
    identities: IdentityCache,
    stats: Arc<Stats>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn identify(path: &Path) -> Result<Identity, String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut words = text.split_whitespace();
    let mut version = None;
    while let Some(w) = words.next() {
        if w == "version" {
            version = words.next().map(str::to_owned);
            break;
        }
    }
    let version =
        version.ok_or_else(|| format!("cannot read a version from {}", path.display()))?;
    let binary = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Identity {
        name: "z3".to_owned(),
        version,
        binary_sha256: hex(&sha256(&binary)),
        platform: SolverIdentity::host_platform(),
        protocol_version: sail_api::protocol::PROTOCOL_VERSION,
    })
}

impl State {
    fn hello(&self, path: &str) -> Result<Identity, String> {
        let path = PathBuf::from(path);
        let meta =
            std::fs::metadata(&path).map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        let id = (path.clone(), meta.len(), meta.modified().ok());
        if let Some(found) = self
            .identities
            .lock()
            .ok()
            .and_then(|m| m.get(&id).cloned())
        {
            return Ok(found);
        }
        let identity = identify(&path)?;
        if let Ok(mut m) = self.identities.lock() {
            m.insert(id, identity.clone());
        }
        Ok(identity)
    }

    fn handle(&self, stream: UnixStream) {
        let Ok(read_half) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(read_half);
        let mut writer = BufWriter::new(stream);
        loop {
            let Ok(Some(request)) = Request::read(&mut reader) else {
                return;
            };
            let result = match request {
                Request::Hello(path) => {
                    self.stats.hellos.fetch_add(1, Ordering::Relaxed);
                    let r = self.hello(&path);
                    write_hello_response(&mut writer, r.as_ref().map_err(String::as_str))
                }
                Request::Get(key) => {
                    self.stats.gets.fetch_add(1, Ordering::Relaxed);
                    let found = self.cache.get(&QueryKey(key));
                    if found.is_some() {
                        self.stats.hits.fetch_add(1, Ordering::Relaxed);
                    }
                    let effort = found.as_ref().and_then(|(m, _)| m.effort.clone()).map(|e| {
                        sail_api::protocol::Effort {
                            limit: e.limit,
                            used: e.used,
                            reason: e.reason,
                        }
                    });
                    write_get_response(
                        &mut writer,
                        found
                            .as_ref()
                            .map(|(_, reply)| (reply.as_slice(), effort.as_ref())),
                    )
                }
                Request::Put { key, meta, reply } => {
                    self.stats.puts.fetch_add(1, Ordering::Relaxed);
                    // A failed store only costs a future miss.
                    let effort = meta.effort.map(|e| sail_cache::Effort {
                        limit: e.limit,
                        used: e.used,
                        reason: e.reason,
                    });
                    let _ = self.cache.put(
                        &QueryKey(key),
                        &reply,
                        &meta.command,
                        "",
                        &meta.provenance,
                        effort.as_ref(),
                    );
                    write_put_response(&mut writer)
                }
                Request::Stats => write_stats_response(&mut writer, &self.stats.snapshot()),
            };
            if result.is_err() {
                return;
            }
        }
    }
}

/// Serves the cache rooted at `cache_root` on the Unix socket `socket` until
/// the process is stopped. A stale socket file is replaced.
///
/// # Errors
///
/// Binding errors.
pub fn serve(socket: &Path, cache_root: &Path, counters: Arc<Stats>) -> std::io::Result<()> {
    if socket.exists() && UnixStream::connect(socket).is_err() {
        std::fs::remove_file(socket)?;
    }
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(socket)?;
    let shared = Arc::new(State {
        cache: DiskCache::new(cache_root),
        identities: Mutex::new(HashMap::new()),
        stats: counters,
    });
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let worker = Arc::clone(&shared);
        let _ = std::thread::Builder::new()
            .name("sail-daemon-conn".to_owned())
            .spawn(move || worker.handle(conn));
    }
    Ok(())
}
