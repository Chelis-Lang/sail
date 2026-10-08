//! Running a fresh pinned Z3 on a complete input: [`sail_solver::process`],
//! with errors mapped to [`BenchError`].

pub use sail_solver::{Identity, Outcome, Run, Solver, hex};

use crate::error::BenchError;

/// Runs the solver on `input` in a fresh process.
///
/// # Errors
///
/// [`BenchError::Solver`] if the process cannot be started or waited for.
pub fn run(solver: &Solver, input: &[u8]) -> Result<Run, BenchError> {
    sail_solver::run(solver, input).map_err(|e| BenchError::Solver(e.0))
}
