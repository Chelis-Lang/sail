//! Benchmark workloads, history replay and reporting.
//!
//! Status (M3): the cold workload with the plain-Verus baseline ([`cold`])
//! and the generated `docs/benchmarks.md` ([`report`]).
#![forbid(unsafe_code)]

pub mod cold;
pub mod report;
