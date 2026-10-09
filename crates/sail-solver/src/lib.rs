//! Warm solver process pool, session replay, resource limits and crash recovery.
//!
//! Status: the pool is not built yet (see `docs/ROADMAP.md`). What exists is
//! [`process`]: running the pinned Z3 in a fresh process on a complete,
//! exact input, with a wall-clock safety net that is reported as an
//! infrastructure failure, never as a solver answer. Session replay for
//! measurement (`sail-bench`, `sail-portfolio`) is built on it (ADR 0018).
#![forbid(unsafe_code)]

pub mod process;

pub use process::{Identity, Outcome, Run, Solver, SolverError, hex, run};
