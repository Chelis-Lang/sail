//! SMT-LIB parsing and printing, faithful to the bytes Z3 sends and receives.
//!
//! Everything here works on byte slices and reports positions as byte
//! offsets into the original input, so callers can always recover the exact
//! bytes a solver saw. Nothing in this crate rewrites input: "printing" a
//! parsed command means slicing the original bytes.
//!
//! - [`lex`] splits input into tokens with spans.
//! - [`sexpr`] parses tokens into s-expressions (iteratively, with a depth
//!   limit, so hostile input cannot overflow the stack).
//! - [`command`] classifies a top-level command by looking only at its first
//!   few tokens, which is cheap even for very large assertions.
//! - [`split`] incrementally splits a byte stream into top-level commands
//!   (solver input) or responses (solver output), independent of how the
//!   stream is chunked.
//!
//! No function in this crate panics on any input.
#![forbid(unsafe_code)]

pub mod command;
mod error;
pub mod lex;
pub mod sexpr;
pub mod split;

pub use error::{Error, ErrorCode};
