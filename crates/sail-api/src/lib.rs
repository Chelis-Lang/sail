//! The embeddable API, the daemon wire protocol and the structured output
//! schema.
//!
//! - [`protocol`] and [`client`]: the daemon protocol and the thin client
//!   the shim uses (ADR 0011);
//! - [`output`]: the `sail verify --output json` document (ADR 0014);
//! - [`schema`]: a small JSON Schema checker for that document's schema;
//! - [`policy`]: the serving policy for cached `unknown` answers (ADR 0009).
#![forbid(unsafe_code)]

/// Sail's version, as `sail --version` and every report state it:
/// `SAIL_VERSION` when the build sets it (`scripts/release.sh` passes the
/// release version, for example `0.1.0-rc.1`), else the crate version.
pub const VERSION: &str = match option_env!("SAIL_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

pub mod evidence;
pub mod jobs;
pub mod output;
pub mod policy;
pub mod project;
pub mod proof;
pub mod protocol;
pub mod repair;
pub mod runtime;
pub mod schema;

#[cfg(unix)]
pub mod client;

/// Neutral source job requests and source-bound contract evidence.
pub mod contracts;
