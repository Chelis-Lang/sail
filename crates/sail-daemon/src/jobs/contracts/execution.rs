//! Bounded process-group execution for neutral workers only.
use std::io::{self, Write as _};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use sail_api::contracts::{MAX_STDERR_BYTES, MAX_STDOUT_BYTES, WALL_TIME_SECS};
use sail_api::jobs::Lifecycle;
use sail_api::project::ProjectResult;

use super::{State, decode, environment, error, source, store};

pub(in crate::jobs) fn execute(
    state: &State,
    job: &store::Stored,
) -> io::Result<(Option<i32>, ProjectResult, store::Projection)> {
    let deadline = Instant::now() + Duration::from_secs(WALL_TIME_SECS);
    let input = source(state, job)?;
    let directory = state.root.join(&job.status.id);
    for name in ["events.jsonl", "proof-source.jsonl"] {
        store::output(&directory.join(name))?;
    }
    let stdout = store::output(&directory.join("stdout"))?;
    let stderr = store::output(&directory.join("stderr"))?;
    let mut command = Command::new(&state.executable);
    command
        .arg("_contracts-worker")
        .arg(directory.join("contract-source.json"))
        .current_dir(&directory)
        .env_clear()
        .envs(environment()?)
        .env("SAIL_JOB_GUARD_PARENT", "1")
        .env("SAIL_JOB_EVENTS", directory.join("events.jsonl"))
        .env("SAIL_PROOF_EVENTS", directory.join("proof-source.jsonl"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = Group {
        child: command.spawn()?,
        settled: false,
    };
    let _parent = child.child.stdin.take();
    let failed = Arc::new(AtomicBool::new(false));
    let output_reader = drain(
        child
            .child
            .stdout
            .take()
            .ok_or_else(|| error("missing stdout pipe"))?,
        stdout,
        MAX_STDOUT_BYTES,
        failed.clone(),
    );
    let error_reader = drain(
        child
            .child
            .stderr
            .take()
            .ok_or_else(|| error("missing stderr pipe"))?,
        stderr,
        MAX_STDERR_BYTES,
        failed.clone(),
    );
    let outcome = running(state, job).and_then(|()| {
        wait(
            &mut child.child,
            &output_reader,
            &error_reader,
            &failed,
            deadline,
            || state.stop.load(Ordering::SeqCst) || job.cancel.load(Ordering::SeqCst),
        )
    });
    if outcome.is_err() {
        child.kill();
    }
    // Always join both readers after group cleanup, including persistence and wait errors.
    let output_done = output_reader
        .join()
        .map_err(|_| error("stdout reader failed"));
    let error_done = error_reader
        .join()
        .map_err(|_| error("stderr reader failed"));
    output_done??;
    error_done??;
    let status = outcome?;
    child.settled = true;
    let bytes = store::read_bytes(&directory.join("stdout"))?;
    if bytes.is_empty() {
        return Err(io::Error::other(super::super::worker::no_result(
            &directory, status,
        )));
    }
    let result = decode(&bytes, &input)?;
    let expected_lowering = if input.profile == 2 {
        3
    } else {
        sail_verus::contracts::LOWERING_VERSION
    };
    if result.contracts.manifest.version != expected_lowering {
        return Err(error(
            "new neutral jobs require corrected lowering version two",
        ));
    }
    if result.backend.exit_code != status.code() {
        return Err(error("neutral backend and process exit status disagree"));
    }
    let stderr = store::read_bytes(&directory.join("stderr"))?;
    let projection = store::Projection::validated(&bytes, &result.backend).with_stderr(&stderr);
    Ok((status.code(), result.backend, projection))
}

fn running(state: &State, job: &store::Stored) -> io::Result<()> {
    let mut jobs = state.jobs.lock().map_err(super::super::lock_error)?;
    if let Some(saved) = jobs.get_mut(&job.status.id)
        && !saved.status.state.terminal()
    {
        saved.status.state = Lifecycle::Running;
        saved.status.started_at_ms = Some(store::now());
        store::save(&state.root, saved)?;
    }
    Ok(())
}

fn wait(
    child: &mut Child,
    stdout: &std::thread::JoinHandle<io::Result<()>>,
    stderr: &std::thread::JoinHandle<io::Result<()>>,
    failed: &AtomicBool,
    deadline: Instant,
    cancelled: impl Fn() -> bool,
) -> io::Result<ExitStatus> {
    let mut status = None;
    loop {
        let fault = if cancelled() {
            Some("neutral verification cancelled")
        } else if failed.load(Ordering::SeqCst) {
            Some("neutral worker output limit or stream failure")
        } else if Instant::now() >= deadline {
            Some("neutral worker safety deadline expired")
        } else {
            None
        };
        if let Some(fault) = fault {
            return Err(io::Error::other(format!("SAIL3143: {fault}")));
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if let Some(status) = status
            && stdout.is_finished()
            && stderr.is_finished()
        {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// Unlike the legacy guard, error cleanup must kill descendants even after the leader
// exited: inherited output pipes otherwise keep readers alive past the deadline.
struct Group {
    child: Child,
    settled: bool,
}
impl Group {
    fn kill(&mut self) {
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{}", self.child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        if !self.settled {
            self.kill();
        }
    }
}

fn drain(
    mut input: impl std::io::Read + Send + 'static,
    mut file: std::fs::File,
    limit: usize,
    failed: Arc<AtomicBool>,
) -> std::thread::JoinHandle<io::Result<()>> {
    std::thread::spawn(move || {
        let mut count = 0;
        let mut buffer = [0; 16 * 1024];
        loop {
            let size = match input.read(&mut buffer) {
                Ok(size) => size,
                Err(issue) => {
                    failed.store(true, Ordering::SeqCst);
                    return Err(issue);
                }
            };
            if size == 0 {
                break;
            }
            let retained = size.min(limit - count);
            if let Err(issue) = file.write_all(&buffer[..retained]) {
                failed.store(true, Ordering::SeqCst);
                return Err(issue);
            }
            count += retained;
            if retained != size {
                failed.store(true, Ordering::SeqCst);
                return file.sync_all();
            }
        }
        if let Err(issue) = file.sync_all() {
            failed.store(true, Ordering::SeqCst);
            return Err(issue);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn files() -> io::Result<(std::path::PathBuf, std::fs::File, std::fs::File)> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-neutral-drain-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let stdout = std::fs::File::create(root.join("out"))?;
        let stderr = std::fs::File::create(root.join("err"))?;
        Ok((root, stdout, stderr))
    }
    #[test]
    fn overflow_retains_only_exact_prefix_and_marks_failure() -> TestResult {
        let (root, stdout, _) = files()?;
        let failed = Arc::new(AtomicBool::new(false));
        drain(
            std::io::Cursor::new(b"original stream"),
            stdout,
            8,
            failed.clone(),
        )
        .join()
        .map_err(|_| "reader thread")??;
        assert!(failed.load(Ordering::SeqCst));
        assert_eq!(std::fs::read(root.join("out"))?, b"original");
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn cancellation_and_deadline_kill_descendant_pipes_after_leader_exits() -> TestResult {
        for cancelled in [false, true] {
            let (root, stdout, stderr) = files()?;
            let child = Command::new("sh")
                .args(["-c", "sleep 30 & exit 0"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0)
                .spawn()?;
            let mut group = Group {
                child,
                settled: false,
            };
            let failed = Arc::new(AtomicBool::new(false));
            let output = drain(
                group.child.stdout.take().ok_or("stdout")?,
                stdout,
                1024,
                failed.clone(),
            );
            let errors = drain(
                group.child.stderr.take().ok_or("stderr")?,
                stderr,
                1024,
                failed.clone(),
            );
            let started = Instant::now();
            let result = wait(
                &mut group.child,
                &output,
                &errors,
                &failed,
                started + Duration::from_millis(120),
                || cancelled && started.elapsed() >= Duration::from_millis(60),
            );
            assert!(result.is_err());
            assert!(group.child.try_wait()?.is_some(), "leader already exited");
            group.kill();
            output.join().map_err(|_| "output reader")??;
            errors.join().map_err(|_| "error reader")??;
            assert!(started.elapsed() < Duration::from_secs(3));
            std::fs::remove_dir_all(root)?;
        }
        Ok(())
    }
}
