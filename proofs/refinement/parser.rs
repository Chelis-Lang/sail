//! The complete parser root compiles the actual grammar, splitter and lexer.
//! Nonempty raw chunk/literal/sticky witnesses live in those shared modules.
//! Native adapters, physical EOF, capacity and solver meaning remain separate.
use vstd::prelude::*;

verus! {
    /// A real paired input delimiter emits an atom and unmatched closer once.
    pub fn paired_stream_constructive_control() -> (passed:bool)
        ensures passed,
    { crate::query_split::outer_atom_offsets_control() }
}
