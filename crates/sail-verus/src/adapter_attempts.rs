//! Low-level child observations for the source-shared attempt reducer.
//!
//! OS command/pipe/wait attribution is an explicit native adapter boundary.
//! These records do not certify compiler input closure or verifier semantics.

use crate::{Error, Result};
use sail_acceptance::authority::{
    AttemptLedger, CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt,
    RunningAttempt, UnconfiguredAttempt,
};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};
use std::sync::Arc;

pub(crate) struct Journal<'a> {
    directory: &'a Path,
    ledger: AttemptLedger<'a>,
    unavailable: Option<String>,
    owner: Arc<()>,
}

/// Exclusive native namespace lease, allocated before request paths are prepared.
/// The private owner prevents transferring a lease into a different journal.
pub(crate) struct Allocation<'a> {
    attempt: UnconfiguredAttempt<'a>,
    owner: Arc<()>,
    marker: File,
    directory: PathBuf,
}

impl<'a> Allocation<'a> {
    /// Exclusive payload namespace owned by this pre-spawn allocation.
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
    pub(crate) fn run(&self) -> &'a [u8] {
        self.attempt.run()
    }
    pub(crate) fn unit(&self) -> &'a CompilerUnit {
        self.attempt.unit()
    }
    pub(crate) fn slot(&self) -> usize {
        self.attempt.slot()
    }
}

enum Payload<'a> {
    Observed(ObservedAttempt<'a>),
    Unavailable { stdout: Vec<u8>, stderr: Vec<u8> },
}

/// Owns fresh output regardless of whether optional observation could be recorded.
pub(crate) struct CapturedChild<'a> {
    status: ExitStatus,
    payload: Payload<'a>,
    unavailable_reason: Option<String>,
    directory: Option<PathBuf>,
    predecessor: Option<Box<CapturedChild<'a>>>,
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
    pub(crate) fn selected_attempt(
        &self,
        identity: &crate::managed::TargetIdentity,
    ) -> Option<SelectedAttempt> {
        let predecessor = match &self.predecessor {
            Some(previous) => Some(previous.selected_attempt(identity)?),
            None => None,
        };
        SelectedAttempt::observed(
            self.observation()?,
            identity,
            self.directory()?,
            predecessor,
        )
    }
    pub(crate) fn with_predecessor(mut self, previous: Self) -> Self {
        self.predecessor = Some(Box::new(previous));
        self
    }
    pub(crate) fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }
    pub(crate) fn observation(&self) -> Option<&ObservedAttempt<'_>> {
        match &self.payload {
            Payload::Observed(observed) => Some(observed),
            Payload::Unavailable { .. } => None,
        }
    }
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable_reason.as_deref()
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
            unavailable_reason: None,
            directory: None,
            predecessor: None,
        }
    }

    fn unavailable_because(output: Output, reason: Option<&str>) -> Self {
        let mut captured = Self::unavailable(output);
        captured.unavailable_reason = reason.map(str::to_owned);
        captured
    }
}

impl<'a> Journal<'a> {
    pub(crate) fn new(run: &'a [u8], unit: &'a CompilerUnit, directory: &'a Path) -> Self {
        Self {
            directory,
            ledger: AttemptLedger::new(run, unit),
            unavailable: None,
            owner: Arc::new(()),
        }
    }

    /// Reserve and persist the slot before the caller can spawn its child.
    #[cfg(test)]
    pub(super) fn reserve(&mut self, command: &Command) -> Option<ReservedAttempt<'a>> {
        let allocation = self.allocate()?;
        self.configure(allocation, command)
    }

    /// Allocate request paths before sealing the final command arguments.
    /// Exclusive markers coordinate separate driver invocations in the same run.
    pub(crate) fn allocate(&mut self) -> Option<Allocation<'a>> {
        if self.unavailable.is_some() {
            return None;
        }
        match self.try_allocate() {
            Ok(allocation) => Some(allocation),
            Err(failure) => {
                self.refuse(&failure.to_string());
                None
            }
        }
    }

    fn try_allocate(&mut self) -> Result<Allocation<'a>> {
        // Observation is optional: an occupied or hostile directory must not make
        // the fresh compiler wait without bound. Exhaustion withholds authority.
        const MAX_COLLISIONS: usize = 1024;
        std::fs::create_dir_all(self.directory).map_err(|failure| error(failure.to_string()))?;
        for _ in 0..MAX_COLLISIONS {
            let attempt = self
                .ledger
                .allocate()
                .ok_or_else(|| error("compiler attempt counter exhausted"))?;
            if self.legacy_slot_occupied(attempt.slot())? {
                continue;
            }
            let path = self.directory.join(format!("{}.allocated", attempt.slot()));
            let marker = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(marker) => marker,
                Err(failure) if failure.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(failure) => {
                    return Err(error(format!(
                        "cannot allocate compiler attempt {}: {failure}",
                        path.display()
                    )));
                }
            };
            // A payload directory is never reused, even when a failed or
            // legacy producer left one without its allocation marker.
            let directory = self.directory.join(attempt.slot().to_string());
            match std::fs::create_dir(&directory) {
                Ok(()) => (),
                Err(failure) if failure.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(failure) => {
                    return Err(error(format!(
                        "cannot create compiler payload namespace {}: {failure}",
                        directory.display()
                    )));
                }
            }
            let mut allocation = Allocation {
                attempt,
                owner: Arc::clone(&self.owner),
                marker,
                directory,
            };
            // Preserve pre-marker histories too; never reinterpret an old closed
            // slot as a fresh launch. A concurrent legacy writer fails safe.
            if self.legacy_slot_occupied(allocation.slot())? {
                continue;
            }
            let unit = allocation.unit();
            let bytes = serde_json::to_vec(&serde_json::json!({
                "version": 1, "run": allocation.run(), "slot": allocation.slot(),
                "unit": {"manifest":unit.manifest,"name":unit.name,
                    "crate_type":unit.crate_type,"cargo_name":unit.cargo_name},
            }))
            .map_err(|failure| error(failure.to_string()))?;
            allocation
                .marker
                .write_all(&bytes)
                .and_then(|()| allocation.marker.sync_data())
                .map_err(|failure| {
                    error(format!(
                        "cannot seal compiler allocation {}: {failure}",
                        path.display()
                    ))
                })?;
            return Ok(allocation);
        }
        Err(error("compiler attempt namespace collision limit reached"))
    }

    fn legacy_slot_occupied(&self, slot: usize) -> Result<bool> {
        for suffix in ["reserved", "observed"] {
            let path = self.directory.join(format!("{slot}.{suffix}"));
            match std::fs::symlink_metadata(&path) {
                Ok(_) => return Ok(true),
                Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => (),
                Err(failure) => {
                    return Err(error(format!(
                        "cannot inspect compiler attempt {}: {failure}",
                        path.display()
                    )));
                }
            }
        }
        Ok(false)
    }

    /// Observe and persist the final command after request preparation.
    pub(crate) fn configure(
        &mut self,
        allocation: Allocation<'a>,
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

    pub(crate) fn started(
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
    pub(crate) fn capture(
        &mut self,
        running: Option<RunningAttempt<'a>>,
        output: Output,
    ) -> CapturedChild<'a> {
        if self.unavailable.is_some() {
            return CapturedChild::unavailable_because(output, self.unavailable.as_deref());
        }
        let Some(running) = running else {
            return CapturedChild::unavailable_because(
                output,
                Some("no started process observation"),
            );
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
                    CapturedChild::unavailable_because(
                        Output {
                            stdout,
                            stderr,
                            status,
                        },
                        self.unavailable.as_deref(),
                    )
                } else {
                    CapturedChild {
                        status,
                        directory: Some(
                            self.directory.join(observed.reserved().slot().to_string()),
                        ),
                        payload: Payload::Observed(observed),
                        unavailable_reason: None,
                        predecessor: None,
                    }
                }
            }
            Err(output) => {
                self.unavailable = Some("ambiguous compiler observations".into());
                CapturedChild::unavailable_because(
                    Output {
                        stdout: output.stdout,
                        stderr: output.stderr,
                        status,
                    },
                    self.unavailable.as_deref(),
                )
            }
        }
    }

    fn try_configure(
        &self,
        allocation: Allocation<'a>,
        command: &Command,
    ) -> Result<ReservedAttempt<'a>> {
        if !Arc::ptr_eq(&allocation.owner, &self.owner) {
            return Err(error("compiler allocation belongs to another journal"));
        }
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
        let reserved = allocation.attempt.configure(descriptor);
        let bytes = serde_json::to_vec(&ReservedRecord::of(&reserved))
            .map_err(|failure| error(failure.to_string()))?;
        std::fs::create_dir_all(self.directory).map_err(|failure| error(failure.to_string()))?;
        let path = self.directory.join(format!("{}.reserved", reserved.slot()));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_data()))
            .map_err(|failure| {
                error(format!(
                    "cannot reserve compiler attempt {}: {failure}",
                    path.display()
                ))
            })?;
        Ok(reserved)
    }

    /// Persist exact payloads before the terminal record. Neither payload can
    /// be overwritten by another attempt. Native protected-writer and filesystem
    /// semantics remain separate from the source-shared observation reducer.
    fn persist_observed(&self, observed: &ObservedAttempt<'_>) -> Result<()> {
        let directory = self.directory.join(observed.reserved().slot().to_string());
        persist_new(&directory.join("verus.stdout"), observed.stdout())?;
        persist_new(&directory.join("verus.stderr"), observed.stderr())?;
        let bytes = serde_json::to_vec(&TerminalRecord::of(observed))
            .map_err(|failure| error(failure.to_string()))?;
        let path = self
            .directory
            .join(format!("{}.observed", observed.reserved().slot()));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_data()))
            .map_err(|failure| {
                error(format!(
                    "cannot record compiler observation {}: {failure}",
                    path.display()
                ))
            })
    }
}

fn persist_new(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_data()))
        .map_err(|failure| {
            error(format!(
                "cannot persist compiler payload {}: {failure}",
                path.display()
            ))
        })
}

fn error(message: impl Into<String>) -> Error {
    Error::new("SAIL3111", message)
}

/// Persisted identity of one actual observed child. Decoding is untrusted: every
/// consumer joins this record to the independently supplied run/unit and reads
/// the exact reserved/terminal/payload namespace. It never creates a live seal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedAttempt {
    key: String,
    reserved: ReservedRecord,
    terminal: TerminalRecord,
    files: std::collections::BTreeMap<String, String>,
    image_sha256: String,
    predecessor: Option<Box<Self>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitRecord {
    manifest: Vec<u8>,
    name: Vec<u8>,
    crate_type: Vec<u8>,
    cargo_name: Option<Vec<u8>>,
}

impl UnitRecord {
    fn of(unit: &CompilerUnit) -> Self {
        Self {
            manifest: unit.manifest.clone(),
            name: unit.name.clone(),
            crate_type: unit.crate_type.clone(),
            cargo_name: unit.cargo_name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorRecord {
    program: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    directory: Option<Vec<u8>>,
    overrides: Vec<(Vec<u8>, Option<Vec<u8>>)>,
}

impl DescriptorRecord {
    fn of(value: &ProcessDescriptor) -> Self {
        Self {
            program: value.program.clone(),
            arguments: value.arguments.clone(),
            directory: value.directory.clone(),
            overrides: value.overrides.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservedRecord {
    version: u32,
    run: Vec<u8>,
    slot: usize,
    unit: UnitRecord,
    command: DescriptorRecord,
}

impl ReservedRecord {
    fn of(value: &ReservedAttempt<'_>) -> Self {
        Self {
            version: 1,
            run: value.run().to_vec(),
            slot: value.slot(),
            unit: UnitRecord::of(value.unit()),
            command: DescriptorRecord::of(value.descriptor()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalRecord {
    version: u32,
    run: Vec<u8>,
    slot: usize,
    pid: u32,
    exit: Option<i32>,
    stdout_sha256: String,
    stderr_sha256: String,
}

impl TerminalRecord {
    fn of(value: &ObservedAttempt<'_>) -> Self {
        Self {
            version: 1,
            run: value.reserved().run().to_vec(),
            slot: value.reserved().slot(),
            pid: value.pid(),
            exit: value.exit(),
            stdout_sha256: crate::managed::digest(value.stdout()),
            stderr_sha256: crate::managed::digest(value.stderr()),
        }
    }
}

impl SelectedAttempt {
    /// Only the actual complete observation supplies a new producer identity.
    /// Protection of the directory and the OS image/pipe/wait joins remain native
    /// obligations; neither an archive nor a normal exit establishes a proof.
    pub(crate) fn observed(
        value: &ObservedAttempt<'_>,
        identity: &crate::managed::TargetIdentity,
        directory: &Path,
        predecessor: Option<Self>,
    ) -> Option<Self> {
        let origin = sail_acceptance::authority::AttemptIdentity::from_observed(value);
        let reserved = ReservedRecord {
            version: 1,
            run: origin.run().to_vec(),
            slot: origin.slot(),
            unit: UnitRecord::of(origin.unit()),
            command: DescriptorRecord::of(origin.descriptor()),
        };
        if reserved.unit != UnitRecord::of(&identity.compiler_unit()) {
            return None;
        }
        let terminal = TerminalRecord {
            version: 1,
            run: origin.run().to_vec(),
            slot: origin.slot(),
            pid: origin.pid(),
            exit: origin.exit(),
            stdout_sha256: crate::managed::digest(value.stdout()),
            stderr_sha256: crate::managed::digest(value.stderr()),
        };
        if let Some(previous) = &predecessor
            && (previous.reserved.run != reserved.run
                || previous.reserved.unit != reserved.unit
                || previous.reserved.slot >= reserved.slot)
        {
            return None;
        }
        let image = Path::new(std::str::from_utf8(&reserved.command.program).ok()?);
        if !image.is_absolute() {
            return None;
        }
        let image_sha256 = crate::managed::file_digest(image).ok()?;
        let files = selected_files(directory)?;
        let candidate = Self {
            key: identity.key(),
            reserved,
            terminal,
            files,
            image_sha256,
            predecessor: predecessor.map(Box::new),
        };
        candidate
            .records_match(directory.parent()?)?
            .then_some(candidate)
    }

    /// Derive the namespace from independent run/target identity. A path stored
    /// inside a result is never allowed to choose the files that justify it.
    pub(crate) fn directory(
        &self,
        run: &Path,
        identity: &crate::managed::TargetIdentity,
        exit: Option<i32>,
    ) -> Option<PathBuf> {
        self.validate_directory(run, identity, exit, true)
    }

    fn validate_directory(
        &self,
        run: &Path,
        identity: &crate::managed::TargetIdentity,
        exit: Option<i32>,
        current: bool,
    ) -> Option<PathBuf> {
        if self.key != identity.key()
            || self.reserved.version != 1
            || self.terminal.version != 1
            || self.reserved.run != run.file_name()?.as_encoded_bytes()
            || self.reserved.unit != UnitRecord::of(&identity.compiler_unit())
            || self.terminal.run != self.reserved.run
            || self.terminal.slot != self.reserved.slot
            || self.terminal.pid == 0
            || self.terminal.exit != exit
        {
            return None;
        }
        let attempts = run.join("sidecars").join(&self.key).join("attempts");
        if !self.records_match(&attempts)?
            || current && latest_reservation(&attempts)? != self.reserved.slot
        {
            return None;
        }
        if let Some(previous) = &self.predecessor
            && (previous.reserved.run != self.reserved.run
                || previous.reserved.unit != self.reserved.unit
                || previous.reserved.slot >= self.reserved.slot
                || previous
                    .validate_directory(run, identity, previous.terminal.exit, false)
                    .is_none())
        {
            return None;
        }
        Some(attempts.join(self.reserved.slot.to_string()))
    }

    pub(crate) fn matches_observed(&self, value: &ObservedAttempt<'_>) -> bool {
        self.reserved == ReservedRecord::of(value.reserved())
            && self.terminal == TerminalRecord::of(value)
    }

    pub(crate) fn slot(&self) -> usize {
        self.reserved.slot
    }

    /// Validate copied history as an archive. This never returns a live process
    /// observation or gives archived events a new run/slot/PID.
    pub(crate) fn archive_matches(
        &self,
        attempts: &Path,
        identity: &crate::managed::TargetIdentity,
        run: &str,
        exit: Option<i32>,
    ) -> bool {
        let check = || -> Option<bool> {
            if self.reserved.version != 1
                || self.terminal.version != 1
                || self.key != identity.key()
                || self.reserved.run != run.as_bytes()
                || self.reserved.unit != UnitRecord::of(&identity.compiler_unit())
                || self.terminal.run != self.reserved.run
                || self.terminal.slot != self.reserved.slot
                || self.terminal.exit != exit
                || self.terminal.pid == 0
                || latest_reservation(attempts)? != self.reserved.slot
                || !self.records_match(attempts)?
            {
                return Some(false);
            }
            let mut previous = self.predecessor.as_deref();
            let mut slot = self.reserved.slot;
            while let Some(history) = previous {
                if history.reserved.version != 1
                    || history.terminal.version != 1
                    || history.key != self.key
                    || history.reserved.run != self.reserved.run
                    || history.reserved.unit != self.reserved.unit
                    || history.reserved.slot >= slot
                    || history.terminal.run != history.reserved.run
                    || history.terminal.slot != history.reserved.slot
                    || history.terminal.pid == 0
                    || !history.records_match(attempts)?
                {
                    return Some(false);
                }
                slot = history.reserved.slot;
                previous = history.predecessor.as_deref();
            }
            Some(true)
        };
        check() == Some(true)
    }

    fn records_match(&self, attempts: &Path) -> Option<bool> {
        let reserved: ReservedRecord =
            read_selected(&attempts.join(format!("{}.reserved", self.reserved.slot)))?;
        let terminal: TerminalRecord =
            read_selected(&attempts.join(format!("{}.observed", self.reserved.slot)))?;
        let directory = attempts.join(self.reserved.slot.to_string());
        let stdout = bounded_selected(&directory.join("verus.stdout"))?;
        let stderr = bounded_selected(&directory.join("verus.stderr"))?;
        let image = std::str::from_utf8(&self.reserved.command.program).ok()?;
        Some(
            reserved == self.reserved
                && terminal == self.terminal
                && terminal.stdout_sha256 == crate::managed::digest(&stdout)
                && terminal.stderr_sha256 == crate::managed::digest(&stderr)
                && crate::managed::file_digest(Path::new(image)).ok()? == self.image_sha256
                && selected_files(&directory)? == self.files,
        )
    }
}

fn latest_reservation(directory: &Path) -> Option<usize> {
    let mut latest = None;
    for entry in std::fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_str()?;
        if let Some(slot) = name.strip_suffix(".reserved") {
            let parsed: usize = slot.parse().ok()?;
            if parsed.to_string() != slot {
                return None;
            }
            let slot = parsed;
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            latest = Some(latest.map_or(slot, |previous: usize| previous.max(slot)));
        }
    }
    latest
}

fn read_selected<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&bounded_selected(path)?).ok()
}

fn bounded_selected(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 32 * 1024 * 1024).then_some(bytes)
}

fn selected_files(directory: &Path) -> Option<std::collections::BTreeMap<String, String>> {
    fn visit(
        root: &Path,
        directory: &Path,
        out: &mut std::collections::BTreeMap<String, String>,
    ) -> Option<()> {
        if !std::fs::symlink_metadata(directory)
            .ok()?
            .file_type()
            .is_dir()
        {
            return None;
        }
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                visit(root, &entry.path(), out)?;
            } else if kind.is_file() {
                let path = entry.path();
                let relative = path.strip_prefix(root).ok()?.to_str()?.to_owned();
                if out.len() >= 65_536 {
                    return None;
                }
                out.insert(relative, crate::managed::file_digest(&path).ok()?);
            } else {
                return None;
            }
        }
        Some(())
    }
    let mut files = std::collections::BTreeMap::new();
    for name in [
        "verus.stdout",
        "verus.stderr",
        "engine.events.jsonl",
        "focus.source.json",
        "focus.report.json",
        "engine.missing-feature",
        "launcher-options.json",
    ] {
        let path = directory.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                files.insert(name.to_owned(), crate::managed::file_digest(&path).ok()?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Ok(_) | Err(_) => return None,
        }
    }
    for name in ["logs", "provenance"] {
        let path = directory.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_dir() => visit(directory, &path, &mut files)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Ok(_) | Err(_) => return None,
        }
    }
    Some(files)
}

/// Native fixture records actual shell output/wait without claiming a trusted
/// Verus image. The production constructors and persistence checks are unchanged.
#[cfg(test)]
pub(crate) fn fixture_history(
    run: &Path,
    run_id: &str,
    identity: &crate::managed::TargetIdentity,
    stdout: &[u8],
    stderr: &[u8],
    exit: i32,
) -> Option<SelectedAttempt> {
    let unit = identity.compiler_unit();
    let attempts = run.join("sidecars").join(identity.key()).join("attempts");
    let mut journal = Journal::new(run_id.as_bytes(), &unit, &attempts);
    let allocation = journal.allocate()?;
    let directory = allocation.directory().to_owned();
    let out = directory.join("fixture.stdout");
    let err = directory.join("fixture.stderr");
    std::fs::write(&out, stdout).ok()?;
    std::fs::write(&err, stderr).ok()?;
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "cat \"$1\"; cat \"$2\" >&2; exit \"$3\"", "fixture"])
        .arg(out)
        .arg(err)
        .arg(exit.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let reserved = journal.configure(allocation, &command);
    let child = command.spawn().ok()?;
    let running = journal.started(reserved, &child);
    let output = child.wait_with_output().ok()?;
    let captured = journal.capture(running, output);
    captured.selected_attempt(identity)
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
    fn separate_journals_allocate_fresh_slots_and_preserve_prior_history() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-repeated-attempt-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        // Pre-marker versions may have either pre-spawn or terminal history.
        std::fs::write(root.join("0.reserved"), b"original reservation")?;
        std::fs::write(root.join("1.observed"), b"original terminal")?;
        let unit = CompilerUnit {
            manifest: b"Cargo.toml".to_vec(),
            name: b"demo".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let mut old_history = Vec::new();
        for expected in [2, 3] {
            // A legitimate managed retry invokes another driver with a new local
            // ledger, but the independent native namespace persists in this run.
            let mut journal = Journal::new(b"same-run", &unit, &root);
            let mut command = Command::new("/bin/sh");
            command
                .args([
                    "-c",
                    "printf 'same output'; printf 'same diagnostic' >&2; exit 7",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let reserved = journal.reserve(&command).ok_or("reservation refused")?;
            assert_eq!(reserved.slot(), expected);
            let child = command.spawn()?;
            let running = journal.started(Some(reserved), &child);
            let captured = journal.capture(running, child.wait_with_output()?);
            assert_eq!(captured.stdout(), b"same output");
            assert_eq!(captured.stderr(), b"same diagnostic");
            assert_eq!(captured.status().code(), Some(7));
            assert_eq!(
                captured
                    .observation()
                    .ok_or("observation refused")?
                    .reserved()
                    .slot(),
                expected
            );
            if expected == 2 {
                old_history = std::fs::read(root.join("2.observed"))?;
            }
        }
        assert_eq!(
            std::fs::read(root.join("0.reserved"))?,
            b"original reservation"
        );
        assert_eq!(
            std::fs::read(root.join("1.observed"))?,
            b"original terminal"
        );
        assert_eq!(std::fs::read(root.join("2.observed"))?, old_history);
        assert!(root.join("2.allocated").is_file());
        assert!(root.join("3.allocated").is_file());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn allocation_from_another_journal_cannot_configure_a_child() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-cross-owner-attempt-{}", std::process::id()));
        let unit = CompilerUnit {
            manifest: b"Cargo.toml".to_vec(),
            name: b"demo".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let mut first = Journal::new(b"same-run", &unit, &root);
        let allocation = first.allocate().ok_or("allocation refused")?;
        let mut second = Journal::new(b"same-run", &unit, &root);
        let command = Command::new("/bin/sh");
        assert!(second.configure(allocation, &command).is_none());
        assert!(
            second
                .unavailable
                .as_deref()
                .is_some_and(|reason| reason.contains("another journal"))
        );
        assert!(!root.join("0.reserved").exists());
        // Refusal leaves the exclusive marker occupied rather than recycling it.
        let replacement = first.reserve(&command).ok_or("fresh slot refused")?;
        assert_eq!(replacement.slot(), 1);
        assert!(root.join("0.allocated").is_file());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn selected_history_refuses_stale_subject_payload_and_later_unfinished_attempt() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-selected-history-{}", std::process::id()));
        let run = root.join("run");
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            cargo_name: None,
            rustc_args_sha256: "original-arguments".into(),
        };
        let selected = fixture_history(
            &run,
            "run",
            &identity,
            b"original stdout",
            b"original stderr",
            0,
        )
        .ok_or("fixture history missing")?;
        let directory = selected
            .directory(&run, &identity, Some(0))
            .ok_or("original admission refused")?;
        let restored: SelectedAttempt = serde_json::from_slice(&serde_json::to_vec(&selected)?)?;
        assert_eq!(restored, selected);
        assert!(restored.directory(&run, &identity, Some(0)).is_some());
        let mut swapped = identity.clone();
        swapped.rustc_args_sha256 = "different-arguments".into();
        assert!(restored.directory(&run, &swapped, Some(0)).is_none());
        assert!(
            restored
                .directory(&root.join("other-run"), &identity, Some(0))
                .is_none()
        );
        assert!(restored.directory(&run, &identity, Some(1)).is_none());
        let mut wrong_pid = restored.clone();
        wrong_pid.terminal.pid = wrong_pid.terminal.pid.saturating_add(1);
        assert!(wrong_pid.directory(&run, &identity, Some(0)).is_none());
        let mut wrong_command = restored.clone();
        wrong_command
            .reserved
            .command
            .arguments
            .push(b"extra option".to_vec());
        assert!(wrong_command.directory(&run, &identity, Some(0)).is_none());
        std::fs::write(
            directory.join("verus.stdout"),
            b"different valid-looking stdout",
        )?;
        assert!(restored.directory(&run, &identity, Some(0)).is_none());
        std::fs::write(directory.join("verus.stdout"), b"original stdout")?;
        assert!(restored.directory(&run, &identity, Some(0)).is_some());
        // A later configured child makes the earlier selection noncurrent even
        // if the latest invocation JSON was never overwritten by completion.
        let unit = identity.compiler_unit();
        let attempts = directory.parent().ok_or("attempt root")?;
        let mut next = Journal::new(b"run", &unit, attempts);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        let reserved = next.reserve(&command).ok_or("new reservation refused")?;
        assert!(reserved.slot() > selected.slot());
        assert!(restored.directory(&run, &identity, Some(0)).is_none());
        assert_eq!(
            std::fs::read(directory.join("verus.stdout"))?,
            b"original stdout"
        );
        assert!(
            attempts
                .join(format!("{}.observed", selected.slot()))
                .is_file()
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn archived_selected_and_predecessor_records_refuse_unknown_schema_versions() -> TestResult {
        let root = std::env::temp_dir().join(format!("sail-history-schema-{}", std::process::id()));
        let run = root.join("run");
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            cargo_name: None,
            rustc_args_sha256: "args".into(),
        };
        let first =
            fixture_history(&run, "run", &identity, b"original", b"", 0).ok_or("first history")?;
        let mut latest = fixture_history(&run, "run", &identity, b"independent", b"", 0)
            .ok_or("latest history")?;
        latest.predecessor = Some(Box::new(first));
        let attempts = run.join("sidecars").join(identity.key()).join("attempts");
        assert!(latest.archive_matches(&attempts, &identity, "run", Some(0)));
        for predecessor in [false, true] {
            for reserved in [false, true] {
                for version in [0, 2] {
                    let mut candidate: SelectedAttempt =
                        serde_json::from_slice(&serde_json::to_vec(&latest)?)?;
                    let history = if predecessor {
                        candidate
                            .predecessor
                            .as_mut()
                            .ok_or("missing predecessor")?
                    } else {
                        &mut candidate
                    };
                    let path = if reserved {
                        history.reserved.version = version;
                        attempts.join(format!("{}.reserved", history.reserved.slot))
                    } else {
                        history.terminal.version = version;
                        attempts.join(format!("{}.observed", history.reserved.slot))
                    };
                    let original = std::fs::read(&path)?;
                    let encoded = if reserved {
                        serde_json::to_vec(&history.reserved)?
                    } else {
                        serde_json::to_vec(&history.terminal)?
                    };
                    // Both decoded candidate and actual archived record agree;
                    // equality/checksums cannot substitute for supported schema.
                    std::fs::write(&path, encoded)?;
                    assert!(!candidate.archive_matches(&attempts, &identity, "run", Some(0)));
                    assert!(candidate.directory(&run, &identity, Some(0)).is_none());
                    std::fs::write(&path, original)?;
                    assert!(latest.archive_matches(&attempts, &identity, "run", Some(0)));
                }
            }
        }
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
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            cargo_name: None,
            rustc_args_sha256: "args".into(),
        };
        let unit = identity.compiler_unit();
        let sidecar = root.join("sidecars").join(identity.key());
        std::fs::create_dir_all(&sidecar)?;
        let directory = sidecar.join("attempts");
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
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            cargo_name: None,
            rustc_args_sha256: "args".into(),
        };
        let presentation = root.join("sidecars").join(identity.key());
        let sidecar = captured.directory().unwrap_or(&presentation);
        std::fs::create_dir_all(sidecar)?;
        let selected_attempt = captured.selected_attempt(&identity);
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
        let key = identity.key();
        crate::adapter::finish(
            &runtime,
            identity,
            sidecar,
            &Captured {
                stdout: captured.stdout(),
                stderr: captured.stderr(),
                exit: captured.status().code(),
            },
            None,
            None,
            CompletionOrigin::Live(
                captured.observation(),
                captured.unavailable_reason(),
                selected_attempt.as_ref(),
            ),
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
            let reason = captured
                .unavailable_reason()
                .ok_or("lost journal failure reason")?;
            assert!(
                invocation
                    .report
                    .error
                    .as_ref()
                    .ok_or("missing refusal")?
                    .message
                    .contains(reason)
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
