//! Real-process integrity controls for the compiler delegate.
#![cfg(unix)]
#![forbid(unsafe_code)]

use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sail_conformance::invocations::load;

type TestResult = Result<(), Box<dyn Error>>;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "sail-capture-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn command(&self, script: &str) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-conformance-tee"));
        cmd.env("SAIL_CONFORMANCE_TEE_DRIVER", "/bin/sh")
            .env("SAIL_CONFORMANCE_TEE_SOURCE", &self.0)
            .env("SAIL_CONFORMANCE_TEE_DIR", &self.0)
            .args(["-c", script]);
        cmd
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_driver_streams_status_and_corrupt_metadata_are_checked() -> TestResult {
    let scratch = Scratch::new()?;
    let output = scratch
        .command("printf 'stdout\\n'; printf 'diagnostic\\n' >&2; exit 7")
        .output()?;
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"stdout\n");
    assert_eq!(output.stderr, b"diagnostic\n");
    let captures = load(&scratch.0)?;
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].stdout, output.stdout);
    assert_eq!(captures[0].stderr, output.stderr);
    assert_eq!(captures[0].meta.exit_code, Some(7));
    let dir = std::fs::read_dir(&scratch.0)?
        .next()
        .ok_or("missing capture")??
        .path();
    let meta = dir.join("meta.json");
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&meta)?)?;
    value["stdout_len"] = 1.into();
    std::fs::write(&meta, serde_json::to_vec(&value)?)?;
    assert!(load(&scratch.0).is_err());
    Ok(())
}

#[test]
fn killed_delegate_leaves_a_refused_incomplete_record() -> TestResult {
    let scratch = Scratch::new()?;
    let mut child = scratch
        .command("sleep 1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read_dir(&scratch.0)?.next().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill()?;
    child.wait()?;
    assert!(std::fs::read_dir(&scratch.0)?.next().is_some());
    assert!(load(&scratch.0).is_err());
    Ok(())
}

#[test]
fn forwarding_failure_cannot_become_a_complete_accepted_capture() -> TestResult {
    let scratch = Scratch::new()?;
    let mut child = scratch
        .command("dd if=/dev/zero bs=65536 count=4 2>/dev/null")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    drop(child.stdout.take());
    child.wait()?;
    assert!(load(&scratch.0).is_err());
    Ok(())
}
