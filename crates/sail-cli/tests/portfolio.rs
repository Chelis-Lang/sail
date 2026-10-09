//! Complete-run selection, reverification and timeout handling.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;

struct Fixture {
    dir: PathBuf,
    _listener: std::os::unix::net::UnixListener,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(format!("/tmp/sail-pf-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        let listener = std::os::unix::net::UnixListener::bind(dir.join("d.sock"))
            .unwrap_or_else(|e| unreachable!("socket: {e}"));
        let metadata = r#"#!/bin/sh
if [ "$1" = clean ]; then
  echo "$@" >> "$SAIL_FIXTURE/clean.log"
  exit 0
fi
printf '%s\n' '{"packages":[{"id":"path+file:///demo#input@0.1.0","name":"input","metadata":{"verus":{"verify":true}},"targets":[{"kind":["lib"]}]}],"workspace_members":["path+file:///demo#input@0.1.0"],"workspace_default_members":["path+file:///demo#input@0.1.0"]}'
"#;
        let verifier = r"#!/usr/bin/env python3
import json, os, sys, subprocess
seed = next((int(a.split('=')[1]) for a in sys.argv if a.startswith('smt.random_seed=')), 0)
if os.getenv('SAIL_TEST_SLOW'):
    c = subprocess.Popen(['sleep','30'])
    open(os.environ['SAIL_FIXTURE']+'/child.pid','w').write(str(c.pid))
    c.wait()
mode = os.getenv('SAIL_TEST_PATTERN','winner')
ok = [seed != 0, seed == 4] if mode == 'winner' else [seed == 0, seed != 0]
fs = [{'function': 'input::'+name, 'success': success, 'rlimit': 100, 'time-micros': 1} for name,success in zip(['a','b'],ok)]
print(json.dumps({'verification-results': {'verified': sum(ok), 'errors': 2-sum(ok), 'success': all(ok)}, 'times-ms': {'smt': {'smt-run-module-times': [{'module': '', 'function-breakdown': fs}]}}}))
print('diagnostics for seed '+str(seed), file=sys.stderr)
sys.exit(0 if all(ok) else 1)
";
        for (name, content) in [("cargo", metadata), ("cargo-verus", verifier)] {
            let p = dir.join(name);
            assert!(std::fs::write(&p, content).is_ok());
            assert!(std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).is_ok());
        }
        Self {
            dir,
            _listener: listener,
        }
    }

    fn command(&self) -> Command {
        let z3 =
            std::env::var("SAIL_TEST_Z3").unwrap_or_else(|e| unreachable!("pinned solver: {e}"));
        let mut c = Command::new(env!("CARGO_BIN_EXE_sail"));
        c.args([
            "verify",
            "--portfolio",
            "accept-first-proof",
            "--portfolio-seeds",
            "1,4",
            "--output",
            "json",
        ])
        .env("SAIL_CARGO_VERUS", self.dir.join("cargo-verus"))
        .env("CARGO", self.dir.join("cargo"))
        .env("VERUS_Z3_PATH", z3)
        .env("SAIL_DAEMON_SOCKET", self.dir.join("d.sock"))
        .env("SAIL_CACHE_DIR", self.dir.join("cache"))
        .env("SAIL_FIXTURE", &self.dir);
        c
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
#[ignore = "needs the pinned Z3 identity"]
fn complete_run_wins_and_each_seed_reverifies() {
    let f = Fixture::new("winner");
    let o = f
        .command()
        .args(["--", "--verify-root", "--verify-function", "*"])
        .output()
        .unwrap_or_else(|e| unreachable!("run: {e}"));
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let report: sail_api::output::Report =
        serde_json::from_slice(&o.stdout).unwrap_or_else(|e| unreachable!("report: {e}"));
    let p = report
        .portfolio
        .unwrap_or_else(|| unreachable!("portfolio evidence"));
    assert_eq!(p.winner_seed, Some(4));
    assert_eq!(p.attempts.len(), 3);
    assert!(p.attempts.iter().all(|a| {
        a.report
            .as_ref()
            .is_some_and(|r| r.mode == "accept-first-proof")
    }));
    let clean = std::fs::read_to_string(f.dir.join("clean.log")).unwrap_or_default();
    assert_eq!(clean.lines().count(), 3);
    assert!(clean.lines().all(|l| l.contains("/verus-partial")));
}

#[test]
#[ignore = "needs the pinned Z3 identity"]
fn complementary_partial_proofs_preserve_baseline_failure() {
    let f = Fixture::new("hybrid");
    let o = f
        .command()
        .env("SAIL_TEST_PATTERN", "hybrid")
        .output()
        .unwrap_or_else(|e| unreachable!("run: {e}"));
    assert_eq!(o.status.code(), Some(1));
    let r: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
    assert!(r["portfolio"]["winner_seed"].is_null());
    assert_eq!(r["crates"][0]["functions"][0]["verdict"], "verified");
    assert_eq!(r["crates"][0]["functions"][1]["verdict"], "failed");
    assert!(String::from_utf8_lossy(&o.stderr).contains("diagnostics for seed 0"));
}

#[test]
#[ignore = "needs the pinned Z3 identity"]
fn timeout_is_infrastructure_and_kills_descendants() {
    let f = Fixture::new("timeout");
    let o = f
        .command()
        .args(["--portfolio-timeout-secs", "1"])
        .env("SAIL_TEST_SLOW", "1")
        .output()
        .unwrap_or_else(|e| unreachable!("run: {e}"));
    assert_eq!(o.status.code(), Some(2));
    let r: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
    assert!(
        r["portfolio"]["attempts"]
            .as_array()
            .is_some_and(|a| a.iter().all(|v| v["error"] == "attempt timed out"))
    );
    let pid = std::fs::read_to_string(f.dir.join("child.pid")).unwrap_or_default();
    let status = Command::new("ps")
        .args(["-o", "stat=", "-p", pid.trim()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    assert!(
        status.trim().is_empty() || status.trim().starts_with('Z'),
        "descendant still alive: {status}"
    );
}
