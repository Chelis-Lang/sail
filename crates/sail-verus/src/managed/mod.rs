//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
mod build_inputs;
mod configuration;
mod files;
mod input;
mod observe;
mod receipts;
mod runner;
mod source_inputs;

pub(crate) use files::{digest, event, file_digest, json};
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub use runner::{Config, refresh_context, refresh_result, run};

pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;
