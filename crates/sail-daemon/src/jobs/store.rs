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
use super::rpc_diagnostics::{Stage, Trace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Stored {
    pub status: Status,
    pub submission: Submission,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<Projection>,
    #[serde(skip)]
    pub cancel: Arc<AtomicBool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Projection {
    stdout_sha256: String,
    workspace: String,
    context_sha256: String,
    input_revision: String,
    currentness: sail_api::project::Currentness,
}
impl Projection {
    pub(super) fn validated(bytes: &[u8], project: &sail_api::project::ProjectResult) -> Self {
        Self {
            stdout_sha256: stdout_digest(bytes),
            workspace: project.workspace.clone(),
            context_sha256: project.context_sha256.clone(),
            input_revision: project.input_revision.clone(),
            currentness: project.currentness,
        }
    }
}
fn stdout_digest(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
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
            projection: None,
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
        let snapshot = refresh(state, job, &project);
        status.currentness = classify(project.currentness, &project.input_revision, snapshot);
        apply_currentness(&mut project, status.currentness);
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

/// Immutable frames avoid rebuilding large function graphs. The persisted
/// stdout contents and live freshness are still checked on every RPC.
pub(super) fn result_frame(
    state: &State,
    job: &Stored,
    trace: &mut Trace<'_>,
) -> io::Result<super::result_cache::WireFrame> {
    use super::result_cache::Frames;
    if job.status.state != Lifecycle::Completed || job.projection.is_none() {
        return encode_result(result(state, job)?).map(Into::into);
    }
    let projection = job
        .projection
        .as_ref()
        .ok_or_else(|| io::Error::other("SAIL0816: missing result seal"))?;
    let key = trace.measure(Stage::FrameKey, || frame_key(job, projection))?;
    if let Some(frames) = trace.measure(Stage::FrameLookup, || state.result_cache.get(&key))
        && let Some(currentness) = state.outputs.validate(&frames, trace, |trace| {
            trace.measure(Stage::Freshness, || {
                projected_currentness(state, job, projection)
            })
        })
    {
        trace.output_guarded(true);
        trace.currentness(currentness);
        trace.frame_hit(true);
        return Ok(frames.select(currentness));
    }
    // Failed final output coverage discards its prospective input result and
    // repeats the original complete read/SHA and independent freshness below.
    trace.output_guarded(false);
    let bytes = match trace.measure(Stage::OutputSeal, || project_bytes(state, job)) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.result_cache.remove(&key);
            return Err(error);
        }
    };

    let currentness = trace.measure(Stage::Freshness, || {
        projected_currentness(state, job, projection)
    });
    trace.currentness(currentness);
    if let Some(frames) = trace.measure(Stage::FrameLookup, || state.result_cache.get(&key)) {
        trace.frame_hit(true);
        return Ok(frames.select(currentness));
    }
    trace.frame_hit(false);
    let encode_started = trace.begin();
    // This parse uses the same bytes whose exact seal was checked above.
    let (project, repair) = if let sail_api::jobs::Mode::Repair(_) = job.submission.mode {
        let envelope: sail_api::repair::ResultDocument =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        (envelope.batch.full.clone(), Some(Box::new(envelope)))
    } else {
        (
            serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            None,
        )
    };
    let frame = |currentness| {
        let mut project = project.clone();
        let mut status = job.status.clone();
        status.currentness = currentness;
        apply_currentness(&mut project, currentness);
        let mut repair = repair.clone();
        if let Some(envelope) = &mut repair {
            envelope.batch.full = project.clone();
            envelope.accepted = envelope.batch.accepted();
        }
        encode_result(ResultDocument {
            job: status,
            result: Some(serde_json::to_value(project).map_err(io::Error::other)?),
            repair,
        })
    };
    let frames = state.result_cache.insert(
        key,
        Frames::new(
            frame(Currentness::Current)?,
            frame(Currentness::Historical)?,
            frame(Currentness::Unknown)?,
        ),
    );
    state.outputs.offer(
        &frames,
        &state.root.join(&job.status.id).join("stdout"),
        &projection.stdout_sha256,
    );
    let selected = frames.select(currentness);
    trace.end(Stage::Encode, encode_started);
    Ok(selected)
}
/// Identity of a completed job's immutable frame family. It binds exactly what
/// the encoded frames are built from: the public status, the result mode and
/// the sealed stdout digest. Unlike the whole stored record, it excludes the
/// submission environment, which the frames never contain.
fn frame_key(job: &Stored, projection: &Projection) -> io::Result<String> {
    #[derive(Serialize)]
    struct Key<'a> {
        status: &'a Status,
        repair: bool,
        stdout_sha256: &'a str,
    }
    serde_json::to_vec(&Key {
        status: &job.status,
        repair: matches!(job.submission.mode, sail_api::jobs::Mode::Repair(_)),
        stdout_sha256: &projection.stdout_sha256,
    })
    .map(|bytes| stdout_digest(&bytes))
    .map_err(io::Error::other)
}
fn encode_result(document: ResultDocument) -> io::Result<Arc<Vec<u8>>> {
    let mut bytes = Vec::new();
    sail_api::jobs::write_frame(&mut bytes, &sail_api::jobs::Response::Result(document))?;
    Ok(Arc::new(bytes))
}
fn apply_currentness(project: &mut sail_api::project::ProjectResult, currentness: Currentness) {
    let originally_current = project.currentness == sail_api::project::Currentness::Current;
    project.currentness = match currentness {
        Currentness::Current => sail_api::project::Currentness::Current,
        Currentness::Historical => sail_api::project::Currentness::Historical,
        Currentness::Unknown => sail_api::project::Currentness::Unknown,
    };
    if currentness == Currentness::Unknown && originally_current {
        project.notes.push("SAIL3114: current input validation is unavailable or still running; native coverage and kernel read leases must be established before claiming Current".to_owned());
    }
}
fn projected_currentness(state: &State, job: &Stored, projection: &Projection) -> Currentness {
    let snapshot = cache(job).and_then(|cache| {
        state.freshness.inspect(
            &cache,
            Path::new(&projection.workspace),
            &projection.context_sha256,
            &job.submission.environment,
        )
    });
    let currentness = classify(projection.currentness, &projection.input_revision, snapshot);
    #[cfg(test)]
    output_fallback_tests::after_freshness();
    currentness
}

pub(super) fn status(state: &State, job: &Stored) -> io::Result<Status> {
    status_traced(state, job, &mut Trace::disabled())
}
pub(super) fn status_traced(
    state: &State,
    job: &Stored,
    trace: &mut Trace<'_>,
) -> io::Result<Status> {
    if job.status.state != Lifecycle::Completed {
        return Ok(job.status.clone());
    }
    let Some(projection) = &job.projection else {
        // Older records have no validated projection: retain the complete parse.
        return result(state, job).map(|document| document.job);
    };
    // A continuously guarded output owner, established for this exact frame
    // family by an earlier Result, certifies the sealed stdout bytes without
    // rereading them. Its barriers bracket the independent freshness check
    // exactly as on the Result path. Otherwise, repeat the complete read/SHA.
    if let Ok(key) = frame_key(job, projection)
        && let Some(frames) = state.result_cache.get(&key)
        && let Some(currentness) = state.outputs.validate(&frames, trace, |trace| {
            trace.measure(Stage::Freshness, || {
                projected_currentness(state, job, projection)
            })
        })
    {
        trace.output_guarded(true);
        trace.currentness(currentness);
        let mut status = job.status.clone();
        status.currentness = currentness;
        return Ok(status);
    }
    trace.output_guarded(false);
    let _validated = trace.measure(Stage::OutputSeal, || project_bytes(state, job))?;
    let mut status = job.status.clone();
    status.currentness = trace.measure(Stage::Freshness, || {
        projected_currentness(state, job, projection)
    });
    trace.currentness(status.currentness);
    Ok(status)
}
fn classify(
    original: sail_api::project::Currentness,
    revision: &str,
    snapshot: Option<sail_verus::managed::InputSnapshot>,
) -> Currentness {
    match snapshot {
        Some(snapshot) if snapshot.revision != revision => Currentness::Historical,
        Some(snapshot)
            if snapshot.complete && original == sail_api::project::Currentness::Current =>
        {
            Currentness::Current
        }
        _ if original == sail_api::project::Currentness::Historical => Currentness::Historical,
        _ => Currentness::Unknown,
    }
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
    #[cfg(test)]
    output_fallback_tests::before_read();
    let bytes = read_bytes(&state.root.join(&job.status.id).join("stdout"))?;
    if job
        .projection
        .as_ref()
        .is_some_and(|projection| stdout_digest(&bytes) != projection.stdout_sha256)
    {
        return Err(io::Error::other(
            "SAIL0816: retained result differs from its validated output seal",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod output_fallback_tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Probe {
        reads: usize,
        freshness: usize,
        restore: Option<(PathBuf, Vec<u8>)>,
        writes_ok: bool,
    }
    std::thread_local! {
        static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
    }
    pub(super) fn before_read() {
        PROBE.with_borrow_mut(|probe| {
            if let Some(probe) = probe {
                probe.reads += 1;
            }
        });
    }
    pub(super) fn after_freshness() {
        PROBE.with_borrow_mut(|probe| {
            if let Some(probe) = probe {
                probe.freshness += 1;
                if let Some((path, bytes)) = probe.restore.take() {
                    probe.writes_ok = std::fs::write(&path, b"changed between barriers").is_ok()
                        && std::fs::write(path, bytes).is_ok();
                }
            }
        });
    }
    #[cfg(target_os = "linux")]
    fn counts() -> Probe {
        PROBE.with_borrow_mut(|probe| probe.take().unwrap_or_default())
    }
    #[cfg(target_os = "linux")]
    fn measured(
        state: &State,
        job: &Stored,
    ) -> (io::Result<super::super::result_cache::WireFrame>, Probe) {
        PROBE.with_borrow_mut(|probe| *probe = Some(Probe::default()));
        let result = result_frame(state, job, &mut Trace::disabled());
        (result, counts())
    }
    #[cfg(target_os = "linux")]
    fn measured_status(state: &State, job: &Stored) -> (io::Result<Status>, Probe) {
        PROBE.with_borrow_mut(|probe| *probe = Some(Probe::default()));
        let result = status_traced(state, job, &mut Trace::disabled());
        (result, counts())
    }
    #[cfg(target_os = "linux")]
    fn completed_job(state: &State, root: &Path) -> (Stored, PathBuf, Vec<u8>) {
        let project =
            super::currentness_note_tests::project(sail_api::project::Currentness::Current);
        let bytes =
            serde_json::to_vec(&project).unwrap_or_else(|error| unreachable!("fixture: {error}"));
        let mut job = Stored::new(Submission {
            mode: sail_api::jobs::Mode::Stock,
            project_dir: root.to_path_buf(),
            args: Vec::new(),
            force: false,
            environment: BTreeMap::new(),
            input_revision: None,
        });
        job.status.state = Lifecycle::Completed;
        job.projection = Some(Projection::validated(&bytes, &project));
        let directory = state.root.join(&job.status.id);
        assert!(std::fs::create_dir_all(&directory).is_ok());
        let path = directory.join("stdout");
        assert!(std::fs::write(&path, &bytes).is_ok());
        (job, path, bytes)
    }

    /// Status reuses a Result's continuously guarded output owner instead of
    /// rereading stdout; contention, a write bracketed by its own freshness
    /// check and any later invalidation all repeat the complete read and hash.
    #[cfg(target_os = "linux")]
    #[test]
    fn status_reuses_guarded_output_and_falls_back_on_any_doubt() {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-store-status-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let server = super::super::bind(
            &root.join("socket"),
            &root.join("jobs"),
            Path::new("/bin/true"),
        )
        .unwrap_or_else(|error| unreachable!("fixture: {error}"));
        let state = &server.state;
        let (job, path, bytes) = completed_job(state, &root);
        let worker = super::super::output_service::Queue::spawn(&state.outputs)
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        // Without any frame family, Status keeps the original complete check.
        let (unguarded, unguarded_probe) = measured_status(state, &job);
        let first = result_frame(state, &job, &mut Trace::disabled());
        let idle = state.outputs.idle_for_test();
        let projection = job.projection.as_ref().unwrap_or_else(|| unreachable!());
        let key = frame_key(&job, projection).unwrap_or_else(|e| unreachable!("key: {e}"));
        let frames = state
            .result_cache
            .get(&key)
            .unwrap_or_else(|| unreachable!("cached family"));
        let (guarded, guarded_probe) = measured_status(state, &job);
        let lock = frames
            .output
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let (busy, busy_probe) = measured_status(state, &job);
        drop(lock);
        PROBE.with_borrow_mut(|probe| {
            *probe = Some(Probe {
                restore: Some((path.clone(), bytes.clone())),
                ..Probe::default()
            });
        });
        let restored = status_traced(state, &job, &mut Trace::disabled());
        let restored_probe = counts();
        let (after, after_probe) = measured_status(state, &job);
        assert!(std::fs::write(&path, b"wrong persisted SHA").is_ok());
        let (wrong, _) = measured_status(state, &job);
        state.outputs.stop();
        let joined = worker.join().is_ok();
        drop(frames);
        drop(server);
        assert!(std::fs::remove_dir_all(&root).is_ok());
        assert!(idle && joined && first.is_ok());
        let expected = unguarded.unwrap_or_else(|e| unreachable!("status: {e}"));
        assert_eq!((unguarded_probe.reads, unguarded_probe.freshness), (1, 1));
        for status in [guarded, busy, restored, after] {
            let status = status.unwrap_or_else(|e| unreachable!("status: {e}"));
            assert_eq!(status.currentness, expected.currentness);
            assert_eq!(status.id, expected.id);
        }
        assert_eq!((guarded_probe.reads, guarded_probe.freshness), (0, 1));
        assert_eq!((busy_probe.reads, busy_probe.freshness), (1, 1));
        // A write during the guarded freshness check fails the closing barrier;
        // the complete read and an independent freshness check follow.
        assert!(restored_probe.writes_ok);
        assert_eq!((restored_probe.reads, restored_probe.freshness), (1, 2));
        // The failed owner is sticky: every later Status rereads.
        assert_eq!((after_probe.reads, after_probe.freshness), (1, 1));
        assert!(wrong.is_err_and(|e| e.to_string().starts_with("SAIL0816:")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn store_final_barrier_failure_repeats_complete_hash_and_independent_freshness() {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-store-output-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let server = super::super::bind(
            &root.join("socket"),
            &root.join("jobs"),
            Path::new("/bin/true"),
        )
        .unwrap_or_else(|error| unreachable!("fixture: {error}"));
        let state = &server.state;
        let project =
            super::currentness_note_tests::project(sail_api::project::Currentness::Current);
        let bytes =
            serde_json::to_vec(&project).unwrap_or_else(|error| unreachable!("fixture: {error}"));
        let mut job = Stored::new(Submission {
            mode: sail_api::jobs::Mode::Stock,
            project_dir: root.clone(),
            args: Vec::new(),
            force: false,
            environment: BTreeMap::new(),
            input_revision: None,
        });
        job.status.state = Lifecycle::Completed;
        job.projection = Some(Projection::validated(&bytes, &project));
        let directory = state.root.join(&job.status.id);
        assert!(std::fs::create_dir_all(&directory).is_ok());
        let path = directory.join("stdout");
        assert!(std::fs::write(&path, &bytes).is_ok());
        let expected =
            encode_result(result(state, &job).unwrap_or_else(|e| unreachable!("fixture: {e}")))
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let worker = super::super::output_service::Queue::spawn(&state.outputs)
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let first = result_frame(state, &job, &mut Trace::disabled());
        let idle = state.outputs.idle_for_test();
        let key = frame_key(
            &job,
            job.projection.as_ref().unwrap_or_else(|| unreachable!()),
        )
        .unwrap_or_else(|e| unreachable!("key: {e}"));
        let frames = state
            .result_cache
            .get(&key)
            .unwrap_or_else(|| unreachable!("cached family"));
        let (healthy, healthy_probe) = measured(state, &job);
        let lock = frames
            .output
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let (busy, busy_probe) = measured(state, &job);
        drop(lock);
        let quota = state.result_cache.budget().reserve(100 * 1024 * 1024);
        let quota_admitted = quota.is_some();
        let (limited, limited_probe) = measured(state, &job);
        drop(quota);
        PROBE.with_borrow_mut(|probe| {
            *probe = Some(Probe {
                restore: Some((path.clone(), bytes.clone())),
                ..Probe::default()
            });
        });
        let restored = result_frame(state, &job, &mut Trace::disabled());
        let restored_probe = counts();
        assert!(std::fs::write(&path, b"wrong persisted SHA").is_ok());
        let wrong = result_frame(state, &job, &mut Trace::disabled());
        let oversized_file = File::create(&path).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(
            oversized_file
                .set_len(sail_api::jobs::MAX_FRAME as u64 + 1)
                .is_ok()
        );
        drop(oversized_file);
        let oversized = result_frame(state, &job, &mut Trace::disabled());
        state.outputs.stop();
        let joined = worker.join().is_ok();
        drop(frames);
        drop(server);
        assert!(std::fs::remove_dir_all(&root).is_ok());
        // Owned construction is joined before outcome assertions; the probe
        // observes the actual store read and unchanged freshness implementation.
        assert!(idle && joined);
        assert!(quota_admitted);
        for frame in [first, healthy, busy, limited, restored] {
            assert_eq!(
                &*frame.unwrap_or_else(|e| unreachable!("frame: {e}")),
                expected.as_slice()
            );
        }
        assert_eq!((healthy_probe.reads, healthy_probe.freshness), (0, 1));
        assert_eq!((busy_probe.reads, busy_probe.freshness), (1, 1));
        assert_eq!((limited_probe.reads, limited_probe.freshness), (1, 1));
        assert!(restored_probe.writes_ok);
        assert_eq!((restored_probe.reads, restored_probe.freshness), (1, 2));
        assert!(wrong.is_err_and(|e| e.to_string().starts_with("SAIL0816:")));
        assert!(oversized.is_err_and(|e| e.to_string().starts_with("SAIL0814:")));
    }
}
pub(super) fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(sail_api::jobs::MAX_FRAME as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > sail_api::jobs::MAX_FRAME {
        return Err(io::Error::other(
            "SAIL0814: result exceeds RPC limit; retained output is available in the private job store",
        ));
    }
    Ok(bytes)
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
    let snapshot = state.freshness.inspect(
        &cache(job)?,
        Path::new(&project.workspace),
        &project.context_sha256,
        &job.submission.environment,
    )?;
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
        let snapshot = state.freshness.inspect(
            &cache(job)?,
            Path::new(event["workspace"].as_str()?),
            event["context_sha256"].as_str()?,
            &job.submission.environment,
        )?;
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

#[cfg(test)]
mod currentness_note_tests {
    use super::*;
    use sail_api::project::{Execution, ProjectResult};

    pub(super) fn project(currentness: sail_api::project::Currentness) -> ProjectResult {
        ProjectResult {
            kind: "sail-project-result".to_owned(),
            schema_version: sail_api::project::SCHEMA_VERSION,
            run_id: "original-run".to_owned(),
            workspace: "workspace".to_owned(),
            context_sha256: "context".to_owned(),
            input_revision: "revision".to_owned(),
            currentness,
            exit_code: Some(0),
            targets: Vec::new(),
            execution: Execution::default(),
            notes: vec![
                "SAIL3114: native-observation-invalidated; detail=namespace-tracked-input-event"
                    .to_owned(),
                "Build-script input declarations were established during this run".to_owned(),
            ],
            error: None,
            engine: None,
        }
    }

    #[test]
    fn already_unknown_proof_preserves_original_reasons_without_inventing_projection_failure() {
        let mut report = project(sail_api::project::Currentness::Unknown);
        let original = report.clone();
        assert_eq!(
            classify(report.currentness, "revision", None),
            Currentness::Unknown
        );
        apply_currentness(&mut report, Currentness::Unknown);
        assert_eq!(report, original);
    }

    #[test]
    fn lost_current_projection_keeps_proof_and_adds_validation_warning() {
        let mut report = project(sail_api::project::Currentness::Current);
        let original = report.clone();
        assert_eq!(
            classify(report.currentness, "revision", None),
            Currentness::Unknown
        );
        apply_currentness(&mut report, Currentness::Unknown);
        assert_eq!(report.currentness, sail_api::project::Currentness::Unknown);
        assert_eq!(&report.notes[..original.notes.len()], original.notes);
        assert_eq!(report.notes.len(), original.notes.len() + 1);
        assert!(
            report
                .notes
                .last()
                .is_some_and(|note| note.starts_with("SAIL3114: current input validation"))
        );
        report.currentness = original.currentness;
        report.notes = original.notes.clone();
        assert_eq!(report, original);
    }

    #[test]
    fn current_and_historical_projection_do_not_add_uncertainty_notes() {
        for (original, projected) in [
            (
                sail_api::project::Currentness::Current,
                Currentness::Current,
            ),
            (
                sail_api::project::Currentness::Current,
                Currentness::Historical,
            ),
            (
                sail_api::project::Currentness::Historical,
                Currentness::Historical,
            ),
        ] {
            let mut report = project(original);
            let notes = report.notes.clone();
            apply_currentness(&mut report, projected);
            assert_eq!(report.notes, notes);
        }
    }
}
