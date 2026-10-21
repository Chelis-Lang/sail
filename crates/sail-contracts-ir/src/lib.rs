//! Shared, validated neutral contract syntax. This crate never establishes a proof.
#![forbid(unsafe_code)]

mod expression;
mod function;
mod source;

pub use expression::{ExpressionContext, validate_expression};
pub use function::{AttributeKind, ContractFunction, RequirementId, parse_function};
pub use source::{SourceFunction, SourceModel, parse_source};

/// The version of the supported neutral source semantics.
pub const VERSION: u32 = 1;

/// A stable unsupported-source diagnostic at the relevant original syntax.
pub(crate) fn unsupported(node: impl quote::ToTokens, message: &str) -> syn::Error {
    syn::Error::new_spanned(node, format!("SAIL0701: {message}"))
}
