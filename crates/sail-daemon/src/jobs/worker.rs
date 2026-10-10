//! Single FIFO worker, without holding the status lock during verification.

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
    loop {
        let id = next(state);
        let Some(id) = id else { break };
        let Ok(job) = get(state, &id) else { continue };
        if !job.status.state.terminal() {
            let result = execute(state, &id);
            if let Ok(mut jobs) = state.jobs.lock()
                && let Some(job) = jobs.get_mut(&id)
                && !job.status.state.terminal()
            {
                job.status.finished_at_ms = Some(store::now());
                match result {
                    Ok((code, document)) => {
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
                    job.status.error = Some(format!("SAIL0809: cannot persist result: {error}"));
                }
            }
        }
        state.activity.fetch_sub(1, Ordering::SeqCst);
        store::prune(state, &job.status.project_dir);
    }
}
fn next(state: &State) -> Option<String> {
    let mut queue = state.queue.lock().ok()?;
    loop {
        if state.stop.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(id) = queue.pop_front() {
            return Some(id);
        }
        queue = state.ready.wait(queue).ok()?;
    }
}
fn execute(state: &State, id: &str) -> io::Result<(Option<i32>, ProjectResult)> {
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
            command
                .arg("verify")
                .arg("--incremental")
                .args(["--output", "json"]);
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
    let document = read_document(&directory, &job.submission.mode)?;
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
    Ok((status.code(), document))
}

fn read_document(directory: &std::path::Path, mode: &Mode) -> io::Result<ProjectResult> {
    let bytes = std::fs::read(directory.join("stdout"))?;
    let document: ProjectResult = if matches!(mode, Mode::Repair(_)) {
        let repair: sail_api::repair::ResultDocument =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if repair.version != sail_api::repair::VERSION {
            return Err(io::Error::other("unsupported repair result version"));
        }
        repair.batch.full
    } else {
        serde_json::from_slice(&bytes).map_err(io::Error::other)?
    };
    Ok(document)
}
