//! CLI job inspection is readable by default and structured when requested.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, Lifecycle, Request, Response, Submission};

#[test]
fn human_and_json_inspection_preserve_lifecycle_and_reject_non_utf8_environment() {
    use std::os::unix::ffi::OsStringExt as _;
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-cli-jobs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert!(std::fs::create_dir_all(root.join("project")).is_ok());
    let executable = root.join("worker");
    assert!(std::fs::write(&executable, "#!/bin/sh\necho verifying >&2\nsleep 30\n").is_ok());
    assert!(std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).is_ok());
    let cache_socket = root.join("cache.sock");
    let socket = sail_api::jobs::socket_path(&cache_socket);
    let mut daemon = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &executable)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(&socket);
    let submission = Submission {
        mode: sail_api::jobs::Mode::Stock,
        project_dir: root.join("project"),
        args: vec![],
        force: false,
        environment: BTreeMap::from([("PATH".to_owned(), "/usr/bin:/bin".to_owned())]),
        input_revision: Some("revision".to_owned()),
    };
    let Ok(Response::Status(status)) = client.call(&Request::Submit { submission }) else {
        unreachable!("no submit response")
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(client.call(&Request::Status { id:status.id.clone() }), Ok(Response::Status(status)) if status.state == Lifecycle::Running)
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_sail"))
            .args(args)
            .env("SAIL_DAEMON_SOCKET", &cache_socket)
            .env("SAIL_CACHE_DIR", root.join("cache"))
            .current_dir(root.join("project"))
            .output()
    };
    let human =
        run(&["jobs", "status", &status.id]).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("Running"));
    assert!(serde_json::from_slice::<serde_json::Value>(&human.stdout).is_err());
    let json = run(&["jobs", "status", &status.id, "--output", "json"])
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        matches!(serde_json::from_slice::<Response>(&json.stdout), Ok(Response::Status(status)) if status.state == Lifecycle::Running)
    );
    let missing = run(&["jobs", "status", "nonexistent", "--output", "json"])
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("SAIL0805"));
    background_submission_does_not_invoke_cargo(&root, &cache_socket);
    let invalid = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["verify", "--background"])
        .env("SAIL_DAEMON_SOCKET", &cache_socket)
        .env("SAIL_CACHE_DIR", root.join("cache"))
        .env("SAIL_TEST_NONUTF", std::ffi::OsString::from_vec(vec![0xff]))
        .current_dir(root.join("project"))
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("SAIL0812"));
    daemon.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

fn background_submission_does_not_invoke_cargo(root: &std::path::Path, socket: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let cargo = root.join("cargo");
    let marker = root.join("metadata-invoked");
    assert!(
        std::fs::write(
            &cargo,
            format!(
                "#!/bin/sh\necho invoked > '{}'\nsleep 30\nexit 1\n",
                marker.display()
            )
        )
        .is_ok()
    );
    assert!(std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).is_ok());
    let started = Instant::now();
    let submitted = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["verify", "--background", "--output", "json"])
        .env("SAIL_DAEMON_SOCKET", socket)
        .env("SAIL_CACHE_DIR", root.join("cache"))
        .env("CARGO", &cargo)
        .current_dir(root.join("project"))
        .output()
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        submitted.status.success(),
        "{}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    assert!(
        !marker.exists(),
        "submission must not resolve Cargo dependencies"
    );
    assert!(
        matches!(serde_json::from_slice::<sail_api::jobs::Status>(&submitted.stdout), Ok(status) if status.state == Lifecycle::Queued && status.input_revision.is_none())
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}
