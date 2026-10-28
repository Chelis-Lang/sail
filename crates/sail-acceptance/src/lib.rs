//! Source-shared executable validation for Sail evidence.
//!
//! Contracts and proof hints are erased by Cargo and checked directly with
//! stock Verus. The surrounding I/O and process adapters retain their explicit
//! refinement obligations.
#![forbid(unsafe_code)]

#[macro_use]
mod syntax;
#[macro_use]
mod host_syntax;

pub mod authority;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_bucket;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_candidate;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_chain;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_counted;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_indexed;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_order;
/// Exact header and original payload framing for disk records.
pub mod cache_record;
/// Staged cache ownership operation; native adoption is separately qualified.
pub mod cache_slots;
/// Cache record transaction with explicit low-level host assumptions.
pub mod cache_write;
pub mod inputs;
pub mod query;

/// Exact structural byte grammar for streaming frames and lexical tokens.
pub mod parser_grammar;
/// Owned deferred responses preserve their original payload through annotation.
pub mod query_holding;
/// Actual borrowed SMT token cursor, spans and sticky lexical errors.
pub mod query_lex;
/// Actual owned SMT stream splitter operations and ordered emissions.
pub mod query_split;

/// Actual shallow command classifier and response expectation over original bytes.
pub mod query_command;
/// Exact supported successful-command response policy over original head bytes.
pub mod query_policy;
