//! The MCP server exposing verification to agents (`sail mcp`).
//!
//! It speaks the Model Context Protocol over stdio (newline-delimited
//! JSON-RPC 2.0; [`jsonrpc`], [`server`]) with verification and evidence tools
//! ([`tools`]):
//! - `verify`: runs `sail verify --output json` in a project, optionally
//!   for one module or function, and returns the schema-v1 report
//!   (`docs/output-schema.md`) with a short summary;
//! - `counterexample`: counterexample models and diagnostics for a
//!   function or obligation, from the latest recorded run;
//! - `cached_result`: the latest known verdicts, without re-running, with
//!   when and how they were established and whether sources changed since
//!   ([`store`]);
//! - `fragility`: `sail fragility --output json` (ADR 0018).
//! - `start_verification`, `verification_status`, `verification_result` and
//!   `cancel_verification`: durable workspace jobs, with lifecycle kept separate
//!   from proof outcomes;
//! - `start_repair_validation`, `verification_events`, `explain_failure` and
//!   `compare_verification`: independent focus/full repair evidence, provisional
//!   events, recorded failure details and comparisons (ADR 0027).
//! - `save_baseline`, `list_baselines` and `get_baseline`: immutable named
//!   historical evidence, independent of rolling job retention.
//!
//! Verification always runs the `sail` binary as a child process, so this
//! crate depends only on `sail-api` for report types and read-only job clients
//! (ADR 0016). The installed MCP server receives a fresh service reply for every
//! status/result lookup, with bounded decoded-frame reuse across calls.
//! Charter rule 7 holds throughout: a provisional result is never presented
//! as verified.
#![forbid(unsafe_code)]

mod args;
mod compact;
mod job_clients;
mod job_tools;
pub mod jsonrpc;
mod proof_tools;
mod run;
pub mod server;
pub mod store;
mod summary;
#[cfg(all(test, unix))]
mod test_io;
pub mod tools;

pub use server::{serve, serve_with_job_clients};
pub use tools::Config;
