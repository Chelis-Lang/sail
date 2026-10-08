//! The long-lived S.A.I.L. service.
//!
//! Status: serves the local cache over the daemon protocol (ADR 0011). It
//! answers `HELLO` (solver identity, computed once per binary), `GET`, `PUT`,
//! `STATS` (counters) and `INFO` (configuration and state). An in-memory
//! tier sits in front of the disk tier, and `PUT`s are written behind
//! (ADR 0014). It stops after a configurable idle period, logs to a file,
//! and removes its socket on exit. Scheduling, warm solver pools and watch
//! mode come in later waves.
#![forbid(unsafe_code)]

#[cfg(unix)]
mod log;
#[cfg(unix)]
mod server;
#[cfg(unix)]
mod writer;

#[cfg(unix)]
pub use server::{
    Config, DEFAULT_IDLE_TIMEOUT, DEFAULT_MEMORY_BYTES, DEFAULT_QUEUE_BYTES, DEFAULT_QUEUE_WAIT,
    Handle, Server, Stats, bind, serve, serve_with,
};
