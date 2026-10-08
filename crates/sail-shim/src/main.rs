//! The `sail-z3` binary: a Z3-compatible solver executable for Verus.
//!
//! Point Verus at it with `VERUS_Z3_PATH=…/sail-z3` and name the pinned Z3
//! with `SAIL_Z3_PATH`. See the `sail_shim` library docs for behaviour.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_shim::run()
}
