//! Owned native command construction with explicit environment inheritance.
//!
//! Callers cannot obtain a mutable std Command or supply an already configured
//! one. Recording never changes forwarded argv, environment operations or I/O.

use super::environment::ParentEnvironment;
use sail_acceptance::authority::ProcessDescriptor;
use sail_acceptance::phase_inputs::{
    ConfiguredIntent, OwnedPipeCompletionFields, OwnedSpawnFields, OwnerIdentity,
};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Output, Stdio};
use std::sync::Arc;

#[path = "command_pipe.rs"]
mod pipe;
pub(crate) use pipe::{PipeOutput, PipeReader};

#[derive(serde::Serialize)]
pub(crate) struct EnvironmentObservation {
    parent_sha256: String,
    operations_sha256: String,
    effective_sha256: String,
    inherits: bool,
}

pub(crate) struct Command {
    inner: std::process::Command,
    parent: ParentEnvironment,
    inherits: bool,
    recorded: BTreeMap<OsString, Option<OsString>>,
    program: OsString,
    arguments: Vec<OsString>,
    directory: Option<PathBuf>,
}

/// Owns the exact spawned child and the optional final environment observation.
pub(crate) struct Spawned {
    child: Child,
    environment: Option<EnvironmentObservation>,
    configuration: Option<ConfiguredIntent>,
    waited: Option<ExitStatus>,
    owner: Arc<()>,
}

pub(crate) struct Completed {
    status: ExitStatus,
    environment: Option<EnvironmentObservation>,
    fields: OwnedPipeCompletionFields<LaunchLease>,
}

/// Native ownership lease; PID reuse cannot substitute a different child.
pub(crate) struct LaunchLease {
    owner: Arc<()>,
}

/// The sole ownership identity primitive used by the shared completion guard.
pub(crate) struct NativeOwnerIdentity;

impl OwnerIdentity<LaunchLease> for NativeOwnerIdentity {
    fn same_owner(&self, original: &LaunchLease, actual: &LaunchLease) -> bool {
        Arc::ptr_eq(&original.owner, &actual.owner)
    }
}

/// Read-only final command fields; no mutable native builder escapes.
pub(crate) trait CommandView {
    fn get_program(&self) -> &OsStr;
    fn get_args(&self) -> impl Iterator<Item = &OsStr>;
    fn get_current_dir(&self) -> Option<&Path>;
    fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)>;
}

/// Narrow configuration supported by actual compiler observation requests.
pub(crate) trait ConfigureCommand {
    fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self;
    fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self;
}

impl Spawned {
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }
    pub(crate) fn environment(&self) -> Option<&EnvironmentObservation> {
        self.environment.as_ref()
    }

    pub(crate) fn construction_observed(&self) -> bool {
        self.configuration.is_some()
    }

    pub(crate) fn spawn_fields(&self) -> OwnedSpawnFields<'_, LaunchLease> {
        OwnedSpawnFields {
            owner: LaunchLease {
                owner: Arc::clone(&self.owner),
            },
            pid: self.child.id(),
            configuration: self.configuration.as_ref(),
        }
    }
    pub(crate) fn take_stdout(&mut self) -> Option<PipeReader<std::process::ChildStdout>> {
        self.child
            .stdout
            .take()
            .map(|reader| PipeReader::new(reader, Arc::clone(&self.owner), 0))
    }
    pub(crate) fn take_stderr(&mut self) -> Option<PipeReader<std::process::ChildStderr>> {
        self.child
            .stderr
            .take()
            .map(|reader| PipeReader::new(reader, Arc::clone(&self.owner), 1))
    }

    // The original outer Cargo relay is a separate managed-only byte forwarder.
    // Compiler adapters outside `managed` cannot obtain these raw endpoints.
    pub(super) fn take_cargo_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.stdout.take()
    }
    pub(super) fn take_cargo_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }
    pub(crate) fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
    pub(crate) fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.waited = Some(status);
        Ok(status)
    }

    pub(crate) fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if let Some(status) = status {
            self.waited = Some(status);
        }
        Ok(status)
    }

    /// Consume private reader completions; no caller-supplied byte vector can attach.
    pub(crate) fn captured(
        &mut self,
        stdout: PipeOutput,
        stderr: Option<PipeOutput>,
    ) -> std::io::Result<Completed> {
        let status = self
            .waited
            .ok_or_else(|| std::io::Error::other("owned compiler has no normal wait"))?;
        Ok(Completed {
            status,
            environment: self.environment.take(),
            fields: OwnedPipeCompletionFields {
                owner: Some(LaunchLease {
                    owner: Arc::clone(&self.owner),
                }),
                pid: Some(self.child.id()),
                configuration: self.configuration.take(),
                wait: Some(status.code()),
                stdout: stdout.into_readback(),
                stderr: stderr.map_or_else(
                    || sail_acceptance::stream_capture::PipeReadback::Unavailable(Vec::new()),
                    PipeOutput::into_readback,
                ),
            },
        })
    }

    #[cfg(unix)]
    pub(crate) fn wait_with_output(mut self) -> std::io::Result<Completed> {
        // std Child::wait_with_output closes owned stdin before draining pipes.
        // Preserve that ordering: a child waiting for EOF must be allowed to exit.
        drop(self.child.stdin.take());
        let stdout = self.take_stdout();
        let stderr = self.take_stderr();
        let (stdout, stderr) = pipe::read_pair(stdout, stderr)?;
        self.wait()?;
        self.captured(stdout, Some(stderr))
    }

    #[cfg(not(unix))]
    pub(crate) fn wait_with_output(self) -> std::io::Result<Completed> {
        // Preserve fresh std output on an unsupported observation provider;
        // this path supplies no owned-pipe EOF or compiler-completion authority.
        let output = self.child.wait_with_output()?;
        Ok(Completed::unobserved(output, self.environment))
    }
}

impl Completed {
    #[cfg(test)]
    pub(crate) fn environment(&self) -> Option<&EnvironmentObservation> {
        self.environment.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn into_output(self) -> Output {
        self.raw_output()
    }
    pub(crate) fn into_observed_output(self) -> Option<Output> {
        if self.environment.is_none() {
            return None;
        }
        Some(self.raw_output())
    }

    pub(crate) fn into_parts(self) -> (ExitStatus, OwnedPipeCompletionFields<LaunchLease>) {
        (self.status, self.fields)
    }

    fn raw_output(self) -> Output {
        Output {
            status: self.status,
            stdout: self.fields.stdout.into_bytes(),
            stderr: self.fields.stderr.into_bytes(),
        }
    }

    fn unobserved(output: Output, environment: Option<EnvironmentObservation>) -> Self {
        Self {
            status: output.status,
            environment,
            fields: OwnedPipeCompletionFields {
                owner: None,
                pid: None,
                configuration: None,
                wait: Some(output.status.code()),
                stdout: sail_acceptance::stream_capture::PipeReadback::Unavailable(output.stdout),
                stderr: sail_acceptance::stream_capture::PipeReadback::Unavailable(output.stderr),
            },
        }
    }
}

impl Command {
    pub(crate) fn new(program: impl AsRef<OsStr>) -> Self {
        let program = program.as_ref().to_owned();
        Self {
            inner: std::process::Command::new(&program),
            parent: ParentEnvironment::capture(),
            inherits: true,
            recorded: BTreeMap::new(),
            program,
            arguments: Vec::new(),
            directory: None,
        }
    }

    pub(crate) fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        let argument = argument.as_ref();
        self.inner.arg(argument);
        self.arguments.push(argument.to_owned());
        self
    }

    pub(crate) fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for argument in arguments {
            self.arg(argument);
        }
        self
    }

    pub(crate) fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        let directory = directory.as_ref();
        self.inner.current_dir(directory);
        self.directory = Some(directory.to_owned());
        self
    }

    pub(crate) fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(&name, &value);
        self.recorded
            .insert(name.as_ref().to_owned(), Some(value.as_ref().to_owned()));
        self
    }

    pub(crate) fn envs<I, K, V>(&mut self, values: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (name, value) in values {
            self.env(name, value);
        }
        self
    }

    pub(crate) fn env_remove(&mut self, name: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(&name);
        if self.inherits {
            self.recorded.insert(name.as_ref().to_owned(), None);
        } else {
            self.recorded.remove(name.as_ref());
        }
        self
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Production outer Cargo preserves inheritance; native controls exercise explicit clearing before other callers migrate."
        )
    )]
    pub(crate) fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self.inherits = false;
        self.recorded.clear();
        self
    }

    pub(crate) fn stdin(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(stream);
        self
    }

    pub(crate) fn stdout(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stdout(stream);
        self
    }

    pub(crate) fn stderr(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stderr(stream);
        self
    }

    /// Reconcile this owned builder's independent records with its actual native readback.
    /// No caller can supply the command, inheritance flag or observations.
    #[allow(
        clippy::too_many_lines,
        reason = "One borrowed original snapshot keeps independent builder records, actual readback and checked effective rows tied through the same capture."
    )]
    fn observe_environment(&self) -> Option<(EnvironmentObservation, ConfiguredIntent)> {
        use sail_acceptance::inputs::{InputFact, InputValue};
        use sail_acceptance::phase_inputs::{
            EnvironmentInheritance, Inventory, seal_command_environment,
        };
        let inherits = self.inherits;
        let recorded = &self.recorded;
        let command = &self.inner;
        let parent: Vec<_> = self
            .parent
            .entries()
            .iter()
            .map(|(name, value)| InputFact {
                namespace: 3,
                name: name.as_encoded_bytes(),
                value: InputValue::Present(value.as_encoded_bytes()),
            })
            .collect();
        let planned: Vec<_> = recorded
            .iter()
            .map(|(name, value)| InputFact {
                namespace: 3,
                name: name.as_encoded_bytes(),
                value: value.as_ref().map_or(InputValue::Absent, |value| {
                    InputValue::Present(value.as_encoded_bytes())
                }),
            })
            .collect();
        let actual: Vec<_> = command
            .get_envs()
            .map(|(name, value)| InputFact {
                namespace: 3,
                name: name.as_encoded_bytes(),
                value: value.map_or(InputValue::Absent, |value| {
                    InputValue::Present(value.as_encoded_bytes())
                }),
            })
            .collect();
        let mut proposed = BTreeMap::new();
        if inherits {
            for (name, value) in self.parent.entries() {
                proposed.insert(name.as_os_str(), Some(value.as_os_str()));
            }
        }
        for (name, value) in command.get_envs() {
            proposed.insert(name, value);
        }
        let result: Vec<_> = proposed
            .iter()
            .map(|(name, value)| InputFact {
                namespace: 3,
                name: name.as_encoded_bytes(),
                value: value.map_or(InputValue::Absent, |value| {
                    InputValue::Present(value.as_encoded_bytes())
                }),
            })
            .collect();
        let (parent_order, parent_inverse) = crate::parallel::proposed_fact_order(&parent);
        let (planned_order, planned_inverse) = crate::parallel::proposed_fact_order(&planned);
        let (actual_order, actual_inverse) = crate::parallel::proposed_fact_order(&actual);
        let (result_order, result_inverse) = crate::parallel::proposed_fact_order(&result);
        let policy = if inherits {
            EnvironmentInheritance::Inherit
        } else {
            EnvironmentInheritance::Clear
        };
        let admitted = seal_command_environment(
            policy,
            Inventory {
                rows: &parent,
                order: &parent_order,
                inverse: &parent_inverse,
            },
            Inventory {
                rows: &planned,
                order: &planned_order,
                inverse: &planned_inverse,
            },
            Inventory {
                rows: &actual,
                order: &actual_order,
                inverse: &actual_inverse,
            },
            Inventory {
                rows: &result,
                order: &result_order,
                inverse: &result_inverse,
            },
        )?;
        let projection = admitted.projection();
        let effective: Option<Vec<_>> = projection
            .order()
            .iter()
            .map(|index| {
                let row = projection.facts().get(*index)?;
                let value = match &row.value {
                    InputValue::Present(value) => Some(*value),
                    InputValue::Absent => None,
                    InputValue::Unreadable => return None,
                };
                Some((row.name, value))
            })
            .collect();
        let operations: Vec<_> = recorded
            .iter()
            .map(|(name, value)| {
                (
                    name.as_encoded_bytes(),
                    value.as_ref().map(|value| value.as_encoded_bytes()),
                )
            })
            .collect();
        let (recorded, actual) = self.raw_descriptors();
        let configuration =
            sail_acceptance::phase_inputs::seal_configured_intent(&recorded, &actual, &admitted)?;
        let observation = EnvironmentObservation {
            parent_sha256: self.parent.fingerprint()?,
            operations_sha256: super::files::digest(
                &serde_json::to_vec(&("sail-command-operations-v1", operations)).ok()?,
            ),
            effective_sha256: super::files::digest(
                &serde_json::to_vec(&("sail-command-effective-v1", effective?)).ok()?,
            ),
            inherits,
        };
        Some((observation, configuration))
    }

    fn raw_descriptors(&self) -> (ProcessDescriptor, ProcessDescriptor) {
        let recorded = ProcessDescriptor {
            program: self.program.as_encoded_bytes().to_vec(),
            arguments: self
                .arguments
                .iter()
                .map(|value| value.as_encoded_bytes().to_vec())
                .collect(),
            directory: self
                .directory
                .as_ref()
                .map(|path| path.as_os_str().as_encoded_bytes().to_vec()),
            overrides: self
                .recorded
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_encoded_bytes().to_vec(),
                        value
                            .as_ref()
                            .map(|value| value.as_encoded_bytes().to_vec()),
                    )
                })
                .collect(),
        };
        let actual = ProcessDescriptor {
            program: self.inner.get_program().as_encoded_bytes().to_vec(),
            arguments: self
                .inner
                .get_args()
                .map(|value| value.as_encoded_bytes().to_vec())
                .collect(),
            directory: self
                .inner
                .get_current_dir()
                .map(|path| path.as_os_str().as_encoded_bytes().to_vec()),
            overrides: self
                .inner
                .get_envs()
                .map(|(name, value)| {
                    (
                        name.as_encoded_bytes().to_vec(),
                        value.map(|value| value.as_encoded_bytes().to_vec()),
                    )
                })
                .collect(),
        };
        (recorded, actual)
    }

    #[cfg(test)]
    fn checked_descriptor(&self) -> Option<ProcessDescriptor> {
        let (recorded, actual) = self.raw_descriptors();
        sail_acceptance::phase_inputs::descriptors_equal(&recorded, &actual).then_some(recorded)
    }

    pub(crate) fn spawn(&mut self) -> std::io::Result<Spawned> {
        let (environment, configuration) = match self.observe_environment() {
            Some((environment, configuration)) => (Some(environment), Some(configuration)),
            None => (None, None),
        };
        let child = self.inner.spawn()?;
        Ok(Spawned {
            child,
            environment,
            configuration,
            waited: None,
            owner: Arc::new(()),
        })
    }

    pub(crate) fn status(&mut self) -> std::io::Result<ExitStatus> {
        self.spawn()?.wait()
    }

    pub(crate) fn output(&mut self) -> std::io::Result<Completed> {
        let environment = self
            .observe_environment()
            .map(|(environment, _configuration)| environment);
        let output = self.inner.output()?;
        // std Command::output exposes no child PID or actual pipe observations.
        // It supplies planning/fresh output, never a compiler completion token.
        Ok(Completed::unobserved(output, environment))
    }
}

impl CommandView for Command {
    fn get_program(&self) -> &OsStr {
        self.inner.get_program()
    }
    fn get_args(&self) -> impl Iterator<Item = &OsStr> {
        self.inner.get_args()
    }
    fn get_current_dir(&self) -> Option<&Path> {
        self.inner.get_current_dir()
    }
    fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.inner.get_envs()
    }
}

impl ConfigureCommand for Command {
    fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        Self::arg(self, argument)
    }
    fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        Self::env(self, name, value)
    }
}

// Existing ordinary native controls retain their original std Command setup.
// Production compiler configuration has no mutable std Command implementation.
#[cfg(test)]
impl CommandView for std::process::Command {
    fn get_program(&self) -> &OsStr {
        self.get_program()
    }
    fn get_args(&self) -> impl Iterator<Item = &OsStr> {
        self.get_args()
    }
    fn get_current_dir(&self) -> Option<&Path> {
        self.get_current_dir()
    }
    fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.get_envs()
    }
}

#[cfg(test)]
impl ConfigureCommand for std::process::Command {
    fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.arg(argument)
    }
    fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.env(name, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn owned_pipe_drain_closes_stdin_and_preserves_both_large_streams()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "cat >/dev/null; i=0; while [ \"$i\" -lt 8192 ]; do printf '0123456789abcdef'; printf 'fedcba9876543210' >&2; i=$((i + 1)); done; exit 7"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let completed = command.spawn()?.wait_with_output()?;
        assert!(matches!(
            &completed.fields.stdout,
            sail_acceptance::stream_capture::PipeReadback::Observed(_)
        ));
        assert!(matches!(
            &completed.fields.stderr,
            sail_acceptance::stream_capture::PipeReadback::Observed(_)
        ));
        let output = completed.raw_output();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, b"0123456789abcdef".repeat(8192));
        assert_eq!(output.stderr, b"fedcba9876543210".repeat(8192));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn owned_pipe_drain_preserves_descendant_output_after_original_wait()
    -> Result<(), Box<dyn std::error::Error>> {
        struct ReleaseOnDrop(PathBuf);
        impl Drop for ReleaseOnDrop {
            fn drop(&mut self) {
                // Also release the fixture descendant on an early test failure.
                let _ = std::fs::write(&self.0, []);
            }
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "sail-pipe-descendant-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        let release = directory.join("release");
        let _release_on_drop = ReleaseOnDrop(release.clone());
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "(while [ ! -f \"$1\" ]; do sleep 0.01; done; printf tail; printf late >&2) & printf head; exit 7", "sail-pipe-descendant"])
            .arg(&release)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        // The descendant cannot finish until this independently observed wait
        // has completed. No scheduling delay establishes the test's ordering.
        assert_eq!(child.wait()?.code(), Some(7));
        std::fs::write(&release, [])?;
        let completed = child.wait_with_output()?;
        assert!(matches!(
            &completed.fields.stdout,
            sail_acceptance::stream_capture::PipeReadback::Observed(_)
        ));
        assert!(matches!(
            &completed.fields.stderr,
            sail_acceptance::stream_capture::PipeReadback::Observed(_)
        ));
        let output = completed.raw_output();
        std::fs::remove_file(&release)?;
        std::fs::remove_dir(&directory)?;
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, b"headtail");
        assert_eq!(output.stderr, b"late");
        Ok(())
    }

    #[test]
    fn identical_pid_and_descriptor_do_not_replace_original_owned_child() {
        let owner = Arc::new(());
        let original = LaunchLease {
            owner: Arc::clone(&owner),
        };
        let different = LaunchLease {
            owner: Arc::new(()),
        };
        let descriptor = ProcessDescriptor {
            program: b"compiler".to_vec(),
            arguments: vec![b"source.rs".to_vec()],
            directory: None,
            overrides: Vec::new(),
        };
        use sail_acceptance::phase_inputs::owned_identity_matches;
        assert!(owned_identity_matches(
            &NativeOwnerIdentity,
            &original,
            7,
            &descriptor,
            &original,
            7,
            &descriptor
        ));
        assert!(!owned_identity_matches(
            &NativeOwnerIdentity,
            &original,
            7,
            &descriptor,
            &different,
            7,
            &descriptor
        ));
        assert!(!owned_identity_matches(
            &NativeOwnerIdentity,
            &original,
            7,
            &descriptor,
            &original,
            9,
            &descriptor
        ));
    }

    #[test]
    fn final_descriptor_preserves_order_empty_arguments_and_cwd()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::new("compiler");
        command
            .args(["a b", "", "--literal=7"])
            .current_dir("original/../directory");
        command
            .env("SAIL_COMMAND_DESCRIPTOR_VALUE", "")
            .env_remove("SAIL_COMMAND_DESCRIPTOR_REMOVED");
        let descriptor = command
            .checked_descriptor()
            .ok_or("matching descriptor refused")?;
        assert_eq!(descriptor.program, b"compiler");
        assert_eq!(
            descriptor.arguments,
            [b"a b".to_vec(), b"".to_vec(), b"--literal=7".to_vec()]
        );
        assert_eq!(
            descriptor.directory,
            Some(b"original/../directory".to_vec())
        );
        assert!(
            descriptor
                .overrides
                .contains(&(b"SAIL_COMMAND_DESCRIPTOR_VALUE".to_vec(), Some(Vec::new())))
        );
        assert!(
            descriptor
                .overrides
                .contains(&(b"SAIL_COMMAND_DESCRIPTOR_REMOVED".to_vec(), None))
        );
        Ok(())
    }

    #[test]
    fn different_actual_arguments_cwd_or_program_refuse_descriptor() {
        let mut argument = Command::new("compiler");
        argument.arg("original");
        argument.inner.arg("unrecorded");
        assert!(argument.checked_descriptor().is_none());

        let mut directory = Command::new("compiler");
        directory.current_dir("original");
        directory.inner.current_dir("different");
        assert!(directory.checked_descriptor().is_none());

        let mut program = Command::new("compiler");
        program.inner = std::process::Command::new("different");
        assert!(program.checked_descriptor().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn final_descriptor_preserves_non_unicode_argument_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::ffi::OsStringExt as _;
        let raw = vec![b'a', 0xff, b' ', b'\\'];
        let mut command = Command::new("compiler");
        command.arg(OsString::from_vec(raw.clone()));
        let descriptor = command.checked_descriptor().ok_or("raw argument refused")?;
        assert_eq!(descriptor.arguments, [raw]);
        Ok(())
    }

    #[test]
    fn owned_command_keeps_actual_environment_operations_and_output()
    -> Result<(), Box<dyn std::error::Error>> {
        const FLAG: &str = "SAIL_TEST_OWNED_COMMAND_CHILD";
        const NAME: &str = "SAIL_TEST_OWNED_COMMAND_VALUE";
        const TEST: &str = "managed::command_environment::tests::owned_command_keeps_actual_environment_operations_and_output";
        if std::env::var_os(FLAG).is_some() {
            let value = std::env::var_os(NAME).map(OsString::into_encoded_bytes);
            println!(
                "SAIL_OWNED_COMMAND_VALUE={}",
                serde_json::to_string(&value)?
            );
            return Ok(());
        }
        for clear in [false, true] {
            let mut command = Command::new(std::env::current_exe()?);
            command.env(NAME, "discarded");
            if clear {
                command.env_clear();
            }
            command
                .env_remove(NAME)
                .env(NAME, "")
                .env_remove(NAME)
                .env(NAME, "7");
            command
                .arg("--exact")
                .arg(TEST)
                .arg("--nocapture")
                .env(FLAG, "child");
            let result = command.output()?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(result.fields.stderr.bytes())
            );
            let observed = result
                .environment
                .ok_or("actual command environment withheld")?;
            let metadata = serde_json::to_value(observed)?;
            assert_eq!(metadata["inherits"], !clear);
            let stdout = String::from_utf8(result.fields.stdout.into_bytes())?;
            let actual = stdout
                .lines()
                .find_map(|line| line.strip_prefix("SAIL_OWNED_COMMAND_VALUE="))
                .ok_or("child report missing")?;
            let actual: Option<Vec<u8>> = serde_json::from_str(actual)?;
            assert_eq!(actual, Some(b"7".to_vec()));
        }
        Ok(())
    }

    #[test]
    fn disagreement_with_actual_get_envs_withholds_capture_and_preserves_fresh_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf '%s' \"$SAIL_COMMAND_ACTUAL\"; exit 7"])
            .env("SAIL_COMMAND_ACTUAL", "genuine");
        // Only this internal control can mutate the private native command
        // behind the recorded operations. Production callers have no escape.
        command.inner.env("SAIL_COMMAND_ACTUAL", "changed");
        let result = command.output()?;
        assert!(result.environment.is_none());
        assert_eq!(result.status.code(), Some(7));
        assert_eq!(result.fields.stdout.bytes(), b"changed");
        Ok(())
    }

    #[test]
    fn extra_or_missing_readback_operations_cannot_mint_a_launch_observation()
    -> Result<(), Box<dyn std::error::Error>> {
        for extra in [false, true] {
            let mut command = Command::new("/bin/sh");
            command
                .args(["-c", "exit 0"])
                .env("SAIL_COMMAND_REQUIRED", "7");
            if extra {
                command.inner.env("SAIL_COMMAND_FOREIGN", "9");
            } else {
                command.inner.env_remove("SAIL_COMMAND_REQUIRED");
            }
            let mut result = command.spawn()?;
            assert!(result.environment.is_none());
            assert!(result.child.wait()?.success());
        }
        Ok(())
    }
}
