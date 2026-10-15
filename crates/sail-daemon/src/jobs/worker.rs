//! Verification workers: each runs one job at a time, without holding the
//! status lock during verification.

use std::io;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::Duration;

use sail_api::jobs::{Currentness, Lifecycle, Mode};
use sail_api::project::ProjectResult;

use super::{State, get, lock_error, store};

struct Process(Child);
impl std::ops::Deref for Process {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for Process {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{}", self.0.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub(super) fn run(state: &State) {
    while let Some((id, key)) = next(state) {
        let _running = Running { state, key };
        let Ok(job) = get(state, &id) else { continue };
        if !job.status.state.terminal() {
            let result = execute(state, &id);
            if result.is_ok() {
                store::establish_freshness(state, &job);
            }
            if let Ok(mut jobs) = state.jobs.lock()
                && let Some(job) = jobs.get_mut(&id)
                && !job.status.state.terminal()
            {
                job.status.finished_at_ms = Some(store::now());
                match result {
                    Ok((code, document, projection)) => {
                        job.projection = Some(projection);
                        job.status.exit_code = code;
                        job.status.state = Lifecycle::Completed;
                        job.status.input_revision = Some(document.input_revision);
                        job.status.currentness = match document.currentness {
                            sail_api::project::Currentness::Current => Currentness::Current,
                            sail_api::project::Currentness::Historical => Currentness::Historical,
                            sail_api::project::Currentness::Unknown => Currentness::Unknown,
                        };
                    }
                    Err(error) => {
                        job.status.state = Lifecycle::InfrastructureError;
                        job.status.error = Some(format!("SAIL0808: {error}"));
                    }
                }
                if let Err(error) = store::save(&state.root, job) {
                    job.status.state = Lifecycle::InfrastructureError;
                    job.status.error = Some(format!("SAIL0808: cannot persist result: {error}"));
                }
            }
        }
        state.activity.fetch_sub(1, Ordering::SeqCst);
        store::prune(state, &job.status.project_dir);
    }
}
/// Releases a job's workspace when its worker finishes with it, and wakes
/// the workers so a job queued behind it can start.
struct Running<'a> {
    state: &'a State,
    key: std::path::PathBuf,
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.state.queue.lock() {
            queue.finish(&self.key);
        }
        self.state.ready.notify_all();
    }
}
fn next(state: &State) -> Option<(String, std::path::PathBuf)> {
    let mut queue = state.queue.lock().ok()?;
    loop {
        if state.stop.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(job) = queue.take() {
            return Some(job);
        }
        queue = state.ready.wait(queue).ok()?;
    }
}
fn execute(state: &State, id: &str) -> io::Result<(Option<i32>, ProjectResult, store::Projection)> {
    let job = get(state, id)?;
    if job.cancel.load(Ordering::SeqCst) {
        return Err(io::Error::other("cancelled"));
    }
    let directory = state.root.join(id);
    let stdout = store::output(&directory.join("stdout"))?;
    let stderr = store::output(&directory.join("stderr"))?;
    store::output(&directory.join("events.jsonl"))?;
    store::output(&directory.join("proof-source.jsonl"))?;
    let mut command = Command::new(&state.executable);
    match &job.submission.mode {
        Mode::Repair(selection) => {
            command
                .arg("_repair-worker")
                .arg("--selection-json")
                .arg(serde_json::to_string(selection).map_err(io::Error::other)?);
        }
        Mode::Stock | Mode::Interactive => {
            command.arg("verify").args(["--output", "json"]);
            if job.submission.mode == Mode::Interactive {
                command.args(["--engine", "interactive"]);
            }
        }
    }
    command
        .args(&job.submission.args)
        .current_dir(&job.status.project_dir)
        .env_clear()
        .envs(&job.submission.environment)
        .env("SAIL_JOB_EVENTS", directory.join("events.jsonl"))
        .env("SAIL_PROOF_EVENTS", directory.join("proof-source.jsonl"))
        .env("SAIL_JOB_GUARD_PARENT", "1")
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0);
    if job.submission.force {
        command.env("SAIL_MANAGED_FORCE", "1");
    }
    let mut child = Process(command.spawn()?);
    let _parent_pipe = child.stdin.take();
    {
        let mut jobs = state.jobs.lock().map_err(lock_error)?;
        if let Some(job) = jobs.get_mut(id)
            && !job.status.state.terminal()
        {
            job.status.state = Lifecycle::Running;
            job.status.started_at_ms = Some(store::now());
            store::save(&state.root, job)?;
        }
    }
    let status = loop {
        if state.stop.load(Ordering::SeqCst) || job.cancel.load(Ordering::SeqCst) {
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{}", child.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "verification cancelled",
            ));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if std::fs::metadata(directory.join("stdout")).is_ok_and(|meta| meta.len() == 0) {
        return Err(io::Error::other(no_result(&directory, status)));
    }
    let (document, projection) = read_document(&directory, &job.submission.mode)?;
    if document.exit_code != status.code() {
        return Err(io::Error::other(
            "managed report and process exit status disagree",
        ));
    }
    if document.error.is_some() {
        return Err(io::Error::other(
            "managed verifier reported an infrastructure failure",
        ));
    }
    if document.kind != "sail-project-result"
        || document.schema_version != sail_api::project::SCHEMA_VERSION
    {
        return Err(io::Error::other("unsupported or incomplete project result"));
    }
    Ok((status.code(), document, projection))
}

/// Describes a worker that exited without writing a result, quoting its last
/// `error[...]` line (for example a missing engine's `SAIL3120`) so the job
/// error says what went wrong instead of reporting an unparsable result.
fn no_result(directory: &std::path::Path, status: std::process::ExitStatus) -> String {
    let stderr = store::read_bytes(&directory.join("stderr")).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr);
    let cause = stderr
        .lines()
        .rev()
        .find(|line| line.starts_with("error["))
        .map(str::trim)
        .map(|line| line.chars().take(2048).collect::<String>());
    match cause {
        Some(line) => format!("worker exited ({status}) without a result: {line}"),
        None => format!("worker exited ({status}) without a result; see the job's retained stderr"),
    }
}

fn read_document(
    directory: &std::path::Path,
    mode: &Mode,
) -> io::Result<(ProjectResult, store::Projection)> {
    let bytes = store::read_bytes(&directory.join("stdout"))?;
    let document: ProjectResult = if matches!(mode, Mode::Repair(_)) {
        let mut repair: sail_api::repair::ResultDocument =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if repair.version != sail_api::repair::VERSION {
            return Err(io::Error::other("unsupported repair result version"));
        }
        store::full_phase(&mut repair)?
    } else {
        serde_json::from_slice(&bytes).map_err(io::Error::other)?
    };
    let projection = store::Projection::validated(&bytes, &document);
    Ok((document, projection))
}
