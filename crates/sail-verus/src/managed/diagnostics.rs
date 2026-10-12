//! Opt-in bounded setup timings. These records never authorize a proof or reuse.

use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const MAX_FILES: u64 = 64;
const MAX_ROWS: usize = 64;
const MAX_BYTES: usize = 16 * 1024;

fn event_destination(path: Option<std::ffi::OsString>) -> Option<std::ffi::OsString> {
    path.filter(|path| !path.is_empty())
}

fn file_slot(serial: &AtomicU64) -> Option<u64> {
    serial
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            (value < MAX_FILES).then(|| value + 1)
        })
        .ok()
}

/// Fixed labels exclude paths, configuration, source and environment contents.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Phase {
    Metadata,
    RuntimeContext,
    RuntimeBinaries,
    HelperIdentity,
    ToolchainProbes,
    SysrootGuard,
    #[cfg(target_os = "linux")]
    SysrootHandoff,
    SysrootHash,
    SysrootBarrier,
    #[cfg(target_os = "linux")]
    SysrootFillHint,
    ContextConfiguration,
    OwnedPaths,
    InitialSnapshot,
    InputGuards,
    MonitoredSnapshot,
    InputBarrier,
    TargetCleanup,
    InvocationSeal,
    ResultPublication,
    ReceiptPublication,
    CompilerFamilyPublication,
}

#[derive(serde::Serialize)]
struct Record {
    kind: &'static str,
    phase: Phase,
    pid: u32,
    started_ms: f64,
    elapsed_ms: f64,
}

/// Fixed bootstrap stage labels carry no package, path or environment data.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BootstrapStage {
    OwnerEligibility,
    #[cfg(target_os = "linux")]
    OwnerEnvironment,
    #[cfg(target_os = "linux")]
    OwnerRoots,
    #[cfg(target_os = "linux")]
    OwnerObserver,
    #[cfg(target_os = "linux")]
    OwnerLease,
    #[cfg(target_os = "linux")]
    OwnerRetained,
    #[cfg(target_os = "linux")]
    OwnerHashes,
    #[cfg(target_os = "linux")]
    OwnerConfiguration,
    #[cfg(target_os = "linux")]
    OwnerBarriers,
    #[cfg(target_os = "linux")]
    OwnerLedger,
    #[cfg(any(target_os = "linux", test))]
    OwnerComplete,
    InitialRuntime,
    FinalRuntime,
    PublishLedger,
    #[cfg(any(target_os = "linux", test))]
    PublishJournal,
    #[cfg(target_os = "linux")]
    PublishRecordSet,
    #[cfg(target_os = "linux")]
    PublishCancellation,
    #[cfg(target_os = "linux")]
    PublishRecordBudget,
    #[cfg(target_os = "linux")]
    RecordBinding,
    #[cfg(target_os = "linux")]
    RecordPackage,
    #[cfg(target_os = "linux")]
    RecordDeclarations,
    #[cfg(target_os = "linux")]
    RecordDigest,
    #[cfg(target_os = "linux")]
    RecordEnvironment,
    #[cfg(target_os = "linux")]
    RecordExecution,
    #[cfg(target_os = "linux")]
    RecordCompilation,
    #[cfg(target_os = "linux")]
    RecordSource,
    #[cfg(target_os = "linux")]
    RecordBinary,
    #[cfg(target_os = "linux")]
    RecordAliases,
    #[cfg(target_os = "linux")]
    RecordDepInfo,
    #[cfg(target_os = "linux")]
    RecordInputs,
    #[cfg(target_os = "linux")]
    RecordCompilerEnvironment,
    #[cfg(target_os = "linux")]
    PublishHashes,
    #[cfg(target_os = "linux")]
    PublishOutputs,
    #[cfg(target_os = "linux")]
    PublishEnvironment,
    #[cfg(target_os = "linux")]
    PublishObserver,
    #[cfg(target_os = "linux")]
    PublishLease,
    #[cfg(any(target_os = "linux", test))]
    PublishEmission,
}

/// An observation never grants authority; emission is only an attempted write.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BootstrapOutcome {
    Ready,
    Refused,
    Skipped,
    #[cfg(any(target_os = "linux", test))]
    Attempted,
}

#[derive(serde::Serialize)]
struct BootstrapRecord {
    kind: &'static str,
    stage: BootstrapStage,
    outcome: BootstrapOutcome,
    pid: u32,
}

/// Record only the stage at which an attempt ended, including early returns.
pub(super) struct BootstrapAttempt<'a> {
    diagnostics: Option<&'a Diagnostics>,
    stage: BootstrapStage,
    outcome: BootstrapOutcome,
}

impl<'a> BootstrapAttempt<'a> {
    pub(super) fn new(diagnostics: Option<&'a Diagnostics>, stage: BootstrapStage) -> Self {
        Self {
            diagnostics,
            stage,
            outcome: BootstrapOutcome::Refused,
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn stage(&mut self, stage: BootstrapStage) {
        self.stage = stage;
    }

    #[cfg(target_os = "linux")]
    pub(super) fn outcome(&mut self, outcome: BootstrapOutcome) {
        self.outcome = outcome;
    }
}

impl Drop for BootstrapAttempt<'_> {
    fn drop(&mut self) {
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.bootstrap(self.stage, self.outcome);
        }
    }
}

struct Sink {
    file: File,
    bytes: usize,
    rows: usize,
}

#[derive(Default)]
pub(super) struct Diagnostics {
    sink: RefCell<Option<Sink>>,
    origin: Option<Instant>,
}

impl Diagnostics {
    pub(super) fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let Some(path) = event_destination(std::env::var_os("SAIL_JOB_EVENTS")) else {
            return Self::default();
        };
        let Some(serial) = file_slot(&SERIAL) else {
            return Self::default();
        };
        Self::create_at(
            &Path::new(&path)
                .with_extension(format!("setup-{}-{serial}.jsonl", std::process::id())),
        )
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::managed) fn for_test(path: &Path) -> Self {
        Self::create_at(path)
    }

    fn create_at(path: &Path) -> Self {
        let mut options = OpenOptions::new();
        // create_new refuses existing files, symlinks and special files. Never
        // open a caller-substituted FIFO or follow an existing diagnostic alias.
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let Ok(file) = options.open(path) else {
            return Self::default();
        };
        Self {
            sink: RefCell::new(Some(Sink {
                file,
                bytes: 0,
                rows: 0,
            })),
            origin: Some(Instant::now()),
        }
    }

    pub(super) fn phase(&self, phase: Phase) -> Timer<'_> {
        Timer {
            diagnostics: self,
            phase,
            started: self.origin.map(|_| Instant::now()),
        }
    }

    fn record(&self, phase: Phase, started: Instant) {
        let Some(origin) = self.origin else {
            return;
        };
        self.emit(|| {
            serde_json::to_vec(&Record {
                kind: "managed-setup-phase",
                phase,
                pid: std::process::id(),
                started_ms: started.duration_since(origin).as_secs_f64() * 1000.0,
                elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            })
        });
    }

    pub(super) fn bootstrap(&self, stage: BootstrapStage, outcome: BootstrapOutcome) {
        self.emit(|| {
            serde_json::to_vec(&BootstrapRecord {
                kind: "managed-script-bootstrap",
                stage,
                outcome,
                pid: std::process::id(),
            })
        });
    }

    fn emit(&self, encode: impl FnOnce() -> serde_json::Result<Vec<u8>>) {
        let Ok(mut slot) = self.sink.try_borrow_mut() else {
            return;
        };
        let Some(sink) = slot.as_mut() else {
            return;
        };
        let Ok(mut row) = encode() else {
            *slot = None;
            return;
        };
        row.push(b'\n');
        if sink.rows >= MAX_ROWS || row.len() > MAX_BYTES.saturating_sub(sink.bytes) {
            *slot = None;
            return;
        }
        if sink.file.write_all(&row).is_err() {
            *slot = None;
            return;
        }
        sink.rows += 1;
        sink.bytes += row.len();
    }
}

pub(super) struct Timer<'a> {
    diagnostics: &'a Diagnostics,
    phase: Phase,
    started: Option<Instant>,
}

impl Drop for Timer<'_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            self.diagnostics.record(self.phase, started);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn fixture() -> std::io::Result<std::path::PathBuf> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "sail-setup-timing-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(path)
    }

    fn failing_step(diagnostics: &Diagnostics) -> std::io::Result<()> {
        let _span = diagnostics.phase(Phase::Metadata);
        Err(std::io::Error::other("original setup error"))
    }

    #[test]
    fn nested_completed_and_error_exit_spans_record_only_fixed_stage_facts() -> TestResult {
        let root = fixture()?;
        let path = root.join("timing");
        let diagnostics = Diagnostics::create_at(&path);
        {
            let _outer = diagnostics.phase(Phase::RuntimeContext);
            let _inner = diagnostics.phase(Phase::RuntimeBinaries);
        }
        assert!(failing_step(&diagnostics).is_err());
        let rows = std::fs::read_to_string(&path)?;
        let rows: Vec<serde_json::Value> = rows
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["phase"], "runtime-binaries");
        assert_eq!(rows[1]["phase"], "runtime-context");
        assert_eq!(rows[2]["phase"], "metadata");
        for row in rows {
            assert_eq!(row.as_object().map(serde_json::Map::len), Some(5));
            assert_eq!(row["kind"], "managed-setup-phase");
            assert!(row["elapsed_ms"].as_f64().is_some_and(|v| v >= 0.0));
            assert!(row["started_ms"].as_f64().is_some_and(|v| v >= 0.0));
        }
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn publication_phases_keep_fixed_schema_and_original_failure() -> TestResult {
        let root = fixture()?;
        let path = root.join("publication");
        let diagnostics = Diagnostics::create_at(&path);
        drop(diagnostics.phase(Phase::ResultPublication));
        let outcome = {
            let _receipt = diagnostics.phase(Phase::ReceiptPublication);
            Err::<(), _>(std::io::Error::other("original publication error"))
        };
        assert_eq!(
            outcome.err().ok_or("publication error lost")?.to_string(),
            "original publication error"
        );
        drop(diagnostics.phase(Phase::CompilerFamilyPublication));
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(rows.len(), 3);
        for (row, phase) in rows.iter().zip([
            "result-publication",
            "receipt-publication",
            "compiler-family-publication",
        ]) {
            assert_eq!(row["phase"], phase);
            assert_eq!(row.as_object().map(serde_json::Map::len), Some(5));
        }
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn existing_or_missing_destination_disables_evidence_without_overwrite() -> TestResult {
        let root = fixture()?;
        let path = root.join("existing");
        std::fs::write(&path, b"preserve")?;
        let existing = Diagnostics::create_at(&path);
        drop(existing.phase(Phase::Metadata));
        assert_eq!(std::fs::read(&path)?, b"preserve");
        let missing = Diagnostics::create_at(&root.join("absent/file"));
        drop(missing.phase(Phase::Metadata));
        assert!(!root.join("absent").exists());
        assert!(Diagnostics::default().origin.is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn row_and_byte_limits_are_fail_closed_diagnostics_only() -> TestResult {
        let root = fixture()?;
        let path = root.join("rows");
        let diagnostics = Diagnostics::create_at(&path);
        for _ in 0..MAX_ROWS + 100 {
            drop(diagnostics.phase(Phase::Metadata));
        }
        let rows = std::fs::read(&path)?;
        assert!(rows.len() <= MAX_BYTES);
        assert_eq!(std::str::from_utf8(&rows)?.lines().count(), MAX_ROWS);
        let bytes_path = root.join("bytes");
        let limited = Diagnostics::create_at(&bytes_path);
        if let Some(sink) = limited.sink.borrow_mut().as_mut() {
            sink.bytes = MAX_BYTES;
        }
        drop(limited.phase(Phase::Metadata));
        assert_eq!(std::fs::read(bytes_path)?, [] as [u8; 0]);
        drop((diagnostics, limited));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn process_file_budget_exhaustion_never_wraps_or_reactivates() {
        let serial = AtomicU64::new(MAX_FILES - 1);
        assert_eq!(file_slot(&serial), Some(MAX_FILES - 1));
        for _ in 0..100 {
            assert_eq!(file_slot(&serial), None);
        }
        assert_eq!(serial.load(Ordering::Relaxed), MAX_FILES);
        serial.store(u64::MAX, Ordering::Relaxed);
        assert_eq!(file_slot(&serial), None);
    }

    #[test]
    fn unset_and_empty_destinations_are_disabled_before_file_slot_allocation() {
        assert_eq!(event_destination(None), None);
        assert_eq!(event_destination(Some(std::ffi::OsString::new())), None);
        let path = std::ffi::OsString::from("outside-workspace/events.jsonl");
        assert_eq!(event_destination(Some(path.clone())), Some(path));
    }

    #[test]
    fn write_failure_disables_the_sink_without_changing_the_original_error() -> TestResult {
        let root = fixture()?;
        let path = root.join("read-only-descriptor");
        std::fs::write(&path, b"preserve")?;
        let diagnostics = Diagnostics {
            sink: RefCell::new(Some(Sink {
                file: File::open(&path)?,
                bytes: 0,
                rows: 0,
            })),
            origin: Some(Instant::now()),
        };
        let failure = failing_step(&diagnostics).err().ok_or("setup error lost")?;
        assert_eq!(failure.to_string(), "original setup error");
        assert!(diagnostics.sink.borrow().is_none());
        assert_eq!(std::fs::read(path)?, b"preserve");
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_follow_private_files_and_borrow_conflicts_never_change_work() -> TestResult {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let root = fixture()?;
        let target = root.join("target");
        std::fs::write(&target, b"preserve")?;
        let alias = root.join("alias");
        symlink(&target, &alias)?;
        drop(Diagnostics::create_at(&alias).phase(Phase::Metadata));
        assert_eq!(std::fs::read(target)?, b"preserve");
        let path = root.join("private");
        let diagnostics = Diagnostics::create_at(&path);
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        let borrowed = diagnostics.sink.borrow_mut();
        drop(diagnostics.phase(Phase::Metadata));
        drop(borrowed);
        assert_eq!(std::fs::read(&path)?, [] as [u8; 0]);
        drop(diagnostics.phase(Phase::Metadata));
        assert_ne!(std::fs::read(&path)?, [] as [u8; 0]);
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn bootstrap_first_refusal_and_disabled_observation_preserve_original_outcome() -> TestResult {
        let root = fixture()?;
        let path = root.join("bootstrap");
        let diagnostics = Diagnostics::create_at(&path);
        let refuse = |diagnostics: Option<&Diagnostics>| -> Option<()> {
            let mut observation = BootstrapAttempt::new(diagnostics, BootstrapStage::PublishLedger);
            observation.stage(BootstrapStage::PublishJournal);
            None::<()>?;
            observation.stage(BootstrapStage::PublishEmission);
            Some(())
        };
        assert_eq!(refuse(None), None);
        let disabled = Diagnostics::default();
        assert_eq!(refuse(Some(&disabled)), None);
        let encoded = std::cell::Cell::new(false);
        disabled.emit(|| {
            encoded.set(true);
            serde_json::to_vec(&true)
        });
        assert!(
            !encoded.get(),
            "disabled diagnostics must not serialize a row"
        );
        assert_eq!(refuse(Some(&diagnostics)), None);
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["kind"], "managed-script-bootstrap");
        assert_eq!(rows[0]["stage"], "publish-journal");
        assert_eq!(rows[0]["outcome"], "refused");
        assert_eq!(rows[0].as_object().map(serde_json::Map::len), Some(4));
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn bootstrap_rows_share_original_limits_and_busy_sink_refuses_observation() -> TestResult {
        let root = fixture()?;
        let path = root.join("bootstrap-limits");
        let diagnostics = Diagnostics::create_at(&path);
        {
            let _borrowed = diagnostics.sink.borrow_mut();
            diagnostics.bootstrap(BootstrapStage::OwnerComplete, BootstrapOutcome::Ready);
        }
        assert_eq!(std::fs::metadata(&path)?.len(), 0);
        for _ in 0..=MAX_ROWS {
            diagnostics.bootstrap(BootstrapStage::PublishEmission, BootstrapOutcome::Attempted);
        }
        let bytes = std::fs::read(&path)?;
        assert!(bytes.len() <= MAX_BYTES);
        assert_eq!(std::str::from_utf8(&bytes)?.lines().count(), MAX_ROWS);
        assert!(diagnostics.sink.borrow().is_none());
        drop(diagnostics);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
