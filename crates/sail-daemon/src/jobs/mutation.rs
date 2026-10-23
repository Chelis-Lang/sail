//! Owned bounded mutation controllers over the existing neutral source jobs.
use super::{State, contracts, get, lock_error, product, store};
use sail_api::mutation::{self as api, Outcome, State as Lifecycle, View, digest};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write as _},
    os::unix::fs::OpenOptionsExt as _,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(super) type Controllers = std::collections::BTreeMap<String, (Arc<AtomicBool>, JoinHandle<()>)>;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    submission: api::Submission,
    plan: api::Plan,
    jobs: Vec<String>,
    started_at_ms: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct End {
    record_sha256: String,
    state: Lifecycle,
}
fn error(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("SAIL3144: {message}"))
}
fn encode<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(error)
}
fn name(id: &str, index: Option<usize>) -> String {
    index.map_or_else(|| format!("m-{id}-b"), |i| format!("m-{id}-{i}"))
}
fn path(state: &State, id: &str) -> io::Result<std::path::PathBuf> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(error("invalid campaign identity"));
    }
    Ok(state.root.join("mutations").join(id))
}
fn install(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn regular(path: &Path) -> io::Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(error("campaign artifact is not a regular file"));
    }
    store::read_bytes(path)
}
fn record(state: &State, id: &str) -> io::Result<Record> {
    let value: Record = serde_json::from_slice(&regular(&path(state, id)?.join("campaign.json"))?)
        .map_err(error)?;
    if value.version != api::VERSION
        || digest(&encode(&value)?) != id
        || value.jobs.len() != value.plan.candidates.len()
    {
        return Err(error("campaign content or member identity changed"));
    }
    Ok(value)
}

struct Queued<'a> {
    state: &'a State,
    ids: Vec<String>,
    published: bool,
}
impl Drop for Queued<'_> {
    fn drop(&mut self) {
        if !self.published {
            for id in &self.ids {
                let _ = super::cancel(self.state, id, false);
            }
        }
    }
}

pub(super) fn submit(state: &Arc<State>, submission: &api::Submission) -> io::Result<View> {
    let started = Instant::now();
    let started_at_ms = store::now();
    if !(1..=api::MAX_MUTANTS).contains(&submission.max_mutants)
        || submission.wall_time_ms == 0
        || submission.wall_time_ms > api::MAX_WALL_TIME_MS
        || state.stop.load(Ordering::SeqCst)
    {
        return Err(error("invalid mutation budget or stopping service"));
    }
    let mut controllers = state.mutations.lock().map_err(lock_error)?;
    let finished = controllers
        .iter()
        .filter(|(_, (_, h))| h.is_finished())
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for id in finished {
        if let Some((_, h)) = controllers.remove(&id) {
            let _ = h.join();
        }
    }
    if controllers.len() >= 2 {
        return Err(error("two mutation campaigns are already active"));
    }
    let baseline = get(state, &submission.baseline_job_id)?;
    let document = store::result(state, &baseline)?;
    let assurance = sail_api::assurance::from_contract_result(&document).map_err(error)?;
    if assurance
        .requirements
        .iter()
        .flat_map(|r| &r.evidence)
        .any(|e| e.tier != Some(sail_api::assurance::EvidenceTier::SmtVerified))
    {
        return Err(error(
            "baseline lacks a complete authenticated source proof and tool identity",
        ));
    }
    let source = contracts::source(state, &baseline)?;
    let plan =
        sail_verus::contracts::mutation::plan(&source, submission.max_mutants).map_err(error)?;
    let mut queued = Queued {
        state,
        ids: Vec::new(),
        published: false,
    };
    for candidate in &plan.candidates {
        match contracts::submit_mutant(state, &candidate.source) {
            Ok(status) => queued.ids.push(status.id),
            Err(e) => {
                for id in &queued.ids {
                    let _ = super::cancel(state, id, false);
                }
                return Err(e);
            }
        }
    }
    let value = Record {
        version: api::VERSION,
        submission: submission.clone(),
        plan,
        jobs: queued.ids.clone(),
        started_at_ms,
    };
    let bytes = encode(&value)?;
    let id = digest(&bytes);
    let directory = path(state, &id)?;
    store::private_dir(&state.root.join("mutations"))?;
    let staging = state.root.join("mutation-staging").join(&id);
    store::private_dir(&staging)?;
    contracts::save_baseline(state, &submission.baseline_job_id, &name(&id, None))?;
    install(&staging.join("campaign.json"), &bytes)?;
    std::fs::rename(&staging, &directory)?;
    std::fs::File::open(state.root.join("mutations"))?.sync_all()?;
    let cancel = Arc::new(AtomicBool::new(false));
    let owned = Arc::clone(state);
    let token = Arc::clone(&cancel);
    let campaign = id.clone();
    state.activity.fetch_add(1, Ordering::SeqCst);
    let deadline = started + Duration::from_millis(value.submission.wall_time_ms);
    match std::thread::Builder::new()
        .name("sail-neutral-mutation".into())
        .spawn(move || control(&owned, &campaign, &value, &token, deadline))
    {
        Ok(handle) => {
            controllers.insert(id.clone(), (cancel, handle));
            queued.published = true;
        }
        Err(e) => {
            state.activity.fetch_sub(1, Ordering::SeqCst);
            return Err(error(e));
        }
    }
    drop(controllers);
    read(state, &id)
}
fn control(state: &State, id: &str, value: &Record, cancel: &AtomicBool, deadline: Instant) {
    struct Activity<'a>(&'a State);
    impl Drop for Activity<'_> {
        fn drop(&mut self) {
            self.0.activity.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let _activity = Activity(state);
    let mut retained = vec![false; value.jobs.len()];
    let terminal = loop {
        if cancel.load(Ordering::SeqCst) {
            break Lifecycle::Cancelled;
        }
        if state.stop.load(Ordering::SeqCst) {
            break Lifecycle::Interrupted;
        }
        if Instant::now() >= deadline {
            break Lifecycle::TimedOut;
        }
        let mut all_done = true;
        for (index, job_id) in value.jobs.iter().enumerate() {
            let Ok(job) = get(state, job_id) else {
                all_done = false;
                continue;
            };
            if !job.status.state.terminal() {
                all_done = false;
                continue;
            }
            if job.status.state == sail_api::jobs::Lifecycle::Completed && !retained[index] {
                if contracts::save_baseline(state, job_id, &name(id, Some(index))).is_err() {
                    all_done = false;
                    continue;
                }
                retained[index] = true;
            }
        }
        if all_done {
            break Lifecycle::Completed;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if terminal != Lifecycle::Completed {
        for job in &value.jobs {
            let _ = super::cancel(state, job, false);
        }
    }
    if let (Ok(directory), Ok(bytes)) = (
        path(state, id),
        encode(&End {
            record_sha256: id.into(),
            state: terminal,
        }),
    ) {
        let temporary = directory.join(".end-publishing");
        if install(&temporary, &bytes).is_ok()
            && install(&directory.join("end.sha256"), digest(&bytes).as_bytes()).is_ok()
        {
            let _ = std::fs::hard_link(&temporary, directory.join("end.json"));
            let _ = std::fs::remove_file(&temporary);
            if let Ok(directory) = std::fs::File::open(directory) {
                let _ = directory.sync_all();
            }
        }
    }
}

pub(super) fn cancel(state: &State, id: &str) -> io::Result<View> {
    if let Some((token, _)) = state.mutations.lock().map_err(lock_error)?.get(id) {
        token.store(true, Ordering::SeqCst);
    }
    read(state, id)
}

pub(super) fn read(state: &State, id: &str) -> io::Result<View> {
    let value = record(state, id)?;
    let baseline_name = name(id, None);
    if product::baseline(state, &baseline_name)?.job_id != value.submission.baseline_job_id {
        return Err(error("baseline job identity differs"));
    }
    let (source, baseline) = contracts::original_baseline(state, &baseline_name)?;
    let expected = sail_verus::contracts::mutation::plan(&source, value.submission.max_mutants)
        .map_err(error)?;
    if expected != value.plan {
        return Err(error(
            "retained mutation inventory differs from exact original source",
        ));
    }
    let ending = path(state, id)?.join("end.json");
    let terminal = if ending.exists() {
        let bytes = regular(&ending)?;
        if regular(&path(state, id)?.join("end.sha256"))? != digest(&bytes).as_bytes() {
            return Err(error("campaign completion content differs from its seal"));
        }
        let end: End = serde_json::from_slice(&bytes).map_err(error)?;
        if end.record_sha256 != id || end.state == Lifecycle::Running {
            return Err(error("invalid campaign completion identity"));
        }
        end.state
    } else if state
        .mutations
        .lock()
        .map_err(lock_error)?
        .get(id)
        .is_some_and(|(_, h)| !h.is_finished())
    {
        Lifecycle::Running
    } else {
        Lifecycle::Interrupted
    };
    let baseline_original = contracts::export_baseline(state, &baseline_name)?;
    let mut export_bytes = encode(&baseline_original)?.len();
    if export_bytes > sail_api::jobs::MAX_FRAME / 2 {
        return Err(error("complete mutation evidence exceeds export bound"));
    }
    let mut observations = Vec::new();
    for (index, (candidate, job_id)) in value.plan.candidates.iter().zip(&value.jobs).enumerate() {
        let evidence_name = name(id, Some(index));
        let mut observation = api::Observation {
            candidate: candidate.clone(),
            job_id: job_id.clone(),
            evidence_name: None,
            backend_sha256: None,
            original: None,
            outcome: Outcome::Inconclusive,
            reason: "actual source job has no retained conclusive evidence".into(),
        };
        if state
            .root
            .join("baselines")
            .join(format!("{evidence_name}.json"))
            .exists()
        {
            let public = product::baseline(state, &evidence_name)?;
            let (actual, original) = contracts::original_baseline(state, &evidence_name)?;
            if public.job_id != *job_id || actual != candidate.source {
                return Err(error("mutation source or job link differs"));
            }
            let (outcome, reason) = api::classify(&baseline, &original, candidate);
            let exported = contracts::export_baseline(state, &evidence_name)?;
            export_bytes += encode(&exported)?.len();
            if export_bytes > sail_api::jobs::MAX_FRAME / 2 {
                return Err(error("complete mutation evidence exceeds export bound"));
            }
            observation.original = Some(exported);
            observation.evidence_name = Some(evidence_name);
            observation.backend_sha256 = Some(digest(&encode(&original.backend)?));
            observation.outcome = outcome;
            observation.reason = reason;
        }
        observations.push(observation);
    }
    let passed = terminal == Lifecycle::Completed
        && !observations.is_empty()
        && value.plan.unsupported.is_empty()
        && value.plan.omitted == 0
        && observations.iter().all(|o| o.outcome == Outcome::Killed);
    let view = View {
        version: api::VERSION,
        id: id.into(),
        started_at_ms: value.started_at_ms,
        baseline_original,
        submission: value.submission,
        baseline_source: source,
        baseline_evidence_name: baseline_name,
        baseline_backend_sha256: digest(&encode(&baseline.backend)?),
        plan: value.plan,
        state: terminal,
        observations,
        passed,
        qualifiers: api::qualifiers(),
    };
    api::validate_view(&view).map_err(error)?;
    if encode(&view)?.len() > sail_api::jobs::MAX_FRAME - 256 {
        return Err(error("complete mutation response exceeds frame bound"));
    }
    Ok(view)
}

/// Attach only a matching authenticated campaign, without recursively reading assurance.
pub(super) fn attach(
    state: &State,
    job_id: &str,
    assurance: &mut sail_api::assurance::View,
) -> io::Result<()> {
    let entries = match std::fs::read_dir(state.root.join("mutations")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut campaigns = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= 1024 {
            return Err(error("mutation assurance inventory exceeds read bound"));
        }
        let entry = entry?;
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| error("invalid campaign path"))?;
        let record = record(state, &id)?;
        if record.submission.baseline_job_id == job_id {
            campaigns.push((record.started_at_ms, id));
        }
    }
    campaigns.sort();
    let Some((_, id)) = campaigns.last() else {
        return Ok(());
    };
    let campaign = read(state, id)?;
    api::attach_assurance(&campaign, assurance).map_err(error)
}
