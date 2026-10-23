//! Explicit neutral-source admission and deterministic Verus shadow lowering.
//!
//! Lowering constructs no verdict. The original input-to-shadow relationship
//! must remain current when actual managed backend evidence is consumed.

mod admission;
mod evidence;
mod expression;
mod lower;
mod runner;
mod trust;
mod types;

use serde::{Deserialize, Serialize};

pub use lower::{lower, validate_manifest};
pub use runner::{ContractRun, verify};
pub use sail_api::contracts::{
    CheckCounts, ContractBinding, ContractProof, LoweringManifest, ProofState, SourceRange,
    TranslationTrust,
};

/// The exact neutral source lowering semantics version.
pub const LOWERING_VERSION: u32 = 2;

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

/// Reproduce conservative component observations from an actual backend record.
/// This admits no source or toolchain and establishes no standalone requirement tier.
#[must_use]
pub fn observed_proof(
    manifest: &LoweringManifest,
    backend: &sail_api::project::ProjectResult,
) -> ContractProof {
    evidence::summarize(manifest, backend)
}
