//! The Unix-socket server.

use std::io::{BufReader, BufWriter};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sail_api::protocol::{
    Identity, Info, MemoryInfo, QueueInfo, Request, StatsSnapshot, write_get_response,
    write_hello_response, write_info_response, write_put_response, write_stats_response,
};
use sail_cache::memory::{MemoryTier, State as Slot, Value};
use sail_cache::shared::{Rejection, Solver};
use sail_cache::{DiskCache, FileStamp};
use sail_key::{QueryKey, SolverIdentity};

use crate::bundled::{self, BundledConfig, BundledTier};
use crate::digest_memo::{DigestMemo, MAX_RETAINED_BYTES};
use crate::identity_memo::{Found, IdentityMemo, Known};
use crate::log::Log;
use crate::shared::{Lookup, SharedConfig, SharedTier};
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
    /// The shared tier to consult on local misses (`None`: none).
    pub shared: Option<SharedConfig>,
    /// The release's bundled replies, consulted on local misses before the
    /// shared tier (`None`: none; ADR 0062).
    pub bundled: Option<BundledConfig>,
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
            shared: None,
            bundled: None,
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
    /// Local misses looked up in the shared store.
    pub shared_lookups: AtomicU64,
    /// Shared entries that passed every check and were served.
    pub shared_hits: AtomicU64,
    /// Shared entries that failed a check (including the two below).
    pub shared_rejected: AtomicU64,
    /// Shared entries signed for another solver: refused.
    pub shared_other_solver: AtomicU64,
    /// Shared entries signed for this solver on another platform class:
    /// refused (cross-class hits need a named opt-in mode, ADR 0010).
    pub shared_other_platform: AtomicU64,
    /// Shared lookups that timed out.
    pub shared_timeouts: AtomicU64,
    /// Local misses not looked up: the store was backing off after a
    /// timeout, the key missed recently, or too many reads were running.
    pub shared_skipped: AtomicU64,
    /// Local misses looked up in the bundled replies.
    pub bundled_lookups: AtomicU64,
    /// Bundled entries that passed every check and were served.
    pub bundled_hits: AtomicU64,
    /// Bundled entries that failed a check.
    pub bundled_rejected: AtomicU64,
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
            shared_lookups: self.shared_lookups.load(Ordering::Relaxed),
            shared_hits: self.shared_hits.load(Ordering::Relaxed),
            shared_rejected: self.shared_rejected.load(Ordering::Relaxed),
            shared_other_solver: self.shared_other_solver.load(Ordering::Relaxed),
            shared_other_platform: self.shared_other_platform.load(Ordering::Relaxed),
            shared_timeouts: self.shared_timeouts.load(Ordering::Relaxed),
            shared_skipped: self.shared_skipped.load(Ordering::Relaxed),
            bundled_lookups: self.bundled_lookups.load(Ordering::Relaxed),
            bundled_hits: self.bundled_hits.load(Ordering::Relaxed),
            bundled_rejected: self.bundled_rejected.load(Ordering::Relaxed),
        }
    }
}

struct State {
    disk: DiskCache,
    memory: MemoryTier,
    writer: Writer,
    stats: Arc<Stats>,
    shared: Option<SharedTier>,
    bundled: Option<Arc<BundledTier>>,
    /// Shared-tier rejections and timeouts logged so far (the first few
    /// are logged, to help diagnose a trust setup).
    shared_logged: AtomicU64,
    log: Log,
    started: Instant,
    socket: PathBuf,
    idle_timeout: Option<Duration>,
    queue_wait: Duration,
    /// Open connections.
    active: AtomicU64,
    /// When a connection last opened or closed.
    last_activity: Mutex<Instant>,
    /// Digest reuse requires equality of the entire freshly read executable.
    binary_digests: DigestMemo,
    /// Identities of settled executables, by complete stat stamp (ADR 0084).
    identities: IdentityMemo,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// Equal path, length and mtime do not establish equal executable bytes (#213).
// A settled native executable whose complete stat stamp (device, inode, size,
// mode, mtime and ctime) is unchanged keeps the identity a fresh
// identification recorded (ADR 0084): a same-path replacement or rewrite,
// even one restoring mtime, moves inode or ctime. Every other handshake is
// identified afresh from the complete current bytes, and any read or launch
// failure rejects it so the shim takes its fail-safe path.
fn identify(path: &Path, digests: &DigestMemo, memo: &IdentityMemo) -> Result<Identity, String> {
    let known = match memo.lookup(path) {
        Found::Hit(known) => known,
        Found::Miss(observed) => {
            let known = identify_fresh(path, digests)?;
            memo.record(path, &observed, &known);
            known
        }
    };
    Ok(Identity {
        name: "z3".to_owned(),
        version: known.version,
        binary_sha256: hex(&known.digest),
        platform: SolverIdentity::host_platform(),
        protocol_version: sail_api::protocol::PROTOCOL_VERSION,
    })
}

fn identify_fresh(path: &Path, digests: &DigestMemo) -> Result<Known, String> {
    let cannot_run = |e: std::io::Error| format!("cannot run {}: {e}", path.display());
    let version_process = std::process::Command::new(path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(cannot_run)?;
    // Compare the current bytes in place while the version command runs:
    // no executable-sized allocation per handshake.
    let current = digests.current(path);
    let out = version_process.wait_with_output().map_err(cannot_run)?;
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
    let digest = match current {
        Some(digest) => digest,
        None => digests.digest(
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        ),
    };
    Ok(Known { version, digest })
}

/// Which tier a `GET` hit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Memory,
    Disk,
    Shared,
}

fn to_wire(e: &sail_cache::Effort) -> sail_api::protocol::Effort {
    sail_api::protocol::Effort {
        limit: e.limit,
        used: e.used,
        reason: e.reason.clone(),
    }
}

impl State {
    /// The reply for `key`, and which tier it came from.
    ///
    /// A memory slot is served only if it is pending (its write is still
    /// queued, so no entry file exists yet) or its entry file still has the
    /// stamp it had when the slot was filled. Anything else falls through
    /// to a verified disk read, which refills the slot. A disk miss, for a
    /// session whose solver is known (`solver`), falls through to the
    /// shared tier.
    fn get(&self, key: &QueryKey, solver: &Solver, mode: &str) -> Option<(Arc<Value>, Source)> {
        if let Some((value, slot)) = self.memory.lookup(key) {
            if value.mode != mode {
                return None;
            }
            match slot {
                Slot::Pending => return Some((value, Source::Memory)),
                Slot::OnDisk(stamp) => {
                    if self.disk.stamp_for_hit(key) == Some(stamp) {
                        return Some((value, Source::Memory));
                    }
                    self.memory.remove_if(key, &value);
                }
            }
        }
        if let Some(stored) = self.disk.get_stamped(key) {
            if stored.meta.mode != mode {
                return None;
            }
            let value = Arc::new(Value {
                mode: stored.meta.mode,
                reply: stored.reply,
                effort: stored.meta.effort,
            });
            self.memory
                .insert_on_disk(key, Arc::clone(&value), stored.stamp);
            return Some((value, Source::Disk));
        }
        if mode != "default" {
            return None;
        }
        if let Some(bundled) = &self.bundled
            && let Some(value) = self.bundled_get(bundled, key, solver)
        {
            return Some((value, Source::Shared));
        }
        let shared = self.shared.as_ref()?;
        self.shared_get(shared, key, solver)
            .map(|v| (v, Source::Shared))
    }

    /// Looks `key` up in the release's bundled replies (ADR 0062). A
    /// verified entry is also queued for the local disk tier, with
    /// provenance `bundled:<key id>`. Every failure is a miss.
    fn bundled_get(
        &self,
        bundled: &BundledTier,
        key: &QueryKey,
        solver: &Solver,
    ) -> Option<Arc<Value>> {
        let lookup = bundled.lookup(key, solver);
        if matches!(lookup, bundled::Lookup::Miss) {
            return None;
        }
        self.stats.bundled_lookups.fetch_add(1, Ordering::Relaxed);
        let verified = match lookup {
            bundled::Lookup::Hit(v) => v,
            bundled::Lookup::Miss => return None,
            bundled::Lookup::Rejected(r) => {
                self.bundled_rejected(key, r);
                return None;
            }
        };
        let effort = verified.effort.as_ref().map(to_wire);
        if verified.command.starts_with("check-sat")
            && sail_api::policy::is_unknown(&verified.reply)
            && !sail_api::policy::unknown_servable(effort.as_ref())
        {
            self.bundled_rejected(key, Rejection::UnservableUnknown);
            return None;
        }
        self.stats.bundled_hits.fetch_add(1, Ordering::Relaxed);
        let provenance = format!("{}{}", bundled::PROVENANCE_PREFIX, verified.signer);
        Some(self.keep(key, verified, provenance, solver))
    }

    fn bundled_rejected(&self, key: &QueryKey, r: Rejection) {
        self.stats.bundled_rejected.fetch_add(1, Ordering::Relaxed);
        self.shared_log(&format!("bundled entry {key} rejected: {}", r.name()));
    }

    /// Serves a verified copied entry and queues it for the local disk tier
    /// under `provenance`. Never waits for room: a dropped copy only costs
    /// a future lookup.
    fn keep(
        &self,
        key: &QueryKey,
        verified: sail_cache::shared::Verified,
        provenance: String,
        solver: &Solver,
    ) -> Arc<Value> {
        let value = Arc::new(Value {
            mode: "default".to_owned(),
            reply: verified.reply,
            effort: verified.effort,
        });
        let job = Job {
            key: *key,
            value: Arc::clone(&value),
            command: verified.command,
            provenance,
            solver: solver.summary(),
        };
        if !self.disk.contains(key)
            && self.writer.submit(&self.memory, job, Duration::ZERO) == Submitted::Dropped
        {
            self.stats.dropped_puts.fetch_add(1, Ordering::Relaxed);
        }
        value
    }

    /// Looks `key` up in the shared tier. A verified entry is also queued
    /// for the local disk tier, with provenance `shared:<key id>` (charter
    /// rule 7). Every failure is a miss.
    fn shared_get(
        &self,
        shared: &SharedTier,
        key: &QueryKey,
        solver: &Solver,
    ) -> Option<Arc<Value>> {
        let lookup = shared.lookup(key, solver);
        if !matches!(lookup, Lookup::Skipped) {
            self.stats.shared_lookups.fetch_add(1, Ordering::Relaxed);
        }
        let verified = match lookup {
            Lookup::Hit(v) => v,
            Lookup::Miss => return None,
            Lookup::Skipped => {
                self.stats.shared_skipped.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            Lookup::Unavailable => {
                self.shared_log("shared store unavailable; lookups backing off");
                return None;
            }
            Lookup::Timeout => {
                self.stats.shared_timeouts.fetch_add(1, Ordering::Relaxed);
                self.shared_log(&format!(
                    "shared store timed out on {key}: skipping it for {}s",
                    shared.config().backoff.as_secs()
                ));
                return None;
            }
            Lookup::Rejected(r) => {
                self.shared_rejected(key, r);
                return None;
            }
        };
        let effort = verified.effort.as_ref().map(to_wire);
        if verified.command.starts_with("check-sat")
            && sail_api::policy::is_unknown(&verified.reply)
            && !sail_api::policy::unknown_servable(effort.as_ref())
        {
            shared.remember_rejection(key);
            self.shared_rejected(key, Rejection::UnservableUnknown);
            return None;
        }
        self.stats.shared_hits.fetch_add(1, Ordering::Relaxed);
        let provenance = verified.provenance();
        Some(self.keep(key, verified, provenance, solver))
    }

    fn shared_rejected(&self, key: &QueryKey, r: Rejection) {
        self.stats.shared_rejected.fetch_add(1, Ordering::Relaxed);
        match r {
            Rejection::OtherSolver => {
                self.stats
                    .shared_other_solver
                    .fetch_add(1, Ordering::Relaxed);
            }
            Rejection::OtherPlatform => {
                self.stats
                    .shared_other_platform
                    .fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        self.shared_log(&format!("shared entry {key} rejected: {}", r.name()));
    }

    /// Logs the first few shared-tier problems.
    fn shared_log(&self, msg: &str) {
        const LIMIT: u64 = 20;
        let n = self.shared_logged.fetch_add(1, Ordering::Relaxed);
        if n < LIMIT {
            self.log.line(msg);
        } else if n == LIMIT {
            self.log
                .line("further shared-tier problems are counted in STATS, not logged");
        }
    }

    /// Stores a reply: queued for disk and held in memory meanwhile. A key
    /// whose entry file exists, or whose write is already queued, is left
    /// alone, so memory never holds bytes that differ from the entry disk
    /// has or will have. A reply that is not exactly one plausible response
    /// frame for its command is refused, so it can never be served.
    fn put(
        &self,
        key: QueryKey,
        meta: sail_api::protocol::PutMeta,
        reply: Vec<u8>,
        solver: &Solver,
        mode: &str,
    ) {
        if !sail_cache::shape_ok(&meta.command, &reply) {
            self.log.line(&format!(
                "refused PUT {key}: reply does not have the shape of one `{}` response",
                meta.command
            ));
            return;
        }
        if self.disk.contains(&key) {
            return;
        }
        let job = Job {
            key,
            value: Arc::new(Value {
                mode: mode.to_owned(),
                reply,
                effort: meta.effort.map(|e| sail_cache::Effort {
                    limit: e.limit,
                    used: e.used,
                    reason: e.reason,
                }),
            }),
            command: meta.command,
            provenance: meta.provenance,
            solver: solver.summary(),
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
            sail_version: sail_api::VERSION.to_owned(),
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
            shared: self.shared.as_ref().and_then(SharedTier::status),
            bundled: self.bundled.as_ref().map(|b| b.describe()),
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
        // The solver this connection's session runs, from its `HELLO`. It
        // scopes shared lookups and is recorded with every `PUT`.
        let mut session: Option<(Solver, sail_api::protocol::VerificationMode)> = None;
        loop {
            let Ok(Some(request)) = Request::read(&mut reader) else {
                return;
            };
            let result = match request {
                Request::Hello(hello) => {
                    self.stats.hellos.fetch_add(1, Ordering::Relaxed);
                    // One successful handshake binds this connection for life.
                    // Renegotiation cannot silently switch its solver or mode.
                    if session.is_some() {
                        return;
                    }
                    let id = match identify(
                        Path::new(&hello.solver_path),
                        &self.binary_digests,
                        &self.identities,
                    ) {
                        Ok(id) => id,
                        Err(error) => {
                            let _ = write_hello_response(&mut writer, Err(&error));
                            return;
                        }
                    };
                    session = Some((
                        Solver {
                            name: id.name.clone(),
                            version: id.version.clone(),
                            binary_sha256: id.binary_sha256.clone(),
                            platform: id.platform.clone(),
                        },
                        hello.mode,
                    ));
                    write_hello_response(&mut writer, Ok(&id))
                }
                Request::Get(key) => {
                    let Some((solver, mode)) = &session else {
                        return;
                    };
                    self.stats.gets.fetch_add(1, Ordering::Relaxed);
                    let found = self.get(&QueryKey(key), solver, mode.name());
                    if let Some((_, source)) = &found {
                        self.stats.hits.fetch_add(1, Ordering::Relaxed);
                        if *source == Source::Memory {
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
                    let Some((solver, mode)) = &session else {
                        return;
                    };
                    self.stats.puts.fetch_add(1, Ordering::Relaxed);
                    // A failed or dropped store only costs a future miss.
                    self.put(QueryKey(key), meta, reply, solver, mode.name());
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
    stop: Arc<StopSignal>,
    socket: PathBuf,
}

/// Set once, by the first [`Handle::shutdown`].
#[derive(Debug, Default)]
struct StopSignal {
    set: AtomicBool,
    lock: Mutex<()>,
    cv: Condvar,
}

impl StopSignal {
    fn is_set(&self) -> bool {
        self.set.load(Ordering::SeqCst)
    }

    fn wait(&self) {
        let Ok(mut guard) = self.lock.lock() else {
            return;
        };
        while !self.is_set() {
            guard = match self.cv.wait(guard) {
                Ok(g) => g,
                Err(_) => return,
            };
        }
    }
}

impl Handle {
    /// Asks the server to stop: it stops accepting connections, waits
    /// briefly for open ones, writes every queued entry to disk, removes
    /// its socket and returns from [`Server::run`]. This works even if the
    /// socket file has been removed.
    pub fn shutdown(&self) {
        if !self.stop.set.swap(true, Ordering::SeqCst) {
            if let Ok(_guard) = self.stop.lock.lock() {
                self.stop.cv.notify_all();
            }
            // Wake the accept thread so it can exit (best effort: it is
            // left blocked if the socket file is gone).
            let _ = UnixStream::connect(&self.socket);
        }
    }
}

/// A bound daemon, ready to [`run`](Server::run).
pub struct Server {
    listener: Option<UnixListener>,
    state: Arc<State>,
    socket_stamp: Option<FileStamp>,
    stop: Arc<StopSignal>,
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
    // This local daemon owns trusted cache writes. Its socket is private to
    // the user before any requests are accepted, regardless of the umask.
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let socket_stamp = std::fs::symlink_metadata(socket)
        .ok()
        .map(|m| FileStamp::of(&m));
    let state = Arc::new(State {
        disk: DiskCache::new(cache_root),
        memory: MemoryTier::new(config.memory_bytes),
        writer: Writer::new(config.queue_bytes),
        stats: counters,
        shared: Some(SharedTier::auto(config.shared.clone(), cache_root)),
        bundled: config
            .bundled
            .as_ref()
            .map(|b| Arc::new(BundledTier::new(b))),
        shared_logged: AtomicU64::new(0),
        log: Log::open(config.log.as_deref()),
        started: Instant::now(),
        socket: socket.to_path_buf(),
        idle_timeout: config.idle_timeout,
        queue_wait: config.queue_wait,
        active: AtomicU64::new(0),
        last_activity: Mutex::new(Instant::now()),
        binary_digests: DigestMemo::new(MAX_RETAINED_BYTES),
        identities: IdentityMemo::new(),
    });
    if let Some(shared) = &state.shared {
        shared.prepare();
    }
    if state.bundled.is_some() {
        let s = Arc::clone(&state);
        // Without the helper the tier never loads, and every lookup misses.
        let _ = std::thread::Builder::new()
            .name("sail-bundled-load".to_owned())
            .spawn(move || {
                if let Some(bundled) = &s.bundled {
                    bundled.load();
                    s.log
                        .line(&format!("bundled replies: {}", bundled.describe()));
                }
            });
    }
    if let Some(shared) = &state.shared
        && let Some(info) = shared.status()
    {
        state.log.line(&format!(
            "shared tier: store {}, trust {} (cached keys: {}), timeout {} ms",
            info.store,
            info.trust_dir,
            info.trusted_keys.join(", "),
            info.timeout_ms
        ));
    }
    let w = Arc::clone(&state);
    let delay = config.write_delay;
    let writer_thread = std::thread::Builder::new()
        .name("sail-daemon-writer".to_owned())
        .spawn(move || w.writer.run(&w.disk, &w.memory, delay, &w.log))?;
    state.log.line(&format!(
        "started: sail {}, protocol {}, socket {}, cache {}, memory {} bytes, queue {} bytes, idle timeout {}",
        sail_api::VERSION,
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
        listener: Some(listener),
        state,
        socket_stamp,
        stop: Arc::new(StopSignal::default()),
        writer_thread: Some(writer_thread),
    })
}

impl Server {
    /// A handle that stops this server.
    #[must_use]
    pub fn handle(&self) -> Handle {
        Handle {
            stop: Arc::clone(&self.stop),
            socket: self.state.socket.clone(),
        }
    }

    /// Serves until stopped by a [`Handle`] or the idle timeout, then
    /// drains the write-behind queue and removes the socket.
    ///
    /// # Errors
    ///
    /// Thread creation errors.
    pub fn run(self) -> std::io::Result<()> {
        self.run_with_activity(Arc::new(AtomicU64::new(0)))
    }

    /// Serves while also accounting for queued and running verification jobs.
    /// Active jobs keep the service alive even between solver connections.
    ///
    /// # Errors
    ///
    /// Thread creation errors.
    pub fn run_with_activity(mut self, activity: Arc<AtomicU64>) -> std::io::Result<()> {
        if let Some(timeout) = self.state.idle_timeout {
            let state = Arc::clone(&self.state);
            let handle = self.handle();
            let tick = (timeout / 4).clamp(Duration::from_millis(10), Duration::from_secs(1));
            std::thread::Builder::new()
                .name("sail-daemon-idle".to_owned())
                .spawn(move || {
                    while !handle.stop.is_set() {
                        std::thread::sleep(tick);
                        if activity.load(Ordering::SeqCst) > 0 {
                            if let Ok(mut last) = state.last_activity.lock() {
                                *last = Instant::now();
                            }
                        } else if state.active.load(Ordering::SeqCst) == 0
                            && state.idle_for() >= timeout
                        {
                            state.log.line(&format!(
                                "idle for {}s with no connections: stopping",
                                timeout.as_secs()
                            ));
                            handle.shutdown();
                            return;
                        }
                    }
                })?;
        }
        if let Some(listener) = self.listener.take() {
            let state = Arc::clone(&self.state);
            let stop = Arc::clone(&self.stop);
            std::thread::Builder::new()
                .name("sail-daemon-accept".to_owned())
                .spawn(move || accept_loop(&listener, &state, &stop))?;
        }
        self.stop.wait();
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
            "stopped after {}s: {} writes drained; hellos {}, gets {}, hits {} ({} from memory, {} from shared), puts {}, dropped puts {}",
            self.state.started.elapsed().as_secs(),
            queued,
            s.hellos,
            s.gets,
            s.hits,
            s.memory_hits,
            s.shared_hits,
            s.puts,
            s.dropped_puts,
        ));
    }
}

/// Accepts connections until the server is stopping, serving each on its
/// own thread. A connection that raced the stop is still served.
fn accept_loop(listener: &UnixListener, state: &Arc<State>, stop: &StopSignal) {
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        state.active.fetch_add(1, Ordering::SeqCst);
        state.touch();
        let worker = Arc::clone(state);
        let spawned = std::thread::Builder::new()
            .name("sail-daemon-conn".to_owned())
            .spawn(move || {
                worker.handle(conn);
                worker.touch();
                worker.active.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            state.active.fetch_sub(1, Ordering::SeqCst);
        }
        if stop.is_set() {
            return;
        }
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
