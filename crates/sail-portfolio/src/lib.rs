//! Seed portfolios, racing and fragility scoring.
//!
//! Status: fragility **measurement** (ADR 0018). The CLI orchestrates
//! independent complete-run `accept-first-proof` portfolios (ADR 0020);
//! this crate remains observation-only. Racing is deferred.
//!
//! - [`sessions`]: finding captured solver sessions in shim trace
//!   directories;
//! - [`transcript`]: splitting exact solver input and output into commands
//!   and replies, and pairing them;
//! - [`config`]: the configurations a session is re-run under (reseeds,
//!   and semantics-preserving assertion shuffles), applied to exact bytes;
//! - [`fragility`]: per-check answer and effort distributions across
//!   configurations, stability classes, scores and summaries;
//! - [`stats`] and [`parallel`]: shared helpers.
//!
//! Everything here measures and reports. It never changes a verdict, and
//! nothing it produces reaches a cache key: this crate does not depend on
//! `sail-key` or `sail-cache`, and the layering table forbids either from
//! reaching it.
#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod fragility;
pub mod parallel;
pub mod sessions;
pub mod stats;
pub mod transcript;

pub use error::PortfolioError;
pub use sail_solver::{Identity, Solver};
