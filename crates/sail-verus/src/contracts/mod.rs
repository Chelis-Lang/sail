//! Explicit neutral-source admission and deterministic Verus shadow lowering.
//!
//! Lowering constructs no verdict. The original input-to-shadow relationship
//! must remain current when actual managed backend evidence is consumed.

mod admission;
mod expression;
mod lower;
mod runner;
mod trust;
mod types;

use serde::{Deserialize, Serialize};

pub use lower::lower;
pub use runner::{ContractRun, verify};
pub use trust::TranslationTrust;

/// The exact neutral source lowering semantics version.
pub const LOWERING_VERSION: u32 = 1;

/// An original source range. Columns are zero-based, lines are one-based.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    /// First source line.
    pub line: usize,
    /// First source column.
    pub column: usize,
    /// Exclusive final source line.
    pub end_line: usize,
    /// Exclusive final source column.
    pub end_column: usize,
}

/// A requirement-to-guarantee binding, before actual backend obligation mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractBinding {
    /// Stable explicit owner requirement identity.
    pub requirement_id: String,
    /// Original fully qualified function identity.
    pub function: String,
    /// Exact generated Verus symbol; evidence must actually name this symbol.
    pub lowered_function: String,
    /// Hash of the exact parsed guarantee syntax in its canonical token rendering.
    pub guarantee_sha256: String,
    /// Original complete function range.
    pub source: SourceRange,
}

/// Content-bound, deterministic original-to-shadow mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringManifest {
    /// Lowering semantics version.
    pub version: u32,
    /// Neutral contract syntax version.
    pub contract_version: u32,
    /// Validated compiler crate name.
    pub crate_name: String,
    /// Exact original UTF-8 source digest.
    pub original_sha256: String,
    /// Exact emitted Verus source digest.
    pub lowered_sha256: String,
    /// Explicit supported arithmetic interpretation.
    pub arithmetic: String,
    /// Sorted one-guarantee function bindings; no invented solver obligation IDs.
    pub bindings: Vec<ContractBinding>,
}

/// Deterministic Verus source and its original-source binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweredSource {
    /// Complete generated source, to place in an owned shadow crate.
    pub source: String,
    /// Exact relationship to the original neutral source.
    pub manifest: LoweringManifest,
}

fn unsupported(message: impl Into<String>) -> crate::Error {
    crate::Error::new("SAIL3140", message)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err supplies an owned parser diagnostic."
)]
fn syntax(error: syn::Error) -> crate::Error {
    unsupported(error.to_string())
}

fn digest(bytes: &[u8]) -> String {
    crate::managed::digest(bytes)
}
