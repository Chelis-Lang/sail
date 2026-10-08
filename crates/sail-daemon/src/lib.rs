//! The long-lived S.A.I.L. service.
//!
//! Status: serves the local cache over the daemon protocol (ADR 0011). It
//! answers `HELLO` (solver identity, computed once per binary), `GET`, `PUT`
//! and `STATS` (counters, for benchmarks). Scheduling, warm solver pools and
//! watch mode come in later waves.
#![forbid(unsafe_code)]

#[cfg(unix)]
mod server;

#[cfg(unix)]
pub use server::{Stats, serve};
