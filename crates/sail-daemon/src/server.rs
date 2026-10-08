//! The Unix-socket server.

use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use sail_api::protocol::{
    Identity, Info, MemoryInfo, QueueInfo, Request, StatsSnapshot, write_get_response,
    write_hello_response, write_info_response, write_put_response, write_stats_response,
};
use sail_cache::memory::{MemoryTier, State as Slot, Value};
use sail_cache::{DiskCache, FileStamp};
use sail_key::{QueryKey, SolverIdentity, sha256};

use crate::log::Log;
use crate::writer::{Job, Submitted, Writer};

/// Default budget of the in-memory tier: 256 MiB.
pub const DEFAULT_MEMORY_BYTES: u64 = 256 * 1024 * 1024;

/// Default budget of the write-behind queue: 64 MiB.
pub const DEFAULT_QUEUE_BYTES: u64 = 64 * 1024 * 1024;

/// Default idle timeout of `sail serve`: 30 minutes.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(30);

/// Default time a `PUT` waits for room in a full write-behind queue
/// before it is dropped: 1 second.
pub const DEFAULT_QUEUE_WAIT: Duration = Duration::from_secs(1);

/// How long shutdown waits for open connections before draining the queue
/// and returning.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Daemon settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// In-memory tier budget in bytes (0 turns the tier off).
    pub memory_bytes: u64,
    /// Write-behind queue budget in bytes.
    pub queue_bytes: u64,
    /// How long a `PUT` waits for room in a full queue before it is
    /// dropped.
    pub queue_wait: Duration,
    /// Stop after this long with no open connections (`None`: never).
    pub idle_timeout: Option<Duration>,
    /// Log file (`None`: no log).
    pub log: Option<PathBuf>,
    /// Fault injection only: sleep this long before each disk write, so
    /// tests can stop the daemon with writes still queued.
    pub write_delay: Option<Duration>,
}

impl Default for Config {
    /// The library defaults: default budgets, no idle timeout, no log.
    fn default() -> Self {
        Self {
            memory_bytes: DEFAULT_MEMORY_BYTES,
            queue_bytes: DEFAULT_QUEUE_BYTES,
            queue_wait: DEFAULT_QUEUE_WAIT,
            idle_timeout: None,
            log: None,
            write_delay: None,
        }
    }
}

/// Counters since the daemon started.
#[derive(Debug, Default)]
pub struct Stats {
    /// `HELLO` requests (one per cached shim session).
    pub hellos: AtomicU64,
    /// `GET` requests.
    pub gets: AtomicU64,
    /// `GET` hits, from either tier.
    pub hits: AtomicU64,
    /// `PUT` requests.
    pub puts: AtomicU64,
    /// `GET` hits served from the in-memory tier.
    pub memory_hits: AtomicU64,
    /// `PUT`s dropped because the write-behind queue stayed full.
    pub dropped_puts: AtomicU64,
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
            memory_hits: self.memory_hits.load(Ordering::Relaxed),
            dropped_puts: self.dropped_puts.load(Ordering::Relaxed),
        }
    }
}

type IdentityCache = Mutex<HashMap<(PathBuf, u64, Option<SystemTime>), Identity>>;

struct State {
    disk: DiskCache,
    memory: MemoryTier,
    writer: Writer,
    identities: IdentityCache,
    stats: Arc<Stats>,
    log: Log,
    started: Instant,
    socket: PathBuf,
    idle_timeout: Option<Duration>,
    queue_wait: Duration,
    /// Open connections.
    active: AtomicU64,
    /// When a connection last opened or closed.
    last_activity: Mutex<Instant>,
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

fn to_wire(e: &sail_cache::Effort) -> sail_api::protocol::Effort {
    sail_api::protocol::Effort {
        limit: e.limit,
        used: e.used,
        reason: e.reason.clone(),
    }
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

    /// The reply for `key`, and whether it came from memory.
    ///
    /// A memory slot is served only if it is pending (its write is still
    /// queued, so no entry file exists yet) or its entry file still has the
    /// stamp it had when the slot was filled. Anything else falls through
    /// to a verified disk read, which refills the slot.
    fn get(&self, key: &QueryKey) -> Option<(Arc<Value>, bool)> {
        if let Some((value, slot)) = self.memory.lookup(key) {
            match slot {
                Slot::Pending => return Some((value, true)),
                Slot::OnDisk(stamp) => {
                    if self.disk.stamp_for_hit(key) == Some(stamp) {
                        return Some((value, true));
                    }
                    self.memory.remove_if(key, &value);
                }
            }
        }
        let stored = self.disk.get_stamped(key)?;
        let value = Arc::new(Value {
            reply: stored.reply,
            effort: stored.meta.effort,
        });
        self.memory
            .insert_on_disk(key, Arc::clone(&value), stored.stamp);
        Some((value, false))
    }

    /// Stores a reply: queued for disk and held in memory meanwhile. A key
    /// whose entry file exists, or whose write is already queued, is left
    /// alone, so memory never holds bytes that differ from the entry disk
    /// has or will have.
    fn put(&self, key: QueryKey, meta: sail_api::protocol::PutMeta, reply: Vec<u8>) {
        if self.disk.contains(&key) {
            return;
        }
        let job = Job {
            key,
            value: Arc::new(Value {
                reply,
                effort: meta.effort.map(|e| sail_cache::Effort {
                    limit: e.limit,
                    used: e.used,
                    reason: e.reason,
                }),
            }),
            command: meta.command,
            provenance: meta.provenance,
        };
        if self.writer.submit(&self.memory, job, self.queue_wait) == Submitted::Dropped {
            self.stats.dropped_puts.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn info(&self) -> Info {
        let m = self.memory.stats();
        let (queued, queued_bytes) = self.writer.load();
        Info {
            protocol_version: sail_api::protocol::PROTOCOL_VERSION,
            sail_version: env!("CARGO_PKG_VERSION").to_owned(),
            pid: std::process::id(),
            uptime_secs: self.started.elapsed().as_secs(),
            socket: self.socket.display().to_string(),
            cache_root: self.disk.root().display().to_string(),
            log: self.log.path().map(|p| p.display().to_string()),
            idle_timeout_secs: self.idle_timeout.map_or(0, |d| d.as_secs()),
            memory: MemoryInfo {
                capacity_bytes: m.capacity_bytes,
                entries: m.entries,
                bytes: m.bytes,
                pending: m.pending,
            },
            queue: QueueInfo {
                capacity_bytes: self.writer.capacity(),
                entries: queued,
                bytes: queued_bytes,
            },
        }
    }

    fn touch(&self) {
        if let Ok(mut t) = self.last_activity.lock() {
            *t = Instant::now();
        }
    }

    fn idle_for(&self) -> Duration {
        self.last_activity
            .lock()
            .map_or(Duration::ZERO, |t| t.elapsed())
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
                    let found = self.get(&QueryKey(key));
                    if let Some((_, from_memory)) = &found {
                        self.stats.hits.fetch_add(1, Ordering::Relaxed);
                        if *from_memory {
                            self.stats.memory_hits.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    let effort = found
                        .as_ref()
                        .and_then(|(v, _)| v.effort.as_ref())
                        .map(to_wire);
                    write_get_response(
                        &mut writer,
                        found
                            .as_ref()
                            .map(|(v, _)| (v.reply.as_slice(), effort.as_ref())),
                    )
                }
                Request::Put { key, meta, reply } => {
                    self.stats.puts.fetch_add(1, Ordering::Relaxed);
                    // A failed or dropped store only costs a future miss.
                    self.put(QueryKey(key), meta, reply);
                    write_put_response(&mut writer)
                }
                Request::Stats => write_stats_response(&mut writer, &self.stats.snapshot()),
                Request::Info => write_info_response(&mut writer, &self.info()),
            };
            if result.is_err() {
                return;
            }
        }
    }
}

/// Stops a running [`Server`] from another thread (for example a signal
/// handler, or the idle timer).
#[derive(Debug, Clone)]
pub struct Handle {
    stopping: Arc<AtomicBool>,
    socket: PathBuf,
}

impl Handle {
    /// Asks the server to stop: it stops accepting connections, waits
    /// briefly for open ones, writes every queued entry to disk, removes
    /// its socket and returns from [`Server::run`].
    pub fn shutdown(&self) {
        if !self.stopping.swap(true, Ordering::SeqCst) {
            // Wake the accept loop.
            let _ = UnixStream::connect(&self.socket);
        }
    }
}

/// A bound daemon, ready to [`run`](Server::run).
pub struct Server {
    listener: UnixListener,
    state: Arc<State>,
    socket_stamp: Option<FileStamp>,
    stopping: Arc<AtomicBool>,
    writer_thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("socket", &self.state.socket)
            .finish_non_exhaustive()
    }
}

/// Binds the daemon socket at `socket` for the cache rooted at
/// `cache_root`. A stale socket file (nobody listening) is replaced.
///
/// # Errors
///
/// Binding errors, including another daemon already listening.
pub fn bind(
    socket: &Path,
    cache_root: &Path,
    counters: Arc<Stats>,
    config: &Config,
) -> std::io::Result<Server> {
    if socket.exists() && UnixStream::connect(socket).is_err() {
        std::fs::remove_file(socket)?;
    }
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(socket)?;
    let socket_stamp = std::fs::symlink_metadata(socket)
        .ok()
        .map(|m| FileStamp::of(&m));
    let state = Arc::new(State {
        disk: DiskCache::new(cache_root),
        memory: MemoryTier::new(config.memory_bytes),
        writer: Writer::new(config.queue_bytes),
        identities: Mutex::new(HashMap::new()),
        stats: counters,
        log: Log::open(config.log.as_deref()),
        started: Instant::now(),
        socket: socket.to_path_buf(),
        idle_timeout: config.idle_timeout,
        queue_wait: config.queue_wait,
        active: AtomicU64::new(0),
        last_activity: Mutex::new(Instant::now()),
    });
    let w = Arc::clone(&state);
    let delay = config.write_delay;
    let writer_thread = std::thread::Builder::new()
        .name("sail-daemon-writer".to_owned())
        .spawn(move || w.writer.run(&w.disk, &w.memory, delay, &w.log))?;
    state.log.line(&format!(
        "started: sail {}, protocol {}, socket {}, cache {}, memory {} bytes, queue {} bytes, idle timeout {}",
        env!("CARGO_PKG_VERSION"),
        sail_api::protocol::PROTOCOL_VERSION,
        socket.display(),
        cache_root.display(),
        config.memory_bytes,
        config.queue_bytes,
        config
            .idle_timeout
            .map_or_else(|| "none".to_owned(), |d| format!("{}s", d.as_secs())),
    ));
    Ok(Server {
        listener,
        state,
        socket_stamp,
        stopping: Arc::new(AtomicBool::new(false)),
        writer_thread: Some(writer_thread),
    })
}

impl Server {
    /// A handle that stops this server.
    #[must_use]
    pub fn handle(&self) -> Handle {
        Handle {
            stopping: Arc::clone(&self.stopping),
            socket: self.state.socket.clone(),
        }
    }

    /// Serves until stopped by a [`Handle`] or the idle timeout, then
    /// drains the write-behind queue and removes the socket.
    ///
    /// # Errors
    ///
    /// None at present; reserved for fatal listener errors.
    pub fn run(mut self) -> std::io::Result<()> {
        let handle = self.handle();
        if let Some(timeout) = self.state.idle_timeout {
            let state = Arc::clone(&self.state);
            let stopping = Arc::clone(&self.stopping);
            let tick = (timeout / 4).clamp(Duration::from_millis(10), Duration::from_secs(1));
            let _ = std::thread::Builder::new()
                .name("sail-daemon-idle".to_owned())
                .spawn(move || {
                    while !stopping.load(Ordering::SeqCst) {
                        std::thread::sleep(tick);
                        if state.active.load(Ordering::SeqCst) == 0 && state.idle_for() >= timeout {
                            state.log.line(&format!(
                                "idle for {}s with no connections: stopping",
                                timeout.as_secs()
                            ));
                            handle.shutdown();
                            return;
                        }
                    }
                });
        }
        for conn in self.listener.incoming() {
            let Ok(conn) = conn else { continue };
            // A connection that raced a shutdown is still served.
            self.state.active.fetch_add(1, Ordering::SeqCst);
            self.state.touch();
            let worker = Arc::clone(&self.state);
            let spawned = std::thread::Builder::new()
                .name("sail-daemon-conn".to_owned())
                .spawn(move || {
                    worker.handle(conn);
                    worker.touch();
                    worker.active.fetch_sub(1, Ordering::SeqCst);
                });
            if spawned.is_err() {
                self.state.active.fetch_sub(1, Ordering::SeqCst);
            }
            if self.stopping.load(Ordering::SeqCst) {
                break;
            }
        }
        self.stop();
        Ok(())
    }

    fn stop(&mut self) {
        let socket = &self.state.socket;
        // Remove the socket only if it is still the one this server bound.
        let ours = std::fs::symlink_metadata(socket)
            .ok()
            .map(|m| FileStamp::of(&m))
            == self.socket_stamp;
        if ours {
            let _ = std::fs::remove_file(socket);
        }
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while self.state.active.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let (queued, _) = self.state.writer.load();
        self.state.writer.close();
        if let Some(t) = self.writer_thread.take() {
            let _ = t.join();
        }
        let s = self.state.stats.snapshot();
        self.state.log.line(&format!(
            "stopped after {}s: {} writes drained; hellos {}, gets {}, hits {} ({} from memory), puts {}, dropped puts {}",
            self.state.started.elapsed().as_secs(),
            queued,
            s.hellos,
            s.gets,
            s.hits,
            s.memory_hits,
            s.puts,
            s.dropped_puts,
        ));
    }
}

/// Serves the cache rooted at `cache_root` on the Unix socket `socket`
/// with the default [`Config`] (no idle timeout, no log) until the process
/// is stopped. A stale socket file is replaced.
///
/// # Errors
///
/// Binding errors.
pub fn serve(socket: &Path, cache_root: &Path, counters: Arc<Stats>) -> std::io::Result<()> {
    serve_with(socket, cache_root, counters, &Config::default())
}

/// [`serve`] with `config`.
///
/// # Errors
///
/// Binding errors.
pub fn serve_with(
    socket: &Path,
    cache_root: &Path,
    counters: Arc<Stats>,
    config: &Config,
) -> std::io::Result<()> {
    bind(socket, cache_root, counters, config)?.run()
}
