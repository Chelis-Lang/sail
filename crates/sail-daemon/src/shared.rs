//! The daemon's read path into the shared tier (charter rule 9, ADR 0017).
//!
//! On a local miss (memory and disk), the daemon looks the key up in the
//! configured shared store. The read and every check run on a helper
//! thread, and the daemon waits at most the configured timeout for them:
//! - an entry that passes every check of
//!   [`verify_entry`](sail_cache::shared::verify_entry) is served and
//!   written into the local tier with provenance `shared:<key id>`;
//! - anything else is a miss: no file, a failed check, a timeout, or a
//!   store that is backing off.
//!
//! **Bounds.** A timeout makes the daemon skip the store for a back-off
//! period, so a hung store costs at most one timeout per back-off, not one
//! per lookup. At most [`MAX_IN_FLIGHT`] reads run at once; a read stuck
//! in the kernel keeps its thread, never a connection. A key that missed
//! or was rejected is not looked up again for a short while (negative
//! caching), so concurrent sessions sharing a prefix ask the store once.

mod initialization;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use sail_cache::shared::{Rejection, SharedStore, Solver, TrustedKeys, Verified, verify_entry};
use sail_key::QueryKey;

/// Default time the daemon waits for one shared lookup: 250 ms.
pub const DEFAULT_SHARED_TIMEOUT: Duration = Duration::from_millis(250);

/// Default time the daemon skips a store after a lookup timed out: 30 s.
pub const DEFAULT_SHARED_BACKOFF: Duration = Duration::from_secs(30);

/// How long a key that missed in the shared store is not looked up again.
const NEGATIVE_TTL: Duration = Duration::from_secs(60);

/// Most keys remembered as shared misses; the set is emptied when full.
const MAX_NEGATIVE: usize = 1 << 16;

/// Most shared reads running at once. Beyond this, lookups are skipped.
pub const MAX_IN_FLIGHT: usize = 64;

/// How often the trusted keys are re-read from the trust directory.
const TRUST_RELOAD: Duration = Duration::from_secs(5);

/// Shared-tier settings.
#[derive(Debug, Clone)]
pub struct SharedConfig {
    /// The shared store's directory.
    pub store: PathBuf,
    /// The directory of trusted public keys (`*.pub`).
    pub trust_dir: PathBuf,
    /// How long one lookup may take before it counts as a miss.
    pub timeout: Duration,
    /// How long the store is skipped after a lookup timed out.
    pub backoff: Duration,
}

/// What one shared lookup found.
#[derive(Debug, Clone)]
pub(crate) enum Lookup {
    /// A verified entry.
    Hit(Verified),
    /// No entry.
    Miss,
    /// An entry that failed a check.
    Rejected(Rejection),
    /// The store did not answer in time.
    Timeout,
    /// HTTP transport or authentication failed without exhausting the deadline.
    Unavailable,
    /// Not looked up: no trusted keys, backing off, recently missed, or
    /// too many reads in flight.
    Skipped,
}

struct Trust {
    keys: Arc<TrustedKeys>,
    loaded: Instant,
}

/// The shared tier as the daemon uses it.
type Pending = Arc<(Mutex<Option<Lookup>>, Condvar)>;

fn publish(pending: &Pending, result: Lookup) {
    if let Ok(mut value) = pending.0.lock()
        && value.is_none()
    {
        *value = Some(result);
        pending.1.notify_all();
    }
}

#[cfg(test)]
type Phases = Arc<Mutex<Vec<(&'static str, Instant)>>>;

#[cfg(test)]
fn phase(phases: &Phases, label: &'static str) {
    if let Ok(mut records) = phases.lock() {
        records.push((label, Instant::now()));
    }
}

#[cfg(test)]
fn thread_cpu_fields(bytes: &[u8]) -> Option<u64> {
    // The comm field may contain spaces and parentheses. Field 3 starts after
    // its final closing parenthesis; fields 14/15 are user/system CPU ticks.
    let text = std::str::from_utf8(bytes).ok()?;
    let (_, fields) = text.rsplit_once(')')?;
    let mut fields = fields.split_whitespace();
    let user = fields.nth(11)?.parse::<u64>().ok()?;
    let system = fields.next()?.parse::<u64>().ok()?;
    user.checked_add(system)
}

#[cfg(test)]
fn thread_cpu_ticks() -> Option<u64> {
    use std::io::Read as _;
    let file = std::fs::File::open("/proc/thread-self/stat").ok()?;
    let mut bytes = Vec::with_capacity(512);
    file.take(4097).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= 4096)
        .then(|| thread_cpu_fields(&bytes))
        .flatten()
}

pub(crate) struct SharedTier {
    #[cfg(test)]
    phases: Phases,
    store: SharedStore,
    root: PathBuf,
    remote: Arc<initialization::State>,
    pending: Mutex<HashMap<String, Pending>>,
    coalesced: AtomicU64,
    config: SharedConfig,
    trust: Mutex<Option<Trust>>,
    in_flight: Arc<AtomicUsize>,
    backoff_until: Arc<Mutex<Option<Instant>>>,
    negative: Arc<Mutex<HashMap<[u8; 32], Instant>>>,
}

impl SharedTier {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            #[cfg(test)]
            phases: Arc::new(Mutex::new(Vec::new())),
            store: SharedStore::new(&config.store),
            root: config
                .trust_dir
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
            remote: Arc::new(initialization::State::default()),
            pending: Mutex::new(HashMap::new()),
            coalesced: AtomicU64::new(0),
            config,
            trust: Mutex::new(None),
            in_flight: Arc::new(AtomicUsize::new(0)),
            backoff_until: Arc::new(Mutex::new(None)),
            negative: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Always install a cheap observer so connect/disconnect works on an existing daemon.
    pub fn auto(config: Option<SharedConfig>, root: &std::path::Path) -> Self {
        let mut tier = Self::new(config.unwrap_or_else(|| SharedConfig {
            store: PathBuf::new(),
            trust_dir: sail_cache::shared::trust_dir(root),
            timeout: DEFAULT_SHARED_TIMEOUT,
            backoff: DEFAULT_SHARED_BACKOFF,
        }));
        tier.root = root.to_path_buf();
        tier
    }

    pub fn description(&self) -> Option<String> {
        if !self.configured() {
            return None;
        }
        self.remote
            .endpoint()
            .or_else(|| {
                crate::remote::load(&self.root)
                    .ok()
                    .flatten()
                    .map(|c| c.endpoint)
            })
            .or_else(|| {
                (!self.config.store.as_os_str().is_empty()).then(|| {
                    let text = self.config.store.to_string_lossy();
                    if text.starts_with("http://") || text.starts_with("https://") {
                        crate::remote::endpoint(&text).map_or_else(
                            |_| "<invalid HTTP endpoint>".to_owned(),
                            |u| u.to_string(),
                        )
                    } else {
                        text.into_owned()
                    }
                })
            })
    }

    pub fn remote_info(&self) -> Option<sail_api::protocol::RemoteInfo> {
        if !self.configured() {
            return None;
        }
        let mut info = self.remote.info()?;
        info.coalesced = self.coalesced.load(Ordering::Relaxed);
        Some(info)
    }

    pub fn config(&self) -> &SharedConfig {
        &self.config
    }

    fn configured(&self) -> bool {
        !self.config.store.as_os_str().is_empty()
            || self.root.join(crate::remote::CONFIG_FILE).is_file()
    }

    /// The trusted keys, re-read at most every [`TRUST_RELOAD`], and any
    /// problems found when they were last read.
    pub fn trusted(&self) -> (Arc<TrustedKeys>, Vec<String>) {
        let Ok(mut t) = self.trust.lock() else {
            return (Arc::new(TrustedKeys::default()), Vec::new());
        };
        if let Some(cur) = t.as_ref()
            && cur.loaded.elapsed() < TRUST_RELOAD
        {
            return (Arc::clone(&cur.keys), Vec::new());
        }
        let (keys, problems) = TrustedKeys::load_dir(&self.config.trust_dir);
        let keys = Arc::new(keys);
        *t = Some(Trust {
            keys: Arc::clone(&keys),
            loaded: Instant::now(),
        });
        (keys, problems)
    }

    #[cfg(test)]
    fn backing_off(&self) -> bool {
        self.backoff_until
            .lock()
            .ok()
            .and_then(|b| *b)
            .is_some_and(|until| Instant::now() < until)
    }

    #[cfg(test)]
    fn recently_missed(&self, key: &QueryKey) -> bool {
        self.negative
            .lock()
            .ok()
            .and_then(|n| n.get(&key.0).copied())
            .is_some_and(|at| at.elapsed() < NEGATIVE_TTL)
    }

    fn remember_miss(&self, key: &QueryKey) {
        if let Ok(mut n) = self.negative.lock() {
            if n.len() >= MAX_NEGATIVE {
                n.clear();
            }
            n.insert(key.0, Instant::now());
        }
    }

    /// Looks `key` up for a session whose solver is `local`.
    pub fn lookup(&self, key: &QueryKey, local: &Solver) -> Lookup {
        self.lookup_with(key, local, || {})
    }

    fn lookup_with(&self, key: &QueryKey, local: &Solver, after_spawn: impl FnOnce()) -> Lookup {
        #[cfg(test)]
        phase(&self.phases, "lookup-start");
        // No HTTP runtime is constructed on the foreground path.
        if !self.configured() {
            return Lookup::Skipped;
        }
        #[cfg(test)]
        phase(&self.phases, "foreground-ready");
        let id = format!("{}:{key}", local.summary());
        let (pending, leader) = {
            let Ok(mut active) = self.pending.lock() else {
                return Lookup::Skipped;
            };
            if let Some(pending) = active.get(&id) {
                (Arc::clone(pending), false)
            } else {
                if active.len() >= MAX_IN_FLIGHT {
                    return Lookup::Skipped;
                }
                let pending = Arc::new((Mutex::new(None), Condvar::new()));
                active.insert(id.clone(), Arc::clone(&pending));
                (pending, true)
            }
        };
        if !leader {
            self.coalesced.fetch_add(1, Ordering::Relaxed);
            let (lock, ready) = &*pending;
            let Ok(value) = lock.lock() else {
                return Lookup::Skipped;
            };
            let Ok((value, _)) =
                ready.wait_timeout_while(value, self.config.timeout, |v| v.is_none())
            else {
                return Lookup::Skipped;
            };
            return value.clone().unwrap_or(Lookup::Timeout);
        }
        let result = self.lookup_one(key, local, &pending, after_spawn);
        publish(&pending, result.clone());
        if let Ok(mut active) = self.pending.lock() {
            active.remove(&id);
        }
        result
    }

    fn lookup_one(
        &self,
        key: &QueryKey,
        local: &Solver,
        pending: &Pending,
        after_spawn: impl FnOnce(),
    ) -> Lookup {
        let (trusted, _) = self.trusted();
        if trusted.is_empty() {
            return Lookup::Skipped;
        }
        let Some(path) = self.store.entry_path(local, key) else {
            return Lookup::Skipped;
        };
        if self.in_flight.fetch_add(1, Ordering::SeqCst) >= MAX_IN_FLIGHT {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Lookup::Skipped;
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let in_flight = Arc::clone(&self.in_flight);
        let (key_c, local_c) = (*key, local.clone());
        let remote = Arc::clone(&self.remote);
        let root = self.root.clone();
        let config = self.config.clone();
        let backoff = Arc::clone(&self.backoff_until);
        let negative = Arc::clone(&self.negative);
        let store = self.config.store.to_string_lossy();
        let publication = Arc::clone(pending);
        let directory =
            !store.is_empty() && !store.starts_with("http://") && !store.starts_with("https://");
        #[cfg(test)]
        let phases = Arc::clone(&self.phases);
        #[cfg(test)]
        phase(&self.phases, "spawn-start");
        let spawned = std::thread::Builder::new()
            .name("sail-daemon-shared".to_owned())
            .spawn(move || {
                #[cfg(test)]
                phase(&phases, "helper-start");
                let resolved = remote.resolve(&root, &config, || {
                    if let Ok(mut entries) = negative.lock() {
                        entries.clear();
                    }
                    if let Ok(mut until) = backoff.lock() {
                        *until = None;
                    }
                });
                // A constructor may complete after the caller has already
                // published its fallback. Keep the validated client for later
                // requests, without starting an obsolete network request.
                let ended = publication.0.lock().map_or(true, |value| value.is_some());
                let skipped = ended
                    || backoff
                        .lock()
                        .ok()
                        .and_then(|until| *until)
                        .is_some_and(|until| Instant::now() < until)
                    || negative
                        .lock()
                        .ok()
                        .and_then(|entries| entries.get(&key_c.0).copied())
                        .is_some_and(|at| at.elapsed() < NEGATIVE_TTL);
                let data = if skipped {
                    None
                } else {
                    match resolved {
                        initialization::Resolution::Ready(Some(remote)) => Some(
                            remote
                                .get(&local_c, &key_c)
                                .map_err(|_| Rejection::Unreadable),
                        ),
                        initialization::Resolution::Ready(None) if directory => {
                            Some(SharedStore::read(&path))
                        }
                        initialization::Resolution::Ready(None)
                        | initialization::Resolution::Initializing => None,
                    }
                };
                #[cfg(test)]
                phase(&phases, "transport-returned");
                #[cfg(test)]
                let cpu_before = thread_cpu_ticks();
                #[cfg(test)]
                let validation_started = Instant::now();
                let result = match data {
                    None => Lookup::Skipped,
                    Some(Ok(Some(data))) => match verify_entry(&data, &key_c, &trusted, &local_c) {
                        Ok(verified) => Lookup::Hit(verified),
                        Err(reason) => Lookup::Rejected(reason),
                    },
                    Some(Ok(None)) => Lookup::Miss,
                    Some(Err(Rejection::Unreadable)) => Lookup::Unavailable,
                    Some(Err(reason)) => Lookup::Rejected(reason),
                };
                #[cfg(test)]
                phase(&phases, "classified");
                #[cfg(test)]
                {
                    let elapsed = validation_started.elapsed();
                    let cpu_after = thread_cpu_ticks();
                    let ticks = cpu_before
                        .zip(cpu_after)
                        .and_then(|(before, after)| after.checked_sub(before));
                    eprintln!(
                        "shared classification: wall_us={} thread_cpu_ticks={ticks:?}",
                        elapsed.as_micros()
                    );
                }
                in_flight.fetch_sub(1, Ordering::SeqCst);
                // Coalesced readers need not wait for the initiating client to
                // be scheduled again after this independently verified result.
                publish(&publication, result.clone());
                #[cfg(test)]
                phase(&phases, "published");
                let _ = tx.send(result);
            });
        if spawned.is_err() {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Lookup::Skipped;
        }
        after_spawn();
        self.finish_lookup(key, &rx)
    }

    fn finish_lookup(&self, key: &QueryKey, rx: &std::sync::mpsc::Receiver<Lookup>) -> Lookup {
        match rx.recv_timeout(self.config.timeout) {
            Ok(Lookup::Miss) => {
                self.remember_miss(key);
                Lookup::Miss
            }
            Ok(Lookup::Unavailable) => {
                if let Ok(mut b) = self.backoff_until.lock() {
                    *b = Some(Instant::now() + self.config.backoff);
                }
                Lookup::Unavailable
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Ok(mut b) = self.backoff_until.lock() {
                    *b = Some(Instant::now() + self.config.backoff);
                }
                Lookup::Timeout
            }
            Ok(Lookup::Rejected(r)) => {
                self.remember_miss(key);
                Lookup::Rejected(r)
            }
            Ok(result) => result,
            Err(RecvTimeoutError::Disconnected) => Lookup::Miss,
        }
    }

    /// Remembers a rejection the caller made of a verified entry (the
    /// serving policy), so the key is not looked up again for a while.
    pub fn remember_rejection(&self, key: &QueryKey) {
        self.remember_miss(key);
    }
}

#[cfg(test)]
mod http_tests {
    #![allow(clippy::unwrap_used, reason = "test assertions")]
    use super::*;
    use sail_cache::shared::{SecretKey, add_trusted, sign_entry};
    use std::io::{Read as _, Write as _};

    #[test]
    fn cpu_tick_parser_handles_comm_boundaries_truncation_and_overflow() {
        let fields = "S 0 0 0 0 0 0 0 0 0 0 13 7 0";
        assert_eq!(
            thread_cpu_fields(format!("123 (a ) name) {fields}").as_bytes()),
            Some(20)
        );
        assert_eq!(thread_cpu_fields(b"123 (name) S 0"), None);
        assert_eq!(
            thread_cpu_fields(b"123 (name) S 0 0 0 0 0 0 0 0 0 0 x 7"),
            None
        );
        assert_eq!(
            thread_cpu_fields(b"123 (name) S 0 0 0 0 0 0 0 0 0 0 18446744073709551615 1"),
            None
        );
        assert_eq!(thread_cpu_fields(b"\xff"), None);
    }

    #[test]
    fn credential_rotation_clears_previous_transport_backoff() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::temp_dir().join(format!("sail-rotate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let token = root.join("token");
        std::fs::write(&token, "0123456789abcdef0123456789abcdef").unwrap();
        std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
        crate::remote::connect(
            &root,
            &crate::remote::RemoteConfig {
                endpoint: "http://127.0.0.1:1".to_owned(),
                token_file: token.clone(),
                ca_file: None,
            },
        )
        .unwrap();
        let tier = SharedTier::auto(None, &root);
        assert!(matches!(
            tier.remote.resolve(&root, &tier.config, || {
                tier.negative.lock().unwrap().clear();
                *tier.backoff_until.lock().unwrap() = None;
            }),
            initialization::Resolution::Ready(Some(_))
        ));
        *tier.backoff_until.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        let key = QueryKey([22; 32]);
        tier.remember_miss(&key);
        std::fs::write(&token, "fedcba9876543210fedcba9876543210").unwrap();
        tier.remote.expire();
        assert!(matches!(
            tier.remote.resolve(&root, &tier.config, || {
                tier.negative.lock().unwrap().clear();
                *tier.backoff_until.lock().unwrap() = None;
            }),
            initialization::Resolution::Ready(Some(_))
        ));
        assert!(!tier.backing_off());
        assert!(!tier.recently_missed(&key));
        let _ = std::fs::remove_dir_all(root);
    }

    type RequestRead = (Option<usize>, Option<std::io::ErrorKind>, bool);

    struct HttpFixture {
        root: PathBuf,
        tier: Arc<SharedTier>,
        solver: Solver,
        key: QueryKey,
        stop: Arc<std::sync::atomic::AtomicBool>,
        count: Arc<AtomicUsize>,
        phases: Phases,
        requests: Arc<Mutex<Vec<RequestRead>>>,
        server: Option<std::thread::JoinHandle<()>>,
    }

    impl HttpFixture {
        fn establish_transport_and_trust(&self) {
            // These regressions exercise serving on an established transport.
            // Cold construction remains independently covered by the paused
            // constructor test, with the original foreground bound intact.
            let tier = Arc::clone(&self.tier);
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let setup = std::thread::spawn(move || {
                let started = Instant::now();
                let resolved = tier.remote.resolve(&tier.root, &tier.config, || {});
                let (trusted, problems) = tier.trusted();
                let established = matches!(resolved, initialization::Resolution::Ready(Some(_)))
                    && !trusted.is_empty()
                    && problems.is_empty();
                ready_tx.send((established, started.elapsed())).unwrap();
            });
            let (established, elapsed) = ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            setup.join().unwrap();
            assert!(established, "actual HTTP transport and trust must be ready");
            assert_eq!(self.count.load(Ordering::Relaxed), 0);
            assert_eq!(self.tier.remote_info().unwrap().requests, 0);
            eprintln!(
                "HTTP transport/trust setup: {elapsed:?}; serving timeout: {:?}",
                self.tier.config.timeout
            );
            let clock_ticks = std::process::Command::new("getconf")
                .arg("CLK_TCK")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|text| text.trim().parse::<u64>().ok());
            eprintln!("shared diagnostic clock_ticks_per_second={clock_ticks:?}");
        }

        fn phase_report(&self) -> Vec<(&'static str, u128)> {
            let mut records = self.phases.lock().unwrap().clone();
            records.extend(self.tier.remote.phases());
            records.sort_by_key(|(_, time)| *time);
            let first = records.iter().map(|(_, time)| *time).min().unwrap();
            records
                .iter()
                .map(|(name, time)| (*name, time.duration_since(first).as_micros()))
                .collect()
        }

        fn request_report(&self) -> Vec<RequestRead> {
            self.requests.lock().unwrap().clone()
        }

        fn close(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.server.take().unwrap().join().unwrap();
        }
    }

    impl Drop for HttpFixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(server) = self.server.take() {
                let _ = server.join();
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn http_fixture(label: &str, tampered: bool) -> HttpFixture {
        use std::os::unix::fs::PermissionsExt as _;
        let root =
            std::env::temp_dir().join(format!("sail-coalesce-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let secret = SecretKey::from_seed([8; 32]);
        add_trusted(&root, &secret.public()).unwrap();
        let solver = Solver {
            name: "z3".to_owned(),
            version: "4.16.0".to_owned(),
            binary_sha256: "ab".repeat(32),
            platform: "linux-x86_64".to_owned(),
        };
        let key = QueryKey([3; 32]);
        let mut data = sign_entry(&secret, &key, b"unsat\n", "check-sat", &solver, None, 1);
        if tampered {
            *data.last_mut().unwrap() = b'x';
        }
        let token = root.join("read.token");
        std::fs::write(&token, "0123456789abcdef0123456789abcdef").unwrap();
        std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let config = crate::remote::RemoteConfig {
            endpoint: format!("http://{}", listener.local_addr().unwrap()),
            token_file: token,
            ca_file: None,
        };
        crate::remote::connect(&root, &config).unwrap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let count = Arc::new(AtomicUsize::new(0));
        let counts = Arc::clone(&count);
        let phases = Arc::new(Mutex::new(Vec::new()));
        let server_phases = Arc::clone(&phases);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let server = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                };
                phase(&server_phases, "server-accepted");
                counts.fetch_add(1, Ordering::Relaxed);
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut input = [0u8; 4096];
                let read = stream.read(&mut input);
                let report = match read {
                    Ok(bytes) => (
                        Some(bytes),
                        None,
                        input[..bytes].windows(4).any(|v| v == b"\r\n\r\n"),
                    ),
                    Err(error) => (None, Some(error.kind()), false),
                };
                server_requests.lock().unwrap().push(report);
                phase(&server_phases, "server-read");
                std::thread::sleep(Duration::from_millis(75));
                phase(&server_phases, "server-delay-ended");
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    data.len()
                );
                let _ = stream.write_all(&data);
                phase(&server_phases, "server-sent");
            }
        });
        let mut tier = SharedTier::auto(None, &root);
        tier.phases = Arc::clone(&phases);
        HttpFixture {
            tier: Arc::new(tier),
            phases,
            requests,
            root,
            solver,
            key,
            stop,
            count,
            server: Some(server),
        }
    }

    #[test]
    fn concurrent_readers_coalesce_one_http_lookup() {
        let mut fixture = http_fixture("readers", false);
        fixture.establish_transport_and_trust();
        let (tier, solver, key) = (&fixture.tier, &fixture.solver, fixture.key);
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let workers = (0..8)
            .map(|_| {
                let tier = Arc::clone(tier);
                let barrier = Arc::clone(&barrier);
                let solver = solver.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    tier.lookup(&key, &solver)
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            let result = worker.join().unwrap();
            assert!(
                matches!(result, Lookup::Hit(_)),
                "lookup: {result:?}; remote: {:?}; phases_us: {:?}; request_read(bytes,error,headers_complete): {:?}",
                tier.remote_info(),
                fixture.phase_report(),
                fixture.request_report()
            );
        }
        fixture.close();
        eprintln!(
            "reader phases_us: {:?}; request_read(bytes,error,headers_complete): {:?}",
            fixture.phase_report(),
            fixture.request_report()
        );
        assert_eq!(fixture.count.load(Ordering::Relaxed), 1);
    }

    fn paused_initiator_lookup(fixture: &mut HttpFixture) -> (Lookup, Lookup) {
        let tier = Arc::clone(&fixture.tier);
        let solver = fixture.solver.clone();
        let key = fixture.key;
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let initiator = std::thread::spawn(move || {
            tier.lookup_with(&key, &solver, || {
                ready_tx.send(()).unwrap();
                // Dropping the sender also releases this owned worker on a
                // failed assertion; no process or detached waiter is leaked.
                let _ = resume_rx.recv();
            })
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(!initiator.is_finished());
        let started = Instant::now();
        let result = fixture.tier.lookup(&key, &fixture.solver);
        let elapsed = started.elapsed();
        let still_paused = !initiator.is_finished();
        resume_tx.send(()).unwrap();
        let initiating_result = initiator.join().unwrap();
        fixture.close();
        assert!(still_paused, "initiator must remain blocked during lookup");
        assert!(
            elapsed < fixture.tier.config.timeout,
            "elapsed: {elapsed:?}; remote: {:?}; phases_us: {:?}; request_read(bytes,error,headers_complete): {:?}",
            fixture.tier.remote_info(),
            fixture.phase_report(),
            fixture.request_report()
        );
        assert_eq!(fixture.count.load(Ordering::Relaxed), 1);
        eprintln!(
            "paused initiator phases_us: {:?}; request_read(bytes,error,headers_complete): {:?}",
            fixture.phase_report(),
            fixture.request_report()
        );
        (result, initiating_result)
    }

    #[test]
    fn verified_http_reply_reaches_readers_while_initiator_is_paused() {
        let mut fixture = http_fixture("paused", false);
        fixture.establish_transport_and_trust();
        let (result, initiating_result) = paused_initiator_lookup(&mut fixture);
        assert!(matches!(result, Lookup::Hit(_)), "lookup: {result:?}");
        assert!(matches!(initiating_result, Lookup::Hit(_)));
    }

    #[test]
    fn forged_http_reply_is_refused_while_initiator_is_paused() {
        let mut fixture = http_fixture("forged", true);
        fixture.establish_transport_and_trust();
        let (result, initiating_result) = paused_initiator_lookup(&mut fixture);
        assert!(matches!(result, Lookup::Rejected(_)), "lookup: {result:?}");
        assert!(matches!(initiating_result, Lookup::Rejected(_)));
    }

    #[test]
    fn paused_client_constructor_cannot_block_followers_or_status() {
        let mut fixture = http_fixture("construction", false);
        // A shorter supported request budget leaves room inside the unchanged
        // 250-ms service ceiling to detect a constructor with no deadline.
        Arc::get_mut(&mut fixture.tier).unwrap().config.timeout = Duration::from_millis(50);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *fixture.tier.remote.before_constructor.lock().unwrap() = Some(Arc::new(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.lock().unwrap().recv();
        }));
        let tier = Arc::clone(&fixture.tier);
        let solver = fixture.solver.clone();
        let key = fixture.key;
        let (initial_tx, initial_rx) = std::sync::mpsc::channel();
        let initiator = std::thread::spawn(move || {
            initial_tx.send(tier.lookup(&key, &solver)).unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(fixture.count.load(Ordering::Relaxed), 0);
        let started = Instant::now();
        assert!(fixture.tier.remote_info().is_none());
        assert!(fixture.tier.description().is_some());
        assert!(started.elapsed() < DEFAULT_SHARED_TIMEOUT);
        let tier = Arc::clone(&fixture.tier);
        let solver = fixture.solver.clone();
        let (returned_tx, returned_rx) = std::sync::mpsc::channel();
        let follower = std::thread::spawn(move || {
            returned_tx.send(tier.lookup(&key, &solver)).unwrap();
        });
        let result = returned_rx.recv_timeout(DEFAULT_SHARED_TIMEOUT).unwrap();
        assert!(matches!(result, Lookup::Timeout | Lookup::Skipped));
        assert!(matches!(
            initial_rx.recv_timeout(DEFAULT_SHARED_TIMEOUT).unwrap(),
            Lookup::Timeout
        ));
        initiator.join().unwrap();
        follower.join().unwrap();
        assert_eq!(fixture.count.load(Ordering::Relaxed), 0);
        release_tx.send(()).unwrap();
        fixture.close();
    }

    #[test]
    fn invalid_credentials_and_disconnect_do_not_reuse_old_transport() {
        let mut fixture = http_fixture("disconnect", false);
        fixture.establish_transport_and_trust();
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Hit(_)
        ));
        std::fs::write(fixture.root.join("read.token"), "invalid").unwrap();
        fixture.tier.remote.expire();
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Skipped
        ));
        assert!(fixture.tier.remote_info().is_none());
        assert_eq!(fixture.count.load(Ordering::Relaxed), 1);
        crate::remote::disconnect(&fixture.root).unwrap();
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Skipped
        ));
        fixture.close();
    }

    #[test]
    fn status_reads_existing_metrics_during_owned_transport_refresh() {
        let mut fixture = http_fixture("status-refresh", false);
        fixture.establish_transport_and_trust();
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Hit(_)
        ));
        fixture.tier.remote.expire();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *fixture.tier.remote.before_constructor.lock().unwrap() = Some(Arc::new(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.lock().unwrap().recv();
        }));
        let tier = Arc::clone(&fixture.tier);
        let refreshing =
            std::thread::spawn(move || tier.remote.resolve(&tier.root, &tier.config, || {}));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(!refreshing.is_finished());
        let started = Instant::now();
        let info = fixture.tier.remote_info().unwrap();
        assert_eq!(info.requests, 1);
        assert!(fixture.tier.description().is_some());
        assert!(started.elapsed() < DEFAULT_SHARED_TIMEOUT);
        release_tx.send(()).unwrap();
        assert!(matches!(
            refreshing.join().unwrap(),
            initialization::Resolution::Ready(Some(_))
        ));
        assert_eq!(fixture.tier.remote_info().unwrap().requests, 1);
        fixture.close();
    }

    #[test]
    fn disconnect_hides_cached_transport_without_initializing_or_waiting_for_drop() {
        let mut fixture = http_fixture("cached-disconnect", false);
        fixture.establish_transport_and_trust();
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Hit(_)
        ));
        assert!(fixture.tier.remote_info().is_some());
        crate::remote::disconnect(&fixture.root).unwrap();
        assert!(fixture.tier.description().is_none());
        assert!(fixture.tier.remote_info().is_none());
        assert!(matches!(
            fixture.tier.lookup(&fixture.key, &fixture.solver),
            Lookup::Skipped
        ));
        assert_eq!(fixture.count.load(Ordering::Relaxed), 1);
        fixture.close();
    }
}
