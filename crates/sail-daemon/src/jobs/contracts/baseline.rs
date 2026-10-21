//! Independently retained source and worker bytes for neutral baselines.
use std::io::{self, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use sail_api::contracts::Submission;
use sail_api::evidence::Baseline;
use serde::{Deserialize, Serialize};

use super::{State, decode, error, read, sha, store};

#[derive(Serialize, Deserialize)]
struct Seal {
    source: Submission,
    stdout_sha256: String,
    baseline_sha256: String,
}

fn historical(mut backend: sail_api::project::ProjectResult) -> sail_api::project::ProjectResult {
    backend.currentness = sail_api::project::Currentness::Historical;
    backend.notes.push("Immutable baseline evidence applies only to its recorded input revision, not current workspace inputs.".into());
    backend
}

pub(in crate::jobs) fn save_baseline(state: &State, id: &str, name: &str) -> io::Result<Baseline> {
    let job = super::super::get(state, id)?;
    if job.status.state != sail_api::jobs::Lifecycle::Completed {
        return Err(error("neutral evidence is not completed"));
    }
    let (input, stdout, original) = read(state, &job)?;
    let baseline = Baseline {
        version: sail_api::evidence::VERSION,
        name: name.into(),
        job_id: id.into(),
        result: historical(original.backend),
        contracts: Some(Box::new(original.contracts)),
    };
    let bytes = serde_json::to_vec(&baseline).map_err(error)?;
    if bytes.len() > sail_api::jobs::MAX_FRAME {
        return Err(error("neutral baseline exceeds response bound"));
    }
    let seal = Seal {
        source: input,
        stdout_sha256: sha(&stdout),
        baseline_sha256: sha(&bytes),
    };
    let directory = state.root.join("baselines");
    store::private_dir(&directory)?;
    let lock_path = directory.join(format!("{name}.contracts-lock"));
    if std::fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.file_type().is_file()) {
        return Err(error("neutral baseline lock is not a regular file"));
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&lock_path)?;
    lock.lock()?;
    let public = directory.join(format!("{name}.json"));
    let raw = directory.join(format!("{name}.contracts-stdout"));
    let sealed = directory.join(format!("{name}.contracts-seal"));
    if [&public, &raw, &sealed].iter().any(|p| p.exists()) {
        return Err(error(
            "immutable baseline name already exists or has an incomplete retained artifact",
        ));
    }
    // The public record is the commit point. Partial crash-surviving artifacts
    // remain explicit rather than being overwritten by a later request.
    install(&raw, &stdout)?;
    install(&sealed, &serde_json::to_vec(&seal).map_err(error)?)?;
    let temporary = directory.join(format!(".{name}.contracts-publishing"));
    install(&temporary, &bytes)?;
    // Publish only a complete synced inode. An interrupted publication fails closed.
    let published = std::fs::hard_link(&temporary, &public);
    let _ = std::fs::remove_file(&temporary);
    published?;
    std::fs::File::open(&directory)?.sync_all()?;
    Ok(baseline)
}

fn install(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn regular_bytes(path: &Path) -> io::Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(error("neutral baseline artifact is not a regular file"));
    }
    store::read_bytes(path)
}

pub(in crate::jobs) fn validate_baseline(
    state: &State,
    baseline: &Baseline,
    bytes: &[u8],
) -> io::Result<()> {
    let directory = state.root.join("baselines");
    if !std::fs::symlink_metadata(directory.join(format!("{}.json", baseline.name)))?
        .file_type()
        .is_file()
    {
        return Err(error("neutral baseline is not a regular file"));
    }
    let seal: Seal = serde_json::from_slice(&regular_bytes(
        &directory.join(format!("{}.contracts-seal", baseline.name)),
    )?)
    .map_err(error)?;
    if seal.baseline_sha256 != sha(bytes) {
        return Err(error(
            "retained neutral baseline differs from its complete output seal",
        ));
    }
    let stdout = regular_bytes(&directory.join(format!("{}.contracts-stdout", baseline.name)))?;
    if seal.stdout_sha256 != sha(&stdout) {
        return Err(error(
            "retained original neutral worker output differs from its seal",
        ));
    }
    let original = decode(&stdout, &seal.source)?;
    if baseline.contracts.as_deref() != Some(&original.contracts)
        || baseline.result != historical(original.backend)
    {
        return Err(error(
            "neutral baseline differs from its original worker evidence",
        ));
    }
    Ok(())
}
