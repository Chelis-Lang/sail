//! Neutral source submissions reuse durable jobs without caller execution controls.
mod baseline;
mod execution;

use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::sync::atomic::Ordering;

use sail_api::contracts::{Submission, VERSION, WorkerResult};
use sail_api::jobs::{Currentness, Lifecycle, ResultDocument, Status};

use super::{State, lock_error, schedule, store};
pub(super) use baseline::{export_baseline, original_baseline, save_baseline, validate_baseline};
pub(super) use execution::execute;

fn error(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("SAIL3142: {message}"))
}
fn sha(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
}

pub(super) fn submit(state: &State, source: &Submission) -> io::Result<Status> {
    submit_inner(state, source, true)
}
pub(super) fn submit_mutant(state: &State, source: &Submission) -> io::Result<Status> {
    submit_inner(state, source, false)
}
fn submit_inner(state: &State, source: &Submission, deduplicate: bool) -> io::Result<Status> {
    source.validate().map_err(error)?;
    if state.stop.load(Ordering::SeqCst) {
        return Err(error("job service is stopping"));
    }
    let directory = state
        .root
        .join("contract-inputs")
        .join(format!("{}-{}", source.source_sha256, source.crate_name));
    store::private_dir(&directory)?;
    let submission = sail_api::jobs::Submission {
        mode: sail_api::jobs::Mode::Stock,
        project_dir: directory,
        args: Vec::new(),
        force: true,
        environment: environment()?,
        input_revision: Some(source.source_sha256.clone()),
    };
    let key = schedule::workspace_key(&submission.project_dir, &[]);
    let mut jobs = state.jobs.lock().map_err(lock_error)?;
    if let Some(job) = jobs
        .values()
        .find(|j| deduplicate && !j.status.state.terminal() && j.contracts.as_ref() == Some(source))
    {
        return Ok(job.status.clone());
    }
    if jobs.values().filter(|j| !j.status.state.terminal()).count() >= 256 {
        return Err(error("job queue is full"));
    }
    let mut job = store::Stored::new(submission);
    job.contracts = Some(source.clone());
    let directory = state.root.join(&job.status.id);
    store::private_dir(&directory)?;
    let mut input = store::output(&directory.join("contract-source.json"))?;
    input.write_all(&serde_json::to_vec(&source).map_err(error)?)?;
    input.sync_all()?;
    store::save(&state.root, &job)?;
    let status = job.status.clone();
    jobs.insert(status.id.clone(), job);
    state.activity.fetch_add(1, Ordering::SeqCst);
    state
        .queue
        .lock()
        .map_err(lock_error)?
        .push(status.id.clone(), key);
    state.ready.notify_one();
    Ok(status)
}

// Discovery settings belong to the daemon operator, never the submitting client.
fn environment() -> io::Result<BTreeMap<String, String>> {
    const ALLOWED: &[&str] = &[
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "XDG_CACHE_HOME",
        "SAIL_CACHE_DIR",
        "SAIL_DAEMON_SOCKET",
        "SAIL_VERUS_BUILD",
        "SAIL_ENGINE_BUILD",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
    ];
    std::env::vars_os()
        .filter(|(k, _)| k.to_str().is_some_and(|k| ALLOWED.contains(&k)))
        .map(|(k, v)| {
            Ok((
                k.into_string()
                    .map_err(|_| error("non-UTF-8 operator environment"))?,
                v.into_string()
                    .map_err(|_| error("non-UTF-8 operator environment"))?,
            ))
        })
        .collect()
}

pub(super) fn source(state: &State, job: &store::Stored) -> io::Result<Submission> {
    let expected = job
        .contracts
        .as_ref()
        .ok_or_else(|| error("job has no neutral input"))?;
    expected.validate().map_err(error)?;
    let path = state.root.join(&job.status.id).join("contract-source.json");
    if !std::fs::symlink_metadata(&path)?.file_type().is_file() {
        return Err(error("neutral input is not a regular file"));
    }
    let actual: Submission = serde_json::from_slice(&store::read_bytes(&path)?).map_err(error)?;
    if actual != *expected {
        return Err(error("retained neutral input differs from its submission"));
    }
    Ok(actual)
}

fn decode(bytes: &[u8], input: &Submission) -> io::Result<WorkerResult> {
    input.validate().map_err(error)?;
    let result: WorkerResult = serde_json::from_slice(bytes).map_err(error)?;
    sail_verus::contracts::validate_manifest(&input.source, &result.contracts.manifest)
        .map_err(error)?;
    if result.version != VERSION
        || result.contracts.version != VERSION
        || result.contracts.manifest.original_sha256 != input.source_sha256
        || result.contracts.manifest.crate_name != input.crate_name
        || result.backend.kind != "sail-project-result"
        || result.backend.schema_version != sail_api::project::SCHEMA_VERSION
        || result.backend.error.is_some()
        || result.backend.no_reuse.is_some()
        || result.contracts.proof
            != sail_verus::contracts::observed_proof(&result.contracts.manifest, &result.backend)
    {
        return Err(error(
            "neutral worker evidence does not match its exact source or backend",
        ));
    }
    Ok(result)
}

fn stderr(state: &State, job: &store::Stored) -> io::Result<Vec<u8>> {
    let path = state.root.join(&job.status.id).join("stderr");
    if !std::fs::symlink_metadata(&path)?.file_type().is_file() {
        return Err(error("neutral diagnostics are not a regular file"));
    }
    let bytes = store::read_bytes(&path)?;
    if bytes.len() > sail_api::contracts::MAX_STDERR_BYTES
        || !job
            .projection
            .as_ref()
            .is_some_and(|p| p.matches_stderr(&bytes))
    {
        return Err(error(
            "retained neutral diagnostics differ from their exact output seal",
        ));
    }
    Ok(bytes)
}

fn read(state: &State, job: &store::Stored) -> io::Result<(Submission, Vec<u8>, WorkerResult)> {
    if job.projection.is_none() {
        return Err(error("neutral result has no validated output seal"));
    }
    if !std::fs::symlink_metadata(state.root.join(&job.status.id).join("stdout"))?
        .file_type()
        .is_file()
    {
        return Err(error("neutral worker output is not a regular file"));
    }
    let input = source(state, job)?;
    stderr(state, job)?;
    let bytes = store::project_bytes(state, job)?;
    let result = decode(&bytes, &input)?;
    Ok((input, bytes, result))
}

pub(super) fn result(state: &State, job: &store::Stored) -> io::Result<ResultDocument> {
    let mut status = job.status.clone();
    status.currentness = Currentness::Unknown;
    if status.state != Lifecycle::Completed {
        return Ok(ResultDocument {
            job: status,
            result: None,
            repair: None,
            contracts: None,
        });
    }
    let (_, _, result) = read(state, job)?;
    Ok(ResultDocument {
        job: status,
        result: Some(serde_json::to_value(result.backend).map_err(error)?),
        repair: None,
        contracts: Some(Box::new(result.contracts)),
    })
}
