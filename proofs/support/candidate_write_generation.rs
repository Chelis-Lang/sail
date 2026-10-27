//! Full-file direct-stock development root over the actual shared slot operations.
//! This is not selected by canonical drivers until closed capture is reviewed.
#![forbid(unsafe_code)]
#![allow(unused_imports)]
use vstd::prelude::*;
#[macro_use]
#[path = "../../crates/sail-acceptance/src/syntax.rs"]
mod syntax;
#[path = "../../crates/sail-acceptance/src/cache_candidate.rs"]
pub mod cache_candidate;
#[path = "../../crates/sail-acceptance/src/inputs.rs"]
pub mod inputs;
fn main() {}
