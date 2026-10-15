//! Benchmark workloads, history replay and reporting.
//!
//! Status (M4):
//! - [`cold`]: the cold workload with the plain-Verus baseline;
//! - [`warm`]: the warm, no-change workload with the cache on;
//! - [`edit`]: the single-function body and specification edit workloads;
//! - [`report`]: the generated `docs/benchmarks.md`;
//! - [`repair`]: the M7 repair acceptance workload, recorded then judged.
//!
//! Trace analysis over the shim's captured sessions (`SAIL_TRACE_DIR`),
//! driven by the `sail-bench` binary ([`cli`]):
//! - [`profile`]: how solver time is distributed across queries.
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
pub mod hits;
pub mod profile;
pub mod repair;
pub mod replay;
pub mod report;
#[cfg(unix)]
pub mod snapshot;
pub mod solver;
pub mod trace;
pub mod warm;

pub use sail_portfolio::{parallel, stats, transcript};
