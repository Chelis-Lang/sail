//! Low-level child observations for the source-shared attempt reducer.
//!
//! OS command/pipe/wait attribution is an explicit native adapter boundary.
//! These records do not certify compiler input closure or verifier semantics.

use crate::{Error, Result};
use sail_acceptance::authority::{
    AttemptLedger, CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt,
    RunningAttempt, UnconfiguredAttempt,
};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Output};

pub(crate) struct Journal<'a> {
    directory: &'a Path,
    ledger: AttemptLedger<'a>,
    unavailable: Option<String>,
}

enum Payload<'a> {
    Observed(ObservedAttempt<'a>),
    Unavailable { stdout: Vec<u8>, stderr: Vec<u8> },
}

/// Owns fresh output regardless of whether optional observation could be recorded.
pub(crate) struct CapturedChild<'a> {
    status: ExitStatus,
    payload: Payload<'a>,
}

impl CapturedChild<'_> {
    pub(crate) fn stdout(&self) -> &[u8] {
        match &self.payload {
            Payload::Observed(observed) => observed.stdout(),
            Payload::Unavailable { stdout, .. } => stdout,
        }
    }
    pub(crate) fn stderr(&self) -> &[u8] {
        match &self.payload {
            Payload::Observed(observed) => observed.stderr(),
            Payload::Unavailable { stderr, .. } => stderr,
        }
    }
    pub(crate) fn status(&self) -> ExitStatus {
        self.status
    }
    pub(crate) fn observation(&self) -> Option<&ObservedAttempt<'_>> {
        match &self.payload {
            Payload::Observed(observed) => Some(observed),
            Payload::Unavailable { .. } => None,
        }
    }
    #[cfg(test)]
    pub(crate) fn into_output(self) -> Output {
        let (stdout, stderr) = match self.payload {
            Payload::Observed(observed) => {
                let (stdout, stderr, _) = observed.into_output();
                (stdout, stderr)
            }
            Payload::Unavailable { stdout, stderr } => (stdout, stderr),
        };
        Output {
            stdout,
            stderr,
            status: self.status,
        }
    }
    pub(crate) fn unavailable(output: Output) -> Self {
        Self {
            status: output.status,
            payload: Payload::Unavailable {
                stdout: output.stdout,
                stderr: output.stderr,
            },
        }
    }
}

impl<'a> Journal<'a> {
    pub(crate) fn new(run: &'a [u8], unit: &'a CompilerUnit, directory: &'a Path) -> Self {
        Self {
            directory,
            ledger: AttemptLedger::new(run, unit),
            unavailable: None,
        }
    }

    /// Reserve and persist the slot before the caller can spawn its child.
    pub(super) fn reserve(&mut self, command: &Command) -> Option<ReservedAttempt<'a>> {
        let allocation = self.allocate()?;
        self.configure(allocation, command)
    }

    /// Allocate request paths before sealing the final command arguments.
    pub(crate) fn allocate(&mut self) -> Option<UnconfiguredAttempt<'a>> {
        if self.unavailable.is_some() {
            return None;
        }
        let allocation = self.ledger.allocate();
        if allocation.is_none() {
            self.refuse("compiler attempt counter exhausted");
        }
        allocation
    }

    /// Observe and persist the final command after request preparation.
    pub(crate) fn configure(
        &mut self,
        allocation: UnconfiguredAttempt<'a>,
        command: &Command,
    ) -> Option<ReservedAttempt<'a>> {
        if self.unavailable.is_some() {
            return None;
        }
        match self.try_configure(allocation, command) {
            Ok(reserved) => Some(reserved),
            Err(failure) => {
                self.unavailable = Some(failure.to_string());
                None
            }
        }
    }

    pub(super) fn started(
        &mut self,
        reserved: Option<ReservedAttempt<'a>>,
        child: &Child,
    ) -> Option<RunningAttempt<'a>> {
        let reserved = reserved?;
        let running = reserved.started(child.id());
        if running.is_none() {
            self.unavailable = Some("compiler child has no process identity".into());
        }
        running
    }

    pub(crate) fn refuse(&mut self, reason: &str) {
        if self.unavailable.is_none() {
            self.unavailable = Some(reason.into());
        }
    }

    /// Always retains the same fresh bytes/status. Observation failures are sticky
    /// and remove evidence without changing the actual compiler result.
    pub(super) fn capture(
        &mut self,
        running: Option<RunningAttempt<'a>>,
        output: Output,
    ) -> CapturedChild<'a> {
        if self.unavailable.is_some() {
            return CapturedChild::unavailable(output);
        }
        let Some(running) = running else {
            return CapturedChild::unavailable(output);
        };
        let status = output.status;
        let observed = running
            .stdout_eof(output.stdout)
            .stderr_eof(output.stderr)
            .exited(status.code())
            .finish_or_output();
        match observed {
            Ok(observed) => {
                if let Err(failure) = self.persist_observed(&observed) {
                    self.unavailable = Some(failure.to_string());
                    let (stdout, stderr, _) = observed.into_output();
                    CapturedChild::unavailable(Output {
                        stdout,
                        stderr,
                        status,
                    })
                } else {
                    CapturedChild {
                        status,
                        payload: Payload::Observed(observed),
                    }
                }
            }
            Err(output) => {
                self.unavailable = Some("ambiguous compiler observations".into());
                CapturedChild::unavailable(Output {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    status,
                })
            }
        }
    }

    fn try_configure(
        &self,
        allocation: UnconfiguredAttempt<'a>,
        command: &Command,
    ) -> Result<ReservedAttempt<'a>> {
        let descriptor = ProcessDescriptor {
            program: command.get_program().as_encoded_bytes().to_vec(),
            arguments: command
                .get_args()
                .map(|argument| argument.as_encoded_bytes().to_vec())
                .collect(),
            directory: command
                .get_current_dir()
                .map(|path| path.as_os_str().as_encoded_bytes().to_vec()),
            overrides: command
                .get_envs()
                .map(|(name, value)| {
                    (
                        name.as_encoded_bytes().to_vec(),
                        value.map(|value| value.as_encoded_bytes().to_vec()),
                    )
                })
                .collect(),
        };
        let reserved = allocation.configure(descriptor);
        let descriptor = reserved.descriptor();
        let unit = reserved.unit();
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "run": reserved.run(),
            "slot": reserved.slot(),
            "unit": {"manifest":unit.manifest,"name":unit.name,
                "crate_type":unit.crate_type,"cargo_name":unit.cargo_name},
            "command": {"program":descriptor.program,"arguments":descriptor.arguments,
                "directory":descriptor.directory,"overrides":descriptor.overrides},
        }))
        .map_err(|failure| error(failure.to_string()))?;
        std::fs::create_dir_all(self.directory).map_err(|failure| error(failure.to_string()))?;
        let path = self.directory.join(format!("{}.reserved", reserved.slot()));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_data()))
            .map_err(|failure| error(format!("cannot reserve compiler attempt: {failure}")))?;
        Ok(reserved)
    }

    /// Record the terminal observations while the exact payloads remain borrowed.
    /// Payload hashes index the existing raw sidecars; they do not authenticate a run.
    fn persist_observed(&self, observed: &ObservedAttempt<'_>) -> Result<()> {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "run": observed.reserved().run(),
            "slot": observed.reserved().slot(),
            "pid": observed.pid(),
            "exit": observed.exit(),
            "stdout_sha256": crate::managed::digest(observed.stdout()),
            "stderr_sha256": crate::managed::digest(observed.stderr()),
        }))
        .map_err(|failure| error(failure.to_string()))?;
        let path = self
            .directory
            .join(format!("{}.observed", observed.reserved().slot()));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_data()))
            .map_err(|failure| error(format!("cannot record compiler observation: {failure}")))
    }
}

fn error(message: impl Into<String>) -> Error {
    Error::new("SAIL3111", message)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::adapter::{Captured, CompletionOrigin};
    use crate::managed::{Invocation, Runtime, TargetIdentity};
    use sail_api::project::{
        Currentness, ProjectResult, RequestTarget, ResolvedRequest, TargetResult,
    };
    use std::collections::BTreeMap;
    use std::process::Stdio;

    type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[test]
    fn allocation_precedes_paths_but_configuration_seals_actual_final_arguments() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-configured-attempt-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let unit = CompilerUnit {
            manifest: b"fixture/Cargo.toml".to_vec(),
            name: b"demo".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let mut journal = Journal::new(b"run", &unit, &root);
        let allocation = journal.allocate().ok_or("allocation refused")?;
        let request = root.join(format!("{}.request", allocation.slot()));
        std::fs::write(&request, b"prepared request")?;
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf '%s' \"$1\"", "compiler"])
            .arg(&request)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let reserved = journal
            .configure(allocation, &command)
            .ok_or("configuration refused")?;
        assert_eq!(
            reserved.descriptor().arguments.last().map(Vec::as_slice),
            Some(request.as_os_str().as_encoded_bytes())
        );
        let child = command.spawn()?;
        let running = journal.started(Some(reserved), &child);
        let captured = journal.capture(running, child.wait_with_output()?);
        assert_eq!(captured.stdout(), request.as_os_str().as_encoded_bytes());
        assert!(captured.status().success());
        assert!(captured.observation().is_some());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn journal_failures_preserve_output_and_refuse_retained_authority() -> TestResult {
        const CHILD: &str = "SAIL_ATTEMPT_FAILURE_CHILD";
        if let Some(root) = std::env::var_os(CHILD) {
            let partial = std::env::var("SAIL_ATTEMPT_FAILURE_SCOPE").as_deref() == Ok("partial");
            for case in ["positive", "reservation-failure", "terminal-failure"] {
                exercise_case(&std::path::PathBuf::from(&root).join(case), case, partial)?;
            }
            return Ok(());
        }
        let root =
            std::env::temp_dir().join(format!("sail-attempt-failure-{}", std::process::id()));
        for scope in ["whole", "partial"] {
            let directory = root.join(scope);
            std::fs::create_dir_all(&directory)?;
            let output = Command::new(std::env::current_exe()?)
                .args(["--exact", "adapter::attempts::tests::journal_failures_preserve_output_and_refuse_retained_authority", "--nocapture"])
                .env(CHILD, &directory)
                .env("SAIL_ATTEMPT_FAILURE_SCOPE", scope)
                .env("CARGO_PKG_NAME", "demo").env("CARGO_PKG_VERSION", "1.0.0")
                .env("CARGO_MANIFEST_DIR", &directory)
                .env(" __VERUS_DRIVER_ARGS__", if scope == "partial" {
                    "__VERUS_DRIVER_ARGS_SEP__--verify-function__VERUS_DRIVER_ARGS_SEP__demo::selected"
                } else { "" })
                .output()?;
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    fn exercise_case(root: &Path, case: &str, partial: bool) -> TestResult {
        std::fs::create_dir_all(root)?;
        let document = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        let input = root.join("fixture.stdout");
        std::fs::write(&input, document)?;
        let unit = CompilerUnit {
            manifest: b"fixture/Cargo.toml".to_vec(),
            name: b"demo".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let directory = root.join("attempts");
        if case == "reservation-failure" {
            std::fs::write(&directory, b"not a directory")?;
        }
        let mut journal = Journal::new(case.as_bytes(), &unit, &directory);
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "cat \"$1\"; printf 'original diagnostic\\n' >&2; exit 0",
                "compiler",
            ])
            .arg(input)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let reserved = journal.reserve(&command);
        if case == "terminal-failure" {
            std::fs::create_dir(directory.join("0.observed"))?;
        }
        // This actual child must run even when the pre-spawn journal failed.
        let child = command.spawn()?;
        let running = journal.started(reserved, &child);
        let captured = journal.capture(running, child.wait_with_output()?);
        assert_eq!(captured.stdout(), document);
        assert_eq!(captured.stderr(), b"original diagnostic\n");
        assert_eq!(captured.status().code(), Some(0));
        let available = case == "positive";
        assert_eq!(captured.observation().is_some(), available);
        finish_and_reload(root, case, partial, &captured, available)?;
        // Repairing the path later cannot promote this unavailable history.
        if !available {
            let mut other = Command::new("/bin/sh");
            other.args(["-c", "exit 0"]);
            assert!(journal.reserve(&other).is_none());
        }
        Ok(())
    }

    fn finish_and_reload(
        root: &Path,
        case: &str,
        partial: bool,
        captured: &CapturedChild<'_>,
        available: bool,
    ) -> TestResult {
        let args = if partial {
            vec![
                "--".into(),
                "--verify-function".into(),
                "demo::selected".into(),
            ]
        } else {
            Vec::new()
        };
        let invocations = root.join("invocations");
        std::fs::create_dir_all(&invocations)?;
        let sidecar = root.join("sidecar");
        std::fs::create_dir_all(&sidecar)?;
        let runtime = Runtime {
            run_id: case.into(),
            args,
            z3: "/pinned/z3".into(),
            z3_sha256: "solver".into(),
            daemon: false,
            sidecars: root.into(),
            invocations: invocations.clone(),
            original_wrapper: "/bin/sh".into(),
            receipts: root.join("receipts"),
            context_sha256: "context".into(),
            input_revision: "revision".into(),
            packages: BTreeMap::new(),
            target_kinds: BTreeMap::new(),
            engine: None,
            retained: None,
            attribution_logs: false,
        };
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "arguments".into(),
            cargo_name: None,
        };
        let key = identity.key();
        crate::adapter::finish(
            &runtime,
            identity,
            &sidecar,
            &Captured {
                stdout: captured.stdout(),
                stderr: captured.stderr(),
                exit: captured.status().code(),
            },
            None,
            None,
            CompletionOrigin::Live(captured.observation()),
        )?;
        let invocation: Invocation =
            serde_json::from_slice(&std::fs::read(invocations.join(format!("{key}.json")))?)?;
        assert_eq!(
            std::fs::read(sidecar.join("verus.stdout"))?,
            captured.stdout()
        );
        assert_eq!(
            std::fs::read(sidecar.join("verus.stderr"))?,
            captured.stderr()
        );
        assert_eq!(invocation.report.exit_code, Some(0));
        assert_eq!(invocation.success, available && !partial);
        assert_eq!(
            invocation.successful_for(if partial { Some(&runtime.args) } else { None }),
            available
        );
        assert_eq!(
            crate::managed::receipt_complete(&invocation.report),
            available
        );
        if !available {
            assert_eq!(
                invocation.report.error.as_ref().map(|e| e.code.as_str()),
                Some("SAIL3111")
            );
        }
        check_projection(&runtime, invocation, available && !partial)?;
        Ok(())
    }

    fn check_projection(runtime: &Runtime, invocation: Invocation, whole: bool) -> TestResult {
        let target = TargetResult {
            package_id: "pkg".into(),
            target: "demo".into(),
            kind: vec!["lib".into()],
            establishing_run: runtime.run_id.clone(),
            reused: false,
            scope_reuse: None,
            requested: true,
            complete: invocation.success,
            report: invocation.report,
            dependency_store: None,
            dependency_build: None,
        };
        let project = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: 2,
            run_id: runtime.run_id.clone(),
            workspace: "workspace".into(),
            context_sha256: runtime.context_sha256.clone(),
            input_revision: runtime.input_revision.clone(),
            currentness: Currentness::Current,
            exit_code: Some(0),
            targets: vec![target],
            execution: sail_api::project::Execution::default(),
            notes: Vec::new(),
            error: None,
            engine: None,
            no_reuse: None,
        };
        // Retained serialization preserves refusal, independently of input Current.
        let restored: ProjectResult = serde_json::from_slice(&serde_json::to_vec(&project)?)?;
        assert_eq!(restored.currentness, Currentness::Current);
        assert_eq!(
            sail_api::acceptance::complete_target(&restored.targets[0]).is_some(),
            whole
        );
        assert_eq!(
            sail_api::acceptance::complete_selection(&restored).is_some(),
            whole
        );
        let request = ResolvedRequest {
            run_id: runtime.run_id.clone(),
            invocation: "workspace".into(),
            workspace: "workspace".into(),
            context_sha256: runtime.context_sha256.clone(),
            input_revision: runtime.input_revision.clone(),
            args: runtime.args.clone(),
            engine: None,
            function: None,
            repair_package: String::new(),
            repair_target: None,
            repair_kind: String::new(),
            targets: vec![RequestTarget {
                package_id: "pkg".into(),
                target: "demo".into(),
                kind: "lib".into(),
            }],
        };
        assert_eq!(
            sail_api::acceptance::complete_request(&request, &restored).is_some(),
            whole
        );
        if restored.targets[0].report.error.is_some() {
            // A later projection must not launder missing origin by flipping
            // its old selected/complete labels while keeping the same report.
            let mut relabeled = restored;
            relabeled.targets[0].complete = true;
            relabeled.targets[0].reused = true;
            assert!(sail_api::acceptance::complete_target(&relabeled.targets[0]).is_none());
            assert!(sail_api::acceptance::complete_selection(&relabeled).is_none());
            assert!(sail_api::acceptance::complete_request(&request, &relabeled).is_none());
        }
        Ok(())
    }
}
