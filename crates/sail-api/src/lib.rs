//! The embeddable API and the daemon wire protocol.
//!
//! Status: the daemon protocol and the thin client the shim uses (ADR 0011).
//! The structured output schema arrives with `sail verify`.
#![forbid(unsafe_code)]

pub mod protocol;

#[cfg(unix)]
pub mod client;
