//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
mod build_inputs;
mod configuration;
mod dependency_resolution;
mod diagnostics;
mod environment;
mod files;
mod freshness;
mod history;
mod input;
pub mod projects;
mod read_check;
mod receipts;
mod runner;
mod source_inputs;
mod store;

pub(crate) use files::{digest, event, file_digest, json, stat_identity};
pub use freshness::FreshnessIndex;
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub use runner::{Config, refresh_context, refresh_result, run, run_showing_output};
pub(crate) use runner::{run_engine, run_engine_held};

pub(crate) use history::{
    Restored, begin as begin_retained, observe as observe_retained, restore as restore_retained,
};
pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;

/// Whether `args` select partial verification (`--verify-function`,
/// `--verify-module`, `--verify-only-module` or `--verify-root` after `--`).
#[must_use]
pub fn partial(args: &[String]) -> bool {
    focused(args)
}
