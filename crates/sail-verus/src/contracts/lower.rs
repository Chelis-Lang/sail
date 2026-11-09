use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use sail_contracts_ir::SourceFunction;
use syn::ext::IdentExt;
use syn::{fold::Fold, spanned::Spanned, visit::Visit};

use super::admission::{Admitted, admit_closure, identity, signature};
use super::expression::{Environment, Phase};
use super::types::Type;
use super::{
    ContractBinding, LOWERING_VERSION, LoweredSource, LoweringManifest, ModuleDigest, ModuleSource,
    PROFILE_THREE_LOWERING_VERSION, SourceRange, digest, unsupported,
};
use crate::Result;
use sail_api::contracts::{MAX_CLOSURE_BYTES, MAX_SOURCE_BYTES, closure_sha256};

/// Admit the complete source file and lower supported neutral contracts to Verus.
///
/// No source is written, process started or verification verdict constructed.
/// # Errors
/// `SAIL3140` for unsupported, unresolved, effectful or ill-typed source.
pub fn lower(source: &str, crate_name: &str) -> Result<LoweredSource> {
    lower_profile(source, crate_name, 1)
}

/// Lower an explicitly selected source profile without executing verification.
/// # Errors
/// Rejects unsupported profiles, source or type admission.
pub fn lower_profile(source: &str, crate_name: &str, profile: u32) -> Result<LoweredSource> {
    lower_closure(source, &[], crate_name, profile)
}

/// Lower a crate root and its out-of-line module files as one source closure.
///
/// The closure lowers to one Verus file in which each `mod name;` becomes an
/// inline module holding its submitted file. Module files need profile three.
/// # Errors
/// Rejects unsupported profiles, files outside the declared closure, digest
/// mismatches and source or type admission failures.
pub fn lower_closure(
    source: &str,
    modules: &[ModuleSource],
    crate_name: &str,
    profile: u32,
) -> Result<LoweredSource> {
    let version = match profile {
        1 => LOWERING_VERSION,
        2 => 3,
        3 => PROFILE_THREE_LOWERING_VERSION,
        _ => return Err(unsupported("unsupported source profile")),
    };
    lower_version(source, modules, crate_name, version)
}

/// Recheck retained source-to-shadow identity using corrected expression semantics.
///
/// Historical version-one bytes remain valid only when their complete manifest
/// equals the corrected projection with the original version-one header. This
/// never reconstructs the faulty field semantics or rewrites retained evidence.
/// # Errors
/// `SAIL3140` for unsupported versions, source or any manifest mismatch.
pub fn validate_manifest(source: &str, manifest: &LoweringManifest) -> Result<()> {
    validate_closure_manifest(source, &[], manifest)
}

/// Recheck a retained closure lowering against its exact root and module files.
/// # Errors
/// `SAIL3140` for unsupported versions, source or any manifest mismatch.
pub fn validate_closure_manifest(
    source: &str,
    modules: &[ModuleSource],
    manifest: &LoweringManifest,
) -> Result<()> {
    if !matches!(
        manifest.version,
        1 | LOWERING_VERSION | 3 | PROFILE_THREE_LOWERING_VERSION
    ) {
        return Err(unsupported("unsupported retained neutral lowering version"));
    }
    let expected = lower_version(source, modules, &manifest.crate_name, manifest.version)?;
    if expected.manifest != *manifest {
        return Err(unsupported(
            "retained neutral lowering differs from the corrected exact source projection",
        ));
    }
    Ok(())
}

fn lower_version(
    source: &str,
    modules: &[ModuleSource],
    crate_name: &str,
    version: u32,
) -> Result<LoweredSource> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(unsupported(
            "single-file neutral proof input exceeds 64 KiB",
        ));
    }
    if modules.iter().any(|m| {
        m.source.len() > MAX_SOURCE_BYTES || m.source_sha256 != digest(m.source.as_bytes())
    }) || modules.iter().map(|m| m.source.len()).sum::<usize>() + source.len()
        > MAX_CLOSURE_BYTES
        || !modules.windows(2).all(|w| w[0].module < w[1].module)
    {
        return Err(unsupported(
            "module files need exact digests, sorted unique module paths and bounded size",
        ));
    }
    let identifier = syn::parse_str::<syn::Ident>(crate_name).map_err(super::syntax)?;
    if identifier != crate_name || ["self", "Self", "crate", "super", "vstd"].contains(&crate_name)
    {
        return Err(unsupported("invalid compiler crate name"));
    }
    let profile = match version {
        3 => 2,
        PROFILE_THREE_LOWERING_VERSION => 3,
        _ => 1,
    };
    let files = modules
        .iter()
        .map(|m| (m.module.as_str(), m.source.as_str()))
        .collect::<Vec<_>>();
    let admitted = admit_closure(source, &files, profile)?;
    let mut bindings = Vec::new();
    let items = module(
        &admitted,
        &admitted.model.file.items,
        &[],
        crate_name,
        &mut bindings,
    )?;
    let helpers = if profile >= 3 {
        truncating_helpers()
    } else {
        TokenStream::new()
    };
    let source_code = format!(
        "// Generated by Sail neutral-contract lowering version {version}.\nuse vstd::prelude::*;\nverus! {{\n{helpers}{items}\n}}\n"
    );
    let module_digests = modules
        .iter()
        .map(|m| ModuleDigest {
            module: m.module.clone(),
            sha256: m.source_sha256.clone(),
        })
        .collect::<Vec<_>>();
    if source_code.len() > 16 * 1024 * 1024 {
        return Err(unsupported("neutral shadow expansion exceeds 16 MiB"));
    }
    bindings.sort_by(|left, right| {
        (&left.function, &left.requirement_id).cmp(&(&right.function, &right.requirement_id))
    });
    let manifest = LoweringManifest {
        version,
        contract_version: profile,
        crate_name: crate_name.to_owned(),
        original_sha256: closure_sha256(&digest(source.as_bytes()), &module_digests),
        lowered_sha256: digest(source_code.as_bytes()),
        arithmetic: arithmetic(profile).into(),
        bindings,
        modules: module_digests,
    };
    Ok(LoweredSource {
        source: source_code,
        manifest,
    })
}

/// The deepest out-of-line module file that declares a bound function.
///
/// The declaration site decides, not the function identity: an inherent
/// `impl` may sit in a different module file from its receiver type, and its
/// source range belongs to the file that holds the `impl`.
fn module_file(admitted: &Admitted, module: &[String]) -> Option<String> {
    (1..=module.len())
        .rev()
        .map(|end| &module[..end])
        .find(|prefix| admitted.model.module_files.contains(*prefix))
        .map(|prefix| prefix.join("::"))
}

const ARITHMETIC: &str = "fixed-width checked specification arithmetic; executable overflow is a proof obligation; mathematical intermediate values with explicit bounds";
const PROFILE_THREE_ARITHMETIC: &str = "fixed-width checked specification arithmetic; executable overflow is a proof obligation; mathematical intermediate values with explicit bounds; division and remainder truncate toward zero as in Rust, the remainder takes the dividend's sign, and each is defined only for a nonzero divisor and a representable result";

fn arithmetic(profile: u32) -> &'static str {
    if profile >= 3 {
        PROFILE_THREE_ARITHMETIC
    } else {
        ARITHMETIC
    }
}

/// Rust's truncating signed division and remainder over mathematical integers.
///
/// Verus's `int` division is Euclidean. For a nonnegative dividend and a
/// positive divisor the two agree, so division reduces to magnitudes and
/// restores the truncating sign. The remainder has the shape Verus itself
/// gives executable `%`; a Euclidean remainder ignores the divisor's sign, so
/// it equals the magnitude form, and the solver can match it to the body's
/// remainder for a symbolic divisor. Callers add the definedness conditions.
fn truncating_helpers() -> TokenStream {
    quote! {
        #[doc(hidden)]
        pub open spec fn __sail_truncating_div(dividend: int, divisor: int) -> int {
            if dividend >= 0 {
                if divisor > 0 { dividend / divisor } else { -(dividend / (-divisor)) }
            } else if divisor > 0 {
                -((-dividend) / divisor)
            } else {
                (-dividend) / (-divisor)
            }
        }
        #[doc(hidden)]
        pub open spec fn __sail_truncating_rem(dividend: int, divisor: int) -> int {
            if dividend == 0 {
                0
            } else if dividend > 0 {
                dividend % divisor
            } else {
                -((-dividend) % divisor)
            }
        }
    }
}

fn module(
    admitted: &Admitted,
    items: &[syn::Item],
    path: &[String],
    crate_name: &str,
    bindings: &mut Vec<ContractBinding>,
) -> Result<TokenStream> {
    let mut output = Vec::new();
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                let value = find(admitted, path, None, &function.sig.ident)?;
                output.push(function_source(admitted, value, crate_name, bindings)?);
            }
            syn::Item::Struct(value) => output.push(quote!(#value)),
            syn::Item::Enum(value) => output.push(enum_source(value)?),
            // Profile three keeps crate-internal imports, which admission
            // resolved to admitted items; contract imports are dropped.
            syn::Item::Use(value)
                if admitted.model.profile >= 3
                    && matches!(&value.tree, syn::UseTree::Path(p) if p.ident != "sail_contracts") =>
            {
                output.push(quote!(#value));
            }
            syn::Item::Use(_) => {}
            syn::Item::Mod(value) => {
                let (_, items) = value
                    .content
                    .as_ref()
                    .ok_or_else(|| unsupported("unadmitted external module"))?;
                let mut nested = path.to_vec();
                nested.push(value.ident.to_string());
                let body = module(admitted, items, &nested, crate_name, bindings)?;
                let visibility = &value.vis;
                let name = &value.ident;
                // Profile three brings the Verus prelude into each module, so
                // nested enums resolve their structural-equality derive.
                let prelude = if admitted.model.profile >= 3 {
                    quote!(
                        use vstd::prelude::*;
                    )
                } else {
                    TokenStream::new()
                };
                output.push(quote!(#visibility mod #name { #prelude #body }));
            }
            syn::Item::Impl(value) => {
                let syn::Type::Path(owner) = value.self_ty.as_ref() else {
                    return Err(unsupported("unadmitted receiver"));
                };
                let mut functions = Vec::new();
                for item in &value.items {
                    let syn::ImplItem::Fn(function) = item else {
                        return Err(unsupported("unadmitted implementation member"));
                    };
                    let source = find(admitted, path, Some(&owner.path), &function.sig.ident)?;
                    functions.push(function_source(admitted, source, crate_name, bindings)?);
                }
                let owner = &value.self_ty;
                output.push(quote!(impl #owner { #(#functions)* }));
            }
            _ => {
                return Err(unsupported(
                    "item bypassed complete neutral source admission",
                ));
            }
        }
    }
    Ok(quote!(#(#output)*))
}

fn find<'a>(
    admitted: &'a Admitted,
    module: &[String],
    owner: Option<&syn::Path>,
    name: &syn::Ident,
) -> Result<&'a SourceFunction> {
    admitted
        .model
        .functions
        .iter()
        .find(|value| {
            value.module == module
                && value.owner.as_ref() == owner
                && value.function.item.sig.ident == *name
        })
        .ok_or_else(|| unsupported("source function has no validated contract"))
}

fn function_source(
    admitted: &Admitted,
    source: &SourceFunction,
    crate_name: &str,
    bindings: &mut Vec<ContractBinding>,
) -> Result<TokenStream> {
    let (owner, inputs, output) = signature(
        source,
        &admitted.structs,
        &admitted.model.imports,
        admitted.model.profile,
    )?;
    let mut values = inputs.into_iter().collect::<BTreeMap<_, _>>();
    if let Some(owner) = owner.as_ref() {
        values.insert("self".into(), Type::Named(owner.clone()));
    }
    let environment = Environment {
        profile: admitted.model.profile,
        module: &source.module,
        values,
        structs: &admitted.structs,
        predicates: &admitted.predicates,
        imports: &admitted.model.imports,
        mutable_receiver: source
            .function
            .item
            .sig
            .receiver()
            .is_some_and(|r| r.mutability.is_some()),
    };
    if source.function.predicate {
        return predicate_source(source, &environment);
    }
    let body = loops(&environment, source)?;
    let guarantee = source
        .function
        .ensures
        .as_ref()
        .ok_or_else(|| unsupported("unadmitted missing guarantee"))?;
    let Some(syn::Pat::Ident(result)) = guarantee.inputs.first() else {
        return Err(unsupported("unadmitted result binding"));
    };
    let mut environment = environment;
    environment
        .values
        .insert(result.ident.to_string(), output.clone());
    let post = environment.translate(&guarantee.body, Some(&Type::Bool), Phase::Post)?;
    if post.ty != Type::Bool {
        return Err(unsupported("postcondition is not Boolean"));
    }
    let mut preconditions = Vec::new();
    for requirement in &source.function.requires {
        let value = environment.translate(requirement, Some(&Type::Bool), Phase::Current)?;
        if value.ty != Type::Bool {
            return Err(unsupported("precondition is not Boolean"));
        }
        let expression = value.expression;
        let defined = value.defined;
        preconditions.push(quote!((#defined) && (#expression)));
    }
    let mut captures = Captures::default();
    captures.visit_expr(&guarantee.body);
    let mut assertions = Vec::new();
    for capture in captures.expressions {
        let value = environment.translate(&capture, None, Phase::Current)?;
        if !matches!(value.ty, Type::Bool | Type::Integer { .. }) {
            return Err(unsupported("old snapshot is not scalar"));
        }
        let defined = value.defined;
        assertions.push(quote!(assert(#defined);));
    }
    let visibility = &source.function.item.vis;
    let name = &source.function.item.sig.ident;
    let arguments = &source.function.item.sig.inputs;
    let result = &result.ident;
    let output_type = output.rust()?;
    let defined = post.defined;
    let guarantee_value = post.expression;
    let requires = if preconditions.is_empty() {
        TokenStream::new()
    } else {
        quote!(requires #(#preconditions),*,)
    };
    let body_statements = &body.stmts;
    let lowered = quote! {
        #visibility fn #name(#arguments) -> (#result: #output_type)
            #requires
            ensures (#defined) && (#guarantee_value),
        { #(#assertions)* #(#body_statements)* }
    };
    for requirement in &source.function.requirements {
        let mut mapped = binding(
            source,
            owner.as_deref(),
            crate_name,
            &guarantee.to_token_stream(),
        );
        requirement.as_str().clone_into(&mut mapped.requirement_id);
        mapped.module_file = module_file(admitted, &source.module);
        bindings.push(mapped);
    }
    Ok(lowered)
}

/// Lower each profile-three `while` loop's neutral head to Verus loop clauses.
///
/// Invariants use the specification semantics of `requires`, with the same
/// definedness conjunct. The measure is its mathematical value; Verus proves it
/// nonnegative and decreasing. Locals are visible to the head only through an
/// explicit scalar type annotation, and a function with loops needs unique local
/// names, so each name has one type.
fn loops(environment: &Environment<'_>, source: &SourceFunction) -> Result<syn::Block> {
    let block = source.function.item.block.as_ref().clone();
    let mut probe = Locals::default();
    probe.visit_block(&block);
    if environment.profile < 3 || !probe.loops {
        return Ok(block);
    }
    if let Some(error) = probe.error {
        return Err(error);
    }
    let mut values = environment.values.clone();
    let mut names = BTreeSet::new();
    for (name, ty) in probe.names {
        if values.contains_key(&name) || !names.insert(name.clone()) {
            return Err(unsupported(format!(
                "functions with loops need unique local names; {name} is declared twice or shadows an input"
            )));
        }
        if let Some(ty) = ty {
            let ty = Type::parse(
                &ty,
                environment.module,
                environment.structs,
                environment.imports,
            )?;
            if matches!(ty, Type::Bool | Type::Integer { .. }) {
                values.insert(name, ty);
            }
        }
    }
    let scope = Environment {
        profile: environment.profile,
        module: environment.module,
        values,
        structs: environment.structs,
        predicates: environment.predicates,
        imports: environment.imports,
        mutable_receiver: environment.mutable_receiver,
    };
    let mut lowering = Loops {
        environment: &scope,
        error: None,
    };
    let block = lowering.fold_block(block);
    lowering.error.map_or(Ok(block), Err)
}

#[derive(Default)]
struct Locals {
    loops: bool,
    names: Vec<(String, Option<syn::Type>)>,
    error: Option<crate::Error>,
}

impl<'ast> Visit<'ast> for Locals {
    fn visit_expr_while(&mut self, value: &'ast syn::ExprWhile) {
        self.loops = true;
        syn::visit::visit_expr_while(self, value);
    }

    fn visit_local(&mut self, value: &'ast syn::Local) {
        match &value.pat {
            syn::Pat::Ident(pattern) if pattern.subpat.is_none() && pattern.by_ref.is_none() => {
                self.names.push((pattern.ident.unraw().to_string(), None));
            }
            syn::Pat::Type(typed) => match typed.pat.as_ref() {
                syn::Pat::Ident(pattern)
                    if pattern.subpat.is_none() && pattern.by_ref.is_none() =>
                {
                    self.names
                        .push((pattern.ident.unraw().to_string(), Some((*typed.ty).clone())));
                }
                _ => {
                    self.error.get_or_insert_with(|| {
                        unsupported("functions with loops need plain identifier locals")
                    });
                }
            },
            _ => {
                self.error.get_or_insert_with(|| {
                    unsupported("functions with loops need plain identifier locals")
                });
            }
        }
        syn::visit::visit_local(self, value);
    }
}

struct Loops<'a> {
    environment: &'a Environment<'a>,
    error: Option<crate::Error>,
}

impl Loops<'_> {
    fn lower(&self, value: &syn::ExprWhile) -> Result<TokenStream> {
        let head = sail_contracts_ir::loop_head(&value.body)
            .ok_or_else(|| unsupported("unadmitted loop head"))?;
        let mut captures = Captures::default();
        for invariant in &head.invariants {
            captures.visit_expr(invariant);
        }
        captures.visit_expr(&head.measure);
        if !captures.expressions.is_empty() {
            return Err(unsupported(
                "old is unsupported in loop invariants and measures",
            ));
        }
        let mut invariants = Vec::new();
        for invariant in &head.invariants {
            let value = self
                .environment
                .translate(invariant, Some(&Type::Bool), Phase::Current)?;
            if value.ty != Type::Bool {
                return Err(unsupported("loop invariant is not Boolean"));
            }
            let (defined, expression) = (value.defined, value.expression);
            invariants.push(quote!((#defined) && (#expression)));
        }
        let measure = self
            .environment
            .translate(&head.measure, None, Phase::Current)?;
        if !matches!(measure.ty, Type::Integer { .. }) {
            return Err(unsupported(
                "a loop measure must be a fixed-width integer expression",
            ));
        }
        let measure = measure.expression;
        let invariant = if invariants.is_empty() {
            TokenStream::new()
        } else {
            quote!(invariant #(#invariants),*,)
        };
        let condition = &value.cond;
        let statements = &value.body.stmts[head.statements..];
        Ok(quote!(while #condition #invariant decreases #measure, { #(#statements)* }))
    }
}

impl Fold for Loops<'_> {
    fn fold_expr(&mut self, value: syn::Expr) -> syn::Expr {
        let value = syn::fold::fold_expr(self, value);
        let syn::Expr::While(loop_value) = value else {
            return value;
        };
        match self.lower(&loop_value) {
            Ok(tokens) => syn::Expr::Verbatim(tokens),
            Err(error) => {
                self.error.get_or_insert(error);
                syn::Expr::While(loop_value)
            }
        }
    }
}

fn predicate_source(source: &SourceFunction, environment: &Environment<'_>) -> Result<TokenStream> {
    let value = environment.translate(
        &syn::parse2(source.function.item.block.to_token_stream()).map_err(super::syntax)?,
        Some(&Type::Bool),
        Phase::Current,
    )?;
    if value.ty != Type::Bool {
        return Err(unsupported("requirement predicate is not Boolean"));
    }
    let visibility = &source.function.item.vis;
    let openness = if matches!(visibility, syn::Visibility::Inherited) {
        TokenStream::new()
    } else {
        quote!(open)
    };
    let name = &source.function.item.sig.ident;
    let defined_name = format_ident!("__sail_defined_{}", name.unraw());
    let inputs = &source.function.item.sig.inputs;
    let expression = value.expression;
    let defined = value.defined;
    Ok(quote! {
        #visibility #openness spec fn #name(#inputs) -> bool { #expression }
        #visibility #openness spec fn #defined_name(#inputs) -> bool { #defined }
    })
}

fn binding(
    source: &SourceFunction,
    owner: Option<&str>,
    crate_name: &str,
    guarantee: &TokenStream,
) -> ContractBinding {
    let function = match owner {
        Some(owner) => format!("{crate_name}::{owner}::{}", source.function.item.sig.ident),
        None => format!("{crate_name}::{}", identity(source)),
    };
    let span = source.function.item.span();
    let start = span.start();
    let end = span.end();
    ContractBinding {
        requirement_id: source.function.requirement.as_str().to_owned(),
        lowered_function: function.clone(),
        function,
        guarantee_sha256: digest(guarantee.to_string().as_bytes()),
        source: SourceRange {
            line: start.line,
            column: start.column,
            end_line: end.line,
            end_column: end.column,
        },
        module_file: None,
    }
}

#[derive(Default)]
struct Captures {
    expressions: Vec<syn::Expr>,
}

impl<'ast> Visit<'ast> for Captures {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("old")) {
            self.expressions.extend(call.args.iter().cloned());
        }
        syn::visit::visit_expr_call(self, call);
    }
}

fn enum_source(original: &syn::ItemEnum) -> Result<TokenStream> {
    // Admission permits only unit variants and the compiler-derived PartialEq/Eq
    // pair, with no user implementations. Its equality is exactly structural.
    // Verus needs this marker to connect executable equality to ADT identity.
    let mut value = original.clone();
    let derive = value
        .attrs
        .iter_mut()
        .find(|a| a.path().is_ident("derive"))
        .ok_or_else(|| unsupported("admitted enum has no equality derives"))?;
    let mut paths = derive
        .parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        .map_err(super::syntax)?;
    paths.push(syn::parse_quote!(Structural));
    *derive = syn::parse_quote!(#[derive(#paths)]);
    Ok(quote!(#value))
}
