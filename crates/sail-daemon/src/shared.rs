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

struct Remote {
    loaded: Instant,
    client: Option<crate::remote::HttpClient>,
    endpoint: Option<String>,
}

pub(crate) struct SharedTier {
    store: SharedStore,
    root: PathBuf,
    remote: Mutex<Option<Remote>>,
    pending: Mutex<HashMap<String, Pending>>,
    coalesced: AtomicU64,
    config: SharedConfig,
    trust: Mutex<Option<Trust>>,
    in_flight: Arc<AtomicUsize>,
    backoff_until: Mutex<Option<Instant>>,
    negative: Mutex<HashMap<[u8; 32], Instant>>,
}

impl SharedTier {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            store: SharedStore::new(&config.store),
            root: config
                .trust_dir
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
            remote: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            coalesced: AtomicU64::new(0),
            config,
            trust: Mutex::new(None),
            in_flight: Arc::new(AtomicUsize::new(0)),
            backoff_until: Mutex::new(None),
            negative: Mutex::new(HashMap::new()),
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

    fn remote_client(&self) -> Option<crate::remote::HttpClient> {
        let mut current = self.remote.lock().ok()?;
        if let Some(remote) = current.as_ref()
            && remote.loaded.elapsed() < TRUST_RELOAD
        {
            return remote.client.clone();
        }
        let explicit = self.config.store.to_string_lossy();
        let configured = crate::remote::load(&self.root).ok().flatten();
        let config = if explicit.starts_with("http://") || explicit.starts_with("https://") {
            configured
                .filter(|c| c.endpoint.trim_end_matches('/') == explicit.trim_end_matches('/'))
                .or_else(|| {
                    std::env::var_os("SAIL_SHARED_TOKEN_FILE").map(|token| {
                        crate::remote::RemoteConfig {
                            endpoint: explicit.to_string(),
                            token_file: token.into(),
                            ca_file: std::env::var_os("SAIL_SHARED_CA_FILE").map(Into::into),
                        }
                    })
                })
        } else if explicit.is_empty() {
            configured
        } else {
            None
        };
        let endpoint = config
            .as_ref()
            .and_then(|c| crate::remote::endpoint(&c.endpoint).ok())
            .map(|url| url.to_string());
        let mut client =
            config.and_then(|c| crate::remote::HttpClient::new(&c, self.config.timeout).ok());
        if let (Some(new), Some(old)) = (&mut client, current.as_ref())
            && old.endpoint == endpoint
            && let Some(previous) = &old.client
        {
            new.retain_metrics(previous);
        }
        let changed = current.as_ref().is_none_or(|r| {
            r.endpoint != endpoint
                || match (&r.client, &client) {
                    (Some(old), Some(new)) => !old.same_configuration(new),
                    (None, None) => false,
                    _ => true,
                }
        });
        *current = Some(Remote {
            loaded: Instant::now(),
            client: client.clone(),
            endpoint,
        });
        if changed {
            if let Ok(mut negative) = self.negative.lock() {
                negative.clear();
            }
            if let Ok(mut backoff) = self.backoff_until.lock() {
                *backoff = None;
            }
        }
        client
    }

    pub fn description(&self) -> Option<String> {
        let _ = self.remote_client();
        self.remote
            .lock()
            .ok()
            .and_then(|r| r.as_ref().and_then(|r| r.endpoint.clone()))
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
        let _ = self.remote_client();
        let mut info = self.remote.lock().ok()?.as_ref()?.client.as_ref()?.info();
        info.coalesced = self.coalesced.load(Ordering::Relaxed);
        Some(info)
    }

    pub fn config(&self) -> &SharedConfig {
        &self.config
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

    fn backing_off(&self) -> bool {
        self.backoff_until
            .lock()
            .ok()
            .and_then(|b| *b)
            .is_some_and(|until| Instant::now() < until)
    }

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
        // Reload before backoff checks so disconnect or changed configuration takes effect.
        let _ = self.remote_client();
        if self.description().is_none() {
            return Lookup::Skipped;
        }
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
        if self.backing_off() || self.recently_missed(key) {
            return Lookup::Skipped;
        }
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
        let remote = self.remote_client();
        let store = self.config.store.to_string_lossy();
        let publication = Arc::clone(pending);
        let directory =
            !store.is_empty() && !store.starts_with("http://") && !store.starts_with("https://");
        let spawned = std::thread::Builder::new()
            .name("sail-daemon-shared".to_owned())
            .spawn(move || {
                let data = if let Some(remote) = remote {
                    remote
                        .get(&local_c, &key_c)
                        .map_err(|_| Rejection::Unreadable)
                } else if directory {
                    SharedStore::read(&path)
                } else {
                    Ok(None)
                };
                let result = match data {
                    Ok(Some(data)) => match verify_entry(&data, &key_c, &trusted, &local_c) {
                        Ok(verified) => Lookup::Hit(verified),
                        Err(reason) => Lookup::Rejected(reason),
                    },
                    Ok(None) => Lookup::Miss,
                    Err(Rejection::Unreadable) => Lookup::Unavailable,
                    Err(reason) => Lookup::Rejected(reason),
                };
                in_flight.fetch_sub(1, Ordering::SeqCst);
                // Coalesced readers need not wait for the initiating client to
                // be scheduled again after this independently verified result.
                publish(&publication, result.clone());
                let _ = tx.send(result);
            });
        if spawned.is_err() {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Lookup::Skipped;
        }
        after_spawn();
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
        assert!(tier.remote_client().is_some());
        *tier.backoff_until.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        let key = QueryKey([22; 32]);
        tier.remember_miss(&key);
        std::fs::write(&token, "fedcba9876543210fedcba9876543210").unwrap();
        tier.remote.lock().unwrap().as_mut().unwrap().loaded =
            Instant::now().checked_sub(TRUST_RELOAD).unwrap();
        assert!(tier.remote_client().is_some());
        assert!(!tier.backing_off());
        assert!(!tier.recently_missed(&key));
        let _ = std::fs::remove_dir_all(root);
    }

    struct HttpFixture {
        root: PathBuf,
        tier: Arc<SharedTier>,
        solver: Solver,
        key: QueryKey,
        stop: Arc<std::sync::atomic::AtomicBool>,
        count: Arc<AtomicUsize>,
        server: Option<std::thread::JoinHandle<()>>,
    }

    impl HttpFixture {
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
        let server = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                };
                counts.fetch_add(1, Ordering::Relaxed);
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut input = [0u8; 4096];
                let _ = stream.read(&mut input);
                std::thread::sleep(Duration::from_millis(75));
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    data.len()
                );
                let _ = stream.write_all(&data);
            }
        });
        HttpFixture {
            tier: Arc::new(SharedTier::auto(None, &root)),
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
                "lookup: {result:?}; remote: {:?}",
                tier.remote_info()
            );
        }
        fixture.close();
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
            "elapsed: {elapsed:?}"
        );
        assert_eq!(fixture.count.load(Ordering::Relaxed), 1);
        (result, initiating_result)
    }

    #[test]
    fn verified_http_reply_reaches_readers_while_initiator_is_paused() {
        let mut fixture = http_fixture("paused", false);
        let (result, initiating_result) = paused_initiator_lookup(&mut fixture);
        assert!(matches!(result, Lookup::Hit(_)), "lookup: {result:?}");
        assert!(matches!(initiating_result, Lookup::Hit(_)));
    }

    #[test]
    fn forged_http_reply_is_refused_while_initiator_is_paused() {
        let mut fixture = http_fixture("forged", true);
        let (result, initiating_result) = paused_initiator_lookup(&mut fixture);
        assert!(matches!(result, Lookup::Rejected(_)), "lookup: {result:?}");
        assert!(matches!(initiating_result, Lookup::Rejected(_)));
    }
}
