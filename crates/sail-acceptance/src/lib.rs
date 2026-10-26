//! Source-shared executable validation for Sail evidence.
//!
//! Contracts and proof hints are erased by Cargo and checked directly with
//! stock Verus. The surrounding I/O and process adapters retain their explicit
//! refinement obligations.
#![forbid(unsafe_code)]

#[macro_use]
mod syntax;

pub mod authority;
/// Exact cache record-write sequencing without an origin-authority assertion.
pub mod cache_write;
pub mod inputs;
/// Exact incremental key framing through source-bound hash primitives.
pub mod key_prefix;
pub mod phase_inputs;
pub mod query;
/// Actual owned solver-input delivery and terminal key correspondence.
pub mod query_delivery;
/// Default response policy from exact accepted command head bytes.
pub mod query_policy;
/// Joint ownership of exact raw input and its incremental key material.
pub mod query_prefix;
