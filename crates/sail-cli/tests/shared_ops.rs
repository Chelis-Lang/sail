//! `sail cache keygen`, `trust` and `publish`, and `sail serve --shared`,
//! end to end with real processes (ADR 0017). No solver needed: a fake
//! solver answers `--version`, and replies are synthetic.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use sail_api::client::Client;
use sail_cache::DiskCache;
use sail_cache::shared::Solver;
use sail_key::{QueryKey, SolverIdentity};

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = PathBuf::from(format!("/tmp/sail-cs{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(std::fs::create_dir_all(&d).is_ok());
        Self(d)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sail(args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(args)
        .env_remove("SAIL_DAEMON_SOCKET")
        .env_remove("SAIL_SHARED_STORE")
        .output();
    match out {
        Ok(o) => o,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

fn ok(args: &[&str]) -> String {
    let o = sail(args);
    assert!(
        o.status.success(),
        "sail {args:?} failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap_or_default()
}

/// A fake solver, and the identity the daemon will report for it.
fn fake_solver(dir: &Path) -> (PathBuf, Solver) {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("z3");
    let script = "#!/bin/sh\necho 'Z3 version 4.16.0 - 64 bit'\n";
    assert!(std::fs::write(&path, script).is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    let sha = sail_key::sha256(script.as_bytes());
    let solver = Solver {
        name: "z3".to_owned(),
        version: "4.16.0".to_owned(),
        binary_sha256: sha.iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{b:02x}");
            acc
        }),
        platform: SolverIdentity::host_platform(),
    };
    (path, solver)
}

/// A `sail serve` process, killed on drop.
struct Serve(Child);

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `sail cache --cache CACHE publish --to STORE --key KEY EXTRA...`.
fn publish(cache: &Path, store: &Path, key: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["cache", "--cache", s(cache), "publish", "--to", s(store)];
    args.extend(["--key", s(key)]);
    args.extend(extra);
    sail(&args)
}

/// Standard output of a run that must succeed.
fn text(o: &Output) -> String {
    assert!(
        o.status.success(),
        "failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Starts `sail serve` on `cache` with the shared `store`, and connects.
fn serve_shared(socket: &Path, cache: &Path, store: &Path) -> (Serve, Client) {
    let child = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["serve", "--socket", s(socket), "--cache", s(cache)])
        .args(["--shared", s(store), "--idle-timeout", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(child) = child else {
        unreachable!("cannot start sail serve")
    };
    let serve = Serve(child);
    let start = Instant::now();
    loop {
        if let Ok(c) = Client::connect(socket) {
            return (serve, c);
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no daemon");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn keys_trust_publish_and_a_shared_hit_end_to_end() {
    let dir = Scratch::new("e2e");
    let (solver_path, solver) = fake_solver(&dir.0);
    let (writer, reader, store) = (dir.path("a"), dir.path("b"), dir.path("store"));
    let key_file = dir.path("ci.key");

    // keygen: a secret key and its public key; never overwritten.
    let out = ok(&["cache", "keygen", "--out", s(&key_file)]);
    assert!(out.contains("key "), "{out}");
    assert!(
        !sail(&["cache", "keygen", "--out", s(&key_file)])
            .status
            .success()
    );
    let public = dir.path("ci.key.pub");
    assert!(public.is_file());

    // The writer's cache: one pinned-solver entry, one copied from a
    // shared tier, one from before identities were recorded.
    let disk = DiskCache::new(&writer);
    let (k1, k2, k3) = (QueryKey([1; 32]), QueryKey([2; 32]), QueryKey([3; 32]));
    let id = solver.summary();
    for (k, reply, solver, prov) in [
        (&k1, &b"unsat\n"[..], id.as_str(), "pinned-solver"),
        (&k2, &b"sat\n"[..], id.as_str(), "shared:0123456789abcdef"),
        (&k3, &b"unsat\n"[..], "", "pinned-solver"),
    ] {
        assert!(disk.put(k, reply, "check-sat", solver, prov, None).is_ok());
    }
    let out = text(&publish(&writer, &store, &key_file, &["--dry-run"]));
    assert!(out.starts_with("would publish 1 entries"), "{out}");
    let out = text(&publish(&writer, &store, &key_file, &[]));
    assert!(out.starts_with("published 1 entries"), "{out}");
    assert!(
        out.contains("1 from the shared tier, 0 other provenance, 1 without a solver identity"),
        "{out}"
    );
    let bad = publish(&writer, &store, &key_file, &["--filter", "colour=blue"]);
    assert_eq!(bad.status.code(), Some(2));

    // The reader trusts the key.
    let out = ok(&["cache", "--cache", s(&reader), "trust", "add", s(&public)]);
    assert!(out.starts_with("trusted key "), "{out}");
    let listed: serde_json::Value = serde_json::from_str(&ok(&[
        "cache",
        "--cache",
        s(&reader),
        "--json",
        "trust",
        "list",
    ]))
    .unwrap_or_default();
    assert_eq!(listed["keys"].as_array().map(Vec::len), Some(1));
    let key_id = listed["keys"][0].as_str().unwrap_or_default().to_owned();

    // A daemon on the reader's cache, with the shared store.
    let socket = dir.path("d.sock");
    let (_serve, mut client) = serve_shared(&socket, &reader, &store);
    assert!(client.hello(s(&solver_path)).is_ok());
    assert_eq!(
        client.get(&k1.0).ok().flatten().as_deref(),
        Some(&b"unsat\n"[..])
    );
    assert_eq!(client.get(&k3.0).ok().flatten(), None);
    let status = ok(&["status", "--socket", s(&socket)]);
    assert!(status.contains("shared tier  "), "{status}");
    assert!(status.contains("1 hits"), "{status}");

    // The copy is never published again.
    let start = Instant::now();
    while DiskCache::new(&reader).usage().entries == 0 {
        assert!(start.elapsed() < Duration::from_secs(10), "not copied");
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = ok(&[
        "cache",
        "--cache",
        s(&reader),
        "publish",
        "--to",
        s(&dir.path("store2")),
        "--key",
        s(&key_file),
    ]);
    assert!(out.starts_with("published 0 entries"), "{out}");
    assert!(out.contains("1 from the shared tier"), "{out}");

    // Removing trust.
    ok(&["cache", "--cache", s(&reader), "trust", "remove", &key_id]);
    let listed = ok(&["cache", "--cache", s(&reader), "trust", "list"]);
    assert!(listed.contains(": 0"), "{listed}");
}
