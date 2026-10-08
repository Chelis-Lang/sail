//! The in-memory tier, write-behind, cache maintenance under load, and the
//! daemon lifecycle (ADR 0015). No solver needed: replies are synthetic.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sail_api::client::Client;
use sail_api::protocol::PutMeta;
use sail_cache::DiskCache;
use sail_key::QueryKey;

/// A scratch directory, removed on drop. Short, because Unix socket paths
/// are limited to about 100 bytes.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = PathBuf::from(format!("/tmp/sail-dt{}-{tag}", std::process::id()));
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

struct Running {
    socket: PathBuf,
    cache: PathBuf,
    handle: sail_daemon::Handle,
    thread: std::thread::JoinHandle<std::io::Result<()>>,
}

impl Running {
    fn start(dir: &Path, config: &sail_daemon::Config) -> Self {
        let (socket, cache) = (dir.join("d.sock"), dir.join("cache"));
        let stats = Arc::new(sail_daemon::Stats::default());
        let server = match sail_daemon::bind(&socket, &cache, stats, config) {
            Ok(s) => s,
            Err(e) => unreachable!("cannot bind: {e}"),
        };
        let handle = server.handle();
        let thread = std::thread::spawn(move || server.run());
        Self {
            socket,
            cache,
            handle,
            thread,
        }
    }

    fn client(&self) -> Client {
        match Client::connect(&self.socket) {
            Ok(c) => c,
            Err(e) => unreachable!("cannot connect: {e}"),
        }
    }

    fn disk(&self) -> DiskCache {
        DiskCache::new(&self.cache)
    }

    fn stop(self) {
        self.handle.shutdown();
        assert!(matches!(self.thread.join(), Ok(Ok(()))));
    }
}

fn meta(command: &str) -> PutMeta {
    PutMeta {
        command: command.to_owned(),
        provenance: "pinned-solver".to_owned(),
        effort: None,
    }
}

/// The reply every test stores for key `i`: distinct per key, and a model
/// shape so any command name accepts it.
fn reply_for(i: u32) -> Vec<u8> {
    format!("(model {i} {})\n", "x".repeat((i % 7) as usize * 40)).into_bytes()
}

fn key(i: u32) -> QueryKey {
    let mut k = [0u8; 32];
    k[..4].copy_from_slice(&i.to_be_bytes());
    k[31] = 0x5a;
    QueryKey(k)
}

/// What the disk tier holds for `key` right now, without removing
/// anything: the reply of a valid entry file, or `None`.
fn on_disk(disk: &DiskCache, key: &QueryKey) -> Option<Vec<u8>> {
    let data = std::fs::read(disk.entry_path(key)).ok()?;
    sail_cache::parse_entry(&data, &key.to_string())
        .ok()
        .map(|(_, r)| r)
}

fn wait_until(limit: Duration, what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn drained(r: &Running) -> bool {
    r.client().info().is_ok_and(|i| i.queue.entries == 0)
}

#[test]
fn memory_hits_always_match_disk() {
    let dir = Scratch::new("match");
    let r = Running::start(&dir.0, &sail_daemon::Config::default());
    let mut c = r.client();
    let disk = r.disk();
    let n = 24u32;
    for i in 0..n {
        assert!(c.put(&key(i).0, meta("get-model"), reply_for(i)).is_ok());
    }
    wait_until(Duration::from_secs(10), "the writes", || drained(&r));
    // Warm: every reply is served from memory.
    let before = c.stats().unwrap_or_default();
    for i in 0..n {
        assert_eq!(c.get(&key(i).0).ok().flatten(), Some(reply_for(i)));
    }
    let after = c.stats().unwrap_or_default();
    assert_eq!(after.memory_hits - before.memory_hits, u64::from(n));
    // Change the disk under the daemon in every way an outside writer or
    // `sail cache` can, then check each GET against the disk.
    for i in 0..n {
        let path = disk.entry_path(&key(i));
        match i % 6 {
            // Removed (as by gc, verify or clear).
            0 => assert!(std::fs::remove_file(&path).is_ok()),
            // Rewritten in place, same length, one byte flipped: corrupt.
            1 => {
                let mut d = std::fs::read(&path).unwrap_or_default();
                if let Some(b) = d.last_mut() {
                    *b ^= 0x20;
                }
                std::thread::sleep(Duration::from_millis(2));
                assert!(std::fs::write(&path, d).is_ok());
            }
            // Replaced by another key's valid entry (a different inode).
            2 => {
                let other = disk.entry_path(&key(i + 1));
                assert!(std::fs::copy(&other, path.with_extension("x")).is_ok());
                assert!(std::fs::rename(path.with_extension("x"), &path).is_ok());
            }
            // Replaced by a valid entry with a different reply for this key
            // (a forgery as far as the cache can tell: it is served, and
            // memory must follow it).
            3 => {
                assert!(std::fs::remove_file(&path).is_ok());
                let forged = DiskCache::new(&r.cache);
                assert!(
                    forged
                        .put(&key(i), b"(model forged)\n", "get-model", "", "x", None)
                        .is_ok()
                );
            }
            // Left alone.
            _ => {}
        }
    }
    for round in 0..2 {
        for i in 0..n {
            let want = on_disk(&disk, &key(i));
            let got = c.get(&key(i).0).ok().flatten();
            assert_eq!(got, want, "round {round}, key {i}");
        }
    }
    drop(c);
    r.stop();
}

#[test]
fn put_is_acknowledged_before_the_write_and_drained_on_shutdown() {
    let dir = Scratch::new("behind");
    let config = sail_daemon::Config {
        write_delay: Some(Duration::from_millis(50)),
        ..sail_daemon::Config::default()
    };
    let r = Running::start(&dir.0, &config);
    let mut c = r.client();
    let n = 30u32;
    let start = Instant::now();
    for i in 0..n {
        assert!(c.put(&key(i).0, meta("get-model"), reply_for(i)).is_ok());
    }
    // 30 writes take at least 1.5 s; the acknowledgements did not wait.
    assert!(start.elapsed() < Duration::from_secs(1));
    let info = c.info().unwrap_or_default();
    assert!(info.queue.entries > 0, "{info:?}");
    assert!(info.memory.pending > 0, "{info:?}");
    // Served from memory while queued.
    for i in 0..n {
        assert_eq!(c.get(&key(i).0).ok().flatten(), Some(reply_for(i)));
    }
    let disk = r.disk();
    drop(c);
    r.stop();
    // Shutdown drained the queue: every entry is on disk and valid.
    for i in 0..n {
        assert_eq!(on_disk(&disk, &key(i)), Some(reply_for(i)), "key {i}");
    }
    assert_eq!(disk.verify(false).bad, Vec::new());
}

#[test]
fn a_full_queue_drops_puts_without_storing_anything_wrong() {
    let dir = Scratch::new("full");
    let config = sail_daemon::Config {
        queue_bytes: 600,
        queue_wait: Duration::from_millis(50),
        write_delay: Some(Duration::from_millis(200)),
        ..sail_daemon::Config::default()
    };
    let r = Running::start(&dir.0, &config);
    let mut c = r.client();
    for i in 0..5u32 {
        assert!(c.put(&key(i).0, meta("get-model"), reply_for(i)).is_ok());
    }
    let stats = c.stats().unwrap_or_default();
    assert!(stats.dropped_puts > 0, "{stats:?}");
    // Every reply is either served exactly or a miss.
    for i in 0..5u32 {
        let got = c.get(&key(i).0).ok().flatten();
        assert!(got.is_none() || got == Some(reply_for(i)), "key {i}");
    }
    drop(c);
    let disk = r.disk();
    r.stop();
    for i in 0..5u32 {
        let got = on_disk(&disk, &key(i));
        assert!(got.is_none() || got == Some(reply_for(i)), "key {i}");
    }
}

#[test]
fn gc_and_clear_concurrent_with_reads_and_writes_only_cost_misses() {
    let dir = Scratch::new("gc");
    let config = sail_daemon::Config {
        // Small, so the memory tier evicts too.
        memory_bytes: 40_000,
        ..sail_daemon::Config::default()
    };
    let r = Running::start(&dir.0, &config);
    let disk = r.disk();
    let stop = AtomicBool::new(false);
    let n = 200u32;
    let (mut hits, mut misses) = (0u64, 0u64);
    std::thread::scope(|s| {
        let maint = s.spawn(|| {
            let mut rounds = 0u32;
            while !stop.load(Ordering::SeqCst) {
                let budget = disk.usage().bytes / 2;
                let _ = disk.gc(budget);
                if rounds % 10 == 9 {
                    let _ = disk.clear();
                }
                let _ = disk.verify(true);
                rounds += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            rounds
        });
        let clients: Vec<_> = (0..6u32)
            .map(|t| {
                let socket = &r.socket;
                s.spawn(move || {
                    let Ok(mut client) = Client::connect(socket) else {
                        unreachable!("cannot connect")
                    };
                    let (mut hits, mut misses) = (0u64, 0u64);
                    for round in 0..15u32 {
                        for j in 0..n {
                            let i = (j * 7 + t * 31 + round * 13) % n;
                            match client.get(&key(i).0) {
                                Ok(Some(got)) => {
                                    assert_eq!(got, reply_for(i), "wrong reply for key {i}");
                                    hits += 1;
                                }
                                Ok(None) => {
                                    misses += 1;
                                    assert!(
                                        client
                                            .put(&key(i).0, meta("get-model"), reply_for(i))
                                            .is_ok()
                                    );
                                }
                                Err(e) => unreachable!("daemon error: {e}"),
                            }
                        }
                    }
                    (hits, misses)
                })
            })
            .collect();
        for h in clients {
            if let Ok((h, m)) = h.join() {
                hits += h;
                misses += m;
            }
        }
        stop.store(true, Ordering::SeqCst);
        assert!(maint.join().is_ok_and(|rounds| rounds > 0));
    });
    assert!(hits > 0 && misses > 0, "hits {hits}, misses {misses}");
    r.stop();
    // Whatever is left on disk is valid and correct.
    assert_eq!(disk.verify(false).bad, Vec::new());
    for i in 0..n {
        let got = on_disk(&disk, &key(i));
        assert!(got.is_none() || got == Some(reply_for(i)), "key {i}");
    }
}

#[test]
fn idle_daemon_stops_removes_its_socket_and_logs() {
    let dir = Scratch::new("idle");
    let log = dir.0.join("cache").join("daemon.log");
    let config = sail_daemon::Config {
        idle_timeout: Some(Duration::from_millis(300)),
        log: Some(log.clone()),
        ..sail_daemon::Config::default()
    };
    let r = Running::start(&dir.0, &config);
    let info = r.client().info().unwrap_or_default();
    assert_eq!(info.idle_timeout_secs, 0); // 300 ms rounds down
    assert_eq!(info.log.as_deref(), log.to_str());
    assert_eq!(info.cache_root, r.cache.display().to_string());
    // An open connection keeps it alive.
    let held = r.client();
    std::thread::sleep(Duration::from_millis(700));
    assert!(r.socket.exists());
    drop(held);
    wait_until(Duration::from_secs(5), "the idle stop", || {
        r.thread.is_finished()
    });
    assert!(!r.socket.exists());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(text.contains("started"), "{text}");
    assert!(text.contains("idle"), "{text}");
    assert!(text.contains("stopped"), "{text}");
}

#[test]
fn shutdown_works_after_the_socket_file_is_removed() {
    let dir = Scratch::new("gone");
    let r = Running::start(&dir.0, &sail_daemon::Config::default());
    put_all_and_get(&r);
    assert!(std::fs::remove_file(&r.socket).is_ok());
    let start = Instant::now();
    let disk = r.disk();
    r.stop();
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(on_disk(&disk, &key(1)), Some(reply_for(1)));
}

fn put_all_and_get(r: &Running) {
    let mut c = r.client();
    for i in 0..3 {
        assert!(c.put(&key(i).0, meta("get-model"), reply_for(i)).is_ok());
        assert_eq!(c.get(&key(i).0).ok().flatten(), Some(reply_for(i)));
    }
}
