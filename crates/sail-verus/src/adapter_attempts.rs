//! Low-level child observations for the source-shared attempt reducer.
//!
//! OS command/pipe/wait attribution is an explicit native adapter boundary.
//! These records do not certify compiler input closure or verifier semantics.

use crate::{Error, Result};
use sail_acceptance::authority::{
    AttemptLedger, CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt,
};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub(crate) struct Journal<'a> {
    directory: &'a Path,
    ledger: AttemptLedger<'a>,
}

impl<'a> Journal<'a> {
    pub(crate) fn new(run: &'a [u8], unit: &'a CompilerUnit, directory: &'a Path) -> Self {
        Self {
            directory,
            ledger: AttemptLedger::new(run, unit),
        }
    }

    /// Reserve and persist the slot before the caller can spawn its child.
    pub(super) fn reserve(&mut self, command: &Command) -> Result<ReservedAttempt<'a>> {
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
        let reserved = self
            .ledger
            .reserve(descriptor)
            .ok_or_else(|| error("compiler attempt counter exhausted"))?;
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
    pub(super) fn observed(&self, observed: &ObservedAttempt<'_>) -> Result<()> {
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
