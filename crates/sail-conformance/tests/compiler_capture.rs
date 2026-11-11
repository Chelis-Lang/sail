//! Real-process integrity controls for the compiler delegate.
#![cfg(unix)]
#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sail_conformance::invocations::load;

type TestResult = Result<(), Box<dyn Error>>;
static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "sail-capture-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        std::fs::write(path.with_extension("errors"), b"")?;
        Ok(Self(path))
    }
    fn command(&self, script: &str) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-conformance-tee"));
        cmd.env("SAIL_CONFORMANCE_TEE_DRIVER", "/bin/sh")
            .env("SAIL_CONFORMANCE_TEE_SOURCE", &self.0)
            .env("SAIL_CONFORMANCE_TEE_DIR", &self.0)
            .env(
                "SAIL_CONFORMANCE_TEE_ERRORS",
                self.0.with_extension("errors"),
            )
            .args(["-c", script]);
        cmd
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("errors"));
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

#[test]
fn reservation_failure_reaches_an_independent_health_record() -> TestResult {
    let scratch = Scratch::new()?;
    let blocked = scratch.0.join("not-a-directory");
    std::fs::write(&blocked, b"blocked")?;
    let output = scratch
        .command("exit 0")
        .env("SAIL_CONFORMANCE_TEE_DIR", blocked)
        .output()?;
    assert_eq!(output.status.code(), Some(125));
    let health = std::fs::read_to_string(scratch.0.with_extension("errors"))?;
    assert!(health.contains("cannot reserve capture"), "{health}");
    Ok(())
}

#[test]
fn physical_packed_options_are_recorded_with_their_exact_spelling() -> TestResult {
    let scratch = Scratch::new()?;
    let packed = "__VERUS_DRIVER_ARGS_SEP__--verify-function__VERUS_DRIVER_ARGS_SEP__p::one";
    let output = scratch
        .command("exit 0")
        .env(" __VERUS_DRIVER_ARGS__", packed)
        .output()?;
    assert!(output.status.success());
    let records = load(&scratch.0)?;
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0]
            .meta
            .driver_env
            .get(" __VERUS_DRIVER_ARGS__")
            .map(String::as_str),
        Some(packed)
    );
    assert_eq!(
        records[0].meta.requested_driver_env,
        records[0].meta.driver_env
    );
    Ok(())
}

#[test]
fn compiler_stdin_is_forwarded_retained_and_bound_to_probe_outcomes() -> TestResult {
    fn invoke(scratch: &Scratch, bytes: &[u8], swap: bool) -> TestResult {
        let mut child = scratch.command("read value; if [ \"$value\" = one ]; then code=0; else code=7; fi; if [ \"$SAIL_CAPTURE_SWAP\" = yes ]; then code=$((7-code)); fi; exit $code")
            .arg("-").env("SAIL_CAPTURE_SWAP", if swap { "yes" } else { "no" })
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        let mut input = child.stdin.take().ok_or("missing input pipe")?;
        std::io::Write::write_all(&mut input, bytes)?;
        drop(input);
        child.wait()?;
        Ok(())
    }
    let plain = Scratch::new()?;
    let changed = Scratch::new()?;
    for (scratch, swap) in [(&plain, false), (&changed, true)] {
        invoke(scratch, b"one\n", swap)?;
        invoke(scratch, b"two\n", swap)?;
    }
    let p = load(&plain.0)?;
    let s = load(&changed.0)?;
    assert_eq!(p.len(), 2);
    assert_eq!(s.len(), 2);
    assert!(p.iter().any(|p| p.stdin == b"one\n"));
    assert!(p.iter().any(|p| p.stdin == b"two\n"));
    assert_ne!(
        sail_conformance::invocations::compare_arms(&p, &s).differences,
        [] as [String; 0]
    );
    Ok(())
}

#[test]
fn failed_health_preflight_cannot_hide_after_an_earlier_complete_job() -> TestResult {
    let scratch = Scratch::new()?;
    assert!(scratch.command("exit 0").output()?.status.success());
    assert_eq!(load(&scratch.0)?.len(), 1);
    let health = scratch.0.with_extension("errors");
    let saved = scratch.0.with_extension("saved-health");
    std::fs::rename(&health, &saved)?;
    // A directory is unopenable for append even when Linux CI runs as root.
    std::fs::create_dir(&health)?;
    let output = scratch.command("exit 0").output()?;
    std::fs::remove_dir(&health)?;
    std::fs::rename(&saved, &health)?;
    assert_eq!(output.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&output.stderr).contains("health record is inaccessible"));
    assert_eq!(std::fs::read(&health)?, [] as [u8; 0]);
    assert!(
        load(&scratch.0).is_err(),
        "failed preflight left no incomplete reservation"
    );
    Ok(())
}

#[test]
fn supervisor_observes_failure_when_both_file_channels_or_environment_are_lost() -> TestResult {
    struct Companion(std::process::Child);
    impl Drop for Companion {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for cleared in [false, true] {
        let scratch = Scratch::new()?;
        let mut owner = Companion(
            Command::new(env!("CARGO_BIN_EXE_sail-conformance-tee"))
                .arg(sail_verus::run::SUPERVISOR_ARG)
                .env(sail_verus::run::SUPERVISOR_ENV, "1")
                .process_group(0)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let stage = scratch.0.join(format!("capture-frontend-{}", owner.0.id()));
        std::fs::create_dir(&stage)?;
        let executable = stage.join("verus");
        std::fs::copy(env!("CARGO_BIN_EXE_sail-conformance-tee"), &executable)?;
        let mut command = Command::new(&executable);
        command.process_group(i32::try_from(owner.0.id())?);
        command
            .env("SAIL_CONFORMANCE_TEE_DRIVER", "/bin/sh")
            .args(["-c", "exit 0"]);
        if cleared {
            command
                .env_remove("SAIL_CONFORMANCE_TEE_DIR")
                .env_remove("SAIL_CONFORMANCE_TEE_ERRORS");
        } else {
            let blocked = scratch.0.join("not-a-directory");
            std::fs::write(&blocked, "blocked")?;
            command
                .env("SAIL_CONFORMANCE_TEE_DIR", blocked)
                .env("SAIL_CONFORMANCE_TEE_ERRORS", &scratch.0);
        }
        let failed = command.output()?;
        assert!(!failed.status.success());
        assert!(
            failed.status.code().is_none(),
            "capture group was not terminated"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            if let Some(status) = owner.0.try_wait()? {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "capture error was lost by the companion"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(!status.success());
    }
    // Merely using the staged alias does not fail a healthy capture.
    let scratch = Scratch::new()?;
    let mut owner = Companion(
        Command::new(env!("CARGO_BIN_EXE_sail-conformance-tee"))
            .arg(sail_verus::run::SUPERVISOR_ARG)
            .env(sail_verus::run::SUPERVISOR_ENV, "1")
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let stage = scratch.0.join(format!("capture-frontend-{}", owner.0.id()));
    std::fs::create_dir(&stage)?;
    let executable = stage.join("verus");
    std::fs::copy(env!("CARGO_BIN_EXE_sail-conformance-tee"), &executable)?;
    let base = scratch.command("exit 0");
    let output = Command::new(executable)
        .process_group(i32::try_from(owner.0.id())?)
        .args(base.get_args())
        .envs(base.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .output()?;
    assert!(output.status.success());
    assert!(owner.0.try_wait()?.is_none());
    drop(owner.0.stdin.take());
    assert!(owner.0.wait()?.success());
    Ok(())
}

#[test]
fn retained_alias_cannot_signal_a_process_outside_its_current_group() -> TestResult {
    let scratch = Scratch::new()?;
    let mut victim = Command::new("/bin/sh")
        .args(["-c", "cat >/dev/null"])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let run = (|| -> TestResult {
        let stage = scratch.0.join(format!("capture-frontend-{}", victim.id()));
        std::fs::create_dir(&stage)?;
        let executable = stage.join("verus");
        std::fs::copy(env!("CARGO_BIN_EXE_sail-conformance-tee"), &executable)?;
        let failed = Command::new(executable)
            .env_remove("SAIL_CONFORMANCE_TEE_DIR")
            .env_remove("SAIL_CONFORMANCE_TEE_ERRORS")
            .args(["-c", "exit 0"])
            .output()?;
        assert_eq!(failed.status.code(), Some(125));
        assert!(
            victim.try_wait()?.is_none(),
            "retained alias signaled an unrelated process"
        );
        Ok(())
    })();
    let _ = victim.kill();
    let _ = victim.wait();
    run
}
