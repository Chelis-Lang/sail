//! Complete optional CLI setup/publication/disconnection workflow with private credentials.
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test setup and assertions"
)]

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use sail_cache::DiskCache;
use sail_cache::shared::{SecretKey, Solver};
use sail_key::QueryKey;

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn command(args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn solo_cli_can_start_publish_connect_and_disconnect_without_container_or_daemon_setup() {
    let root = std::env::temp_dir().join(format!("sail-cli-http-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let store = root.join("store");
    let (server, endpoint, read, write) = start(&store);
    let secret = SecretKey::from_seed([44; 32]);
    let key_file = root.join("team.key");
    let public_file = secret.write_files(&key_file).unwrap();
    let writer_root = root.join("writer");
    let solver = Solver {
        name: "z3".to_owned(),
        version: "4.16.0".to_owned(),
        binary_sha256: "ab".repeat(32),
        platform: "linux-x86_64".to_owned(),
    };
    let disk = DiskCache::new(&writer_root);
    disk.put(
        &QueryKey([9; 32]),
        b"unsat\n",
        "check-sat",
        &solver.summary(),
        "pinned-solver",
        None,
    )
    .unwrap();
    disk.put(
        &QueryKey([10; 32]),
        b"unsat\n",
        "check-sat",
        &solver.summary(),
        "shared:other",
        None,
    )
    .unwrap();
    let published = command(&[
        "cache",
        "--cache",
        text(&writer_root),
        "publish",
        "--to",
        &endpoint,
        "--key",
        text(&key_file),
        "--token-file",
        text(&write),
        "--json",
    ]);
    let report: serde_json::Value = serde_json::from_slice(&published.stdout).unwrap();
    assert_eq!(report["publish"]["published"], 1);
    assert_eq!(report["publish"]["from_shared"], 1);
    let reader_root = root.join("reader");
    let connected = command(&[
        "cache",
        "--cache",
        text(&reader_root),
        "connect",
        &endpoint,
        "--token-file",
        text(&read),
        "--trust",
        text(&public_file),
    ]);
    assert!(
        !String::from_utf8_lossy(&connected.stdout)
            .contains(&std::fs::read_to_string(&read).unwrap())
    );
    assert!(reader_root.join("sail-remote.json").is_file());
    assert!(
        reader_root
            .join("trust")
            .join(format!("{}.pub", secret.public().id()))
            .is_file()
    );
    command(&["cache", "--cache", text(&reader_root), "disconnect"]);
    assert!(!reader_root.join("sail-remote.json").exists());
    assert!(reader_root.join("trust").exists());
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

fn start(store: &Path) -> (Server, String, PathBuf, PathBuf) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["cache", "serve", "--store", text(store)])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let server = Server(child);
    let mut lines = BufReader::new(stdout).lines();
    let endpoint = lines
        .next()
        .unwrap()
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap()
        .to_owned();
    let read = PathBuf::from(
        lines
            .next()
            .unwrap()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap(),
    );
    let write = PathBuf::from(
        lines
            .next()
            .unwrap()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap(),
    );
    (server, endpoint, read, write)
}
