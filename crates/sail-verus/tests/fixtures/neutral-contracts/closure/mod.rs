//! Profile three: a crate root whose helpers live in out-of-line module files.

pub mod rules;
pub mod shapes;

use self::shapes::Grid;
use self::shapes::measure;

/// The grid's span stays within the requirement's bound.
#[sail_contracts::satisfies("CL-SPAN")]
#[sail_contracts::requires(grid.width <= 256 && grid.height <= 256)]
#[sail_contracts::ensures(|ret| rules::fits(ret))]
pub fn span(grid: &Grid) -> u16 {
    measure::half_perimeter(grid)
}

/// A grid orientation. Enums stay at the crate root (VI-0042).
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// Wider than tall.
    Wide,
    /// Taller than wide.
    Tall,
}

/// Wide grids report wide mode.
#[sail_contracts::satisfies("CL-MODE")]
#[sail_contracts::ensures(|ret| ret == (*mode == Mode::Wide))]
pub fn wide(mode: &Mode) -> bool {
    *mode == Mode::Wide
}
