//! The MCP server exposing verification to agents (`sail mcp`).
//!
//! It speaks the Model Context Protocol over stdio (newline-delimited
//! JSON-RPC 2.0; [`jsonrpc`], [`server`]) and offers four tools ([`tools`]):
//! - `verify`: runs `sail verify --output json` in a project, optionally
//!   for one module or function, and returns the schema-v1 report
//!   (`docs/output-schema.md`) with a short summary;
//! - `counterexample`: counterexample models and diagnostics for a
//!   function or obligation, from the latest recorded run;
//! - `cached_result`: the latest known verdicts, without re-running, with
//!   when and how they were established and whether sources changed since
//!   ([`store`]);
//! - `fragility`: `sail fragility --output json` (ADR 0018).
//!
//! Verification always runs the `sail` binary as a child process, so this
//! crate depends only on `sail-api` for report types and read-only job clients
//! (ADR 0016). The installed MCP server receives a fresh service reply for every
//! status/result lookup, with bounded decoded-frame reuse across calls.
//! Charter rule 7 holds throughout: a provisional result is never presented
//! as verified.
#![forbid(unsafe_code)]

mod args;
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
