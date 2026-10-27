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

/// A native value observation made from the resolution inputs, never from a
/// saved result or request Strings. It proves no filesystem/launch closure.
pub(super) struct NativeRequestPaths {
    invocation: Vec<u8>,
    workspace: Vec<u8>,
}

impl NativeRequestPaths {
    pub(super) fn bind<'a>(
        &'a self,
        request: &'a ResolvedRequest,
    ) -> Option<sail_api::acceptance::RequestPathBinding<'a>> {
        sail_api::acceptance::bind_request_paths(request, &self.invocation, &self.workspace)
    }
}

impl Resolution<'_> {
    pub(super) fn capture_paths(&self) -> NativeRequestPaths {
        NativeRequestPaths {
            invocation: self.invocation.as_os_str().as_encoded_bytes().to_vec(),
            workspace: self.workspace.as_os_str().as_encoded_bytes().to_vec(),
        }
    }

    pub(super) fn resolve(&self) -> ResolvedRequest {
        let selection = self.engine.and_then(|engine| engine.selection.as_ref());
        let invocation = self.invocation.to_string_lossy();
        let workspace = self.workspace.to_string_lossy();
        let targets = self
            .targets
            .iter()
            .map(sail_api::acceptance::construct_request_target)
            .collect();
        sail_api::acceptance::construct_resolved_request(
            sail_api::acceptance::RequestConstruction {
                run_id: self.run_id,
                invocation: invocation.as_ref(),
                workspace: workspace.as_ref(),
                context: self.context,
                revision: self.revision,
                arguments: self.args,
                engine: self.engine.map(crate::engine::Request::identity),
                function: self.engine.and_then(|engine| engine.function.as_deref()),
                repair_package: selection.map_or("", |s| s.package.as_str()),
                repair_target: selection.and_then(|s| s.target.as_deref()),
                repair_kind: selection.map_or("", |s| s.kind.as_str()),
                targets,
            },
        )
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

#[cfg(all(test, unix))]
mod native_path_tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::Path;

    /// Exercises the actual pre-execution builder with independent sibling
    /// targets and compile-only arguments; no result rows supply its domain.
    #[test]
    fn nonempty_resolution_keeps_original_scope_and_compile_only_arguments() {
        let args = vec![
            "-p".into(),
            "original".into(),
            "--".into(),
            "--no-verify".into(),
        ];
        let targets = BTreeSet::from([
            ("package-a".into(), "original".into(), "lib".into()),
            ("package-b".into(), "sibling".into(), "bin".into()),
        ]);
        let resolution = Resolution {
            run_id: "original-run",
            invocation: Path::new("/original/invocation"),
            workspace: Path::new("/original/workspace"),
            context: "original-context",
            revision: "original-revision",
            args: &args,
            engine: None,
            targets: &targets,
        };
        let request = resolution.resolve();
        assert_eq!(request.run_id, "original-run");
        assert_eq!(request.invocation, "/original/invocation");
        assert_eq!(request.workspace, "/original/workspace");
        assert_eq!(request.context_sha256, "original-context");
        assert_eq!(request.input_revision, "original-revision");
        assert_eq!(request.args, args);
        assert!(!sail_api::acceptance::verification_enabled_command(
            &request.args
        ));
        assert_eq!(request.targets.len(), 2);
        assert_eq!(request.targets[0].package_id, "package-a");
        assert_eq!(request.targets[0].target, "original");
        assert_eq!(request.targets[0].kind, "lib");
        assert_eq!(request.targets[1].package_id, "package-b");
        assert_eq!(request.targets[1].target, "sibling");
        assert_eq!(request.targets[1].kind, "bin");
        assert!(request.engine.is_none() && request.function.is_none());
        assert_eq!(request.repair_package, "");
        assert!(request.repair_target.is_none());
        assert_eq!(request.repair_kind, "");
        assert!(resolution.capture_paths().bind(&request).is_some());
    }

    /// Path values only: no filesystem, Cargo, verifier or global input-domain
    /// qualification is implied by this native byte-projection regression.
    #[test]
    fn native_path_observation_never_reencodes_invalid_bytes() {
        let valid_invocation = Path::new("/native/invocation");
        let valid_workspace = Path::new("/native/workspace");
        let invalid_invocation = Path::new(OsStr::from_bytes(b"/native/invocation-\xff"));
        let invalid_workspace = Path::new(OsStr::from_bytes(b"/native/workspace-\xff"));
        let args = Vec::new();
        let targets = BTreeSet::new();
        for (invocation, workspace, expected) in [
            (valid_invocation, valid_workspace, true),
            (invalid_invocation, valid_workspace, false),
            (valid_invocation, invalid_workspace, false),
        ] {
            let resolution = Resolution {
                run_id: "native-path-value",
                invocation,
                workspace,
                context: "context",
                revision: "revision",
                args: &args,
                engine: None,
                targets: &targets,
            };
            let native = resolution.capture_paths();
            assert_eq!(native.invocation, invocation.as_os_str().as_encoded_bytes());
            assert_eq!(native.workspace, workspace.as_os_str().as_encoded_bytes());
            let request = resolution.resolve();
            assert_eq!(native.bind(&request).is_some(), expected);
        }
    }
}
