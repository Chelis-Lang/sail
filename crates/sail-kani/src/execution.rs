//! Bounded original-driver attempts, without source closure or reusable proofs.
//!
//! Callers own immutable tool/source prefixes and a prepared Cargo home. Cargo
//! configuration, dependencies and build scripts are not authenticated here.
//! These captures cannot establish currentness or authorize cache reuse.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, WaitId, WaitIdOptions};
use serde::Serialize;

use crate::{MAX_CAPTURE_BYTES, Observation, tools::Stage};

/// A refused, interrupted or incomplete execution; never a backend outcome.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "error[SAIL6304]: Kani execution: {}", self.0)
    }
}

impl std::error::Error for Error {}

fn refuse(reason: impl std::fmt::Display) -> Error {
    Error(reason.to_string())
}

/// Explicit inputs to one uncached original-driver attempt.
/// Source/dependency/configuration closure remains the caller's responsibility.
pub struct Request<'a> {
    /// Immutable Cargo workspace containing the hand-written harness.
    pub project: &'a Path,
    /// Prepared Cargo home; execution uses offline mode and never fetches.
    pub cargo_home: &'a Path,
    /// Exact fully qualified harness, with no suffix matching.
    pub harness: &'a str,
    /// Explicit CBMC unwind bound, independent of ambient driver options.
    pub unwind: u32,
    /// Child-process budget; filesystem inspection is not interruptible.
    pub process_limit: Duration,
}

/// Original raw captures and their strictly decoded bounded observation.
/// This carries no authenticated source closure, currentness or receipt.
pub struct Captured {
    /// Owned directory retaining the request, streams, export and compiler data.
    pub directory: PathBuf,
    /// Strict native observation; never an ordinary verified verdict.
    pub observation: Observation,
}

#[derive(Serialize)]
struct Invocation<'a> {
    program: String,
    harness: &'a str,
    unwind: u32,
    process_limit_ms: u128,
    argv: Vec<String>,
    environment: BTreeMap<String, String>,
    input_closure_authenticated: bool,
    proof_authority: bool,
}

/// Execute one original staged Kani driver and retain its complete raw streams.
/// The new destination must have a private, caller-owned parent. All source,
/// tool and Cargo-home inputs must remain caller-owned and immutable throughout.
///
/// # Errors
/// `SAIL6304` on invalid inputs, changed tools, cancellation, timeout, excessive
/// captures, process errors or strict decoder refusal. Partial files remain
/// diagnostic material, never readiness or reusable proof authority.
pub fn run(
    stage: &Stage,
    request: &Request<'_>,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<Captured, Error> {
    validate(request, cancel)?;
    stage.recheck().map_err(refuse)?;
    let directory = new_directory(destination)?;
    let project = request.project.canonicalize().map_err(refuse)?;
    let cargo_home = request.cargo_home.canonicalize().map_err(refuse)?;
    if !std::fs::symlink_metadata(project.join("Cargo.toml"))
        .map_err(refuse)?
        .is_file()
        || !cargo_home.is_dir()
    {
        return Err(refuse("missing regular manifest or prepared Cargo home"));
    }
    let invocation = invocation(stage, request, &directory, &project, &cargo_home)?;
    output(&directory.join("request.json"))?
        .write_all(&serde_json::to_vec_pretty(&invocation).map_err(refuse)?)
        .map_err(refuse)?;
    let mut command = Command::new(stage.driver());
    command
        .args(&invocation.argv)
        .current_dir(&project)
        .env_clear()
        .envs(&invocation.environment);
    let exit = capture(&mut command, &directory, request.process_limit, cancel)?;
    stage.recheck().map_err(refuse)?;
    let stdout = read(&directory.join("stdout"))?;
    let export = read(&directory.join("export.json"))?;
    let observation = crate::decode(request.harness, exit, &stdout, &export).map_err(refuse)?;
    if cancel.load(Ordering::SeqCst) {
        return Err(refuse("execution cancelled before observation admission"));
    }
    Ok(Captured {
        directory,
        observation,
    })
}

fn validate(request: &Request<'_>, cancel: &AtomicBool) -> Result<(), Error> {
    if cancel.load(Ordering::SeqCst)
        || request.harness.is_empty()
        || request.harness.len() > 4096
        || request.harness.starts_with('-')
        || request.harness.chars().any(char::is_control)
        || request.unwind == 0
        || request.process_limit < Duration::from_millis(1)
        || request.process_limit > Duration::from_secs(600)
        || !request.project.is_absolute()
        || !request.cargo_home.is_absolute()
    {
        return Err(refuse("invalid or cancelled original-driver request"));
    }
    Ok(())
}

fn text(path: &Path) -> Result<String, Error> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| refuse("execution paths must be valid UTF-8"))
}

fn invocation<'a>(
    stage: &Stage,
    request: &Request<'a>,
    directory: &Path,
    project: &Path,
    cargo_home: &Path,
) -> Result<Invocation<'a>, Error> {
    let toolchain = stage.root().join("toolchain");
    for name in ["home", "tmp", "target"] {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(directory.join(name))
            .map_err(refuse)?;
    }
    let rust_bin = text(&toolchain.join("bin"))?;
    let bundle_bin = text(&stage.root().join("bin"))?;
    if rust_bin.contains(':') || bundle_bin.contains(':') {
        return Err(refuse("tool paths cannot contain PATH separators"));
    }
    let environment = BTreeMap::from([
        ("LC_ALL".into(), "C".into()),
        ("HOME".into(), text(&directory.join("home"))?),
        ("TMPDIR".into(), text(&directory.join("tmp"))?),
        ("CARGO_HOME".into(), text(cargo_home)?),
        ("CARGO_NET_OFFLINE".into(), "true".into()),
        (
            "PATH".into(),
            format!("{rust_bin}:{bundle_bin}:/usr/bin:/bin"),
        ),
    ]);
    Ok(Invocation {
        program: text(&stage.driver())?,
        harness: request.harness,
        unwind: request.unwind,
        process_limit_ms: request.process_limit.as_millis(),
        argv: vec![
            "kani".into(),
            "-Z".into(),
            "unstable-options".into(),
            "--export-json".into(),
            text(&directory.join("export.json"))?,
            "--harness".into(),
            request.harness.into(),
            "--exact".into(),
            "--unwind".into(),
            request.unwind.to_string(),
            "--manifest-path".into(),
            text(&project.join("Cargo.toml"))?,
            "--target-dir".into(),
            text(&directory.join("target"))?,
        ],
        environment,
        input_closure_authenticated: false,
        proof_authority: false,
    })
}

fn new_directory(destination: &Path) -> Result<PathBuf, Error> {
    if !destination.is_absolute() {
        return Err(refuse("execution destination must be absolute"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| refuse("missing execution parent"))?
        .canonicalize()
        .map_err(refuse)?;
    if !parent.is_dir()
        || std::fs::metadata(&parent)
            .map_err(refuse)?
            .permissions()
            .mode()
            & 0o077
            != 0
    {
        return Err(refuse("execution parent must be private"));
    }
    let directory = parent.join(
        destination
            .file_name()
            .ok_or_else(|| refuse("missing execution name"))?,
    );
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(refuse)?;
    Ok(directory)
}

fn output(path: &Path) -> Result<File, Error> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(refuse)
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(refuse)?;
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(refuse)?;
    if !metadata.is_file() || metadata.len() > MAX_CAPTURE_BYTES as u64 {
        return Err(refuse("unsupported or excessive original capture"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_CAPTURE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(refuse)?;
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err(refuse("capture grew beyond its bound"));
    }
    Ok(bytes)
}

struct OwnedChild(Option<Child>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = rustix::process::kill_process_group(Pid::from_child(child), Signal::KILL);
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn capture(
    command: &mut Command,
    directory: &Path,
    limit: Duration,
    cancel: &AtomicBool,
) -> Result<i32, Error> {
    if cancel.load(Ordering::SeqCst) {
        return Err(refuse("execution cancelled before spawn"));
    }
    let stdout = output(&directory.join("stdout"))?;
    let stderr = output(&directory.join("stderr"))?;
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    let deadline = Instant::now() + limit;
    let mut process = OwnedChild(Some(command.spawn().map_err(refuse)?));
    let child = process
        .0
        .as_mut()
        .ok_or_else(|| refuse("missing original child"))?;
    let pid = Pid::from_child(child);
    loop {
        if cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return Err(refuse("execution cancelled or timed out"));
        }
        for name in ["stdout", "stderr", "export.json"] {
            match std::fs::symlink_metadata(directory.join(name)) {
                Ok(metadata)
                    if !metadata.is_file() || metadata.len() > MAX_CAPTURE_BYTES as u64 =>
                {
                    return Err(refuse("unsupported or excessive capture"));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(refuse(error)),
            }
        }
        // Observe without reaping: the group identifier cannot be recycled
        // between completion detection and killing inherited descendants.
        if rustix::process::waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .map_err(refuse)?
        .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    match rustix::process::kill_process_group(pid, Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => return Err(refuse(error)),
    }
    let status = child.wait().map_err(refuse)?;
    process.0 = None;
    let exit = status
        .code()
        .ok_or_else(|| refuse("original driver terminated by a signal"))?;
    output(&directory.join("exit"))?
        .write_all(format!("{exit}\n").as_bytes())
        .map_err(refuse)?;
    // Recheck final file types and sizes after all inherited writers stop.
    for name in ["stdout", "stderr"] {
        read(&directory.join(name))?;
    }
    Ok(exit)
}

#[cfg(test)]
mod tests;
