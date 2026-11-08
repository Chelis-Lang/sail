//! Explicit neutral-source admission and deterministic Verus shadow lowering.
//!
//! Lowering constructs no verdict. The original input-to-shadow relationship
//! must remain current when actual managed backend evidence is consumed.

mod admission;
mod evidence;
mod expression;
pub mod literal_witness;
mod lower;
pub mod mutation;
mod placement;
mod runner;
mod trust;
mod types;

use serde::{Deserialize, Serialize};

pub use lower::{lower, lower_profile, validate_manifest};
pub use runner::{ContractRun, verify, verify_profile};
pub use sail_api::contracts::{
    CheckCounts, ContractBinding, ContractProof, LoweringManifest, ProofState, SourceRange,
    TranslationTrust,
};

/// The exact neutral source lowering semantics version.
pub const LOWERING_VERSION: u32 = 2;

/// The lowering version emitted for explicitly selected source profile three.
pub const PROFILE_THREE_LOWERING_VERSION: u32 = 4;

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

/// Named-field declaration order of every struct in `source`, including inline
/// modules, for naming decoded counterexample fields. Unparseable source, tuple
/// structs and names declared more than once yield no entry; this admits nothing.
#[must_use]
pub fn struct_field_names(source: &str) -> sail_api::counterexample::FieldNames {
    fn collect(
        items: &[syn::Item],
        names: &mut sail_api::counterexample::FieldNames,
        repeated: &mut std::collections::BTreeSet<String>,
    ) {
        for item in items {
            match item {
                syn::Item::Struct(value) => {
                    let name = value.ident.to_string();
                    let syn::Fields::Named(fields) = &value.fields else {
                        repeated.insert(name);
                        continue;
                    };
                    let fields = fields
                        .named
                        .iter()
                        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
                        .collect();
                    if names.insert(name.clone(), fields).is_some() {
                        repeated.insert(name);
                    }
                }
                syn::Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        collect(items, names, repeated);
                    }
                }
                _ => {}
            }
        }
    }
    let mut names = sail_api::counterexample::FieldNames::new();
    let mut repeated = std::collections::BTreeSet::new();
    if let Ok(file) = syn::parse_file(source) {
        collect(&file.items, &mut names, &mut repeated);
    }
    names.retain(|name, _| !repeated.contains(name));
    names
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
