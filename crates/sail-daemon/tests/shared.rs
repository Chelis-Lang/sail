//! The shared tier's read path (charter rule 9, ADR 0017): fault injection
//! against a real daemon, with synthetic replies, so no solver is needed.
//!
//! Every entry that fails a check must be a miss: forged (valid hash, wrong
//! answer, no valid signature), signed by an untrusted key, signed for
//! another solver or platform, corrupted, truncated, an `unknown` the
//! serving policy refuses, or behind a hung store. A verified entry is
//! served, copied into the local tier with provenance `shared:<key id>`,
//! and never published again.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sail_api::client::Client;
use sail_api::protocol::StatsSnapshot;
use sail_cache::shared::{
    PublishOptions, SecretKey, SharedStore, Solver, add_trusted, publish, sign_entry, trust_dir,
};
use sail_cache::{DiskCache, Effort};
use sail_key::QueryKey;

/// A short lookup timeout, so the hung-store test is quick.
const TIMEOUT: Duration = Duration::from_millis(200);

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = PathBuf::from(format!("/tmp/sail-ds{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(std::fs::create_dir_all(&d).is_ok());
        Self(d)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A daemon with a shared store, a trusted key and a fake solver.
struct Rig {
    _dir: Scratch,
    socket: PathBuf,
    cache: PathBuf,
    store: SharedStore,
    solver_path: String,
    solver: Solver,
    trusted: SecretKey,
    handle: sail_daemon::Handle,
}

/// A fake solver: `--version` is all the daemon runs.
fn fake_solver(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("z3");
    assert!(std::fs::write(&path, "#!/bin/sh\necho 'Z3 version 4.16.0 - 64 bit'\n").is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    path
}

impl Rig {
    fn new(tag: &str, backoff: Duration) -> Self {
        let dir = Scratch::new(tag);
        let (socket, cache, store) = (
            dir.0.join("d.sock"),
            dir.0.join("cache"),
            dir.0.join("store"),
        );
        let trusted = SecretKey::from_seed([11; 32]);
        assert!(add_trusted(&cache, &trusted.public()).is_ok());
        let config = sail_daemon::Config {
            shared: Some(sail_daemon::SharedConfig {
                store: store.clone(),
                trust_dir: trust_dir(&cache),
                timeout: TIMEOUT,
                backoff,
            }),
            ..sail_daemon::Config::default()
        };
        let stats = Arc::new(sail_daemon::Stats::default());
        let server = match sail_daemon::bind(&socket, &cache, stats, &config) {
            Ok(s) => s,
            Err(e) => unreachable!("cannot bind: {e}"),
        };
        let handle = server.handle();
        std::thread::spawn(move || server.run());
        let solver_path = fake_solver(&dir.0).display().to_string();
        let mut rig = Self {
            _dir: dir,
            socket,
            cache,
            store: SharedStore::new(store),
            solver_path,
            solver: Solver {
                name: String::new(),
                version: String::new(),
                binary_sha256: String::new(),
                platform: String::new(),
            },
            trusted,
            handle,
        };
        let id = rig.session().1;
        rig.solver = id;
        rig
    }

    /// A connection that has said `HELLO`, as a shim session does, and
    /// the solver identity the daemon reported.
    fn session(&self) -> (Client, Solver) {
        let mut c = match Client::connect(&self.socket) {
            Ok(c) => c,
            Err(e) => unreachable!("cannot connect: {e}"),
        };
        let id = match c.hello(&self.solver_path) {
            Ok(id) => id,
            Err(e) => unreachable!("HELLO failed: {e}"),
        };
        let solver = Solver {
            name: id.name,
            version: id.version,
            binary_sha256: id.binary_sha256,
            platform: id.platform,
        };
        (c, solver)
    }

    fn stats(&self) -> StatsSnapshot {
        Client::connect(&self.socket)
            .and_then(|mut c| c.stats())
            .unwrap_or_default()
    }

    fn drain(&self) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            let queued = Client::connect(&self.socket)
                .and_then(|mut c| c.info())
                .map(|i| i.queue.entries);
            if matches!(queued, Ok(0) | Err(_)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Writes `data` as the shared entry for `key` (for this rig's solver).
    fn plant(&self, key: &QueryKey, data: &[u8]) -> PathBuf {
        let path = self.store.entry_path(&self.solver, key);
        let Some(path) = path else {
            unreachable!("no entry path")
        };
        assert!(SharedStore::write(&path, data).is_ok());
        path
    }

    fn signed(&self, key: &QueryKey, reply: &[u8], command: &str) -> Vec<u8> {
        sign_entry(&self.trusted, key, reply, command, &self.solver, None, 1)
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.handle.shutdown();
    }
}

fn key(i: u8) -> QueryKey {
    let mut k = [0x33; 32];
    k[0] = i;
    QueryKey(k)
}

fn get(c: &mut Client, key: &QueryKey) -> Option<Vec<u8>> {
    match c.get(&key.0) {
        Ok(r) => r,
        Err(e) => unreachable!("GET failed: {e}"),
    }
}

/// Replaces the reply of a signed entry and fixes up its hash, as a forger
/// without the key would: the hash is valid, the signature is not.
fn reanswer(data: &[u8], reply: &[u8]) -> Vec<u8> {
    let nl = data.iter().position(|&b| b == b'\n').unwrap_or(0);
    let mut header: serde_json::Value =
        serde_json::from_slice(data.get(..nl).unwrap_or_default()).unwrap_or_default();
    let digest = sail_key::sha256(reply);
    header["reply_sha256"] = digest
        .iter()
        .fold(String::new(), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
        .into();
    let mut out = serde_json::to_vec(&header).unwrap_or_default();
    out.push(b'\n');
    out.extend_from_slice(reply);
    out
}

#[test]
fn a_verified_entry_is_served_copied_locally_and_never_republished() {
    let rig = Rig::new("hit", Duration::from_secs(30));
    let k = key(1);
    rig.plant(&k, &rig.signed(&k, b"unsat\n", "check-sat"));
    let (mut c, _) = rig.session();
    assert_eq!(get(&mut c, &k).as_deref(), Some(&b"unsat\n"[..]));
    let s = rig.stats();
    assert_eq!((s.shared_lookups, s.shared_hits, s.hits), (1, 1, 1));

    // The local tier now holds it, with provenance `shared:<key id>`.
    rig.drain();
    let disk = DiskCache::new(&rig.cache);
    let local = disk.get(&k);
    assert_eq!(
        local
            .as_ref()
            .map(|(m, r)| (m.provenance.as_str(), r.as_slice())),
        Some((
            format!("shared:{}", rig.trusted.public().id()).as_str(),
            &b"unsat\n"[..]
        ))
    );
    assert_eq!(
        local.map(|(m, _)| m.solver),
        Some(rig.solver.summary()),
        "the local copy records the solver identity"
    );
    // Later lookups are local.
    let (mut c2, _) = rig.session();
    assert_eq!(get(&mut c2, &k).as_deref(), Some(&b"unsat\n"[..]));
    assert_eq!(rig.stats().shared_lookups, 1);

    // Publishing this cache never re-exports the shared copy.
    let other_store = SharedStore::new(rig.cache.join("..").join("other-store"));
    let r = publish(
        &disk,
        &other_store,
        &SecretKey::from_seed([12; 32]),
        &PublishOptions::default(),
        &|_| true,
    );
    assert_eq!((r.from_shared, r.published), (1, 0));
}

/// Shared entries that must each be a miss, with what is wrong with them.
fn rejection_cases(rig: &Rig) -> Vec<(&'static str, QueryKey, Vec<u8>)> {
    let untrusted = SecretKey::from_seed([99; 32]);
    let good = |k: &QueryKey| rig.signed(k, b"unsat\n", "check-sat");
    let mut other_solver = rig.solver.clone();
    other_solver.binary_sha256 = "cd".repeat(32);
    let mut other_platform = rig.solver.clone();
    "plan9-mips".clone_into(&mut other_platform.platform);
    let near = Effort {
        limit: Some(100),
        used: Some(90),
        reason: Some("(:reason-unknown \"incomplete quantifiers\")".to_owned()),
    };
    vec![
        // A forgery: valid hash, wrong answer, no valid signature.
        ("forged", key(10), reanswer(&good(&key(10)), b"sat\n")),
        (
            "untrusted signer",
            key(11),
            sign_entry(
                &untrusted,
                &key(11),
                b"sat\n",
                "check-sat",
                &rig.solver,
                None,
                1,
            ),
        ),
        (
            "other solver",
            key(12),
            sign_entry(
                &rig.trusted,
                &key(12),
                b"sat\n",
                "check-sat",
                &other_solver,
                None,
                1,
            ),
        ),
        (
            "other platform",
            key(13),
            sign_entry(
                &rig.trusted,
                &key(13),
                b"sat\n",
                "check-sat",
                &other_platform,
                None,
                1,
            ),
        ),
        ("corrupt", key(14), {
            let mut d = good(&key(14));
            if let Some(last) = d.iter_mut().rev().nth(2) {
                *last ^= 0x20;
            }
            d
        }),
        ("truncated", key(15), {
            let d = good(&key(15));
            d.get(..d.len() / 2).unwrap_or_default().to_vec()
        }),
        ("empty", key(16), Vec::new()),
        ("another key's entry", key(17), good(&key(18))),
        (
            "near-limit unknown",
            key(19),
            sign_entry(
                &rig.trusted,
                &key(19),
                b"unknown\n",
                "check-sat",
                &rig.solver,
                Some(&near),
                1,
            ),
        ),
        (
            "wrong shape",
            key(20),
            rig.signed(&key(20), b"(error \"x\")\n", "check-sat"),
        ),
    ]
}

#[test]
fn unverifiable_entries_are_misses() {
    let rig = Rig::new("reject", Duration::from_secs(30));
    let cases = rejection_cases(&rig);
    for (_, k, data) in &cases {
        rig.plant(k, data);
    }
    // A directory where an entry should be.
    let dir_at = rig.store.entry_path(&rig.solver, &key(21));
    assert!(dir_at.is_some_and(|p| std::fs::create_dir_all(p).is_ok()));

    let (mut c, _) = rig.session();
    for (what, k, _) in &cases {
        assert_eq!(get(&mut c, k), None, "{what} must be a miss");
    }
    assert_eq!(get(&mut c, &key(21)), None, "directory");
    let s = rig.stats();
    assert_eq!(s.shared_hits, 0);
    assert_eq!(s.shared_rejected, 11, "{s:?}");
    assert_eq!((s.shared_other_solver, s.shared_other_platform), (1, 1));
    // Nothing reached the local tier.
    rig.drain();
    assert_eq!(DiskCache::new(&rig.cache).usage().entries, 0);
    // A rejected key is not looked up again for a while.
    assert_eq!(get(&mut c, &key(10)), None);
    let again = rig.stats();
    assert_eq!(
        (again.shared_lookups, again.shared_skipped),
        (s.shared_lookups, s.shared_skipped + 1)
    );
}

#[test]
fn far_from_limit_unknowns_are_served_and_misses_are_negative_cached() {
    let rig = Rig::new("unknown", Duration::from_secs(30));
    let far = Effort {
        limit: Some(1000),
        used: Some(3),
        reason: Some("(:reason-unknown \"incomplete quantifiers\")".to_owned()),
    };
    let k = key(30);
    rig.plant(
        &k,
        &sign_entry(
            &rig.trusted,
            &k,
            b"unknown\n",
            "check-sat",
            &rig.solver,
            Some(&far),
            1,
        ),
    );
    let (mut c, _) = rig.session();
    let got = c.get_with_effort(&k.0).ok().flatten();
    assert_eq!(
        got.as_ref()
            .map(|(r, e)| (r.as_slice(), e.as_ref().and_then(|e| e.used))),
        Some((&b"unknown\n"[..], Some(3))),
        "the signed effort travels with the reply, for the shim's policy"
    );
    // A missing key is looked up once.
    assert_eq!(get(&mut c, &key(31)), None);
    assert_eq!(get(&mut c, &key(31)), None);
    let s = rig.stats();
    assert_eq!((s.shared_lookups, s.shared_skipped), (2, 1));
}

#[test]
fn sessions_without_a_solver_identity_never_consult_the_shared_tier() {
    let rig = Rig::new("nohello", Duration::from_secs(30));
    let k = key(40);
    rig.plant(&k, &rig.signed(&k, b"unsat\n", "check-sat"));
    let mut c = match Client::connect(&rig.socket) {
        Ok(c) => c,
        Err(e) => unreachable!("{e}"),
    };
    assert!(c.get(&k.0).is_err());
    assert_eq!(rig.stats().shared_lookups, 0);
}

#[test]
fn a_hung_store_costs_one_timeout_then_is_skipped() {
    let rig = Rig::new("hung", Duration::from_secs(30));
    // A FIFO at an entry's path: opening it blocks until a writer comes,
    // like a hung network mount.
    let k = key(50);
    let Some(fifo) = rig.store.entry_path(&rig.solver, &k) else {
        unreachable!("no entry path")
    };
    assert!(
        fifo.parent()
            .is_some_and(|p| std::fs::create_dir_all(p).is_ok())
    );
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    assert!(made.is_ok_and(|s| s.success()));
    let later = key(51);
    rig.plant(&later, &rig.signed(&later, b"unsat\n", "check-sat"));

    let (mut c, _) = rig.session();
    let start = Instant::now();
    assert_eq!(get(&mut c, &k), None);
    let first = start.elapsed();
    assert!(first >= TIMEOUT, "returned before the timeout: {first:?}");
    assert!(first < TIMEOUT * 5, "took {first:?}");
    // During the back-off even a valid entry is skipped, at once.
    let start = Instant::now();
    assert_eq!(get(&mut c, &later), None);
    assert!(start.elapsed() < TIMEOUT, "took {:?}", start.elapsed());
    let s = rig.stats();
    assert_eq!((s.shared_timeouts, s.shared_skipped), (1, 1));
    // Release the stuck reader.
    let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
}

#[test]
fn the_store_recovers_after_its_back_off() {
    let rig = Rig::new("recover", Duration::from_millis(300));
    let k = key(60);
    let Some(fifo) = rig.store.entry_path(&rig.solver, &k) else {
        unreachable!("no entry path")
    };
    assert!(
        fifo.parent()
            .is_some_and(|p| std::fs::create_dir_all(p).is_ok())
    );
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    assert!(made.is_ok_and(|s| s.success()));
    let later = key(61);
    rig.plant(&later, &rig.signed(&later, b"unsat\n", "check-sat"));
    let (mut c, _) = rig.session();
    assert_eq!(get(&mut c, &k), None);
    let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(get(&mut c, &later).as_deref(), Some(&b"unsat\n"[..]));
    assert_eq!(rig.stats().shared_hits, 1);
}
