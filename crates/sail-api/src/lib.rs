//! The embeddable API, the daemon wire protocol and the structured output
//! schema.
//!
//! - [`protocol`] and [`client`]: the daemon protocol and the thin client
//!   the shim uses (ADR 0011);
//! - [`output`]: the `sail verify --output json` document (ADR 0014);
//! - [`schema`]: a small JSON Schema checker for that document's schema;
//! - [`policy`]: the serving policy for cached `unknown` answers (ADR 0009).
#![forbid(unsafe_code)]

pub mod jobs;
pub mod output;
pub mod policy;
pub mod project;
pub mod protocol;
pub mod schema;

#[cfg(unix)]
pub mod client;
