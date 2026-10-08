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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
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
#[derive(Debug)]
pub(crate) enum Lookup {
    /// A verified entry.
    Hit(Verified),
    /// No entry.
    Miss,
    /// An entry that failed a check.
    Rejected(Rejection),
    /// The store did not answer in time.
    Timeout,
    /// Not looked up: no trusted keys, backing off, recently missed, or
    /// too many reads in flight.
    Skipped,
}

struct Trust {
    keys: Arc<TrustedKeys>,
    loaded: Instant,
}

/// The shared tier as the daemon uses it.
pub(crate) struct SharedTier {
    store: SharedStore,
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
            config,
            trust: Mutex::new(None),
            in_flight: Arc::new(AtomicUsize::new(0)),
            backoff_until: Mutex::new(None),
            negative: Mutex::new(HashMap::new()),
        }
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
        let spawned = std::thread::Builder::new()
            .name("sail-daemon-shared".to_owned())
            .spawn(move || {
                let result = SharedStore::read(&path)
                    .map(|data| data.map(|d| verify_entry(&d, &key_c, &trusted, &local_c)));
                in_flight.fetch_sub(1, Ordering::SeqCst);
                let _ = tx.send(result);
            });
        if spawned.is_err() {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Lookup::Skipped;
        }
        match rx.recv_timeout(self.config.timeout) {
            Ok(Ok(Some(Ok(v)))) => Lookup::Hit(v),
            Ok(Ok(None)) => {
                self.remember_miss(key);
                Lookup::Miss
            }
            Ok(Ok(Some(Err(r))) | Err(r)) => {
                self.remember_miss(key);
                Lookup::Rejected(r)
            }
            Err(RecvTimeoutError::Disconnected) => Lookup::Miss,
            Err(RecvTimeoutError::Timeout) => {
                if let Ok(mut b) = self.backoff_until.lock() {
                    *b = Some(Instant::now() + self.config.backoff);
                }
                Lookup::Timeout
            }
        }
    }

    /// Remembers a rejection the caller made of a verified entry (the
    /// serving policy), so the key is not looked up again for a while.
    pub fn remember_rejection(&self, key: &QueryKey) {
        self.remember_miss(key);
    }
}
