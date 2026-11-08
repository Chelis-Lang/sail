//! A contracted helper in a nested module file.

use super::Grid;

/// The perimeter's half.
#[sail_contracts::satisfies("CL-SUM")]
#[sail_contracts::requires(grid.width <= 256 && grid.height <= 256)]
#[sail_contracts::ensures(|ret| ret == grid.width + grid.height)]
pub fn half_perimeter(grid: &Grid) -> u16 {
    grid.width + grid.height
}
