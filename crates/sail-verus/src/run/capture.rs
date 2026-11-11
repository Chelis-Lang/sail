//! A capture-failure witness independent of writable files and child environment.
//!
//! The parent owns this companion until Cargo has finished. A staged delegate
//! knows its PID from its executable's owned directory, so loss of all capture
//! environment or both file channels can still terminate the witness. Only the
//! parent closes its stdin to authorize successful completion.

use std::io::Read as _;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::{Error, Result};

/// Private companion mode of the compiler-capture executable.
pub const SUPERVISOR_ARG: &str = "--sail-capture-supervisor";
/// Private environment marker set only on the companion, never on Cargo.
pub const SUPERVISOR_ENV: &str = "SAIL_CONFORMANCE_SUPERVISOR";
const PREFIX: &str = "capture-frontend-";

pub(super) struct Supervisor {
    child: Child,
    alias: Option<std::path::PathBuf>,
}
impl Supervisor {
    pub(super) fn start(tee: &std::path::Path) -> Result<Self> {
        let child = Command::new(tee)
            .arg(SUPERVISOR_ARG)
            .env(SUPERVISOR_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| capture_error(format!("cannot start capture supervisor: {e}")))?;
        Ok(Self { child, alias: None })
    }
    pub(super) fn stage(&mut self, work: &std::path::Path) -> std::path::PathBuf {
        let stage = work.join(format!("{PREFIX}{}", self.child.id()));
        self.alias = Some(stage.join("verus"));
        stage
    }
    fn retire_alias(&mut self) -> std::io::Result<()> {
        if let Some(alias) = &self.alias {
            match std::fs::rename(alias, alias.with_file_name("verus.finished")) {
                Ok(()) => self.alias = None,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.alias = None,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<()> {
        // Retire the callable alias before releasing the owned PID. Retained
        // artifacts must not later signal a recycled process identifier.
        self.retire_alias().map_err(capture_error)?;
        if self.child.try_wait().map_err(capture_error)?.is_some() {
            return Err(capture_error(
                "compiler capture supervisor ended before Cargo completed",
            ));
        }
        // This is the only successful completion route. A delegate's fatal
        // signal is independent of this pipe and cannot be turned into EOF.
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(status) = self.child.try_wait().map_err(capture_error)? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(capture_error(format!(
                        "compiler capture supervisor failed: {status}"
                    )))
                };
            }
            if Instant::now() >= deadline {
                return Err(capture_error(
                    "compiler capture supervisor did not finalize",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        let _ = self.retire_alias();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
fn capture_error(error: impl std::fmt::Display) -> Error {
    Error::new("SAIL3018", error.to_string())
}

/// Waits for the owning harness to close the private companion's stdin.
///
/// # Errors
/// Returns a standard-input read error. It never reads compiler source input:
/// only the executable's explicitly selected private companion mode calls it.
pub fn capture_supervisor_main() -> std::io::Result<()> {
    let mut buffer = [0u8; 256];
    while std::io::stdin().read(&mut buffer)? != 0 {}
    Ok(())
}

/// Marks a staged compiler-capture attempt failed without writing any file or
/// reading capture environment. The owned supervisor remains the parent's child
/// until reaped, preventing PID reuse; killing it cannot become successful EOF.
/// Unstaged direct delegate controls have no supervisor and return false.
///
/// # Errors
/// Returns executable-identity, PID-conversion or signal-delivery errors.
pub fn fail_capture_supervisor() -> std::io::Result<bool> {
    let executable = std::env::current_exe()?;
    if executable.file_name() != Some(std::ffi::OsStr::new("verus")) || !executable.is_file() {
        return Ok(false);
    }
    let name = executable
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let Some(number) = name.strip_prefix(PREFIX) else {
        return Ok(false);
    };
    let number = number.parse::<i32>().map_err(std::io::Error::other)?;
    if number <= 1 {
        return Err(std::io::Error::other("invalid capture supervisor PID"));
    }
    #[cfg(unix)]
    {
        let pid = rustix::process::Pid::from_raw(number)
            .ok_or_else(|| std::io::Error::other("invalid capture supervisor PID"))?;
        rustix::process::kill_process(pid, rustix::process::Signal::TERM)?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = number;
        Err(std::io::Error::other(
            "compiler capture requires Unix process signals",
        ))
    }
}
