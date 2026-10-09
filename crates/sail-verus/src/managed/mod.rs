//! Managed Cargo artifacts and successful per-target verification receipts.

mod files;
mod input;
mod receipts;
mod runner;

pub(crate) use files::{digest, event, file_digest, json};
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity};
pub use runner::{Config, run};

pub(crate) use receipts::complete as receipt_complete;
