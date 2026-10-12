//! Experimental timing for rescue lane L11 (warm exact-prefix sessions).
//!
//! Active only in cached sessions with `SAIL_TRACE_DIR` set, which the
//! observation tap otherwise ignores in that mode. After every reply has
//! been forwarded and the solver has exited, writes `<id>.timing.json` and
//! `<id>.in.smt2` (the session's whole input). Best effort: nothing here can
//! change the forwarded bytes.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Wall-clock nanoseconds since the Unix epoch.
pub(crate) fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// One session's timing; all instants are Unix nanoseconds.
#[derive(Default, Clone)]
pub(crate) struct Timing {
    pub start: u128,
    pub miss: Option<u128>,
    pub miss_kind: Option<&'static str>,
    pub miss_served: u64,
    pub miss_start_offset: Option<u64>,
    pub miss_end_offset: Option<u64>,
    pub miss_command: Option<String>,
    pub spawned: Option<u128>,
    pub raw_written: Option<u128>,
    pub first_output: Option<u128>,
    pub replay_done: Option<u128>,
    pub first_forward: Option<u128>,
    pub first_failure: Option<u128>,
}

static TIMING: Mutex<Option<Timing>> = Mutex::new(None);

/// Applies `f` to the timing record, if one is active.
pub(crate) fn with(f: impl FnOnce(&mut Timing)) {
    if let Ok(mut t) = TIMING.lock()
        && let Some(t) = t.as_mut()
    {
        f(t);
    }
}

/// Starts recording when `SAIL_TRACE_DIR` is set.
pub(crate) fn begin() {
    if std::env::var_os(crate::ENV_TRACE_DIR).is_some_and(|v| !v.is_empty())
        && let Ok(mut t) = TIMING.lock()
    {
        *t = Some(Timing {
            start: now(),
            ..Timing::default()
        });
    }
}

/// Writes the record and the input.
pub(crate) fn finish(input: &[u8], served: u64, solver_started: bool) {
    let end = now();
    let Some(dir) = std::env::var_os(crate::ENV_TRACE_DIR).filter(|v| !v.is_empty()) else {
        return;
    };
    let Some(t) = TIMING.lock().ok().and_then(|t| t.clone()) else {
        return;
    };
    let record = serde_json::json!({
        "l11_version": 1,
        "pid": std::process::id(),
        "start_ns": t.start.to_string(),
        "end_ns": end.to_string(),
        "miss_ns": t.miss.map(|v| v.to_string()),
        "miss_kind": t.miss_kind,
        "miss_served": t.miss_served,
        "miss_start_offset": t.miss_start_offset,
        "miss_end_offset": t.miss_end_offset,
        "miss_command": t.miss_command,
        "spawned_ns": t.spawned.map(|v| v.to_string()),
        "raw_written_ns": t.raw_written.map(|v| v.to_string()),
        "first_output_ns": t.first_output.map(|v| v.to_string()),
        "replay_done_ns": t.replay_done.map(|v| v.to_string()),
        "first_forward_ns": t.first_forward.map(|v| v.to_string()),
        "first_failure_ns": t.first_failure.map(|v| v.to_string()),
        "served": served,
        "solver_started": solver_started,
        "input_bytes": input.len(),
    });
    let dir = std::path::Path::new(&dir);
    let name = format!("{}-{}", std::process::id(), t.start);
    let _ = std::fs::write(dir.join(format!("{name}.in.smt2")), input);
    let _ = std::fs::write(
        dir.join(format!("{name}.timing.json")),
        record.to_string() + "\n",
    );
}
