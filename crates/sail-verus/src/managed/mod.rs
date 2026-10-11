//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
mod build_inputs;
mod coherent;
mod configuration;
mod diagnostics;
mod files;
mod freshness;
mod guarded_output;
mod input;
#[cfg(target_os = "linux")]
mod kernel_leases;
#[cfg(target_os = "linux")]
mod kernel_mounts;
#[cfg(target_os = "linux")]
mod lease;
mod observe;
#[cfg(target_os = "linux")]
mod precise;
mod receipts;
#[cfg(any(target_os = "linux", test))]
mod retained;
mod runner;
mod source_inputs;
mod sysroot;
mod sysroot_cache;
#[cfg(any(target_os = "linux", test))]
mod sysroot_selection;

pub(crate) use files::{digest, event, file_digest, json};
pub use freshness::FreshnessIndex;
pub use guarded_output::{GUARDED_OUTPUT_RETAINED_BYTES, GuardedOutput};
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub(crate) use runner::run_engine;
pub use runner::{Config, refresh_context, refresh_result, run};
pub use sysroot_cache::SysrootIndex;

pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;
