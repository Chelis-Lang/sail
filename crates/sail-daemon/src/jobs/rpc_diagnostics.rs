//! Optional bounded RPC metadata. Never used for response or freshness authority.

use std::collections::VecDeque;
use std::io::{self, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

const LIMIT: usize = 256;

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operation {
    Result,
    Status,
    #[default]
    Other,
}

#[derive(Clone, Copy)]
pub(super) enum Stage {
    Read,
    Lookup,
    OutputSeal,
    Freshness,
    FrameKey,
    FrameLookup,
    Encode,
    Write,
}

#[derive(Clone, Copy, Default, Serialize)]
struct Record {
    sequence: u64,
    started_us: u64,
    dispatch_delay_us: u64,
    total_us: u64,
    operation: Operation,
    read_us: Option<u64>,
    lookup_us: Option<u64>,
    output_seal_us: Option<u64>,
    freshness_us: Option<u64>,
    frame_key_us: Option<u64>,
    frame_lookup_us: Option<u64>,
    encode_us: Option<u64>,
    write_us: Option<u64>,
    frame_hit: Option<bool>,
    frame_bytes: Option<usize>,
    currentness: Option<sail_api::jobs::Currentness>,
    write_succeeded: bool,
}

struct Enabled {
    origin: Instant,
    origin_unix_ms: u64,
    closed: AtomicBool,
    sequence: AtomicU64,
    dropped: AtomicU64,
    overwritten: AtomicU64,
    rows: Mutex<VecDeque<Record>>,
}

pub(super) struct Recorder {
    inner: Option<Enabled>,
}

#[derive(Clone, Copy)]
pub(super) struct Accepted {
    started: Option<(Instant, u64)>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::selected(std::env::var_os("SAIL_JOB_RPC_TIMINGS").as_deref())
    }

    #[cfg(test)]
    pub fn for_test(enabled: bool) -> Self {
        Self::selected(enabled.then_some(std::ffi::OsStr::new("1")))
    }

    #[cfg(test)]
    pub fn row_count(&self) -> usize {
        self.inner
            .as_ref()
            .and_then(|owner| owner.rows.try_lock().ok().map(|rows| rows.len()))
            .unwrap_or(0)
    }

    fn selected(value: Option<&std::ffi::OsStr>) -> Self {
        Self {
            inner: value.filter(|value| *value == "1").and_then(|_| {
                let mut rows = VecDeque::new();
                rows.try_reserve_exact(LIMIT).ok()?;
                // Reject an unexpectedly oversized allocator reservation. No
                // ring operation grows this initial owned capacity.
                if rows.capacity() > LIMIT * 2 {
                    return None;
                }
                Some(Enabled {
                    origin: Instant::now(),
                    origin_unix_ms: u64::try_from(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis(),
                    )
                    .unwrap_or(u64::MAX),
                    closed: AtomicBool::new(false),
                    sequence: AtomicU64::new(0),
                    dropped: AtomicU64::new(0),
                    overwritten: AtomicU64::new(0),
                    rows: Mutex::new(rows),
                })
            }),
        }
    }

    pub fn accepted(&self) -> Accepted {
        Accepted {
            started: self
                .inner
                .as_ref()
                .filter(|owner| !owner.closed.load(Ordering::Acquire))
                .map(|owner| {
                    (
                        Instant::now(),
                        owner.sequence.fetch_add(1, Ordering::Relaxed),
                    )
                }),
        }
    }

    #[cfg(test)]
    pub fn start(&self) -> Trace<'_> {
        self.start_at(self.accepted())
    }

    pub fn start_at(&self, accepted: Accepted) -> Trace<'_> {
        let owner = self
            .inner
            .as_ref()
            .filter(|owner| !owner.closed.load(Ordering::Acquire));
        let started = owner.and(accepted.started.map(|(started, _)| started));
        let record = owner
            .zip(accepted.started)
            .map(|(owner, (started, sequence))| Record {
                sequence,
                started_us: micros(started.duration_since(owner.origin)),
                dispatch_delay_us: micros(started.elapsed()),
                ..Record::default()
            });
        Trace {
            owner,
            started,
            record,
        }
    }

    /// Only shutdown performs trace I/O, after serving has stopped. It cannot
    /// affect a timed RPC. Failed export never changes a proof or cleanup result.
    pub fn export(&self, root: &Path) -> io::Result<()> {
        #[derive(Serialize)]
        struct Export<'a> {
            kind: &'static str,
            daemon_pid: u32,
            origin_unix_ms: u64,
            capacity: usize,
            started: u64,
            dropped: u64,
            overwritten: u64,
            rows: &'a VecDeque<Record>,
        }
        let Some(owner) = &self.inner else {
            return Ok(());
        };
        if owner.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let Ok(mut rows) = owner.rows.try_lock() else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(&Export {
            kind: "diagnostic-job-rpc-stages",
            daemon_pid: std::process::id(),
            origin_unix_ms: owner.origin_unix_ms,
            capacity: LIMIT,
            started: owner.sequence.load(Ordering::Relaxed),
            dropped: owner.dropped.load(Ordering::Relaxed),
            overwritten: owner.overwritten.load(Ordering::Relaxed),
            rows: &rows,
        })
        .map_err(io::Error::other)?;
        rows.clear();
        drop(rows);
        // Fixed metadata rows have no strings, paths or model payload. Refuse
        // unexpected serialization growth rather than emit an unbounded file.
        if bytes.len() > 256 * 1024 {
            return Err(io::Error::other(
                "SAIL0817: RPC diagnostics exceed their bound",
            ));
        }
        let temporary = root.join(format!("rpc-timings-{}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let outcome = file
            .write_all(&bytes)
            .and_then(|()| std::fs::rename(&temporary, root.join("rpc-timings.json")));
        if outcome.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        outcome
    }
}

pub(super) struct Trace<'a> {
    owner: Option<&'a Enabled>,
    started: Option<Instant>,
    record: Option<Record>,
}

impl Trace<'_> {
    pub fn disabled() -> Self {
        Self {
            owner: None,
            started: None,
            record: None,
        }
    }

    pub fn operation(&mut self, operation: Operation) {
        if let Some(record) = &mut self.record {
            record.operation = operation;
        }
    }

    pub fn begin(&self) -> Option<Instant> {
        self.record.as_ref().map(|_| Instant::now())
    }

    pub fn end(&mut self, stage: Stage, started: Option<Instant>) {
        if let Some(record) = &mut self.record
            && let Some(started) = started
        {
            let destination = match stage {
                Stage::Read => &mut record.read_us,
                Stage::Lookup => &mut record.lookup_us,
                Stage::OutputSeal => &mut record.output_seal_us,
                Stage::Freshness => &mut record.freshness_us,
                Stage::FrameKey => &mut record.frame_key_us,
                Stage::FrameLookup => &mut record.frame_lookup_us,
                Stage::Encode => &mut record.encode_us,
                Stage::Write => &mut record.write_us,
            };
            *destination = Some(micros(started.elapsed()));
        }
    }

    pub fn measure<T>(&mut self, stage: Stage, operation: impl FnOnce() -> T) -> T {
        let started = self.begin();
        let result = operation();
        self.end(stage, started);
        result
    }

    pub fn frame_hit(&mut self, hit: bool) {
        if let Some(record) = &mut self.record {
            record.frame_hit = Some(hit);
        }
    }

    pub fn currentness(&mut self, currentness: sail_api::jobs::Currentness) {
        if let Some(record) = &mut self.record {
            record.currentness = Some(currentness);
        }
    }

    pub fn frame_bytes(&mut self, bytes: usize) {
        if let Some(record) = &mut self.record {
            record.frame_bytes = Some(bytes);
        }
    }

    pub fn finish(mut self, write_succeeded: bool) {
        let (Some(owner), Some(started), Some(mut record)) =
            (self.owner, self.started, self.record.take())
        else {
            return;
        };
        record.total_us = micros(started.elapsed());
        record.write_succeeded = write_succeeded;
        if owner.closed.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut rows) = owner.rows.try_lock() else {
            owner.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if owner.closed.load(Ordering::Acquire) {
            return;
        }
        if rows.len() == LIMIT {
            rows.pop_front();
            owner.overwritten.fetch_add(1, Ordering::Relaxed);
        }
        rows.push_back(record);
    }
}

fn micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn enabled() -> Recorder {
        Recorder::selected(Some(std::ffi::OsStr::new("1")))
    }

    #[test]
    fn disabled_recorder_has_no_clock_rows_or_export() -> io::Result<()> {
        for value in [
            None,
            Some(std::ffi::OsStr::new("")),
            Some(std::ffi::OsStr::new("0")),
        ] {
            let recorder = Recorder::selected(value);
            assert!(recorder.inner.is_none());
            let mut trace = recorder.start();
            assert!(trace.started.is_none());
            assert!(trace.begin().is_none());
            assert_eq!(trace.measure(Stage::Write, || 7), 7);
            trace.finish(true);
            // No directory is created or touched when diagnostics are off.
            recorder.export(Path::new("/sail-diagnostic-disabled-no-directory"))?;
        }
        Ok(())
    }

    #[test]
    fn latest_ring_retains_order_and_does_not_grow() -> io::Result<()> {
        let recorder = enabled();
        let owner = recorder
            .inner
            .as_ref()
            .ok_or_else(|| io::Error::other("missing recorder"))?;
        let capacity = owner
            .rows
            .lock()
            .map_err(|_| io::Error::other("diagnostic lock poisoned"))?
            .capacity();
        for _ in 0..LIMIT + 44 {
            let mut trace = recorder.start();
            trace.operation(Operation::Result);
            trace.frame_hit(true);
            trace.frame_bytes(4_312_249);
            assert_eq!(trace.measure(Stage::Lookup, || 23), 23);
            trace.finish(true);
        }
        let rows = owner
            .rows
            .lock()
            .map_err(|_| io::Error::other("diagnostic lock poisoned"))?;
        assert_eq!(rows.len(), LIMIT);
        assert_eq!(rows.capacity(), capacity);
        assert_eq!(rows.front().map(|row| row.sequence), Some(44));
        assert_eq!(rows.back().map(|row| row.sequence), Some(299));
        assert!(
            rows.iter()
                .all(|row| row.frame_hit == Some(true) && row.lookup_us.is_some())
        );
        assert_eq!(owner.overwritten.load(Ordering::Relaxed), 44);
        Ok(())
    }

    #[test]
    fn contended_and_closed_storage_cannot_block_rpc_or_change_result() -> io::Result<()> {
        let recorder = enabled();
        let owner = recorder
            .inner
            .as_ref()
            .ok_or_else(|| io::Error::other("missing recorder"))?;
        let rows = owner
            .rows
            .lock()
            .map_err(|_| io::Error::other("diagnostic lock poisoned"))?;
        let mut trace = recorder.start();
        let response = trace.measure(Stage::Freshness, || Ok::<_, io::Error>("whole response"))?;
        trace.finish(true);
        assert_eq!(response, "whole response");
        assert_eq!(owner.dropped.load(Ordering::Relaxed), 1);
        assert!(rows.is_empty());
        drop(rows);
        // A closed generation also refuses a trace already in flight.
        let trace = recorder.start();
        owner.closed.store(true, Ordering::Release);
        trace.finish(true);
        assert!(
            owner
                .rows
                .lock()
                .map_err(|_| io::Error::other("diagnostic lock poisoned"))?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn export_is_bounded_private_metadata_and_seals_late_rows() -> io::Result<()> {
        let recorder = enabled();
        let mut trace = recorder.start();
        trace.operation(Operation::Status);
        trace.measure(Stage::OutputSeal, || ());
        trace.finish(true);
        let late = recorder.start();
        let root = std::env::temp_dir().join(format!("sail-rpc-stage-test-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let outcome = (|| {
            recorder.export(&root)?;
            late.finish(true);
            let path = root.join("rpc-timings.json");
            let bytes = std::fs::read(&path)?;
            assert!(bytes.len() <= 256 * 1024);
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            assert_eq!(value["rows"].as_array().map(Vec::len), Some(1));
            assert_eq!(value["rows"][0]["operation"], "status");
            assert_eq!(value["started"], 2);
            assert_eq!(value["daemon_pid"], std::process::id());
            assert_eq!(std::fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
            assert!(recorder.start().begin().is_none());
            Ok(())
        })();
        std::fs::remove_dir_all(root)?;
        outcome
    }

    #[test]
    #[allow(clippy::panic)] // Deliberate lock poison; no production input panics.
    fn poisoned_sink_discards_metadata_without_changing_operation() -> io::Result<()> {
        let recorder = enabled();
        let owner = recorder
            .inner
            .as_ref()
            .ok_or_else(|| io::Error::other("missing recorder"))?;
        let caught = std::panic::catch_unwind(|| {
            let Ok(_rows) = owner.rows.lock() else { return };
            panic!("intentional diagnostic sink poison");
        });
        assert!(caught.is_err());
        let mut trace = recorder.start();
        let complete = trace.measure(Stage::Freshness, || "complete fresh response");
        trace.finish(true);
        assert_eq!(complete, "complete fresh response");
        assert_eq!(owner.dropped.load(Ordering::Relaxed), 1);
        assert!(owner.rows.is_poisoned());
        Ok(())
    }
}
