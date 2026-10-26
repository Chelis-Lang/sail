//! Pre-execution request capture, separate from verifier result construction.

use std::fs::OpenOptions;
use std::io::Write as _;

use sail_api::project::{REQUEST_CAPTURE_VERSION, RequestCapture, ResolvedRequest};

/// Independent metadata, configuration and observation supplied before reading
/// a verifier result. Ordinary execution and unchanged-call preflight share
/// this builder; neither derives target scope from saved result rows.
pub(super) struct Resolution<'a> {
    pub run_id: &'a str,
    pub invocation: &'a std::path::Path,
    pub workspace: &'a std::path::Path,
    pub context: &'a str,
    pub revision: &'a str,
    pub args: &'a [String],
    pub engine: Option<&'a crate::engine::Request>,
    pub targets: &'a std::collections::BTreeSet<(String, String, String)>,
}

impl Resolution<'_> {
    pub(super) fn resolve(&self) -> ResolvedRequest {
        let selection = self.engine.and_then(|engine| engine.selection.as_ref());
        ResolvedRequest {
            run_id: self.run_id.to_owned(),
            invocation: self.invocation.to_string_lossy().into_owned(),
            workspace: self.workspace.to_string_lossy().into_owned(),
            context_sha256: self.context.to_owned(),
            input_revision: self.revision.to_owned(),
            args: self.args.to_vec(),
            engine: self.engine.map(crate::engine::Request::identity),
            function: self.engine.and_then(|engine| engine.function.clone()),
            repair_package: selection.map_or_else(String::new, |s| s.package.clone()),
            repair_target: selection.and_then(|s| s.target.clone()),
            repair_kind: selection.map_or_else(String::new, |s| s.kind.clone()),
            targets: self
                .targets
                .iter()
                .map(
                    |(package_id, target, kind)| sail_api::project::RequestTarget {
                        package_id: package_id.clone(),
                        target: target.clone(),
                        kind: kind.clone(),
                    },
                )
                .collect(),
        }
    }
}

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
