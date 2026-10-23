//! Attributed native temporal execution through the existing isolated worker.
use crate::{Cancellation, execution, invalid};
use sail_api::{
    runtime::Outcome,
    temporal::{Capability, DIVE_REVISION, Job, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Trusted installation choice; callers cannot supply it in a wire request.
#[derive(Clone, Debug)]
pub struct Worker {
    /// Fixed installed executable, authenticated before every dispatch.
    pub executable: PathBuf,
    /// Expected digest from the installation manifest.
    pub sha256: String,
}

/// Versioned worker installation manifest, covered by the release file index.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Installation format, currently one.
    pub version: u32,
    /// Native payload version.
    pub protocol: u32,
    /// Exact linked native source revision.
    pub backend_revision: String,
    /// OS and architecture of the executable.
    pub platform: String,
    /// SHA-256 of the installed executable bytes.
    pub sha256: String,
}

impl Worker {
    /// Resolve a fixed sibling worker and manifest without searching PATH.
    ///
    /// # Errors
    /// Refuses malformed, incompatible, oversized or unreadable manifests.
    pub fn discover(sail: &Path) -> std::result::Result<Option<Self>, sail_api::runtime::Error> {
        let parent = sail
            .parent()
            .ok_or_else(|| invalid("invalid_config", "sail executable has no parent"))?;
        let manifest = parent.join("sail-runtime-worker.json");
        let mut bytes = Vec::new();
        match std::fs::File::open(&manifest).and_then(|f| f.take(4097).read_to_end(&mut bytes)) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(invalid(
                    "invalid_config",
                    format!("cannot read worker installation manifest: {e}"),
                ));
            }
        }
        if bytes.len() > 4096 {
            return Err(invalid(
                "invalid_config",
                "worker installation manifest exceeds 4 KiB",
            ));
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| {
            invalid(
                "invalid_config",
                format!("invalid worker installation manifest: {e}"),
            )
        })?;
        if manifest.version != 1
            || manifest.protocol != sail_api::temporal::VERSION
            || manifest.backend_revision != DIVE_REVISION
            || manifest.platform != platform()
            || !digest(&manifest.sha256)
        {
            return Err(invalid(
                "invalid_config",
                "worker installation identity is incompatible",
            ));
        }
        Ok(Some(Self {
            executable: parent.join("sail-runtime-worker"),
            sha256: manifest.sha256,
        }))
    }

    fn authenticate(&self) -> std::result::Result<(), sail_api::runtime::Error> {
        let mut bytes = Vec::new();
        std::fs::File::open(&self.executable)
            .and_then(|f| f.take(512 * 1024 * 1024 + 1).read_to_end(&mut bytes))
            .map_err(|e| {
                execution(
                    "unavailable",
                    format!("native temporal worker unavailable: {e}"),
                )
            })?;
        if !digest(&self.sha256)
            || bytes.len() > 512 * 1024 * 1024
            || crate::hash(&bytes) != self.sha256
        {
            return Err(execution(
                "unavailable",
                "native temporal worker differs from installation identity",
            ));
        }
        Ok(())
    }
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Stable host identity used in worker installation manifests.
#[must_use]
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub(crate) fn capability(worker: Option<&Worker>) -> Capability {
    let authenticated = worker
        .ok_or_else(|| execution("unavailable", "native temporal worker is not installed"))
        .and_then(Worker::authenticate);
    Capability {
        implemented: true,
        installed: authenticated.is_ok(),
        backend_revision: DIVE_REVISION.into(),
        worker_sha256: authenticated
            .as_ref()
            .ok()
            .and_then(|()| worker.map(|w| w.sha256.clone())),
        error: authenticated.err(),
    }
}

pub(crate) fn execute(
    job: &Job,
    worker: Option<&Worker>,
    cancellation: &Cancellation,
) -> std::result::Result<Result, sail_api::runtime::Error> {
    let started = Instant::now();
    if job.obligation_id.trim().is_empty()
        || job.obligation_id.len() > 1024
        || job.requirement_ids.len() > 1024
        || job
            .requirement_ids
            .iter()
            .any(|id| id.trim().is_empty() || id.len() > 1024)
        || job.requirement_ids.windows(2).any(|ids| ids[0] >= ids[1])
        || job.wall_time_ms == 0
        || job.wall_time_ms > sail_api::runtime::MAX_WALL_TIME_MS
        || job.request.version != sail_api::temporal::VERSION
    {
        return Err(invalid(
            "invalid_request",
            "invalid temporal identity, version or deadline",
        ));
    }
    let bytes =
        serde_json::to_vec(&job.request).map_err(|e| invalid("invalid_request", e.to_string()))?;
    if bytes.len() > sail_api::temporal::MAX_DOCUMENT_BYTES {
        return Err(invalid(
            "invalid_request",
            "temporal payload exceeds 16 MiB",
        ));
    }
    let mut result = Result {
        obligation_id: job.obligation_id.clone(),
        requirement_ids: job.requirement_ids.clone(),
        request_sha256: sail_api::temporal::request_sha256(&job.request)
            .map_err(|e| invalid("invalid_request", e.to_string()))?,
        backend_revision: DIVE_REVISION.into(),
        worker_sha256: None,
        wall_time_ms: job.wall_time_ms,
        outcome: Outcome::Unavailable,
        response: None,
        error: None,
        cache_hit: false,
    };
    let ready = capability(worker);
    if !ready.installed {
        result.error = ready.error;
        return Ok(result);
    }
    let Some(worker) = worker else {
        return Err(execution(
            "infrastructure",
            "installed temporal worker missing configuration",
        ));
    };
    result.worker_sha256 = Some(worker.sha256.clone());
    match sail_dive::client::run(
        &worker.executable,
        &worker.sha256,
        &job.request,
        Duration::from_millis(job.wall_time_ms).saturating_sub(started.elapsed()),
        &cancellation.0,
    ) {
        sail_dive::client::RunResult::Completed(response) => {
            result.outcome = Outcome::Completed;
            result.response = Some(*response);
        }
        sail_dive::client::RunResult::TimedOut { .. } => {
            result.outcome = Outcome::Timeout;
            result.error = Some(execution("timeout", "native temporal deadline expired"));
        }
        sail_dive::client::RunResult::Cancelled { .. } => {
            result.outcome = Outcome::Cancelled;
            result.error = Some(execution(
                "cancelled",
                "native temporal execution cancelled",
            ));
        }
        sail_dive::client::RunResult::Failed(message) => {
            result.outcome = Outcome::Infrastructure;
            result.error = Some(execution("infrastructure", message));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_manifest_is_unavailable_and_wrong_manifest_cannot_select_an_executable()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-worker-manifest-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let path = root.join("sail");
        assert!(Worker::discover(&path)?.is_none());
        let manifest = root.join("sail-runtime-worker.json");
        std::fs::write(&manifest, "{}")?;
        assert!(Worker::discover(&path).is_err());
        std::fs::write(
            &manifest,
            serde_json::to_vec(&Manifest {
                version: 1,
                protocol: sail_api::temporal::VERSION,
                backend_revision: DIVE_REVISION.into(),
                platform: platform(),
                sha256: "0".repeat(64),
            })?,
        )?;
        let worker = Worker::discover(&path)?.ok_or("missing worker configuration")?;
        assert!(!capability(Some(&worker)).installed);
        std::fs::write(&worker.executable, "changed")?;
        assert!(!capability(Some(&worker)).installed);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
