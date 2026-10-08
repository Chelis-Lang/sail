//! `sail watch` restarts on edits and cancels the run in flight.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn count(log: &Path, word: &str) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.trim() == word)
        .count()
}

#[test]
fn edits_cancel_the_run_in_flight_and_restart() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-watch-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let krate = root.join("crate");
    assert!(std::fs::create_dir_all(krate.join("src")).is_ok());
    assert!(std::fs::write(krate.join("src/lib.rs"), "fn a() {}").is_ok());
    let log = root.join("runs.log");
    // A stand-in for cargo-verus: logs each start, then runs "forever".
    let fake = root.join("cargo-verus");
    let script = format!(
        "#!/bin/sh\necho start >> '{0}'\necho $$ >> '{1}'\nsleep 30\necho end >> '{0}'\n",
        log.display(),
        root.join("pids").display()
    );
    assert!(std::fs::write(&fake, script).is_ok());
    assert!(std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).is_ok());
    let fake_z3 = root.join("z3");
    assert!(std::fs::write(&fake_z3, "").is_ok());
    let socket = root.join("d.sock");

    let mut watch = match Command::new(env!("CARGO_BIN_EXE_sail"))
        .arg("watch")
        .current_dir(&krate)
        .env("SAIL_CARGO_VERUS", &fake)
        .env("VERUS_Z3_PATH", &fake_z3)
        .env("SAIL_DAEMON_SOCKET", &socket)
        .env("SAIL_CACHE_DIR", root.join("cache"))
        .stdout(Stdio::null())
        .stderr(
            std::fs::File::create(root.join("watch.err"))
                .map_or_else(|_| Stdio::null(), Stdio::from),
        )
        .spawn()
    {
        Ok(c) => c,
        Err(e) => unreachable!("cannot start sail watch: {e}"),
    };

    wait_for("the first run", || count(&log, "start") == 1);
    assert!(std::fs::write(krate.join("src/lib.rs"), "fn a() { }").is_ok());
    wait_for("the restarted run", || count(&log, "start") == 2);
    assert_eq!(
        count(&log, "end"),
        0,
        "the first run must have been cancelled"
    );

    // Ctrl-C stops sail watch and the run in flight.
    let _ = Command::new("kill")
        .args(["-INT", &watch.id().to_string()])
        .status();
    wait_for("sail watch to exit", || {
        matches!(watch.try_wait(), Ok(Some(_)))
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        count(&log, "end"),
        0,
        "the run in flight must have been killed"
    );

    let _ = Command::new("pkill")
        .args(["-f", &format!("serve --socket {}", socket.display())])
        .status();
    let _ = std::fs::remove_dir_all(&root);
}
