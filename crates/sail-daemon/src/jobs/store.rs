//! Private atomic job records and bounded output slices.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sail_api::jobs::{Chunk, Currentness, Lifecycle, ResultDocument, Status, Submission, VERSION};
use serde::{Deserialize, Serialize};

use super::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Stored {
    pub status: Status,
    pub submission: Submission,
    #[serde(skip)]
    pub cancel: Arc<AtomicBool>,
}
impl Stored {
    pub fn new(submission: Submission) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let time = now();
        Self {
            status: Status {
                version: VERSION,
                id: format!(
                    "{time}-{}-{}",
                    std::process::id(),
                    SERIAL.fetch_add(1, Ordering::Relaxed)
                ),
                project_dir: submission.project_dir.clone(),
                args: submission.args.clone(),
                state: Lifecycle::Queued,
                submitted_at_ms: time,
                started_at_ms: None,
                finished_at_ms: None,
                exit_code: None,
                input_revision: submission.input_revision.clone(),
                currentness: Currentness::Unknown,
                error: None,
            },
            submission,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .unwrap_or(0)
}
pub(super) fn private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}
pub(super) fn output(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}
pub(super) fn save(root: &Path, job: &Stored) -> io::Result<()> {
    let directory = root.join(&job.status.id);
    private_dir(&directory)?;
    let temporary = directory.join("record.tmp");
    let mut file = output(&temporary)?;
    file.write_all(&serde_json::to_vec(job).map_err(io::Error::other)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, directory.join("record.json"))
}
pub(super) fn load(root: &Path) -> io::Result<BTreeMap<String, Stored>> {
    let mut jobs = BTreeMap::new();
    for entry in std::fs::read_dir(root)? {
        let directory = entry?.path();
        let Ok(bytes) = std::fs::read(directory.join("record.json")) else {
            continue;
        };
        let Ok(mut job) = serde_json::from_slice::<Stored>(&bytes) else {
            continue;
        };
        // Never let a corrupt record redirect reads or cleanup outside its directory.
        if job.status.version != VERSION
            || directory.file_name().and_then(|name| name.to_str()) != Some(&job.status.id)
        {
            continue;
        }
        if !job.status.state.terminal() {
            job.status.state = Lifecycle::InfrastructureError;
            job.status.finished_at_ms = Some(now());
            job.status.error =
                Some("SAIL0807: daemon restarted before verification completed".to_owned());
            save(root, &job)?;
        }
        jobs.insert(job.status.id.clone(), job);
    }
    Ok(jobs)
}
pub(super) fn result(state: &State, job: &Stored) -> io::Result<ResultDocument> {
    let mut status = job.status.clone();
    let mut repair = None;
    let result = if status.state == Lifecycle::Completed {
        let mut project = project(state, job)?;
        let snapshot = refresh(job, &project);
        status.currentness = match snapshot {
            Some(snapshot) if snapshot.revision != project.input_revision => {
                Currentness::Historical
            }
            Some(snapshot)
                if snapshot.complete
                    && project.currentness == sail_api::project::Currentness::Current =>
            {
                Currentness::Current
            }
            _ if project.currentness == sail_api::project::Currentness::Historical => {
                Currentness::Historical
            }
            _ => Currentness::Unknown,
        };
        project.currentness = match status.currentness {
            Currentness::Current => sail_api::project::Currentness::Current,
            Currentness::Historical => sail_api::project::Currentness::Historical,
            Currentness::Unknown => sail_api::project::Currentness::Unknown,
        };
        if status.currentness == Currentness::Unknown {
            project.notes.push("SAIL3114: current input validation is unavailable or still running; native coverage and kernel read leases must be established before claiming Current".to_owned());
        }
        if let sail_api::jobs::Mode::Repair(_) = job.submission.mode {
            let mut envelope = repair_project(state, job)?;
            envelope.batch.full = project.clone();
            envelope.accepted = envelope.batch.accepted();
            repair = Some(Box::new(envelope));
        }
        Some(serde_json::to_value(project).map_err(io::Error::other)?)
    } else {
        None
    };
    Ok(ResultDocument {
        repair,
        job: status,
        result,
    })
}

fn project(state: &State, job: &Stored) -> io::Result<sail_api::project::ProjectResult> {
    let bytes = project_bytes(state, job)?;
    if let sail_api::jobs::Mode::Repair(_) = job.submission.mode {
        let envelope: sail_api::repair::ResultDocument =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        Ok(envelope.batch.full)
    } else {
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }
}
fn repair_project(state: &State, job: &Stored) -> io::Result<sail_api::repair::ResultDocument> {
    serde_json::from_slice(&project_bytes(state, job)?).map_err(io::Error::other)
}
fn project_bytes(state: &State, job: &Stored) -> io::Result<Vec<u8>> {
    let path = state.root.join(&job.status.id).join("stdout");
    if std::fs::metadata(&path)?.len() > sail_api::jobs::MAX_FRAME as u64 {
        return Err(io::Error::other(
            "SAIL0814: result exceeds RPC limit; retained output is available in the private job store",
        ));
    }
    std::fs::read(path)
}

fn refresh(
    state: &State,
    job: &Stored,
    project: &sail_api::project::ProjectResult,
) -> Option<sail_verus::managed::InputSnapshot> {
    let environment = &job.submission.environment;
    let cache = cache(job)?;
    state.freshness.inspect(
        &cache,
        Path::new(&project.workspace),
        &project.context_sha256,
        environment,
    )
}
fn cache(job: &Stored) -> Option<PathBuf> {
    let environment = &job.submission.environment;
    let cache = environment
        .get("SAIL_CACHE_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            environment
                .get("XDG_CACHE_HOME")
                .map(|home| std::path::PathBuf::from(home).join("sail"))
        })
        .or_else(|| {
            environment
                .get("HOME")
                .map(|home| std::path::PathBuf::from(home).join(".cache/sail"))
        })?;
    let cache = if cache.is_absolute() {
        cache
    } else {
        job.submission.project_dir.join(cache)
    };
    Some(cache)
}
pub(super) fn establish_freshness(state: &State, job: &Stored) {
    if let (Some(cache), Ok(project)) = (cache(job), project(state, job)) {
        let _ = state.freshness.establish(
            &cache,
            Path::new(&project.workspace),
            &project.context_sha256,
            &job.submission.environment,
        );
    }
}

pub(super) fn known_revision(state: &State, job: &Stored) -> Option<String> {
    let project = project(state, job).ok()?;
    let snapshot = state
        .freshness
        .establish(
            &cache(job)?,
            Path::new(&project.workspace),
            &project.context_sha256,
            &job.submission.environment,
        )
        .ok()?;
    snapshot.complete.then_some(snapshot.revision)
}

pub(super) fn running_revision(state: &State, job: &Stored) -> Option<(String, String)> {
    let mut bytes = Vec::new();
    File::open(state.root.join(&job.status.id).join("events.jsonl"))
        .ok()?
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    for line in bytes.split(|byte| *byte == b'\n') {
        let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if event["kind"] != "context-ready"
            || event["complete"] != true
            || event["source_observation"] != true
        {
            continue;
        }
        let original = event["input_revision"].as_str()?.to_owned();
        let snapshot = state
            .freshness
            .establish(
                &cache(job)?,
                Path::new(event["workspace"].as_str()?),
                event["context_sha256"].as_str()?,
                &job.submission.environment,
            )
            .ok()?;
        return snapshot.complete.then_some((original, snapshot.revision));
    }
    None
}

pub(super) fn chunk(state: &State, job: &Stored, name: &str, offset: u64) -> io::Result<Chunk> {
    let path = state.root.join(&job.status.id).join(name);
    let mut bytes = Vec::new();
    match File::open(path) {
        Ok(mut file) => {
            file.seek(SeekFrom::Start(offset))?;
            file.take(64 * 1024).read_to_end(&mut bytes)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // Keep complete UTF-8 characters together across RPC slices and append races.
    // Invalid subprocess bytes remain lossy diagnostics, but a valid JSONL event
    // must not acquire replacement characters merely because it crossed 64 KiB.
    if (bytes.len() == 64 * 1024 || !job.status.state.terminal())
        && let Err(error) = std::str::from_utf8(&bytes)
        && error.error_len().is_none()
    {
        bytes.truncate(error.valid_up_to());
    }
    Ok(Chunk {
        next: offset + bytes.len() as u64,
        text: String::from_utf8_lossy(&bytes).into_owned(),
        complete: job.status.state.terminal(),
    })
}
pub(super) fn prune(state: &State, project: &Path) {
    let Ok(mut jobs) = state.jobs.lock() else {
        return;
    };
    let mut retained: Vec<_> = jobs
        .values()
        .filter(|job| job.status.state.terminal() && job.status.project_dir == project)
        .map(|job| (job.status.submitted_at_ms, job.status.id.clone()))
        .collect();
    retained.sort();
    let remove = retained.len().saturating_sub(20);
    for (_, id) in retained.into_iter().take(remove) {
        if std::fs::remove_dir_all(state.root.join(&id)).is_ok() {
            jobs.remove(&id);
        }
    }
}
