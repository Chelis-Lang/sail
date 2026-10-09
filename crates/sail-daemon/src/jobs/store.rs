//! Private atomic job records and bounded output slices.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
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
    let result = if job.status.state == Lifecycle::Completed {
        let bytes = std::fs::read(state.root.join(&job.status.id).join("stdout"))?;
        Some(serde_json::from_slice(&bytes).map_err(io::Error::other)?)
    } else {
        None
    };
    Ok(ResultDocument {
        job: job.status.clone(),
        result,
    })
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
