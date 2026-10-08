//! The `sail-z3` binary: a Z3-compatible solver executable for Verus.
//!
//! Status: placeholder. The pass-through implementation lands in milestone
//! M2 (`docs/ROADMAP.md`). Until then it refuses to run rather than pretend
//! to be a solver.
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("error[SAIL0001]: sail-z3 is not implemented yet (see docs/ROADMAP.md)");
    ExitCode::from(3)
}
