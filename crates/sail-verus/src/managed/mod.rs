//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
mod build_inputs;
mod coherent;
mod configuration;
mod files;
mod freshness;
mod input;
#[cfg(target_os = "linux")]
mod kernel_leases;
#[cfg(target_os = "linux")]
mod lease;
mod observe;
#[cfg(target_os = "linux")]
mod precise;
mod receipts;
mod runner;
mod source_inputs;
mod sysroot;

pub(crate) use files::{digest, event, file_digest, json};
pub use freshness::FreshnessIndex;
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub(crate) use runner::run_engine;
pub use runner::{Config, refresh_context, refresh_result, run};

pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;
