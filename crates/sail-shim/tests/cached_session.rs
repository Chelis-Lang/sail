//! Cached sessions against a real daemon and the pinned Z3 (ADR 0011).
//!
//! Ignored by default; run with
//! `SAIL_TEST_Z3=<pinned z3> cargo nextest run -p sail-shim --run-ignored all`.
#![cfg(unix)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, ResponseSplitter};

const INPUT: &[u8] = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.in.smt2");

fn z3() -> String {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(p) if !p.is_empty() => p,
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3"),
    }
}

fn scratch() -> PathBuf {
    // Short: Unix socket paths are limited to about 100 bytes.
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = PathBuf::from(format!("/tmp/sail-cs-{}-{serial}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    assert!(std::fs::create_dir_all(&d).is_ok());
    d
}

/// A solver wrapper that logs each session start, then runs the real z3.
fn logging_solver(dir: &Path, real: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("z3");
    let log = dir.join("starts.log");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> '{}'\nexec '{real}' \"$@\"\n",
        log.display()
    );
    assert!(std::fs::write(&path, script).is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    path
}

fn session_starts(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("starts.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.trim() == "-smt2 -in")
        .count()
}

fn run_shim(solver: &Path, socket: Option<&Path>, provenance: Option<&Path>) -> Vec<u8> {
    run_shim_capped(solver, socket, provenance, None)
}

fn run_shim_capped(
    solver: &Path,
    socket: Option<&Path>,
    provenance: Option<&Path>,
    cap: Option<usize>,
) -> Vec<u8> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
    cmd.args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", solver)
        .env_remove("SAIL_TRACE_DIR")
        .env_remove("SAIL_DAEMON_SOCKET")
        .env_remove("SAIL_PROVENANCE_DIR")
        .env_remove("SAIL_SHIM_BUFFER_BYTES")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(cap) = cap {
        cmd.env("SAIL_SHIM_BUFFER_BYTES", cap.to_string());
    }
    if let Some(s) = socket {
        cmd.env("SAIL_DAEMON_SOCKET", s);
    }
    if let Some(p) = provenance {
        cmd.env("SAIL_PROVENANCE_DIR", p);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => unreachable!("cannot start sail-z3: {e}"),
    };
    let mut stdin = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(INPUT);
        }
    });
    let out = child.wait_with_output();
    let _ = writer.join();
    match out {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => unreachable!("sail-z3 exited with {}", o.status),
        Err(e) => unreachable!("{e}"),
    }
}

/// `(is_statistics, reply)` per reply.
fn replies(output: &[u8]) -> Vec<(bool, Vec<u8>)> {
    let mut commands = Vec::new();
    CommandSplitter::new().push(INPUT, &mut commands);
    let stats: Vec<bool> = commands
        .iter()
        .filter_map(|c| classify(&c.bytes).ok())
        .filter(|c| c.kind.output() == Output::One)
        .map(|c| {
            matches!(
                c.kind,
                CommandKind::GetInfo {
                    keyword: b":all-statistics"
                }
            )
        })
        .collect();
    let mut items = Vec::new();
    ResponseSplitter::new().push(output, &mut items);
    assert_eq!(items.len(), stats.len());
    stats
        .into_iter()
        .zip(items.into_iter().map(|i| i.bytes))
        .collect()
}

/// How many replies the input asks for.
fn outputs() -> u64 {
    let mut commands = Vec::new();
    CommandSplitter::new().push(INPUT, &mut commands);
    commands
        .iter()
        .filter_map(|c| classify(&c.bytes).ok())
        .filter(|c| c.kind.output() == Output::One)
        .count() as u64
}

/// The single provenance record in `dir`, which is then emptied.
fn take_record(dir: &Path) -> serde_json::Value {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let text = std::fs::read(&files[0]).unwrap_or_default();
    let _ = std::fs::remove_file(&files[0]);
    serde_json::from_slice(&text).unwrap_or_default()
}

fn rlimit(reply: &[u8]) -> Option<&[u8]> {
    let line = reply
        .split(|&b| b == b'\n')
        .find(|l| l.windows(13).any(|w| w == b":rlimit-count"))?;
    line.split(u8::is_ascii_whitespace)
        .rfind(|w| !w.is_empty())
        .map(|w| w.strip_suffix(b")").unwrap_or(w))
}

fn assert_equivalent(a: &[u8], b: &[u8]) {
    for (i, ((stats, x), (_, y))) in replies(a).iter().zip(&replies(b)).enumerate() {
        if *stats {
            assert_eq!(rlimit(x), rlimit(y), "reply {i}");
        } else {
            assert!(
                x == y,
                "reply {i}: {:?} vs {:?}",
                String::from_utf8_lossy(x),
                String::from_utf8_lossy(y)
            );
        }
    }
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn second_session_replays_from_cache_without_unknowns_skipped() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    let counters = Arc::new(sail_daemon::Stats::default());
    std::thread::spawn(move || sail_daemon::serve(&sock, &cache, counters));
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let direct = run_shim(&solver, None, None);
    let before = session_starts(&dir);
    let records = dir.join("provenance");
    assert!(std::fs::create_dir_all(&records).is_ok());
    let fill = run_shim(&solver, Some(&socket), Some(&records));
    assert_equivalent(&direct, &fill);
    assert_eq!(
        session_starts(&dir),
        before + 1,
        "the fill run starts a solver"
    );
    // Provenance records (ADR 0014): keyed by the input's SHA-256, they
    // count the replies served before a solver started.
    let hex = sail_key::sha256(INPUT)
        .iter()
        .fold(String::new(), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        });
    let record = take_record(&records);
    assert_eq!(record["version"], 1);
    assert_eq!(record["input_sha256"], hex.as_str());
    assert_eq!(record["solver_started"], true);
    assert_eq!(record["replies_served"], 0, "the fill run misses at once");
    let warm = run_shim(&solver, Some(&socket), Some(&records));
    assert_equivalent(&direct, &warm);
    let record = take_record(&records);
    assert_eq!(record["input_sha256"], hex.as_str());
    assert_eq!(record["solver_started"], true);
    let served = record["replies_served"].as_u64().unwrap_or(0);
    assert!(
        served > 0 && served < outputs(),
        "warm run served {served} replies"
    );
    // This session answers `unknown` three times; cached `unknown` is never
    // served (ADR 0009), so the solver starts again at the first of them and
    // replays everything before it.
    assert_eq!(session_starts(&dir), before + 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn sessions_over_the_buffer_bound_continue_as_pass_through() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    let counters = Arc::new(sail_daemon::Stats::default());
    std::thread::spawn(move || sail_daemon::serve(&sock, &cache, counters));
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let records = dir.join("provenance");
    assert!(std::fs::create_dir_all(&records).is_ok());
    let direct = run_shim(&solver, None, None);
    // A cold session over the bound forwards exactly and records nothing.
    let capped = run_shim_capped(&solver, Some(&socket), Some(&records), Some(1));
    assert_equivalent(&direct, &capped);
    assert_eq!(std::fs::read_dir(&records).into_iter().flatten().count(), 0);
    let fill = run_shim(&solver, Some(&socket), Some(&records));
    assert_equivalent(&direct, &fill);
    assert_eq!(
        take_record(&records)["replies_served"],
        0,
        "the over-bound session published nothing"
    );
    // A warm session that served replies before reaching the bound still
    // gives the solver's exact replies afterwards.
    let starts = session_starts(&dir);
    let warm = run_shim_capped(&solver, Some(&socket), Some(&records), Some(1));
    assert_equivalent(&direct, &warm);
    assert_eq!(session_starts(&dir), starts + 1);
    assert_eq!(std::fs::read_dir(&records).into_iter().flatten().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn an_unwritable_cached_reply_exits_non_zero() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    let counters = Arc::new(sail_daemon::Stats::default());
    std::thread::spawn(move || sail_daemon::serve(&sock, &cache, counters));
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = run_shim(&solver, Some(&socket), None);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sail-z3"))
        .args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", &solver)
        .env("SAIL_DAEMON_SOCKET", &socket)
        .env_remove("SAIL_TRACE_DIR")
        .env_remove("SAIL_PROVENANCE_DIR")
        .env_remove("SAIL_SHIM_BUFFER_BYTES")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| unreachable!("shim: {e}"));
    // The reader is gone before the first cached reply is written.
    drop(child.stdout.take());
    let mut stdin = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(INPUT);
        }
    });
    let status = child.wait().unwrap_or_else(|e| unreachable!("{e}"));
    let _ = writer.join();
    assert!(!status.success(), "{status}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn input_run(solver: &Path, socket: Option<&Path>, input: &[u8]) -> std::process::Output {
    input_run_released(solver, socket, input, None)
}

fn input_run_released(
    solver: &Path,
    socket: Option<&Path>,
    input: &[u8],
    release: Option<PathBuf>,
) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
    cmd.args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", solver)
        .env_remove("SAIL_DAEMON_SOCKET")
        .env_remove("SAIL_TRACE_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(socket) = socket {
        cmd.env("SAIL_DAEMON_SOCKET", socket);
    }
    let mut child = cmd.spawn().unwrap_or_else(|e| unreachable!("shim: {e}"));
    let mut stdin = child.stdin.take();
    let bytes = input.to_vec();
    let writer = std::thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            let _ = stdin.write_all(&bytes);
        }
        drop(stdin);
        if let Some(path) = release {
            let _ = std::fs::write(path, b"released");
        }
    });
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| unreachable!("output: {e}"));
    let _ = writer.join();
    output
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn stderr_warnings_survive_cold_and_warm_sessions() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    let root = cache.clone();
    std::thread::spawn(move || {
        sail_daemon::serve(&sock, &cache, Arc::new(sail_daemon::Stats::default()))
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let input = [
        b"(get-info :version)\n".as_slice(),
        include_bytes!("../../../corpus/repros/z3-stderr-cache-warning/input.smt2"),
    ]
    .concat();
    let direct = input_run(&solver, None, &input);
    assert!(direct.status.success());
    assert!(
        String::from_utf8_lossy(&direct.stderr)
            .contains("pattern does not contain all quantified variables")
    );
    let before = session_starts(&dir);
    for round in 1..=2 {
        let actual = input_run(&solver, Some(&socket), &input);
        assert_eq!(actual.status.code(), direct.status.code());
        assert_eq!(actual.stdout, direct.stdout);
        assert_eq!(
            actual.stderr, direct.stderr,
            "stderr missing in round {round}"
        );
        assert_eq!(session_starts(&dir), before + round);
    }
    let files = std::fs::read_dir(root.join("v2"))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|bucket| {
            std::fs::read_dir(bucket.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .count();
    assert_eq!(files, 0, "warning session must not publish any reply");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn late_and_large_stderr_prevent_publication_without_blocking() {
    use std::os::unix::fs::PermissionsExt as _;
    for late in [false, true] {
        let dir = scratch();
        let real = serde_json::to_string(&z3()).unwrap_or_default();
        let script = format!(
            r"#!/usr/bin/env python3
import subprocess, sys, time
real = {real}
if '-smt2' not in sys.argv:
    p = subprocess.run([real] + sys.argv[1:])
    sys.exit(p.returncode)
p = subprocess.run([real] + sys.argv[1:], input=sys.stdin.buffer.read(), capture_output=True)
warning = b'late stderr\n' if {late} else b'large stderr\n' * 100000
if {late}:
    sys.stdout.buffer.write(p.stdout)
    sys.stdout.buffer.flush()
    time.sleep(0.05)
sys.stderr.buffer.write(warning)
sys.stderr.buffer.flush()
if not {late}:
    sys.stdout.buffer.write(p.stdout)
    sys.stdout.buffer.flush()
sys.exit(p.returncode)
",
            late = if late { "True" } else { "False" }
        );
        let solver = dir.join("z3");
        assert!(std::fs::write(&solver, script).is_ok());
        assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o755)).is_ok());
        let (socket, cache) = (dir.join("d.sock"), dir.join("cache"));
        let (sock, root) = (socket.clone(), cache.clone());
        std::thread::spawn(move || {
            sail_daemon::serve(&sock, &root, Arc::new(sail_daemon::Stats::default()))
        });
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let input = b"(check-sat)\n(exit)\n";
        let direct = input_run(&solver, None, input);
        for _ in 0..2 {
            let actual = input_run(&solver, Some(&socket), input);
            assert!(actual.status.success());
            assert_eq!(actual.stdout, direct.stdout);
            assert_eq!(actual.stderr, direct.stderr);
        }
        let files = std::fs::read_dir(cache.join("v2"))
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|bucket| {
                std::fs::read_dir(bucket.path())
                    .into_iter()
                    .flatten()
                    .flatten()
            })
            .count();
        assert_eq!(files, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn unmatched_clean_exit_preserves_output_and_publishes_nothing() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scratch();
    let real = serde_json::to_string(&z3()).unwrap_or_default();
    // A faulty solver producer exits successfully after only its first reply.
    // Its version/probe invocations remain the real pinned solver's responses.
    let script = format!(
        r"#!/usr/bin/env python3
import subprocess, sys
real = {real}
if '-smt2' not in sys.argv:
    sys.exit(subprocess.run([real] + sys.argv[1:]).returncode)
p = subprocess.run([real] + sys.argv[1:], input=sys.stdin.buffer.read(), capture_output=True)
sys.stdout.buffer.write(p.stdout.splitlines(keepends=True)[0])
sys.stdout.buffer.flush()
",
    );
    let solver = dir.join("z3");
    assert!(std::fs::write(&solver, script).is_ok());
    assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o755)).is_ok());
    let (socket, cache) = (dir.join("d.sock"), dir.join("cache"));
    let (sock, root) = (socket.clone(), cache.clone());
    std::thread::spawn(move || {
        sail_daemon::serve(&sock, &root, Arc::new(sail_daemon::Stats::default()))
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let input = b"(get-info :version)\n(echo \"unanswered\")\n(exit)\n";
    let direct = input_run(&solver, None, input);
    assert!(direct.status.success());
    assert_ne!(direct.stdout, [] as [u8; 0]);
    for _ in 0..2 {
        let actual = input_run(&solver, Some(&socket), input);
        assert_eq!(actual.status.code(), direct.status.code());
        assert_eq!(actual.stdout, direct.stdout);
        assert_eq!(actual.stderr, direct.stderr);
    }
    assert!(
        sail_cache::DiskCache::new(&cache).list().is_empty(),
        "an unmatched session must not publish its earlier paired reply"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn incomplete_or_undelivered_input_never_publishes_completed_origins() {
    use std::os::unix::fs::PermissionsExt as _;
    use std::sync::atomic::Ordering;
    for close_early in [false, true] {
        let dir = scratch();
        let real = serde_json::to_string(&z3()).unwrap_or_default();
        let release = dir.join("release");
        let release_json = serde_json::to_string(&release).unwrap_or_default();
        let script = format!(
            r"#!/usr/bin/env python3
import os, pathlib, subprocess, sys, time
real = {real}
if '-smt2' not in sys.argv:
    sys.exit(subprocess.run([real] + sys.argv[1:]).returncode)
if {close_early}:
    while os.read(0, 1) != b'\n':
        pass
    os.close(0)
else:
    sys.stdin.buffer.read()
p = subprocess.run([real] + sys.argv[1:], input=b'(get-info :version)\n(exit)\n', capture_output=True)
sys.stdout.buffer.write(p.stdout)
sys.stdout.buffer.flush()
deadline = time.monotonic() + 10
while not pathlib.Path({release_json}).exists():
    if time.monotonic() >= deadline:
        sys.exit(1)
    time.sleep(0.005)
time.sleep(0.1)
",
            close_early = if close_early { "True" } else { "False" },
        );
        let solver = dir.join("z3");
        assert!(std::fs::write(&solver, script).is_ok());
        assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o755)).is_ok());
        let (socket, cache) = (dir.join("d.sock"), dir.join("cache"));
        let (sock, root) = (socket.clone(), cache.clone());
        let counters = Arc::new(sail_daemon::Stats::default());
        let observed = Arc::clone(&counters);
        std::thread::spawn(move || sail_daemon::serve(&sock, &root, observed));
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut input = b"(get-info :version)\n".to_vec();
        if close_early {
            // More bytes than a pipe can buffer: delivery must fail once the
            // wrapper closes stdin after consuming only the first command.
            input.extend(std::iter::repeat_n(b';', 2 * 1024 * 1024));
            input.extend_from_slice(b"\n(exit)\n");
        } else {
            // A successful quiet producer can still have incomplete parser input.
            input.extend_from_slice(b"(assert true");
        }
        assert!(std::fs::write(&release, b"released").is_ok());
        let direct = input_run(&solver, None, &input);
        assert!(direct.status.success());
        assert!(std::fs::remove_file(&release).is_ok());
        let actual = input_run_released(&solver, Some(&socket), &input, Some(release));
        assert_eq!(actual.status.code(), direct.status.code());
        assert_eq!(actual.stdout, direct.stdout);
        assert_eq!(actual.stderr, direct.stderr);
        assert_eq!(
            counters.puts.load(Ordering::Relaxed),
            0,
            "incomplete or failed input delivery cannot publish pairs or silence"
        );
        assert!(sail_cache::DiskCache::new(&cache).list().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn new_no_output_suffix_cannot_hide_diagnostics_after_cache_hits() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    std::thread::spawn(move || {
        sail_daemon::serve(&sock, &cache, Arc::new(sail_daemon::Stats::default()))
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let quiet = b"(get-info :version)\n(exit)\n";
    assert!(input_run(&solver, Some(&socket), quiet).status.success());
    let starts = session_starts(&dir);
    assert!(input_run(&solver, Some(&socket), quiet).status.success());
    assert_eq!(
        session_starts(&dir),
        starts,
        "certified quiet input must hit"
    );
    let mut warning = b"(get-info :version)\n".to_vec();
    let repro = include_bytes!("../../../corpus/repros/z3-stderr-cache-warning/input.smt2");
    warning.extend(
        String::from_utf8_lossy(repro)
            .replace("(check-sat)\n", "")
            .as_bytes(),
    );
    let direct = input_run(&solver, None, &warning);
    assert!(
        String::from_utf8_lossy(&direct.stderr)
            .contains("pattern does not contain all quantified variables")
    );
    for _ in 0..2 {
        let actual = input_run(&solver, Some(&socket), &warning);
        assert_eq!(actual.status.code(), direct.status.code());
        assert_eq!(actual.stdout, direct.stdout);
        assert_eq!(actual.stderr, direct.stderr);
    }
    let malformed = b"(get-info :version)\n(assert 1)\n(exit)\n";
    let error = input_run(&solver, None, malformed);
    assert!(String::from_utf8_lossy(&error.stdout).contains("(error"));
    for _ in 0..2 {
        let actual = input_run(&solver, Some(&socket), malformed);
        assert_eq!(actual.status.code(), error.status.code());
        assert_eq!(actual.stdout, error.stdout);
        assert_eq!(actual.stderr, error.stderr);
    }
    for input in [
        b"(get-info :version)\n(exit 1)\n(echo \"after\")\n(exit)\n".as_slice(),
        b"(get-info :version)\n(set-option :print-success true)\n(get-option :produce-models)\n(exit)\n".as_slice(),
    ] {
        let direct = input_run(&solver, None, input);
        for _ in 0..2 {
            let actual = input_run(&solver, Some(&socket), input);
            assert_eq!(actual.status.code(), direct.status.code());
            assert_eq!(actual.stdout, direct.stdout);
            assert_eq!(actual.stderr, direct.stderr);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn input_run_open(solver: &Path, socket: &Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sail-z3"))
        .args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", solver)
        .env("SAIL_DAEMON_SOCKET", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| unreachable!("start shim: {e}"));
    let mut writer = child.stdin.take().unwrap_or_else(|| unreachable!("stdin"));
    assert!(writer.write_all(input).is_ok());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().ok().flatten().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            unreachable!("exit waited for stdin EOF");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Keep the separate writer open until after the child exits.
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| unreachable!("read: {e}"));
    drop(writer);
    output
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn cached_exit_does_not_wait_for_client_to_close_stdin() {
    let dir = scratch();
    let solver = logging_solver(&dir, &z3());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    std::thread::spawn(move || {
        sail_daemon::serve(&sock, &cache, Arc::new(sail_daemon::Stats::default()))
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let quiet = b"(get-info :version)\n(exit)\n";
    assert!(input_run(&solver, Some(&socket), quiet).status.success());
    let starts = session_starts(&dir);
    let warm = input_run_open(&solver, &socket, quiet);
    assert!(warm.status.success());
    assert_eq!(session_starts(&dir), starts);
    let warning = b"(get-info :version)\n(declare-fun f (Int) Int)\n(assert (forall ((x Int) (y Int)) (! (= (f x) y) :pattern ((f x)))))\n( exit )\n";
    let direct = input_run(&solver, None, warning);
    let actual = input_run_open(&solver, &socket, warning);
    assert_eq!(actual.status.code(), direct.status.code());
    assert_eq!(actual.stdout, direct.stdout);
    assert_eq!(actual.stderr, direct.stderr);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn closed_stderr_sink_does_not_deadlock_a_large_diagnostic() {
    use std::os::fd::OwnedFd;
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixStream;

    let dir = scratch();
    let real = serde_json::to_string(&z3()).unwrap_or_default();
    let solver = dir.join("z3");
    let script = format!(
        "#!/usr/bin/env python3\nimport subprocess, sys\nif '-smt2' not in sys.argv:\n    sys.exit(subprocess.call([{real}] + sys.argv[1:]))\nsys.stderr.buffer.write(b'diagnostic\\n' * 100000)\nsys.stderr.buffer.flush()\n"
    );
    assert!(std::fs::write(&solver, script).is_ok());
    assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o755)).is_ok());
    let (socket, cache) = (dir.join("d.sock"), dir.join("cache"));
    let (sock, root) = (socket.clone(), cache.clone());
    std::thread::spawn(move || {
        sail_daemon::serve(&sock, &root, Arc::new(sail_daemon::Stats::default()))
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (reader, writer) = UnixStream::pair().unwrap_or_else(|e| unreachable!("stderr: {e}"));
    drop(reader);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sail-z3"))
        .args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", &solver)
        .env("SAIL_DAEMON_SOCKET", &socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(OwnedFd::from(writer)))
        .spawn()
        .unwrap_or_else(|e| unreachable!("start: {e}"));
    let mut input = child.stdin.take().unwrap_or_else(|| unreachable!("stdin"));
    assert!(input.write_all(b"(check-sat)\n(exit)\n").is_ok());
    drop(input);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().ok().flatten().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            unreachable!("closed stderr sink blocked solver shutdown");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| unreachable!("read: {e}"));
    assert!(!output.status.success());
    assert_eq!(output.stdout, [] as [u8; 0]);
    let entries = std::fs::read_dir(cache.join("v2"))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|bucket| {
            std::fs::read_dir(bucket.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .count();
    assert_eq!(
        entries, 0,
        "failed stderr forwarding must prevent publication"
    );
    let _ = std::fs::remove_dir_all(dir);
}
