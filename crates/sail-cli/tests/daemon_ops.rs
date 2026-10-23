//! `sail status`, `sail cache` and the `sail serve` lifecycle, against a
//! real daemon process (ADR 0015). No solver needed: replies are synthetic.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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
        let d = PathBuf::from(format!("/tmp/sail-co{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(std::fs::create_dir_all(&d).is_ok());
        Self(d)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("d.sock")
    }

    fn cache(&self) -> PathBuf {
        self.0.join("cache")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sail(args: &[&str], env: &[(&str, &Path)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail"));
    cmd.args(args).env_remove("SAIL_DAEMON_SOCKET");
    for (k, v) in env {
        cmd.env(k, v);
    }
    match cmd.output() {
        Ok(o) => o,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn wait_until(limit: Duration, what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A `sail serve` process, killed on drop.
struct Serve(Child);

impl Serve {
    fn start(s: &Scratch, args: &[&str], env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail"));
        cmd.arg("serve")
            .arg("--socket")
            .arg(s.socket())
            .arg("--cache")
            .arg(s.cache())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => unreachable!("cannot start sail serve: {e}"),
        };
        let socket = s.socket();
        wait_until(Duration::from_secs(10), "the daemon", || {
            Client::connect(&socket).is_ok()
        });
        Self(child)
    }

    fn signal(&self, sig: &str) {
        let ok = Command::new("kill")
            .args([sig, &self.0.id().to_string()])
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "kill {sig} failed");
    }

    fn wait_exit(&mut self, limit: Duration) -> Option<std::process::ExitStatus> {
        let start = Instant::now();
        while start.elapsed() < limit {
            if let Ok(Some(status)) = self.0.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn key(i: u32) -> QueryKey {
    let mut k = [0u8; 32];
    k[..4].copy_from_slice(&i.to_be_bytes());
    QueryKey(k)
}

fn reply_for(i: u32) -> Vec<u8> {
    format!("(model {i})\n").into_bytes()
}

fn cache_client(socket: &Path) -> Client {
    use std::os::unix::fs::PermissionsExt as _;
    let solver = socket.with_extension("solver");
    if !solver.exists() {
        assert!(std::fs::write(&solver, b"#!/bin/sh\necho 'Z3 version 4.16.0'\n").is_ok());
        assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o700)).is_ok());
    }
    let Ok(mut c) = Client::connect(socket) else {
        unreachable!("cannot connect")
    };
    assert!(c.hello(&solver.to_string_lossy()).is_ok());
    c
}

fn put_all(socket: &Path, n: u32) {
    let mut c = cache_client(socket);
    for i in 0..n {
        let meta = PutMeta {
            command: "get-model".to_owned(),
            provenance: "pinned-solver".to_owned(),
            effort: None,
        };
        assert!(c.put(&key(i).0, meta, reply_for(i)).is_ok());
    }
}

/// Every entry on disk is valid and holds the reply put for its key.
/// Returns how many there are.
fn assert_entries_correct(cache: &Path, n: u32) -> u32 {
    let disk = DiskCache::new(cache);
    let mut found = 0;
    for i in 0..n {
        let path = disk.entry_path(&key(i));
        if !path.exists() {
            continue;
        }
        let got = DiskCache::check_file(&path, &key(i)).map(|(_, r)| r);
        assert_eq!(got, Ok(reply_for(i)), "entry {i}");
        found += 1;
    }
    found
}

#[test]
fn status_without_a_daemon_exits_4() {
    let s = Scratch::new("none");
    let out = sail(&["status"], &[("SAIL_DAEMON_SOCKET", &s.socket())]);
    assert_eq!(out.status.code(), Some(4));
    assert!(text(&out.stderr).contains("SAIL0401"));
    let out = sail(
        &[
            "status",
            "--json",
            "--socket",
            &s.socket().to_string_lossy(),
        ],
        &[],
    );
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    assert_eq!(v["running"], serde_json::Value::Bool(false));
}

#[test]
fn status_and_cache_commands_against_a_running_daemon() {
    let s = Scratch::new("ops");
    let serve = Serve::start(&s, &["--memory-bytes", "1M"], &[]);
    put_all(&s.socket(), 20);
    let env = [("SAIL_DAEMON_SOCKET", s.socket())];
    let env: Vec<(&str, &Path)> = env.iter().map(|(k, v)| (*k, v.as_path())).collect();
    wait_until(Duration::from_secs(10), "the writes", || {
        DiskCache::new(s.cache()).usage().entries == 20
    });
    let out = sail(&["status", "--json"], &env);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    assert_eq!(v["running"], serde_json::Value::Bool(true));
    assert_eq!(v["stats"]["puts"], 20);
    assert_eq!(v["disk"]["entries"], 20);
    assert_eq!(v["info"]["memory"]["capacity_bytes"], 1 << 20);
    assert_eq!(v["info"]["idle_timeout_secs"], 1800);
    assert_eq!(v["info"]["cache_root"].as_str(), s.cache().to_str(), "{v}");
    let out = sail(&["status"], &env);
    let t = text(&out.stdout);
    assert!(t.contains("20 puts") && t.contains("memory tier"), "{t}");
    // `sail cache` finds the root by asking the daemon.
    let out = sail(&["cache", "stats", "--json"], &env);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    assert_eq!(v["stats"]["entries"], 20);
    assert_eq!(v["stats"]["by_command"]["get-model"]["entries"], 20);
    // Corrupt one entry: verify reports it (exit 5), then removes it.
    let bad = DiskCache::new(s.cache()).entry_path(&key(3));
    assert!(std::fs::write(&bad, b"garbage").is_ok());
    let out = sail(&["cache", "verify"], &env);
    assert_eq!(out.status.code(), Some(5), "{}", text(&out.stdout));
    assert!(text(&out.stdout).contains("bad-header"));
    let out = sail(&["cache", "verify", "--remove"], &env);
    assert_eq!(out.status.code(), Some(0));
    assert!(!bad.exists());
    // The daemon now misses that key, and still serves the others exactly.
    let mut c = cache_client(&s.socket());
    assert!(matches!(c.get(&key(3).0), Ok(None)));
    assert_eq!(c.get(&key(4).0).ok().flatten(), Some(reply_for(4)));
    // gc to half the bytes, then clear.
    let half = DiskCache::new(s.cache()).usage().bytes / 2;
    let out = sail(&["cache", "gc", "--max-bytes", &half.to_string()], &env);
    assert_eq!(out.status.code(), Some(0));
    assert!(DiskCache::new(s.cache()).usage().bytes <= half);
    let out = sail(&["cache", "clear"], &env);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("--yes"));
    let out = sail(&["cache", "clear", "--yes"], &env);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(DiskCache::new(s.cache()).usage().entries, 0);
    // After clear, nothing is served from memory either.
    for i in 0..20 {
        assert!(matches!(c.get(&key(i).0), Ok(None)), "key {i}");
    }
    // `--cache` works without a daemon.
    drop(serve);
    let out = sail(
        &["cache", "--cache", &s.cache().to_string_lossy(), "stats"],
        &[],
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("0 entries"));
}

#[test]
fn sigterm_drains_queued_writes_and_removes_the_socket() {
    let s = Scratch::new("term");
    let mut serve = Serve::start(&s, &[], &[("SAIL_DAEMON_WRITE_DELAY_MS", "20")]);
    put_all(&s.socket(), 40);
    // 40 writes at 20 ms each: most are still queued.
    assert!(DiskCache::new(s.cache()).usage().entries < 40);
    serve.signal("-TERM");
    let status = serve.wait_exit(Duration::from_secs(20));
    assert!(status.is_some_and(|st| st.success()), "{status:?}");
    assert_eq!(assert_entries_correct(&s.cache(), 40), 40);
    assert!(!s.socket().exists());
    let log = std::fs::read_to_string(s.cache().join("daemon.log")).unwrap_or_default();
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn killed_daemon_with_queued_writes_loses_entries_but_stores_nothing_wrong() {
    let s = Scratch::new("kill");
    let mut serve = Serve::start(&s, &[], &[("SAIL_DAEMON_WRITE_DELAY_MS", "15")]);
    let n = 60;
    put_all(&s.socket(), n);
    std::thread::sleep(Duration::from_millis(100));
    serve.signal("-KILL");
    assert!(serve.wait_exit(Duration::from_secs(10)).is_some());
    let stored = assert_entries_correct(&s.cache(), n);
    assert!(stored < n, "every write finished before the kill");
    // Nothing on disk is bad (a temporary file from the interrupted write
    // is too young to count), and a new daemon serves what survived.
    let out = sail(
        &["cache", "--cache", &s.cache().to_string_lossy(), "verify"],
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let _serve = Serve::start(&s, &[], &[]);
    let mut c = cache_client(&s.socket());
    for i in 0..n {
        let got = c.get(&key(i).0).ok().flatten();
        assert!(got.is_none() || got == Some(reply_for(i)), "key {i}");
    }
}

#[test]
fn idle_daemon_exits_on_its_own() {
    let s = Scratch::new("idle");
    let mut serve = Serve::start(&s, &["--idle-timeout", "1"], &[]);
    let status = serve.wait_exit(Duration::from_secs(10));
    assert!(status.is_some_and(|st| st.success()), "{status:?}");
    assert!(!s.socket().exists());
    let log = std::fs::read_to_string(s.cache().join("daemon.log")).unwrap_or_default();
    assert!(log.contains("idle"), "{log}");
}
