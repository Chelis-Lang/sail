//! Workspace watch cancels durable jobs, sees external path edits and emits JSONL.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn wait_for(what: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn count(log: &Path, word: &str) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.trim() == word)
        .count()
}
fn package(root: &Path, name: &str, dependencies: &str) {
    assert!(std::fs::create_dir_all(root.join("src")).is_ok());
    assert!(
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n{dependencies}\n")
        )
        .is_ok()
    );
    assert!(std::fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").is_ok());
}

#[test]
fn edits_cancel_the_run_in_flight_and_restart() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let krate = root.join("crate");
    let dependency = root.join("dependency");
    package(&dependency, "watch_dependency", "");
    package(
        &krate,
        "watch_fixture",
        "[dependencies]\nwatch_dependency={path='../dependency'}",
    );
    let log = root.join("runs.log");
    let fake = root.join("worker");
    let script = format!(
        "#!/bin/sh\necho start >> '{0}'\necho $$ >> '{1}'\nsleep 30\necho end >> '{0}'\n",
        log.display(),
        root.join("pids").display()
    );
    assert!(std::fs::write(&fake, script).is_ok());
    assert!(std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).is_ok());
    let cache_socket = root.join("d.sock");
    let socket = sail_api::jobs::socket_path(&cache_socket);
    let mut daemon = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &fake)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let mut watch = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["watch", "--output", "jsonl"])
        .current_dir(&krate)
        .env("SAIL_DAEMON_SOCKET", &cache_socket)
        .env("SAIL_CACHE_DIR", root.join("cache"))
        .stdout(
            std::fs::File::create(root.join("events.jsonl"))
                .map_or_else(|_| Stdio::null(), Stdio::from),
        )
        .stderr(
            std::fs::File::create(root.join("watch.err"))
                .map_or_else(|_| Stdio::null(), Stdio::from),
        )
        .spawn()
        .unwrap_or_else(|error| unreachable!("{error}"));
    wait_for("the first run", || count(&log, "start") == 1);
    // An external path dependency changes with identical size and a restored mtime.
    let source = dependency.join("src/lib.rs");
    let modified = std::fs::metadata(&source)
        .and_then(|metadata| metadata.modified())
        .ok();
    assert!(std::fs::write(&source, "pub fn b() {}\n").is_ok());
    if let Some(modified) = modified {
        assert!(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&source)
                .and_then(|file| file.set_times(std::fs::FileTimes::new().set_modified(modified)))
                .is_ok()
        );
    }
    wait_for("the restarted run", || count(&log, "start") == 2);
    assert_eq!(count(&log, "end"), 0);
    let _ = Command::new("kill")
        .args(["-INT", &watch.id().to_string()])
        .status();
    wait_for("watch exit", || matches!(watch.try_wait(), Ok(Some(_))));
    daemon.shutdown();
    let events = std::fs::read_to_string(root.join("events.jsonl")).unwrap_or_default();
    assert!(
        events
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
    assert!(events.contains("queued"));
    assert!(events.contains("superseded"));
    assert_eq!(count(&log, "end"), 0);
    for pid in std::fs::read_to_string(root.join("pids"))
        .unwrap_or_default()
        .lines()
    {
        assert!(
            !Command::new("kill")
                .args(["-0", pid])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success()),
            "cancelled worker {pid} remained alive"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}
