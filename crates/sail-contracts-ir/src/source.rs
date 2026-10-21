use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::{Expr, Item, ItemFn, Stmt, visit::Visit};

use crate::{AttributeKind, ContractFunction, parse_function, unsupported};

/// One contract-bearing function in its original lexical namespace.
#[derive(Clone, Debug)]
pub struct SourceFunction {
    /// Inline module path, relative to the crate root.
    pub module: Vec<String>,
    /// The inherent implementation's named receiver type, if present.
    pub owner: Option<syn::Path>,
    /// Validated original syntax and contracts.
    pub function: ContractFunction,
}

/// Validated source and resolved requirement-predicate call closure.
#[derive(Clone, Debug)]
pub struct SourceModel {
    /// Original complete file, retained for explicit backend admission.
    pub file: syn::File,
    /// Contract-bearing functions in original lexical order.
    pub functions: Vec<SourceFunction>,
    /// Original struct definitions by qualified source identity.
    pub structs: BTreeMap<String, syn::ItemStruct>,
}

/// Parse source and revalidate every referenced predicate's actual body.
///
/// This syntax admission is not compiler type checking or backend verification.
/// # Errors
/// Rejects invalid contracts, duplicate predicate IDs, unresolved predicate calls
/// and calls whose predicate implements another requirement.
pub fn parse_source(text: &str) -> syn::Result<SourceModel> {
    if text.len() > 1024 * 1024 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "SAIL0701: neutral source exceeds one MiB",
        ));
    }
    let file = syn::parse_file(text)?;
    let mut functions = Vec::new();
    let mut structs = BTreeMap::new();
    collect(&file.items, &[], &mut functions, &mut structs)?;
    let mut predicates = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for value in &functions {
        if value.function.predicate {
            if !identities.insert(value.function.requirement.clone()) {
                return Err(unsupported(
                    &value.function.item.sig,
                    "a requirement ID has two predicate definitions",
                ));
            }
            let name = qualified(&value.module, &value.function.item.sig.ident.to_string());
            if predicates.insert(name, &value.function).is_some() {
                return Err(unsupported(
                    &value.function.item.sig,
                    "duplicate predicate source identity",
                ));
            }
        }
    }
    for value in &functions {
        let mut calls = Calls::default();
        for expression in &value.function.requires {
            calls.visit_expr(expression);
        }
        if let Some(guarantee) = &value.function.ensures {
            calls.visit_expr(&guarantee.body);
        }
        for path in calls.paths {
            let name = resolve(&value.module, &path)?;
            let predicate = predicates.get(&name).ok_or_else(|| {
                unsupported(
                    &path,
                    "contract call does not resolve to a validated requirement predicate",
                )
            })?;
            if predicate.requirement != value.function.requirement {
                return Err(unsupported(
                    path,
                    "predicate requirement differs from the function's satisfies identity",
                ));
            }
        }
    }
    Ok(SourceModel {
        file,
        functions,
        structs,
    })
}

fn qualified(module: &[String], name: &str) -> String {
    module
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join("::")
}

fn resolve(module: &[String], path: &syn::Path) -> syn::Result<String> {
    let mut parts = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>();
    if parts.first().is_some_and(|s| s == "crate") {
        parts.remove(0);
    } else {
        if path.leading_colon.is_some() || parts.first().is_some_and(|s| s == "super") {
            return Err(unsupported(
                path,
                "predicate paths must be local, self-qualified or crate-qualified",
            ));
        }
        if parts.first().is_some_and(|s| s == "self") {
            parts.remove(0);
        }
        parts.splice(0..0, module.iter().cloned());
    }
    Ok(parts.join("::"))
}

fn collect(
    items: &[Item],
    module: &[String],
    functions: &mut Vec<SourceFunction>,
    structs: &mut BTreeMap<String, syn::ItemStruct>,
) -> syn::Result<()> {
    for item in items {
        match item {
            Item::Fn(value) if annotated(&value.attrs) => functions.push(SourceFunction {
                module: module.to_vec(),
                owner: None,
                function: parse_function(value.clone(), None)?,
            }),
            Item::Mod(value) => {
                if let Some((_, items)) = &value.content {
                    let mut nested = module.to_vec();
                    nested.push(value.ident.to_string());
                    collect(items, &nested, functions, structs)?;
                }
            }
            Item::Struct(value) => {
                structs.insert(qualified(module, &value.ident.to_string()), value.clone());
            }
            Item::Impl(value) => {
                if !value
                    .items
                    .iter()
                    .any(|item| matches!(item, syn::ImplItem::Fn(f) if annotated(&f.attrs)))
                {
                    continue;
                }
                if value.trait_.is_some()
                    || value.unsafety.is_some()
                    || !value.generics.params.is_empty()
                    || value.generics.where_clause.is_some()
                {
                    return Err(unsupported(
                        value,
                        "trait, unsafe and generic implementations are unsupported",
                    ));
                }
                let syn::Type::Path(owner) = value.self_ty.as_ref() else {
                    return Err(unsupported(
                        value,
                        "inherent receiver must be a named struct",
                    ));
                };
                for item in &value.items {
                    if let syn::ImplItem::Fn(function) = item
                        && annotated(&function.attrs)
                    {
                        let function =
                            parse_function(syn::parse2(function.to_token_stream())?, None)?;
                        functions.push(SourceFunction {
                            module: module.to_vec(),
                            owner: Some(owner.path.clone()),
                            function,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn annotated(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|a| AttributeKind::from_attribute(a).is_some())
}

#[derive(Default)]
struct Calls {
    paths: Vec<syn::Path>,
}

impl<'ast> Visit<'ast> for Calls {
    fn visit_expr_call(&mut self, value: &'ast syn::ExprCall) {
        if let Expr::Path(path) = value.func.as_ref()
            && !path.path.is_ident("old")
        {
            self.paths.push(path.path.clone());
        }
        syn::visit::visit_expr_call(self, value);
    }
}

pub(crate) fn validate_body(item: &ItemFn) -> syn::Result<()> {
    let mut validator = Body { error: None };
    validator.visit_block(&item.block);
    validator.error.map_or(Ok(()), Err)
}

struct Body {
    error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for Body {
    fn visit_stmt(&mut self, value: &'ast Stmt) {
        if !matches!(value, Stmt::Local(_) | Stmt::Expr(_, _)) {
            self.error.get_or_insert_with(|| {
                unsupported(
                    value,
                    "items and macros in neutral function bodies are unsupported",
                )
            });
        }
        syn::visit::visit_stmt(self, value);
    }

    fn visit_expr(&mut self, value: &'ast Expr) {
        if !matches!(
            value,
            Expr::Lit(_)
                | Expr::Path(_)
                | Expr::Field(_)
                | Expr::Paren(_)
                | Expr::Group(_)
                | Expr::Unary(_)
                | Expr::Binary(_)
                | Expr::If(_)
                | Expr::Block(_)
                | Expr::Assign(_)
                | Expr::Return(_)
                | Expr::Struct(_)
                | Expr::Tuple(_)
        ) {
            self.error.get_or_insert_with(|| unsupported(value, "unsupported neutral executable body; calls, effects, loops and macros are not admitted"));
        }
        if let Expr::Unary(value) = value
            && !matches!(value.op, syn::UnOp::Not(_) | syn::UnOp::Neg(_))
        {
            self.error.get_or_insert_with(|| {
                unsupported(
                    value,
                    "dereferencing and arbitrary borrowing are unsupported",
                )
            });
        }
        syn::visit::visit_expr(self, value);
    }
}
