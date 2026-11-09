//! The editor bridge behind `sail lsp` (spike; needs an ADR before it ships).
//!
//! It holds everything about the Language Server Protocol that does not need
//! a job service: the base-protocol framing ([`jsonrpc`]), the lifecycle
//! state machine ([`session`]), and the pure mapping from a managed project
//! result to `textDocument/publishDiagnostics` ([`diagnostics`]). The
//! command in `sail-cli` drives it from the same loop as `sail watch`.
//!
//! This crate depends only on `sail-api` for the result types. Diagnostics
//! are Verus's own, relayed with their text unchanged; a result that is not
//! current is never published as current.
#![forbid(unsafe_code)]

pub mod diagnostics;
pub mod jsonrpc;
pub mod session;
pub mod uri;
