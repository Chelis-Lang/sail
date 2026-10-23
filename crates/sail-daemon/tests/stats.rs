//! The `STATS` operation reports the daemon's counters (ADR 0012).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::sync::Arc;

use sail_api::client::Client;
use sail_api::protocol::{PutMeta, StatsSnapshot};

#[test]
fn stats_count_hellos_gets_hits_and_puts() {
    // Short: Unix socket paths are limited to about 100 bytes.
    let dir = PathBuf::from(format!("/tmp/sail-ds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let socket = dir.join("d.sock");
    let (sock, cache) = (socket.clone(), dir.join("cache"));
    let counters = Arc::new(sail_daemon::Stats::default());
    let shared = Arc::clone(&counters);
    std::thread::spawn(move || sail_daemon::serve(&sock, &cache, shared));
    let mut client = None;
    for _ in 0..100 {
        if let Ok(c) = Client::connect(&socket) {
            client = Some(c);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let Some(mut c) = client else {
        unreachable!("the daemon did not start")
    };
    assert_eq!(c.stats().ok(), Some(StatsSnapshot::default()));
    // A solver that cannot be identified is still a HELLO.
    assert!(c.hello("/nonexistent/z3").is_err());
    assert!(c.get(&[7u8; 32]).is_err());
    let Ok(mut c) = Client::connect(&socket) else {
        unreachable!("cannot reconnect")
    };
    let solver = dir.join("solver");
    assert!(std::fs::write(&solver, b"#!/bin/sh\necho 'Z3 version 4.16.0'\n").is_ok());
    assert!(std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o700)).is_ok());
    assert!(c.hello(&solver.to_string_lossy()).is_ok());
    let key = [7u8; 32];
    assert!(matches!(c.get(&key), Ok(None)));
    let meta = PutMeta {
        command: "check-sat".to_owned(),
        provenance: "pinned-solver".to_owned(),
        effort: None,
    };
    assert!(c.put(&key, meta, b"unsat\n".to_vec()).is_ok());
    assert_eq!(c.get(&key).ok().flatten().as_deref(), Some(&b"unsat\n"[..]));
    let expected = StatsSnapshot {
        hellos: 2,
        gets: 2,
        hits: 1,
        puts: 1,
        // The reply was still held in memory after the PUT (ADR 0015).
        memory_hits: 1,
        dropped_puts: 0,
        // The admitted miss now has a solver identity; the unconfigured
        // shared tier is considered and skipped once.
        shared_skipped: 1,
        ..StatsSnapshot::default()
    };
    assert_eq!(c.stats().ok(), Some(expected));
    assert_eq!(counters.snapshot(), expected);
    let _ = std::fs::remove_dir_all(&dir);
}
