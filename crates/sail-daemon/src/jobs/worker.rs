//! Single FIFO worker, without holding the status lock during verification.

use std::io;
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::Duration;

use sail_api::jobs::{Currentness, Lifecycle};
use sail_api::project::ProjectResult;

use super::{State, get, lock_error, store};

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
    let mut command = Command::new(&state.executable);
    command
        .arg("verify")
        .arg("--incremental")
        .args(["--output", "json"])
        .args(&job.submission.args)
        .current_dir(&job.status.project_dir)
        .env_clear()
        .envs(&job.submission.environment)
        .env("SAIL_JOB_EVENTS", directory.join("events.jsonl"))
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0);
    if job.submission.force {
        command.env("SAIL_MANAGED_FORCE", "1");
    }
    let mut child = command.spawn()?;
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
    let bytes = std::fs::read(directory.join("stdout"))?;
    let document: ProjectResult = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if document.kind != "sail-project-result"
        || document.schema_version != sail_api::project::SCHEMA_VERSION
    {
        return Err(io::Error::other("unsupported or incomplete project result"));
    }
    Ok((status.code(), document))
}
