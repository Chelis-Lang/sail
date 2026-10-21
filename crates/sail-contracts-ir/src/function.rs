use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::visit::Visit;
use syn::{Attribute, Expr, ExprClosure, FnArg, ItemFn, Pat, ReturnType, Type};

use crate::{ExpressionContext, unsupported, validate_expression};

/// One registered neutral contract attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeKind {
    /// A precondition evaluated at entry.
    Requires,
    /// A guarantee evaluated after normal return.
    Ensures,
    /// A pure requirement predicate definition.
    Requirement,
    /// The requirement identity implemented by a contracted function.
    Satisfies,
}

impl AttributeKind {
    /// Recognize unqualified or `sail_contracts`-qualified source attributes.
    #[must_use]
    pub fn from_attribute(attribute: &Attribute) -> Option<Self> {
        let path = attribute.path();
        if path.segments.len() > 2
            || (path.segments.len() == 2 && path.segments[0].ident != "sail_contracts")
        {
            return None;
        }
        match path.segments.last()?.ident.to_string().as_str() {
            "requires" => Some(Self::Requires),
            "ensures" => Some(Self::Ensures),
            "requirement" => Some(Self::Requirement),
            "satisfies" => Some(Self::Satisfies),
            _ => None,
        }
    }
}

/// An explicit, stable owner requirement identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequirementId(String);

impl RequirementId {
    /// Validate an owner requirement identity.
    /// # Errors
    /// Rejects empty, oversized or nonportable identities.
    pub fn parse(value: &str) -> syn::Result<Self> {
        if value.is_empty()
            || value.len() > 256
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.:/-".contains(&b))
        {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "SAIL0701: invalid requirement ID",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The original owner identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated function and its unexpanded source contracts.
#[derive(Clone, Debug)]
pub struct ContractFunction {
    /// Original function, with registered contract attributes removed.
    pub item: ItemFn,
    /// Explicit owner identity, never a generated source position.
    pub requirement: RequirementId,
    /// Whether this defines the pure requirement predicate.
    pub predicate: bool,
    /// Preconditions, in original evaluation order.
    pub requires: Vec<Expr>,
    /// The one postcondition; absent only on a predicate definition.
    pub ensures: Option<ExprClosure>,
}

/// Parse a neutral function, optionally restoring the outer attribute consumed by Rust.
/// # Errors
/// Rejects unsupported signatures, bodies, attributes and specification expressions.
pub fn parse_function(
    mut item: ItemFn,
    outer: Option<(AttributeKind, TokenStream)>,
) -> syn::Result<ContractFunction> {
    validate_signature(&item)?;
    let attributes = attributes(&mut item, outer)?;
    let bindings = bindings(&item)?;
    let mut requires = Vec::new();
    let mut ensures = None;
    let mut identity = None;
    let mut predicate = false;
    for (kind, tokens) in attributes {
        match kind {
            AttributeKind::Requires => {
                let expression = syn::parse2::<Expr>(tokens)?;
                validate_expression(
                    &expression,
                    &ExpressionContext {
                        bindings: bindings.clone(),
                        old: false,
                        predicate_calls: true,
                    },
                )?;
                requires.push(expression);
            }
            AttributeKind::Ensures => {
                if ensures.is_some() {
                    return Err(unsupported(&item.sig, "exactly one ensures is supported"));
                }
                ensures = Some(parse_guarantee(tokens, &bindings)?);
            }
            AttributeKind::Requirement | AttributeKind::Satisfies => {
                if identity.is_some() {
                    return Err(unsupported(
                        &item.sig,
                        "exactly one requirement identity is supported",
                    ));
                }
                let text = syn::parse2::<syn::LitStr>(tokens)?;
                identity = Some(RequirementId::parse(&text.value())?);
                predicate = kind == AttributeKind::Requirement;
            }
        }
    }
    let requirement = identity.ok_or_else(|| {
        unsupported(
            &item.sig,
            "a neutral function needs requirement or satisfies",
        )
    })?;
    if predicate {
        if !requires.is_empty()
            || ensures.is_some()
            || item.sig.receiver().is_some()
            || !returns_bool(&item.sig.output)
        {
            return Err(unsupported(
                &item.sig,
                "a requirement predicate needs scalar inputs and a Boolean result, without contracts",
            ));
        }
        validate_expression(
            &syn::parse2::<Expr>(item.block.to_token_stream())?,
            &ExpressionContext {
                bindings,
                old: false,
                predicate_calls: false,
            },
        )?;
    } else {
        if ensures.is_none() {
            return Err(unsupported(
                &item.sig,
                "a contracted function needs one ensures",
            ));
        }
        super::source::validate_body(&item)?;
    }
    Ok(ContractFunction {
        item,
        requirement,
        predicate,
        requires,
        ensures,
    })
}

fn returns_bool(output: &ReturnType) -> bool {
    matches!(output, ReturnType::Type(_, value) if matches!(value.as_ref(), Type::Path(p) if p.path.is_ident("bool")))
}

fn validate_signature(item: &ItemFn) -> syn::Result<()> {
    let signature = &item.sig;
    if signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.constness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
    {
        return Err(unsupported(
            signature,
            "async, unsafe, const, extern, variadic and generic neutral functions are unsupported",
        ));
    }
    for argument in &signature.inputs {
        if let FnArg::Typed(value) = argument {
            validate_type(&value.ty)?;
        }
    }
    if let ReturnType::Type(_, value) = &signature.output {
        validate_type(value)?;
    }
    Ok(())
}

pub(crate) fn validate_type(value: &Type) -> syn::Result<()> {
    match value {
        Type::Path(path)
            if path.qself.is_none()
                && path.path.segments.iter().all(|s| s.arguments.is_empty()) =>
        {
            Ok(())
        }
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(()),
        _ => Err(unsupported(
            value,
            "only owned scalar or scalar-struct types are supported",
        )),
    }
}

fn bindings(item: &ItemFn) -> syn::Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for argument in &item.sig.inputs {
        match argument {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_none() || receiver.colon_token.is_some() {
                    return Err(unsupported(
                        receiver,
                        "only shared or mutable reference receivers are supported",
                    ));
                }
                names.insert("self".into());
            }
            FnArg::Typed(value) => {
                let Pat::Ident(pattern) = value.pat.as_ref() else {
                    return Err(unsupported(value, "function inputs need plain identifiers"));
                };
                if pattern.by_ref.is_some() || pattern.subpat.is_some() {
                    return Err(unsupported(
                        pattern,
                        "input patterns cannot borrow or destructure",
                    ));
                }
                names.insert(pattern.ident.to_string());
            }
        }
    }
    Ok(names)
}

fn parse_guarantee(tokens: TokenStream, bindings: &BTreeSet<String>) -> syn::Result<ExprClosure> {
    let closure = syn::parse2::<ExprClosure>(tokens)?;
    if closure.inputs.len() != 1
        || closure.asyncness.is_some()
        || closure.movability.is_some()
        || closure.capture.is_some()
        || !matches!(closure.output, ReturnType::Default)
    {
        return Err(unsupported(
            closure,
            "ensures needs one plain result binding",
        ));
    }
    let Some(Pat::Ident(result)) = closure.inputs.first() else {
        return Err(unsupported(closure, "ensures result must be an identifier"));
    };
    if result.by_ref.is_some() || result.mutability.is_some() || result.subpat.is_some() {
        return Err(unsupported(
            closure,
            "ensures result must be a plain identifier",
        ));
    }
    if bindings.contains(&result.ident.to_string()) {
        return Err(unsupported(
            result,
            "ensures result shadows an input binding",
        ));
    }
    let mut post_bindings = bindings.clone();
    post_bindings.insert(result.ident.to_string());
    validate_expression(
        &closure.body,
        &ExpressionContext {
            bindings: post_bindings,
            old: true,
            predicate_calls: true,
        },
    )?;
    let mut entry = EntryCaptures {
        bindings,
        error: None,
    };
    entry.visit_expr(&closure.body);
    if let Some(error) = entry.error {
        return Err(error);
    }
    Ok(closure)
}

struct EntryCaptures<'a> {
    bindings: &'a BTreeSet<String>,
    error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for EntryCaptures<'_> {
    fn visit_expr_call(&mut self, value: &'ast syn::ExprCall) {
        if matches!(value.func.as_ref(), Expr::Path(path) if path.path.is_ident("old")) {
            for argument in &value.args {
                if let Err(error) = validate_expression(
                    argument,
                    &ExpressionContext {
                        bindings: self.bindings.clone(),
                        old: false,
                        predicate_calls: false,
                    },
                ) {
                    self.error.get_or_insert(error);
                }
            }
        }
        syn::visit::visit_expr_call(self, value);
    }
}

fn attributes(
    item: &mut ItemFn,
    outer: Option<(AttributeKind, TokenStream)>,
) -> syn::Result<Vec<(AttributeKind, TokenStream)>> {
    let mut attributes = Vec::new();
    if let Some(value) = outer {
        attributes.push(value);
    }
    let mut retained = Vec::new();
    for attribute in std::mem::take(&mut item.attrs) {
        if let Some(kind) = AttributeKind::from_attribute(&attribute) {
            let syn::Meta::List(meta) = attribute.meta else {
                return Err(unsupported(attribute, "contract attributes need arguments"));
            };
            attributes.push((kind, meta.tokens));
        } else if ["doc", "inline", "must_use"]
            .iter()
            .any(|name| attribute.path().is_ident(name))
        {
            retained.push(attribute);
        } else {
            return Err(unsupported(
                attribute,
                "unsupported attribute on a neutral function",
            ));
        }
    }
    item.attrs = retained;
    Ok(attributes)
}
