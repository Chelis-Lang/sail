//! Pre-execution request capture, separate from verifier result construction.

use std::fs::OpenOptions;
use std::io::Write as _;

use sail_api::project::{REQUEST_CAPTURE_VERSION, RequestCapture, ResolvedRequest};

/// Private worker transport consumed by Sail before constructing compiler commands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestCaptureDestination {
    /// Parent-owned append-only journal destination.
    pub path: std::path::PathBuf,
    /// Parent-allocated job identity.
    pub worker_id: String,
}

/// A worker-only destination records every actual managed run, including a
/// fresh independent fallback. The parent selects exactly one matching run.
pub(super) fn capture(
    request: &ResolvedRequest,
    destination: Option<&RequestCaptureDestination>,
) -> crate::Result<()> {
    let Some(destination) = destination else {
        return Ok(());
    };
    let capture = RequestCapture {
        version: REQUEST_CAPTURE_VERSION,
        worker_id: destination.worker_id.clone(),
        request: request.clone(),
    };
    let bytes =
        serde_json::to_vec(&capture).map_err(|e| crate::Error::new("SAIL3115", e.to_string()))?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(&destination.path)
        .map_err(|e| crate::Error::new("SAIL3115", e.to_string()))?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|e| crate::Error::new("SAIL3115", e.to_string()))
}
