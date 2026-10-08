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
    let d = PathBuf::from(format!("/tmp/sail-cs-{}", std::process::id()));
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

fn run_shim(solver: &Path, socket: Option<&Path>) -> Vec<u8> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
    cmd.args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", solver)
        .env_remove("SAIL_TRACE_DIR")
        .env_remove("SAIL_DAEMON_SOCKET")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(s) = socket {
        cmd.env("SAIL_DAEMON_SOCKET", s);
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
    let direct = run_shim(&solver, None);
    let before = session_starts(&dir);
    let fill = run_shim(&solver, Some(&socket));
    assert_equivalent(&direct, &fill);
    assert_eq!(
        session_starts(&dir),
        before + 1,
        "the fill run starts a solver"
    );
    let warm = run_shim(&solver, Some(&socket));
    assert_equivalent(&direct, &warm);
    // This session answers `unknown` three times; cached `unknown` is never
    // served (ADR 0009), so the solver starts again at the first of them and
    // replays everything before it.
    assert_eq!(session_starts(&dir), before + 2);
    let _ = std::fs::remove_dir_all(&dir);
}
