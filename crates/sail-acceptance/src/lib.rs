//! Source-shared executable validation for Sail evidence.
//!
//! Contracts and proof hints are erased by Cargo and checked directly with
//! stock Verus. The surrounding I/O and process adapters retain their explicit
//! refinement obligations.
#![forbid(unsafe_code)]

#[macro_use]
mod syntax;

pub mod authority;
pub mod inputs;
/// Exact incremental key framing through source-bound hash primitives.
pub mod key_prefix;
/// One owned operational argument layer with exact ordinary-argv restoration.
pub mod observation_arguments;
pub mod query;

/// Exact retained compiler attempt binding for solver-launch journals.
pub mod solver_invocations;
/// Exact solver-process observation lifecycle and complete journal admission.
pub mod solver_observations;
