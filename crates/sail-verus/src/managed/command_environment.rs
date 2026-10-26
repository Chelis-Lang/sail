//! Owned native command construction with explicit environment inheritance.
//!
//! Callers cannot obtain a mutable std Command or supply an already configured
//! one. Recording never changes forwarded argv, environment operations or I/O.

use super::environment::ParentEnvironment;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Child, Output, Stdio};

#[derive(serde::Serialize)]
pub(super) struct EnvironmentObservation {
    parent_sha256: String,
    operations_sha256: String,
    effective_sha256: String,
    inherits: bool,
}

pub(super) struct Command {
    inner: std::process::Command,
    parent: ParentEnvironment,
    inherits: bool,
    recorded: BTreeMap<OsString, Option<OsString>>,
}

/// Owns the exact spawned child and the optional final environment observation.
pub(super) struct Spawned {
    child: Child,
    environment: Option<EnvironmentObservation>,
}

pub(super) struct Completed {
    output: Output,
    environment: Option<EnvironmentObservation>,
}

impl Spawned {
    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }
    pub(super) fn environment(&self) -> Option<&EnvironmentObservation> {
        self.environment.as_ref()
    }
    pub(super) fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.stdout.take()
    }
    pub(super) fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }
    pub(super) fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
    pub(super) fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
    }
}

impl Completed {
    #[cfg(test)]
    pub(super) fn environment(&self) -> Option<&EnvironmentObservation> {
        self.environment.as_ref()
    }
    #[cfg(test)]
    pub(super) fn into_output(self) -> Output {
        self.output
    }
    pub(super) fn into_observed_output(self) -> Option<Output> {
        self.environment?;
        Some(self.output)
    }
}

impl Command {
    pub(super) fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            inner: std::process::Command::new(program),
            parent: ParentEnvironment::capture(),
            inherits: true,
            recorded: BTreeMap::new(),
        }
    }

    pub(super) fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(argument);
        self
    }

    pub(super) fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(arguments);
        self
    }

    pub(super) fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.inner.current_dir(directory);
        self
    }

    pub(super) fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(&name, &value);
        self.recorded
            .insert(name.as_ref().to_owned(), Some(value.as_ref().to_owned()));
        self
    }

    pub(super) fn env_remove(&mut self, name: impl AsRef<OsStr>) -> &mut Self {
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
    pub(super) fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self.inherits = false;
        self.recorded.clear();
        self
    }

    pub(super) fn stdin(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(stream);
        self
    }

    pub(super) fn stdout(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stdout(stream);
        self
    }

    pub(super) fn stderr(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.inner.stderr(stream);
        self
    }

    /// Reconcile this owned builder's independent records with its actual native readback.
    /// No caller can supply the command, inheritance flag or observations.
    #[allow(
        clippy::too_many_lines,
        reason = "One borrowed original snapshot keeps independent builder records, actual readback and checked effective rows tied through the same capture."
    )]
    fn observe_environment(&self) -> Option<EnvironmentObservation> {
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
        Some(EnvironmentObservation {
            parent_sha256: self.parent.fingerprint()?,
            operations_sha256: super::files::digest(
                &serde_json::to_vec(&("sail-command-operations-v1", operations)).ok()?,
            ),
            effective_sha256: super::files::digest(
                &serde_json::to_vec(&("sail-command-effective-v1", effective?)).ok()?,
            ),
            inherits,
        })
    }

    pub(super) fn spawn(&mut self) -> std::io::Result<Spawned> {
        let environment = self.observe_environment();
        let child = self.inner.spawn()?;
        Ok(Spawned { child, environment })
    }

    pub(super) fn output(&mut self) -> std::io::Result<Completed> {
        let environment = self.observe_environment();
        let output = self.inner.output()?;
        Ok(Completed {
            output,
            environment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                result.output.status.success(),
                "{}",
                String::from_utf8_lossy(&result.output.stderr)
            );
            let observed = result
                .environment
                .ok_or("actual command environment withheld")?;
            let metadata = serde_json::to_value(observed)?;
            assert_eq!(metadata["inherits"], !clear);
            let stdout = String::from_utf8(result.output.stdout)?;
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
        assert_eq!(result.output.status.code(), Some(7));
        assert_eq!(result.output.stdout, b"changed");
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
