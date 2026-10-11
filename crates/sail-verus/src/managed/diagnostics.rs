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
    SysrootHash,
    SysrootBarrier,
    ContextConfiguration,
    OwnedPaths,
    InitialSnapshot,
    InputGuards,
    MonitoredSnapshot,
    InputBarrier,
    TargetCleanup,
    InvocationSeal,
}

#[derive(serde::Serialize)]
struct Record {
    kind: &'static str,
    phase: Phase,
    pid: u32,
    started_ms: f64,
    elapsed_ms: f64,
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
        let Some(path) = std::env::var_os("SAIL_JOB_EVENTS") else {
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
        let Ok(mut slot) = self.sink.try_borrow_mut() else {
            return;
        };
        let Some(sink) = slot.as_mut() else {
            return;
        };
        let Ok(mut row) = serde_json::to_vec(&Record {
            kind: "managed-setup-phase",
            phase,
            pid: std::process::id(),
            started_ms: started.duration_since(origin).as_secs_f64() * 1000.0,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        }) else {
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
}
