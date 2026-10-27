//! Source-shared executable validation for Sail evidence.
//!
//! Contracts and proof hints are erased by Cargo and checked directly with
//! stock Verus. The surrounding I/O and process adapters retain their explicit
//! refinement obligations.
#![forbid(unsafe_code)]

#[macro_use]
mod syntax;

pub mod authority;
/// Insertion-issued candidate generations and transitions over actual memory slots.
pub mod cache_candidate;
/// Exact cache record-write sequencing without an origin-authority assertion.
pub mod cache_write;
pub mod inputs;
/// Exact incremental key framing through source-bound hash primitives.
pub mod key_prefix;
pub mod phase_inputs;
pub mod query;
/// Original checkpoints checked against one owned actual acknowledged input tape.
pub mod query_checkpoints;
/// Actual owned solver-input delivery and terminal key correspondence.
pub mod query_delivery;
/// Actual immutable held payloads and independently mutable effort metadata.
pub mod query_holding;
/// Actual borrowed SMT lexical cursors and quoted/string/word endpoints.
pub mod query_lex;
/// Exact raw scalar option transitions and persistent native Session readback.
pub mod query_options;
/// Exact stdout coverage and same-owner response intervals.
pub mod query_output;
/// Actual emitted-item, frame, replay and queue-handover ordering.
pub mod query_output_order;
/// Original opaque stream leases retained through frame copies and publication.
pub mod query_output_owner;
/// Actual output splitter and retained stdout bytes consumed through one controller.
pub mod query_output_split;
/// Default response policy from exact accepted command head bytes.
pub mod query_policy;
/// Joint ownership of exact raw input and its incremental key material.
pub mod query_prefix;
/// Typed local publication over owned delivered input and complete stdout.
pub mod query_publication;
/// Original served bytes and the mechanical replay-comparison transition.
pub mod query_replay;
/// Actual owned pending-list bytes and first-root streaming completion.
pub mod query_split;
pub mod stream_capture;
