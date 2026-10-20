//! The bundled tier (ADR 0062): a release's signed vstd replies, trusted
//! through the release index, against a real daemon with synthetic replies
//! (no solver needed).
//!
//! A verified entry is served and copied locally with provenance
//! `bundled:<key id>`. Everything else is a miss: a modified entry, a
//! swapped key (with or without a matching index), a key or pack outside
//! the index, an entry for another solver, and keys crossing between the
//! bundled tier and the team store's trust directory.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sail_api::client::Client;
use sail_api::protocol::StatsSnapshot;
use sail_cache::DiskCache;
use sail_cache::pack::{self, MAGIC};
use sail_cache::shared::{
    PublishOptions, SecretKey, SharedStore, Solver, add_trusted, sign_entry, trust_dir,
};
use sail_key::QueryKey;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = PathBuf::from(format!("/tmp/sail-db{}-{tag}", std::process::id()));
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

fn fake_solver(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("z3");
    assert!(std::fs::write(&path, "#!/bin/sh\necho 'Z3 version 4.16.0 - 64 bit'\n").is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    path
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn key(i: u8) -> QueryKey {
    let mut k = [0x44; 32];
    k[0] = i;
    QueryKey(k)
}

/// Serializes `entries` as a pack, in key order.
fn pack_of(mut entries: Vec<(QueryKey, Vec<u8>)>) -> Vec<u8> {
    entries.sort_by_key(|(k, _)| k.0);
    let mut out = MAGIC.to_vec();
    for (k, data) in entries {
        out.extend_from_slice(&k.0);
        out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&data);
    }
    out
}

/// Writes a release root: `replies/vstd.pack`, `replies/release.pub` (the
/// public half of `signer`) and a `release-files.json` that lists `listed`
/// with their actual digests.
fn release(root: &Path, pack: &[u8], signer: &SecretKey, listed: &[&str]) {
    let replies = root.join("replies");
    assert!(std::fs::create_dir_all(&replies).is_ok());
    assert!(std::fs::write(replies.join("vstd.pack"), pack).is_ok());
    assert!(std::fs::write(replies.join("release.pub"), signer.public().to_text()).is_ok());
    index(root, listed);
}

fn index(root: &Path, listed: &[&str]) {
    let mut files = serde_json::Map::new();
    for name in listed {
        let data = std::fs::read(root.join(name)).unwrap_or_default();
        files.insert(
            (*name).to_owned(),
            serde_json::json!({"sha256": hex(&sail_key::sha256(&data)), "bytes": data.len(), "mode": 420}),
        );
    }
    let doc = serde_json::json!({"schema": 1, "files": files});
    assert!(std::fs::write(root.join("release-files.json"), doc.to_string()).is_ok());
}

const BOTH: [&str; 2] = ["replies/vstd.pack", "replies/release.pub"];

struct Rig {
    dir: Scratch,
    socket: PathBuf,
    cache: PathBuf,
    solver_path: String,
    handle: Option<sail_daemon::Handle>,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let dir = Scratch::new(tag);
        let solver_path = fake_solver(&dir.0).display().to_string();
        Self {
            socket: dir.0.join("d.sock"),
            cache: dir.0.join("cache"),
            solver_path,
            dir,
            handle: None,
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.0.join("release")
    }

    /// Starts the daemon with the bundled tier at [`Rig::root`] and,
    /// optionally, a team store; waits until the tier has loaded.
    fn start(&mut self, team: Option<PathBuf>) {
        let config = sail_daemon::Config {
            bundled: Some(sail_daemon::BundledConfig { root: self.root() }),
            shared: team.map(|store| sail_daemon::SharedConfig {
                store,
                trust_dir: trust_dir(&self.cache),
                timeout: Duration::from_millis(500),
                backoff: Duration::from_secs(30),
            }),
            ..sail_daemon::Config::default()
        };
        let stats = Arc::new(sail_daemon::Stats::default());
        let server = match sail_daemon::bind(&self.socket, &self.cache, stats, &config) {
            Ok(s) => s,
            Err(e) => unreachable!("cannot bind: {e}"),
        };
        self.handle = Some(server.handle());
        std::thread::spawn(move || server.run());
        let start = Instant::now();
        while self.bundled().is_none_or(|b| b.ends_with("(loading)")) {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "tier never loaded"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn bundled(&self) -> Option<String> {
        Client::connect(&self.socket)
            .and_then(|mut c| c.info())
            .ok()
            .and_then(|i| i.bundled)
    }

    /// A connection that has said `HELLO`, and the solver identity.
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

    /// The solver identity a session reports, from a throwaway daemon.
    fn identity(&mut self) -> Solver {
        let probe = self.dir.0.join("probe");
        let _ = std::fs::create_dir_all(&probe);
        let socket = probe.join("p.sock");
        let server = match sail_daemon::bind(
            &socket,
            &probe.join("cache"),
            Arc::new(sail_daemon::Stats::default()),
            &sail_daemon::Config::default(),
        ) {
            Ok(s) => s,
            Err(e) => unreachable!("cannot bind: {e}"),
        };
        let handle = server.handle();
        std::thread::spawn(move || server.run());
        let mut c = match Client::connect(&socket) {
            Ok(c) => c,
            Err(e) => unreachable!("cannot connect: {e}"),
        };
        let id = match c.hello(&self.solver_path) {
            Ok(id) => id,
            Err(e) => unreachable!("HELLO failed: {e}"),
        };
        handle.shutdown();
        Solver {
            name: id.name,
            version: id.version,
            binary_sha256: id.binary_sha256,
            platform: id.platform,
        }
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
}

impl Drop for Rig {
    fn drop(&mut self) {
        if let Some(h) = &self.handle {
            h.shutdown();
        }
    }
}

fn get(c: &mut Client, key: &QueryKey) -> Option<Vec<u8>> {
    match c.get(&key.0) {
        Ok(r) => r,
        Err(e) => unreachable!("GET failed: {e}"),
    }
}

fn signed(secret: &SecretKey, k: &QueryKey, reply: &[u8], solver: &Solver) -> Vec<u8> {
    sign_entry(secret, k, reply, "check-sat", solver, None, 1)
}

#[test]
fn a_factory_pack_is_served_copied_locally_and_never_republished() {
    let mut rig = Rig::new("hit");
    let solver = rig.identity();
    // The factory's path: a local cache recorded from the pinned solver,
    // packed with a one-off key.
    let producer = DiskCache::new(rig.dir.0.join("producer"));
    for i in 1..=3 {
        assert!(
            producer
                .put(
                    &key(i),
                    b"unsat\n",
                    "check-sat",
                    &solver.summary(),
                    "pinned-solver",
                    None
                )
                .is_ok()
        );
    }
    let secret = SecretKey::from_seed([21; 32]);
    let (packed, report) = pack::build(&producer, &secret, &PublishOptions::default(), &|_| true);
    assert_eq!((report.published, report.failed), (3, 0));
    release(&rig.root(), &packed, &secret, &BOTH);
    rig.start(None);
    assert!(
        rig.bundled()
            .is_some_and(|b| b.contains("3 entries") && b.contains(&secret.public().id())),
        "{:?}",
        rig.bundled()
    );

    let (mut c, _) = rig.session();
    assert_eq!(get(&mut c, &key(1)).as_deref(), Some(&b"unsat\n"[..]));
    assert_eq!(get(&mut c, &key(9)), None, "a key the pack lacks is a miss");
    let s = rig.stats();
    assert_eq!((s.bundled_lookups, s.bundled_hits, s.hits), (1, 1, 1));

    rig.drain();
    let disk = DiskCache::new(&rig.cache);
    let local = disk.get(&key(1));
    assert_eq!(
        local
            .as_ref()
            .map(|(m, r)| (m.provenance.clone(), r.clone())),
        Some((
            format!("bundled:{}", secret.public().id()),
            b"unsat\n".to_vec()
        ))
    );
    // Later lookups are local.
    let (mut c2, _) = rig.session();
    assert_eq!(get(&mut c2, &key(1)).as_deref(), Some(&b"unsat\n"[..]));
    assert_eq!(rig.stats().bundled_lookups, 1);
    // A bundled copy is never published again.
    let r = sail_cache::shared::publish(
        &disk,
        &SharedStore::new(rig.dir.0.join("other-store")),
        &SecretKey::from_seed([22; 32]),
        &PublishOptions::default(),
        &|_| true,
    );
    assert_eq!((r.other_provenance, r.published), (1, 0));
}

#[test]
fn modified_and_foreign_entries_are_rejected_misses() {
    let mut rig = Rig::new("reject");
    let solver = rig.identity();
    let secret = SecretKey::from_seed([31; 32]);
    let good = signed(&secret, &key(1), b"unsat\n", &solver);
    // A modified entry: the reply changed (a wrong answer), header intact.
    let mut modified = signed(&secret, &key(2), b"unsat\n", &solver);
    let len = modified.len();
    modified.splice(len - 6.., b"sat\n\n\n".iter().copied());
    // An entry correctly signed for another solver binary.
    let mut other = solver.clone();
    other.binary_sha256 = "cd".repeat(32);
    let foreign = signed(&secret, &key(3), b"unsat\n", &other);
    // An entry signed by a key other than the release's.
    let stranger = signed(
        &SecretKey::from_seed([32; 32]),
        &key(4),
        b"unsat\n",
        &solver,
    );
    // An entry filed under the wrong key.
    let misfiled = signed(&secret, &key(6), b"unsat\n", &solver);
    let packed = pack_of(vec![
        (key(1), good),
        (key(2), modified),
        (key(3), foreign),
        (key(4), stranger),
        (key(5), misfiled),
    ]);
    release(&rig.root(), &packed, &secret, &BOTH);
    rig.start(None);
    let (mut c, _) = rig.session();
    assert_eq!(get(&mut c, &key(1)).as_deref(), Some(&b"unsat\n"[..]));
    for i in 2..=5 {
        assert_eq!(get(&mut c, &key(i)), None, "entry {i} must be a miss");
    }
    let s = rig.stats();
    assert_eq!((s.bundled_hits, s.bundled_rejected), (1, 4));
    rig.drain();
    for i in 2..=5 {
        assert!(DiskCache::new(&rig.cache).get(&key(i)).is_none());
    }
}

/// Each broken release setup refuses the whole tier: nothing is served.
#[test]
fn a_release_whose_key_or_pack_does_not_match_its_index_is_not_used() {
    type Setup = fn(&Path, &[u8], &SecretKey);
    let cases: [(&str, Setup); 6] = [
        ("key-outside-index", |root, packed, secret| {
            release(root, packed, secret, &["replies/vstd.pack"]);
        }),
        ("pack-outside-index", |root, packed, secret| {
            release(root, packed, secret, &["replies/release.pub"]);
        }),
        ("swapped-key", |root, packed, secret| {
            release(root, packed, secret, &BOTH);
            let other = SecretKey::from_seed([42; 32]);
            assert!(
                std::fs::write(root.join("replies/release.pub"), other.public().to_text()).is_ok()
            );
        }),
        ("modified-pack", |root, packed, secret| {
            release(root, packed, secret, &BOTH);
            let mut data = packed.to_vec();
            if let Some(last) = data.last_mut() {
                *last ^= 1;
            }
            assert!(std::fs::write(root.join("replies/vstd.pack"), data).is_ok());
        }),
        ("malformed-pack", |root, packed, secret| {
            let mut data = packed.to_vec();
            data.push(0);
            release(root, &data, secret, &BOTH);
        }),
        ("no-index", |root, packed, secret| {
            release(root, packed, secret, &BOTH);
            assert!(std::fs::remove_file(root.join("release-files.json")).is_ok());
        }),
    ];
    for (tag, setup) in cases {
        let mut rig = Rig::new(tag);
        let solver = rig.identity();
        let secret = SecretKey::from_seed([41; 32]);
        let packed = pack_of(vec![(
            key(1),
            signed(&secret, &key(1), b"unsat\n", &solver),
        )]);
        setup(&rig.root(), &packed, &secret);
        rig.start(None);
        assert!(
            rig.bundled().is_some_and(|b| b.contains("not used")),
            "{tag}: {:?}",
            rig.bundled()
        );
        let (mut c, _) = rig.session();
        assert_eq!(get(&mut c, &key(1)), None, "{tag}");
        assert_eq!(rig.stats().bundled_lookups, 0, "{tag}");
    }
}

/// A swapped key with a matching index (someone rewrote both) trusts only
/// the new key, so the release's entries are untrusted misses.
#[test]
fn a_swapped_key_with_a_rewritten_index_serves_nothing_signed_by_another_key() {
    let mut rig = Rig::new("swap-index");
    let solver = rig.identity();
    let secret = SecretKey::from_seed([51; 32]);
    let packed = pack_of(vec![(
        key(1),
        signed(&secret, &key(1), b"unsat\n", &solver),
    )]);
    release(&rig.root(), &packed, &SecretKey::from_seed([52; 32]), &BOTH);
    rig.start(None);
    let (mut c, _) = rig.session();
    assert_eq!(get(&mut c, &key(1)), None);
    let s = rig.stats();
    assert_eq!((s.bundled_hits, s.bundled_rejected), (0, 1));
}

/// The release key authorizes only its own pack, and the trust directory's
/// keys authorize nothing in the pack.
#[test]
fn bundled_and_team_trust_never_cross() {
    let mut rig = Rig::new("cross");
    let solver = rig.identity();
    let release_key = SecretKey::from_seed([61; 32]);
    let team_key = SecretKey::from_seed([62; 32]);
    assert!(add_trusted(&rig.cache, &team_key.public()).is_ok());
    // In the pack: an entry signed by the trusted team key.
    let packed = pack_of(vec![(
        key(1),
        signed(&team_key, &key(1), b"unsat\n", &solver),
    )]);
    release(&rig.root(), &packed, &release_key, &BOTH);
    // In the team store: an entry signed by the release key.
    let store = SharedStore::new(rig.dir.0.join("team"));
    let path = store.entry_path(&solver, &key(2));
    let Some(path) = path else {
        unreachable!("no entry path")
    };
    assert!(SharedStore::write(&path, &signed(&release_key, &key(2), b"unsat\n", &solver)).is_ok());
    rig.start(Some(store.root().to_path_buf()));
    let (mut c, _) = rig.session();
    assert_eq!(
        get(&mut c, &key(1)),
        None,
        "team key must not authorize the pack"
    );
    assert_eq!(
        get(&mut c, &key(2)),
        None,
        "release key must not authorize the team store"
    );
    let s = rig.stats();
    assert_eq!((s.bundled_rejected, s.shared_rejected, s.hits), (1, 1, 0));
}
