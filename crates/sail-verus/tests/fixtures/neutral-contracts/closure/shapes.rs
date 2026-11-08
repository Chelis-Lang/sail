//! Nominal types, with a nested out-of-line module.

pub mod measure;

/// A grid size.
#[derive(Debug)]
pub struct Grid {
    /// Columns.
    pub width: u16,
    /// Rows.
    pub height: u16,
}
