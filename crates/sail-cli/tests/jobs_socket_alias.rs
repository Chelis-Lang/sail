//! Socket aliases must share one durable job service and FIFO verifier.
#![cfg(unix)]

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sail_api::jobs::{Lifecycle, Response, Status};

struct Fixture {
    root: PathBuf,
    executable: PathBuf,
    daemon: Option<Child>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(mut daemon) = self.daemon.take() {
            let _ = Command::new("kill")
                .args(["-s", "TERM", "--", &daemon.id().to_string()])
                .status();
            let _ = daemon.wait();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    fn command(&self, socket: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(self.root.join("project"))
            .env("SAIL_DAEMON_SOCKET", socket)
            .env("SAIL_CACHE_DIR", "../cache")
            .env("CARGO", self.root.join("bin/cargo"))
            .env("SAIL_CARGO_VERUS", self.root.join("bin/cargo-verus"))
            .env("VERUS_Z3_PATH", self.root.join("bin/z3"))
            .env("SAIL_ALIAS_MARKER", self.root.join("metadata-started"))
            .stdin(Stdio::null());
        // Cargo/nextest inject loader paths for test binaries. This fixture
        // copies an installed CLI and supplies its own fake backend; preserve
        // that ordinary environment instead of selecting a harness runtime.
        for variable in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
        ] {
            command.env_remove(variable);
        }
        command
    }
    fn inspect(&self, socket: &Path, args: &[&str]) -> Response {
        let output = self
            .command(socket)
            .args(args)
            .args(["--output", "json"])
            .output()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| unreachable!("{e}"))
    }
    fn submit(&self, socket: &Path) -> Status {
        let output = self
            .command(socket)
            .args(["verify", "--background", "--output", "json"])
            .output()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| unreachable!("{e}"))
    }
    fn wait(&self, socket: &Path, id: &str, expected: Lifecycle) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let Response::Status(status) = self.inspect(socket, &["jobs", "status", id]) else {
                unreachable!("missing status")
            };
            if status.state == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "unexpected lifecycle: {status:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
fn script(path: &Path, content: &str) {
    assert!(std::fs::write(path, content).is_ok());
    assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).is_ok());
}

#[test]
fn relative_serve_absolute_and_symlink_clients_share_jobs_and_fifo() {
    // macOS's default temporary directory alone nearly fills a Unix socket name.
    let root = PathBuf::from("/tmp").join(format!("sail-job-alias-cli-{}", std::process::id()));
    for directory in ["bin", "project/src", "cache"] {
        assert!(std::fs::create_dir_all(root.join(directory)).is_ok());
    }
    let root = root.canonicalize().unwrap_or_else(|e| unreachable!("{e}"));
    let executable = root.join("bin/sail");
    assert!(std::fs::copy(env!("CARGO_BIN_EXE_sail"), &executable).is_ok());
    for name in ["sail-z3", "z3", "cargo-verus"] {
        script(&root.join("bin").join(name), "#!/bin/sh\nexit 0\n");
    }
    script(
        &root.join("bin/cargo"),
        "#!/bin/sh\necho started > \"$SAIL_ALIAS_MARKER\"\nsleep 30\n",
    );
    assert!(
        std::fs::write(
            root.join("project/Cargo.toml"),
            "[package]\nname='alias_fixture'\nversion='0.1.0'\nedition='2024'\n"
        )
        .is_ok()
    );
    assert!(std::fs::write(root.join("project/src/lib.rs"), "pub fn one() {}\n").is_ok());
    let relative = Path::new("../cache/cache.sock");
    let absolute = root.join("cache/cache.sock");
    let mut fixture = Fixture {
        root,
        executable,
        daemon: None,
    };
    fixture.daemon = Some(
        fixture
            .command(relative)
            .args([
                "serve",
                "--socket",
                "../cache/cache.sock",
                "--cache",
                "../cache/results",
                "--idle-timeout",
                "0",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| unreachable!("{e}")),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !absolute.exists() || !sail_api::jobs::socket_path(&absolute).exists() {
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let alias = fixture.root.join("alias.sock");
    assert!(symlink(&absolute, &alias).is_ok());
    let first = fixture.submit(relative);
    fixture.wait(&absolute, &first.id, Lifecycle::Running);
    let second = fixture.submit(&absolute);
    let third = fixture.submit(&alias);
    for socket in [relative, absolute.as_path(), alias.as_path()] {
        let Response::List(jobs) = fixture.inspect(socket, &["jobs", "list"]) else {
            unreachable!("missing list")
        };
        assert_eq!(
            jobs.len(),
            3,
            "aliases must not launch an independent service"
        );
        assert!(
            jobs.iter()
                .any(|job| job.id == first.id && job.state == Lifecycle::Running)
        );
        assert!(
            jobs.iter()
                .any(|job| job.id == second.id && job.state == Lifecycle::Queued)
        );
        assert!(
            jobs.iter()
                .any(|job| job.id == third.id && job.state == Lifecycle::Queued)
        );
    }
    let Response::Status(cancelled) = fixture.inspect(&alias, &["jobs", "cancel", &first.id])
    else {
        unreachable!("missing cancellation")
    };
    assert_eq!(cancelled.state, Lifecycle::Cancelled);
    fixture.wait(relative, &second.id, Lifecycle::Running);
    fixture.wait(&alias, &third.id, Lifecycle::Queued);
    let _ = fixture.inspect(relative, &["jobs", "cancel", &second.id]);
    fixture.wait(&absolute, &third.id, Lifecycle::Running);
    let _ = fixture.inspect(&absolute, &["jobs", "cancel", &third.id]);
}
