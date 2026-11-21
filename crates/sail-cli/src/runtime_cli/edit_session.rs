//! Embedded, process-isolated execution of a closed group of native checks.
use sail_dive::client::RunResult;
use sail_dive::edits::{Batch, Entry, Evidence};
use sail_runtime::temporal::Worker;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
enum Status<'a> {
    Completed { entries: &'a [Entry] },
    TimedOut { worker_started: bool },
    Cancelled { worker_started: bool },
    Failed { message: String },
}

#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    worker_sha256: Option<&'a str>,
    backend_revision: &'a str,
    backend_source: &'a str,
    #[serde(flatten)]
    status: &'a Status<'a>,
}

impl Status<'_> {
    fn emit(&self, out: &mut dyn Write, worker: Option<&Worker>) -> Result<u8, String> {
        let complete = matches!(self, Self::Completed { entries } if entries.iter().all(|r| {
            match &r.evidence {
                Evidence::Checked { .. } | Evidence::EstablishedFirstTemporalClaim(_) | Evidence::ExactRequest { .. } => true,
                Evidence::Fresh { response } => matches!(response.outcome, sail_dive::Outcome::Holds | sail_dive::Outcome::Refuted),
            }
        }));
        super::write_json(
            out,
            &Report {
                version: 1,
                worker_sha256: worker.map(|w| w.sha256.as_str()),
                backend_revision: sail_dive::DIVE_VERSION,
                backend_source: sail_dive::edits::BACKEND_SOURCE,
                status: self,
            },
        )?;
        Ok(u8::from(!complete))
    }
}

pub(super) fn run(
    path: &Path,
    extensions: &crate::Extensions,
    out: &mut dyn Write,
) -> Result<u8, String> {
    let started = Instant::now();
    let input = super::open_input(path)?;
    let mut bytes = Vec::new();
    input
        .take(sail_dive::MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("error[SAIL6101]: cannot read edit batch: {e}"))?;
    if bytes.len() > sail_dive::MAX_DOCUMENT_BYTES {
        return Err("error[SAIL6101]: edit batch exceeds 16 MiB".into());
    }
    let batch: Batch = sail_api::temporal::decode(&bytes)
        .map_err(|e| format!("error[SAIL6101]: invalid edit batch JSON: {e}"))?;
    drop(bytes);
    batch.admit().map_err(|e| format!("error[SAIL6101]: {e}"))?;

    let signal = Arc::new(AtomicBool::new(false));
    let cancel = signal.clone();
    ctrlc::set_handler(move || cancel.store(true, Ordering::Release))
        .map_err(|e| format!("error[SAIL6102]: cannot install cancellation handler: {e}"))?;
    if started.elapsed() >= std::time::Duration::from_millis(batch.wall_time_ms) {
        return Status::TimedOut {
            worker_started: false,
        }
        .emit(out, None);
    }
    if extensions.temporal.is_none() {
        return Status::Failed {
            message: sail_runtime::temporal::NOT_INCLUDED.into(),
        }
        .emit(out, None);
    }
    let sail = std::env::current_exe()
        .map_err(|e| format!("error[SAIL6102]: cannot locate sail executable: {e}"))?;
    let worker = match Worker::discover(&sail) {
        Ok(Some(worker)) => worker,
        Ok(None) => {
            return Status::Failed {
                message: "native temporal worker is not installed".into(),
            }
            .emit(out, None);
        }
        Err(error) => {
            return Status::Failed {
                message: error.message,
            }
            .emit(out, None);
        }
    };
    match sail_dive::client::run_edit_session(
        &worker.executable,
        &worker.sha256,
        &batch,
        started,
        &signal,
    ) {
        RunResult::Completed(batch) => Status::Completed {
            entries: &batch.entries,
        }
        .emit(out, Some(&worker)),
        RunResult::TimedOut { worker_started } => {
            Status::TimedOut { worker_started }.emit(out, Some(&worker))
        }
        RunResult::Cancelled { worker_started } => {
            Status::Cancelled { worker_started }.emit(out, Some(&worker))
        }
        RunResult::Failed(message) => Status::Failed { message }.emit(out, Some(&worker)),
    }
}
