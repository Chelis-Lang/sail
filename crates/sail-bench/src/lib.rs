//! Benchmark workloads, history replay and reporting.
//!
//! Status (M4):
//! - [`cold`]: the cold workload with the plain-Verus baseline;
//! - [`warm`]: the warm, no-change workload with the cache on;
//! - [`edit`]: the single-function body and specification edit workloads;
//! - [`report`]: the generated `docs/benchmarks.md`.
//!
//! Trace analysis over the shim's captured sessions (`SAIL_TRACE_DIR`),
//! driven by the `sail-bench` binary ([`cli`]):
//! - [`profile`]: how solver time is distributed across queries;
//! - [`seeds`]: answer and `rlimit-count` stability across random seeds,
//!   and determinism of an unmodified replay;
//! - [`history`]: whether a check's outcome depends on the solver process's
//!   history or only on the state query key v1 binds (ADR 0004).
//!
//! Everything works on exact bytes. Replays run the pinned Z3 in fresh
//! processes, with a wall-clock safety net whose firing is reported as an
//! infrastructure timeout, never as a solver answer.
#![forbid(unsafe_code)]

pub mod agent_loop;
mod census_report;
pub mod cli;
pub mod cold;
pub mod daemon;
pub mod edit;
pub mod error;
pub mod fragility;
pub mod history;
pub mod hits;
pub mod presolver;
pub mod profile;
pub mod replay;
pub mod report;
pub mod seeds;
pub mod solver;
pub mod trace;
pub mod warm;

pub use sail_portfolio::{parallel, stats, transcript};

use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

/// Turns progress reporting on stderr off (or back on).
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// Reports progress on stderr every 50 units and at the end.
fn progress(what: &str, done: usize, total: usize) {
    if QUIET.load(Ordering::Relaxed) || !(done.is_multiple_of(50) || done == total) {
        return;
    }
    let _ = writeln!(std::io::stderr(), "{what}: {done}/{total}");
}
