//! Verification activity participates in the cache daemon's idle lifecycle.
#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[test]
fn active_verification_keeps_daemon_alive_then_releases_idle_shutdown() {
    let root = std::env::temp_dir().join(format!("sail-job-idle-{}", std::process::id()));
    let socket = root.join("d.sock");
    assert!(std::fs::create_dir_all(&root).is_ok());
    let config = sail_daemon::Config {
        idle_timeout: Some(Duration::from_millis(40)),
        ..sail_daemon::Config::default()
    };
    let server = sail_daemon::bind(
        &socket,
        &root.join("results"),
        Arc::new(sail_daemon::Stats::default()),
        &config,
    )
    .unwrap_or_else(|e| unreachable!("cannot bind test daemon: {e}"));
    let handle = server.handle();
    let activity = Arc::new(AtomicU64::new(1));
    let running = Arc::clone(&activity);
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let result = server.run_with_activity(running);
        let _ = tx.send(result);
    });
    std::thread::sleep(Duration::from_millis(120));
    assert!(rx.try_recv().is_err(), "active job allowed idle shutdown");
    assert!(sail_api::client::Client::connect(&socket).is_ok());
    activity.store(0, Ordering::SeqCst);
    let result = rx.recv_timeout(Duration::from_secs(2));
    handle.shutdown();
    assert!(thread.join().is_ok());
    assert!(
        result.is_ok_and(|r| r.is_ok()),
        "daemon did not stop after work finished"
    );
    assert!(!socket.exists());
    assert!(std::fs::remove_dir_all(root).is_ok());
}
