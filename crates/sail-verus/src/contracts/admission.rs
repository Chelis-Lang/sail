use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use sail_contracts_ir::{SourceFunction, SourceModel};
use syn::visit::Visit;

use super::types::{Struct, Type, qualify};
use super::{syntax, unsupported};
use crate::Result;

pub(super) struct Admitted {
    pub(super) model: SourceModel,
    pub(super) structs: BTreeMap<String, Struct>,
    pub(super) predicates: BTreeMap<String, Predicate>,
}

#[derive(Clone)]
pub(super) struct Predicate {
    pub(super) inputs: Vec<Type>,
}

pub(super) fn admit(text: &str) -> Result<Admitted> {
    let model = sail_contracts_ir::parse_source(text).map_err(syntax)?;
    passive(&model.file.attrs)?;
    items(&model.file.items)?;
    let mut structs = model
        .structs
        .keys()
        .map(|name| {
            (
                name.clone(),
                Struct {
                    fields: BTreeMap::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (name, value) in &model.structs {
        scalar_struct(value)?;
        let module = name.split("::").map(str::to_owned).collect::<Vec<_>>();
        let module = &module[..module.len().saturating_sub(1)];
        let mut fields = BTreeMap::new();
        for field in &value.fields {
            let ty = Type::parse(&field.ty, module, &structs)?;
            if !ty.scalar() {
                return Err(unsupported(
                    "only direct scalar struct fields are supported; references and user-defined autoderef are not admitted",
                ));
            }
            let name = field
                .ident
                .as_ref()
                .ok_or_else(|| unsupported("tuple structs are unsupported"))?;
            fields.insert(name.to_string(), ty);
        }
        structs.insert(name.clone(), Struct { fields });
    }
    let mut predicates = BTreeMap::new();
    for value in &model.functions {
        if value
            .function
            .item
            .sig
            .ident
            .to_string()
            .starts_with("__sail_")
            || value.function.item.sig.ident == "old"
        {
            return Err(unsupported(
                "old and __sail_ names are reserved by neutral lowering",
            ));
        }
        let (_, inputs, _) = signature(value, &structs)?;
        let mut body = BodyAdmission { error: None };
        body.visit_block(&value.function.item.block);
        if let Some(error) = body.error {
            return Err(error);
        }
        if value.function.predicate {
            if value.owner.is_some() || inputs.iter().any(|(_, ty)| !ty.scalar()) {
                return Err(unsupported(
                    "requirement predicates must be free functions with scalar inputs",
                ));
            }
            predicates.insert(
                identity(value),
                Predicate {
                    inputs: inputs.into_iter().map(|(_, ty)| ty).collect(),
                },
            );
        }
    }
    if model.functions.iter().all(|f| f.function.predicate) {
        return Err(unsupported(
            "neutral source contains no contracted executable function",
        ));
    }
    Ok(Admitted {
        model,
        structs,
        predicates,
    })
}

pub(super) fn identity(value: &SourceFunction) -> String {
    value
        .module
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(
            value.function.item.sig.ident.to_string().as_str(),
        ))
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) type Signature = (Option<String>, Vec<(String, Type)>, Type);

pub(super) fn signature(
    value: &SourceFunction,
    structs: &BTreeMap<String, Struct>,
) -> Result<Signature> {
    let owner = value
        .owner
        .as_ref()
        .map(|path| qualify(&value.module, path))
        .transpose()?;
    if owner
        .as_ref()
        .is_some_and(|name| !structs.contains_key(name))
    {
        return Err(unsupported(
            "inherent implementation receiver is not an admitted scalar struct",
        ));
    }
    let mut inputs = Vec::new();
    for input in &value.function.item.sig.inputs {
        match input {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(input) => {
                let syn::Pat::Ident(pattern) = input.pat.as_ref() else {
                    return Err(unsupported("contracted inputs require identifiers"));
                };
                if pattern.mutability.is_some() {
                    return Err(unsupported(
                        "mutable by-value inputs need explicit prestate lowering and are unsupported",
                    ));
                }
                inputs.push((
                    pattern.ident.to_string(),
                    resolve_type(&input.ty, value, owner.as_deref(), structs)?,
                ));
            }
        }
    }
    let output = match &value.function.item.sig.output {
        syn::ReturnType::Default => Type::Unit,
        syn::ReturnType::Type(_, output) => resolve_type(output, value, owner.as_deref(), structs)?,
    };
    Ok((owner, inputs, output))
}

fn resolve_type(
    ty: &syn::Type,
    value: &SourceFunction,
    owner: Option<&str>,
    structs: &BTreeMap<String, Struct>,
) -> Result<Type> {
    if matches!(ty, syn::Type::Path(path) if path.path.is_ident("Self")) {
        return owner
            .map(str::to_owned)
            .map(Type::Named)
            .ok_or_else(|| unsupported("Self outside an admitted inherent implementation"));
    }
    Type::parse(ty, &value.module, structs)
}

fn scalar_struct(value: &syn::ItemStruct) -> Result<()> {
    if [
        "bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "Int",
    ]
    .contains(&value.ident.to_string().as_str())
    {
        return Err(unsupported(
            "a source struct cannot shadow a neutral scalar type",
        ));
    }
    if !value.generics.params.is_empty()
        || value.generics.where_clause.is_some()
        || !matches!(value.fields, syn::Fields::Named(_))
    {
        return Err(unsupported(
            "only nongeneric named scalar structs are supported",
        ));
    }
    let mut derives = BTreeSet::new();
    for attribute in &value.attrs {
        if attribute.path().is_ident("derive") {
            let paths = attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .map_err(syntax)?;
            for path in paths {
                derives.insert(path.to_token_stream().to_string());
            }
        } else {
            passive(std::slice::from_ref(attribute))?;
        }
    }
    if derives != BTreeSet::from(["Clone".to_owned(), "Copy".to_owned()]) {
        return Err(unsupported(
            "scalar structs must derive exactly Clone and Copy; arbitrary trait implementations are unsupported",
        ));
    }
    for field in &value.fields {
        passive(&field.attrs)?;
    }
    Ok(())
}

fn passive(attributes: &[syn::Attribute]) -> Result<()> {
    if attributes.iter().any(|a| {
        !["doc", "inline", "must_use"]
            .iter()
            .any(|name| a.path().is_ident(name))
    }) {
        return Err(unsupported(
            "conditional, effectful and unresolved source attributes are unsupported",
        ));
    }
    Ok(())
}

fn annotated(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|a| sail_contracts_ir::AttributeKind::from_attribute(a).is_some())
}

fn items(values: &[syn::Item]) -> Result<()> {
    for value in values {
        match value {
            syn::Item::Fn(function) if annotated(&function.attrs) => {}
            syn::Item::Struct(_) => {}
            syn::Item::Use(import) if import.leading_colon.is_none() => imports(&import.tree)?,
            syn::Item::Mod(module) => {
                passive(&module.attrs)?;
                let Some((_, nested)) = &module.content else {
                    return Err(unsupported(
                        "out-of-line modules require a source closure and are unsupported in the single-file slice",
                    ));
                };
                items(nested)?;
            }
            syn::Item::Impl(implementation) => {
                if implementation.trait_.is_some()
                    || implementation.unsafety.is_some()
                    || !implementation.generics.params.is_empty()
                    || implementation.generics.where_clause.is_some()
                {
                    return Err(unsupported(
                        "trait, unsafe and generic implementations are unsupported, including unannotated implementations",
                    ));
                }
                passive(&implementation.attrs)?;
                for item in &implementation.items {
                    if !matches!(item, syn::ImplItem::Fn(function) if annotated(&function.attrs)) {
                        return Err(unsupported(
                            "every inherent member must be an admitted contracted function",
                        ));
                    }
                }
            }
            _ => {
                return Err(unsupported(
                    "unannotated functions, globals, traits, aliases, macros and external items are unsupported in neutral proof admission",
                ));
            }
        }
    }
    Ok(())
}

fn imports(tree: &syn::UseTree) -> Result<()> {
    let syn::UseTree::Path(path) = tree else {
        return Err(unsupported(
            "only explicit sail_contracts imports are supported",
        ));
    };
    if path.ident != "sail_contracts" {
        return Err(unsupported(
            "external imports can alter source resolution and are unsupported",
        ));
    }
    match path.tree.as_ref() {
        syn::UseTree::Name(value) if registered(&value.ident.to_string()) => Ok(()),
        syn::UseTree::Group(group) if group.items.iter().all(|item| matches!(item, syn::UseTree::Name(name) if registered(&name.ident.to_string()))) => Ok(()),
        _ => Err(unsupported("renamed, glob and unresolved contract imports are unsupported")),
    }
}

fn registered(name: &str) -> bool {
    ["requires", "ensures", "requirement", "satisfies"].contains(&name)
}

struct BodyAdmission {
    error: Option<crate::Error>,
}

impl<'ast> Visit<'ast> for BodyAdmission {
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {
        self.error.get_or_insert_with(|| unsupported("attributes inside executable bodies can change source admission and are unsupported"));
    }

    fn visit_lit(&mut self, value: &'ast syn::Lit) {
        if !matches!(value, syn::Lit::Bool(_) | syn::Lit::Int(_)) {
            self.error.get_or_insert_with(|| {
                unsupported(
                    "only Boolean and fixed-width integer executable literals are supported",
                )
            });
        }
        syn::visit::visit_lit(self, value);
    }
}
