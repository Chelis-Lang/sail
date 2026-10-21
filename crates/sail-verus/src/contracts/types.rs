use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::unsupported;
use crate::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Type {
    Bool,
    Integer { signed: bool, bits: u32 },
    Named(String),
    Unit,
}

impl Type {
    pub(super) fn parse(
        value: &syn::Type,
        module: &[String],
        structs: &BTreeMap<String, Struct>,
    ) -> Result<Self> {
        if matches!(value, syn::Type::Tuple(tuple) if tuple.elems.is_empty()) {
            return Ok(Self::Unit);
        }
        let syn::Type::Path(path) = value else {
            return Err(unsupported(
                "only owned scalar types and admitted scalar structs are supported",
            ));
        };
        if path.qself.is_some() || path.path.segments.iter().any(|s| !s.arguments.is_empty()) {
            return Err(unsupported(
                "generic and associated contract types are unsupported",
            ));
        }
        let text = path.path.to_token_stream().to_string().replace(' ', "");
        if text == "bool" {
            return Ok(Self::Bool);
        }
        for bits in [8, 16, 32, 64, 128] {
            if text == format!("u{bits}") {
                return Ok(Self::Integer {
                    signed: false,
                    bits,
                });
            }
            if text == format!("i{bits}") {
                return Ok(Self::Integer { signed: true, bits });
            }
        }
        let name = qualify(module, &path.path)?;
        if structs.contains_key(&name) {
            return Ok(Self::Named(name));
        }
        Err(unsupported(format!(
            "unresolved or unsupported contract type {text}; Int execution, pointer-sized types and user-defined dereferencing are unsupported"
        )))
    }

    pub(super) fn numeric(&self) -> Result<()> {
        if matches!(self, Self::Integer { .. }) {
            Ok(())
        } else {
            Err(unsupported(
                "arithmetic needs an admitted fixed-width integer",
            ))
        }
    }

    pub(super) fn bounds(&self, value: &TokenStream) -> Result<TokenStream> {
        let Self::Integer { signed, bits } = self else {
            return Err(unsupported("a numeric bound needs an integer type"));
        };
        let name: syn::Ident =
            syn::parse_str(&format!("{}{bits}", if *signed { 'i' } else { 'u' }))
                .map_err(super::syntax)?;
        Ok(quote! { ((#name::MIN as int) <= (#value) && (#value) <= (#name::MAX as int)) })
    }

    pub(super) fn scalar(&self) -> bool {
        matches!(self, Self::Bool | Self::Integer { .. })
    }

    pub(super) fn rust(&self) -> Result<TokenStream> {
        match self {
            Self::Bool => Ok(quote!(bool)),
            Self::Unit => Ok(quote!(())),
            Self::Integer { signed, bits } => {
                let name: syn::Ident =
                    syn::parse_str(&format!("{}{bits}", if *signed { 'i' } else { 'u' }))
                        .map_err(super::syntax)?;
                Ok(quote!(#name))
            }
            Self::Named(name) => {
                let path: syn::Path =
                    syn::parse_str(&format!("crate::{name}")).map_err(super::syntax)?;
                Ok(quote!(#path))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Struct {
    pub(super) fields: BTreeMap<String, Type>,
}

pub(super) fn qualify(module: &[String], path: &syn::Path) -> Result<String> {
    let mut names = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>();
    if names.first().is_some_and(|s| s == "crate") {
        names.remove(0);
    } else {
        if path.leading_colon.is_some() || names.first().is_some_and(|s| s == "super") {
            return Err(unsupported(
                "external and super-qualified source paths are unsupported",
            ));
        }
        if names.first().is_some_and(|s| s == "self") {
            names.remove(0);
        }
        names.splice(0..0, module.iter().cloned());
    }
    Ok(names.join("::"))
}
