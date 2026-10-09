//! A fresh machine replays signed replies and the session certificate over HTTP.
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test setup and assertions"
)]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use sail_cache::DiskCache;
use sail_cache::maint::FileKind;
use sail_cache::shared::{SecretKey, SharedStore, Solver, add_trusted};
use sail_daemon::http_server::{self, ServerConfig};
use sail_daemon::remote::{HttpClient, Published, RemoteConfig};

const INPUT: &[u8] = b"(set-option :print-success false)\n(declare-const x Int)\n(assert (> x 0))\n(assert (< x 0))\n(check-sat)\n(echo \"end\")\n(exit)\n";

fn run(solver: &Path, socket: Option<&Path>) -> Vec<u8> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
    command
        .args(["-smt2", "-in"])
        .env("SAIL_Z3_PATH", solver)
        .env_remove("SAIL_TRACE_DIR")
        .env_remove("SAIL_PROVENANCE_DIR")
        .env_remove("SAIL_DAEMON_SOCKET")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(socket) = socket {
        command.env("SAIL_DAEMON_SOCKET", socket);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(INPUT).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    output.stdout
}

fn daemon(
    socket: &Path,
    cache: &Path,
) -> (
    sail_daemon::Handle,
    std::thread::JoinHandle<std::io::Result<()>>,
) {
    let server = sail_daemon::bind(
        socket,
        cache,
        Arc::new(sail_daemon::Stats::default()),
        &sail_daemon::Config::default(),
    )
    .unwrap();
    let handle = server.handle();
    let thread = std::thread::spawn(move || server.run());
    (handle, thread)
}

#[test]
#[ignore = "needs pinned Z3 in SAIL_TEST_Z3"]
fn fresh_reader_replays_whole_session_without_starting_solver() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::path::PathBuf::from(format!("/tmp/sail-http-z3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let pinned = std::env::var("SAIL_TEST_Z3").expect("pinned solver required");
    let solver = root.join("solver");
    let log = root.join("starts");
    let quoted = pinned.replace('\'', "'\\''");
    std::fs::write(
        &solver,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            quoted
        ),
    )
    .unwrap();
    std::fs::set_permissions(&solver, std::fs::Permissions::from_mode(0o755)).unwrap();
    let direct = run(&solver, None);
    let first_socket = root.join("one.sock");
    let first_cache = root.join("one");
    let (first_stop, first_thread) = daemon(&first_socket, &first_cache);
    assert_eq!(run(&solver, Some(&first_socket)), direct);
    first_stop.shutdown();
    first_thread.join().unwrap().unwrap();
    let secret = SecretKey::from_seed([42; 32]);
    let staging = SharedStore::new(root.join("staging"));
    let report = sail_cache::shared::publish(
        &DiskCache::new(&first_cache),
        &staging,
        &secret,
        &sail_cache::shared::PublishOptions::default(),
        &|_| false,
    );
    assert!(
        report.published >= 3,
        "replies plus whole-session certificate: {report:?}"
    );
    let server = http_server::bind(&ServerConfig::local(root.join("remote"))).unwrap();
    let read = RemoteConfig {
        endpoint: format!("http://{}", server.local_addr().unwrap()),
        token_file: server.read_token_file.clone(),
        ca_file: None,
    };
    let write = RemoteConfig {
        token_file: server.write_token_file.clone(),
        ..read.clone()
    };
    let (http_stop, http_thread) = server.spawn().unwrap();
    let publisher = HttpClient::new(&write, Duration::from_secs(2)).unwrap();
    for entry in DiskCache::new(&first_cache).list() {
        let FileKind::Entry(key) = entry.kind else {
            continue;
        };
        let (meta, _) = DiskCache::check_file(&entry.path, &key).unwrap();
        let identity = Solver::parse_summary(&meta.solver).unwrap();
        let data = SharedStore::read(&staging.entry_path(&identity, &key).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            publisher.put(&identity, &key, data, &secret.public()),
            Ok(Published::Created)
        );
    }
    let second_socket = root.join("two.sock");
    let second_cache = root.join("two");
    add_trusted(&second_cache, &secret.public()).unwrap();
    sail_daemon::remote::connect(&second_cache, &read).unwrap();
    let (second_stop, second_thread) = daemon(&second_socket, &second_cache);
    let before = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| *l == "-smt2 -in")
        .count();
    assert_eq!(run(&solver, Some(&second_socket)), direct);
    let after = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| *l == "-smt2 -in")
        .count();
    assert_eq!(
        before, after,
        "the remote certificate must prevent even a solver replay"
    );
    let mut client = sail_api::client::Client::connect(&second_socket).unwrap();
    assert!(client.stats().unwrap().shared_hits >= 3);
    drop(client);
    second_stop.shutdown();
    second_thread.join().unwrap().unwrap();
    http_stop.stop();
    assert!(http_thread.join().unwrap().is_ok());
    let _ = std::fs::remove_dir_all(root);
}
