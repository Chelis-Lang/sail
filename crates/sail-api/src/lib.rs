//! The embeddable API, the daemon wire protocol and the structured output
//! schema.
//!
//! - [`protocol`] and [`client`]: the daemon protocol and the thin client
//!   the shim uses (ADR 0011);
//! - [`output`]: the `sail verify --output json` document (ADR 0014);
//! - [`schema`]: a small JSON Schema checker for that document's schema;
//! - [`policy`]: the serving policy for cached `unknown` answers (ADR 0009);
//! - [`stamp`]: complete stat stamps and the cross-run settle rule (ADR 0082).
#![forbid(unsafe_code)]

/// Sail's version, as `sail --version` and every report state it:
/// `SAIL_VERSION` when the build sets it (`scripts/release.sh` passes the
/// release version, for example `0.1.0-rc.1`), else the crate version.
pub const VERSION: &str = match option_env!("SAIL_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

/// Checked positive Verus report and requested-scope authority.
pub mod acceptance;
mod acceptance_sort;
/// Requirement-aware evidence views with explicit scope, strength and establishment.
pub mod assurance;
pub mod evidence;
pub mod jobs;
pub mod literal_witness;
pub mod mutation;
pub mod output;
pub mod policy;
pub mod project;
pub mod proof;
pub mod protocol;
pub mod repair;
/// Internal checked worker journal selection; native origin remains separate.
#[doc(hidden)]
pub mod request_capture;
/// Internal checked report-command construction from observed effective options.
#[doc(hidden)]
pub mod request_mode;
/// Internal immutable original-target dictionary ordering and projection.
#[doc(hidden)]
pub mod request_order;
/// Internal checked construction and projection of the original target set.
#[doc(hidden)]
pub mod request_targets;
pub mod runtime;
pub mod schema;
pub mod stamp;
pub mod temporal;

#[cfg(unix)]
pub mod client;

/// Neutral source job requests and source-bound contract evidence.
pub mod contracts;

#[cfg(test)]
mod request_order_tests;
#[cfg(test)]
mod request_targets_tests;
