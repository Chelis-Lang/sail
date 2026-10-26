//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
pub(crate) mod build_environment;
mod build_inputs;
mod build_unit_plan;
pub mod cargo_toolchain;
mod command_environment;
mod configuration;
mod dependency_snapshot;
mod diagnostics;
mod environment;
mod files;
mod freshness;
mod input;
mod native_include_hints;
pub mod noop;
pub mod projects;
mod read_check;
mod receipts;
mod request;
pub(crate) mod research;
#[expect(
    dead_code,
    reason = "ADR 0050's content-bound successor is source-only until its two-phase compiler admission and real Vest ABA gate qualify."
)]
pub(crate) mod retained;
mod runner;
pub(crate) mod shipped;
pub(crate) mod source_inputs;
mod store;
pub(crate) mod visible_output;

pub use cargo_toolchain::CargoToolchain;
pub(crate) use files::{digest, event, file_digest, json, stat_identity};
pub use freshness::FreshnessIndex;
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub use request::RequestCaptureDestination;
pub use runner::{
    Config, refresh_context, refresh_result, run, run_for_replay, run_showing_output,
};
pub(crate) use runner::{run_engine, run_engine_held};
pub use shipped::new_project_lockfile;

pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;

/// Whether `args` select partial verification (`--verify-function`,
/// `--verify-module`, `--verify-only-module` or `--verify-root` after `--`).
#[must_use]
pub fn partial(args: &[String]) -> bool {
    focused(args)
}
