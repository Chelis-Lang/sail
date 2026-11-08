use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::{Expr, Item, ItemFn, Stmt, visit::Visit};

use crate::{AttributeKind, ContractFunction, parse_function_with_profile, unsupported};

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
    /// Explicitly selected source admission profile.
    pub profile: u32,
    /// Nongeneric source enums, keyed by qualified nominal identity.
    pub enums: BTreeMap<String, syn::ItemEnum>,
    /// Contract-bearing functions in original lexical order.
    pub functions: Vec<SourceFunction>,
    /// Original struct definitions by qualified source identity.
    pub structs: BTreeMap<String, syn::ItemStruct>,
    /// Every module path in the closure, including the crate root.
    pub modules: BTreeSet<Vec<String>>,
    /// Out-of-line module paths whose items came from a submitted module file.
    pub module_files: BTreeSet<Vec<String>>,
    /// Profile-three crate-internal imports: module path to alias to target path.
    pub imports: BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
}

/// Maximum number of out-of-line module files in one source closure.
pub const MAX_MODULE_FILES: usize = 64;

/// Parse source and revalidate every referenced predicate's actual body.
///
/// This syntax admission is not compiler type checking or backend verification.
/// # Errors
/// Rejects invalid contracts, duplicate predicate IDs, unresolved predicate calls
/// and calls whose predicate implements another requirement.
pub fn parse_source(text: &str) -> syn::Result<SourceModel> {
    parse_source_with_profile(text, 1)
}

/// Parse a complete source file under explicitly selected profile one, two or three.
/// # Errors
/// Rejects unknown profiles and invalid contracts or predicate resolution.
pub fn parse_source_with_profile(text: &str, profile: u32) -> syn::Result<SourceModel> {
    parse_closure_with_profile(text, &[], profile)
}

/// Parse a crate root and its out-of-line module files as one source closure.
///
/// Each module file is named by its crate-relative module path, such as
/// `codec` or `codec::bits`, never by a filesystem path. Every `mod name;`
/// declaration must resolve to exactly one submitted file, and every submitted
/// file must be declared. Module files need profile three.
/// # Errors
/// Rejects unknown profiles, files outside the declared closure, `#[path]`
/// redirection, invalid contracts and unresolved predicate calls.
pub fn parse_closure_with_profile(
    text: &str,
    modules: &[(&str, &str)],
    profile: u32,
) -> syn::Result<SourceModel> {
    if !matches!(profile, 1..=3) {
        return Err(unsupported(
            proc_macro2::TokenStream::new(),
            "unsupported source profile",
        ));
    }
    if text.len() > 1024 * 1024 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "SAIL0701: neutral source exceeds one MiB",
        ));
    }
    let mut file = syn::parse_file(text)?;
    let module_files = compose(&mut file, modules, profile)?;
    let mut functions = Vec::new();
    let mut structs = BTreeMap::new();
    let mut enums = BTreeMap::new();
    let mut scope = Scope {
        modules: BTreeSet::from([Vec::new()]),
        imports: BTreeMap::new(),
    };
    collect(
        &file.items,
        &[],
        &mut functions,
        &mut structs,
        &mut enums,
        &mut scope,
        profile,
    )?;
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
            let name = resolve(&value.module, &path, &scope.imports)?;
            let predicate = predicates.get(&name).ok_or_else(|| {
                unsupported(
                    &path,
                    "contract call does not resolve to a validated requirement predicate",
                )
            })?;
            if !value.function.requirements.contains(&predicate.requirement) {
                return Err(unsupported(
                    path,
                    "predicate requirement differs from the function's satisfies identity",
                ));
            }
        }
    }
    Ok(SourceModel {
        file,
        profile,
        enums,
        functions,
        structs,
        modules: scope.modules,
        module_files,
        imports: scope.imports,
    })
}

struct Scope {
    modules: BTreeSet<Vec<String>>,
    imports: BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
}

/// Replace each `mod name;` with its submitted file, recursively.
fn compose(
    file: &mut syn::File,
    modules: &[(&str, &str)],
    profile: u32,
) -> syn::Result<BTreeSet<Vec<String>>> {
    let empty = proc_macro2::TokenStream::new();
    if !modules.is_empty() && profile < 3 {
        return Err(unsupported(
            empty,
            "out-of-line module files require explicit profile three",
        ));
    }
    if modules.len() > MAX_MODULE_FILES {
        return Err(unsupported(
            empty,
            "a source closure has too many module files",
        ));
    }
    let mut pending = BTreeMap::new();
    for (name, text) in modules {
        let path = name.split("::").map(str::to_owned).collect::<Vec<_>>();
        if path
            .iter()
            .any(|part| syn::parse_str::<syn::Ident>(part).is_err() || part.starts_with("__sail_"))
        {
            return Err(unsupported(
                proc_macro2::TokenStream::new(),
                "module file names must be crate-relative module paths of plain identifiers",
            ));
        }
        if text.len() > 1024 * 1024 {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "SAIL0701: neutral source exceeds one MiB",
            ));
        }
        if pending.insert(path, *text).is_some() {
            return Err(unsupported(
                proc_macro2::TokenStream::new(),
                "a module file is submitted twice",
            ));
        }
    }
    let mut used = BTreeSet::new();
    inline(&mut file.items, &[], &pending, &mut used)?;
    if let Some(unused) = pending.keys().find(|name| !used.contains(*name)) {
        return Err(unsupported(
            proc_macro2::TokenStream::new(),
            &format!(
                "submitted module file {} is not declared by an out-of-line module in the closure",
                unused.join("::")
            ),
        ));
    }
    Ok(used)
}

fn inline(
    items: &mut [Item],
    module: &[String],
    files: &BTreeMap<Vec<String>, &str>,
    used: &mut BTreeSet<Vec<String>>,
) -> syn::Result<()> {
    for item in items {
        let Item::Mod(value) = item else { continue };
        let mut nested = module.to_vec();
        nested.push(value.ident.to_string());
        if value.content.is_none() {
            if value.attrs.iter().any(|a| a.path().is_ident("path")) {
                return Err(unsupported(
                    &*value,
                    "#[path] module redirection is outside the admitted source closure",
                ));
            }
            let Some(text) = files.get(&nested) else {
                return Err(unsupported(
                    &*value,
                    &format!(
                        "out-of-line module {} is outside the submitted source closure",
                        nested.join("::")
                    ),
                ));
            };
            let parsed = syn::parse_file(text)?;
            if parsed.shebang.is_some() {
                return Err(unsupported(&*value, "module files cannot carry a shebang"));
            }
            used.insert(nested.clone());
            // A module file's inner attributes apply to its module declaration.
            value.attrs.extend(parsed.attrs);
            value.content = Some((syn::token::Brace::default(), parsed.items));
            value.semi = None;
        }
        if let Some((_, nested_items)) = &mut value.content {
            inline(nested_items, &nested, files, used)?;
        }
    }
    Ok(())
}

fn qualified(module: &[String], name: &str) -> String {
    module
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join("::")
}

fn resolve(
    module: &[String],
    path: &syn::Path,
    imports: &BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
) -> syn::Result<String> {
    let mut parts = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>();
    if path.leading_colon.is_none()
        && let Some(target) = parts
            .first()
            .and_then(|first| imports.get(module).and_then(|aliases| aliases.get(first)))
    {
        parts.splice(0..1, target.iter().cloned());
        return Ok(parts.join("::"));
    }
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
    enums: &mut BTreeMap<String, syn::ItemEnum>,
    scope: &mut Scope,
    profile: u32,
) -> syn::Result<()> {
    for item in items {
        match item {
            Item::Use(value) if profile >= 3 && value.leading_colon.is_none() => {
                let mut aliases = Vec::new();
                flatten(&value.tree, module, None, &mut aliases)?;
                let table = scope.imports.entry(module.to_vec()).or_default();
                for (alias, target) in aliases {
                    if table.insert(alias, target).is_some() {
                        return Err(unsupported(value, "an import name is declared twice"));
                    }
                }
            }
            Item::Fn(value) if annotated(&value.attrs) => functions.push(SourceFunction {
                module: module.to_vec(),
                owner: None,
                function: parse_function_with_profile(value.clone(), None, profile)?,
            }),
            Item::Mod(value) => {
                if let Some((_, items)) = &value.content {
                    let mut nested = module.to_vec();
                    nested.push(value.ident.to_string());
                    scope.modules.insert(nested.clone());
                    collect(items, &nested, functions, structs, enums, scope, profile)?;
                }
            }
            Item::Enum(value) if profile >= 2 => {
                let key = qualified(module, &value.ident.to_string());
                if enums.insert(key, value.clone()).is_some() {
                    return Err(unsupported(value, "duplicate nominal enum"));
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
                        let function = parse_function_with_profile(
                            syn::parse2(function.to_token_stream())?,
                            None,
                            profile,
                        )?;
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

/// Flatten a crate-internal import into alias and absolute target pairs.
///
/// External roots, including `sail_contracts`, are left to backend admission.
fn flatten(
    tree: &syn::UseTree,
    module: &[String],
    prefix: Option<Vec<String>>,
    aliases: &mut Vec<(String, Vec<String>)>,
) -> syn::Result<()> {
    let refuse = |message: &str| Err(unsupported(tree, message));
    match (tree, prefix) {
        (syn::UseTree::Path(path), None) => {
            let root = match path.ident.to_string().as_str() {
                "crate" => Vec::new(),
                "self" => module.to_vec(),
                "super" => match module.split_last() {
                    Some((_, parent)) => parent.to_vec(),
                    None => return refuse("super imports at the crate root leave the crate"),
                },
                _ => return Ok(()),
            };
            flatten(&path.tree, module, Some(root), aliases)
        }
        (syn::UseTree::Path(path), Some(mut prefix)) => {
            if ["crate", "self", "super"].contains(&path.ident.to_string().as_str()) {
                return refuse("crate, self and super must lead an import path");
            }
            prefix.push(path.ident.to_string());
            flatten(&path.tree, module, Some(prefix), aliases)
        }
        (syn::UseTree::Name(name), Some(mut prefix)) => {
            if ["crate", "self", "super"].contains(&name.ident.to_string().as_str()) {
                return refuse("self, crate and super imports are unsupported");
            }
            prefix.push(name.ident.to_string());
            aliases.push((name.ident.to_string(), prefix));
            Ok(())
        }
        (syn::UseTree::Rename(rename), Some(mut prefix)) => {
            if ["crate", "self", "super", "_"].contains(&rename.ident.to_string().as_str())
                || rename.rename == "_"
            {
                return refuse("self, crate, super and underscore imports are unsupported");
            }
            prefix.push(rename.ident.to_string());
            aliases.push((rename.rename.to_string(), prefix));
            Ok(())
        }
        (syn::UseTree::Group(group), Some(prefix)) => {
            for item in &group.items {
                flatten(item, module, Some(prefix.clone()), aliases)?;
            }
            Ok(())
        }
        (syn::UseTree::Glob(_), Some(_)) => {
            refuse("glob imports hide source resolution and are unsupported")
        }
        _ => refuse("crate-internal imports must start with crate, self or super"),
    }
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

pub(crate) fn validate_body(item: &ItemFn, profile: u32) -> syn::Result<()> {
    let mut validator = Body {
        error: None,
        profile,
    };
    validator.visit_block(&item.block);
    validator.error.map_or(Ok(()), Err)
}

struct Body {
    error: Option<syn::Error>,
    profile: u32,
}

impl Body {
    fn refuse(&mut self, node: impl quote::ToTokens, message: &str) {
        self.error.get_or_insert_with(|| unsupported(node, message));
    }

    /// Profile three admits calls, `self` method calls, integer casts, shared
    /// borrows and `match` over literal and path patterns. Resolution of each
    /// callee to an admitted contracted function is checked by the backend.
    fn widened(&mut self, value: &Expr) -> bool {
        if self.profile < 3 {
            return false;
        }
        match value {
            Expr::Call(call) => {
                if !matches!(call.func.as_ref(), Expr::Path(path) if path.qself.is_none() && plain(&path.path))
                {
                    self.refuse(
                        call,
                        "calls must name an admitted function by a plain path; closures, generic and qualified-self calls are unsupported",
                    );
                }
                true
            }
            Expr::MethodCall(call) => {
                if call.turbofish.is_some()
                    || !matches!(call.receiver.as_ref(), Expr::Path(path) if path.path.is_ident("self"))
                {
                    self.refuse(
                        call,
                        "method calls are admitted only on self, without turbofish",
                    );
                }
                true
            }
            Expr::Cast(cast) => {
                if !matches!(cast.ty.as_ref(), syn::Type::Path(path) if path.qself.is_none() && INTEGERS.iter().any(|name| path.path.is_ident(name)))
                {
                    self.refuse(cast, "casts are admitted only to fixed-width integer types");
                }
                true
            }
            Expr::Reference(reference) => {
                if reference.mutability.is_some() {
                    self.refuse(
                        reference,
                        "mutable borrows in executable bodies are unsupported",
                    );
                }
                true
            }
            Expr::Match(value) => {
                for arm in &value.arms {
                    self.pattern(&arm.pat);
                }
                true
            }
            // `visit_expr_while` checks the loop head and skips its declarations.
            Expr::While(_) => true,
            _ => false,
        }
    }

    fn pattern(&mut self, value: &syn::Pat) {
        match value {
            syn::Pat::Wild(_) => {}
            syn::Pat::Lit(literal) if matches!(literal.lit, syn::Lit::Int(_) | syn::Lit::Bool(_)) => {}
            syn::Pat::Path(path) if path.qself.is_none() && path.path.segments.len() >= 2 && plain(&path.path) => {}
            syn::Pat::Range(range)
                if range.start.as_deref().is_none_or(literal_bound)
                    && range.end.as_deref().is_none_or(literal_bound) => {}
            syn::Pat::Or(alternatives) if alternatives.leading_vert.is_none() => {
                for case in &alternatives.cases {
                    self.pattern(case);
                }
            }
            _ => self.refuse(
                value,
                "match patterns are admitted only as integer or Boolean literals, literal ranges, qualified unit enum variants, alternatives and wildcards",
            ),
        }
    }
}

const INTEGERS: [&str; 10] = [
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
];

fn plain(path: &syn::Path) -> bool {
    path.segments
        .iter()
        .all(|s| matches!(s.arguments, syn::PathArguments::None))
}

fn literal_bound(value: &Expr) -> bool {
    match value {
        Expr::Lit(literal) => matches!(literal.lit, syn::Lit::Int(_)),
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_))
                && matches!(unary.expr.as_ref(), Expr::Lit(literal) if matches!(literal.lit, syn::Lit::Int(_)))
        }
        _ => false,
    }
}

/// The neutral loop head: an optional `sail_contracts::invariant!` with one or
/// more conditions, then exactly one `sail_contracts::decreases!` measure.
/// Returns the number of head statements, or `None` when the head is invalid.
#[must_use]
pub fn loop_head(body: &syn::Block) -> Option<LoopHead> {
    let mut statements = body.stmts.iter();
    let mut invariants = Vec::new();
    let mut next = statements.next();
    match next.map_or(Head::Absent, |s| head_macro(s, "invariant")) {
        Head::Present(values) if !values.is_empty() => {
            invariants = values;
            next = statements.next();
        }
        Head::Absent => {}
        Head::Present(_) | Head::Invalid => return None,
    }
    let Head::Present(measure) = next.map_or(Head::Absent, |s| head_macro(s, "decreases")) else {
        return None;
    };
    let [measure] = <[Expr; 1]>::try_from(measure).ok()?;
    Some(LoopHead {
        statements: usize::from(!invariants.is_empty()) + 1,
        invariants,
        measure,
    })
}

enum Head {
    Absent,
    Invalid,
    Present(Vec<Expr>),
}

/// A validated neutral loop head.
#[derive(Clone, Debug)]
pub struct LoopHead {
    /// Number of leading body statements that declare the head.
    pub statements: usize,
    /// Loop invariants in source order.
    pub invariants: Vec<Expr>,
    /// The termination measure.
    pub measure: Expr,
}

fn head_macro(statement: &Stmt, name: &str) -> Head {
    let Stmt::Macro(value) = statement else {
        return Head::Absent;
    };
    let path = &value.mac.path;
    if path.leading_colon.is_some()
        || path.segments.len() != 2
        || path.segments[0].ident != "sail_contracts"
        || path.segments[1].ident != name
        || !plain(path)
    {
        return Head::Absent;
    }
    if !matches!(value.mac.delimiter, syn::MacroDelimiter::Paren(_)) {
        return Head::Invalid;
    }
    value
        .mac
        .parse_body_with(syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated)
        .map_or(Head::Invalid, |values| {
            Head::Present(values.into_iter().collect())
        })
}

impl<'ast> Visit<'ast> for Body {
    fn visit_expr_while(&mut self, value: &'ast syn::ExprWhile) {
        if self.profile < 3 {
            // `visit_expr` has already refused the loop.
            return;
        }
        if value.label.is_some() || !value.attrs.is_empty() {
            self.refuse(value, "loop labels and attributes are unsupported");
        }
        let Some(head) = loop_head(&value.body) else {
            self.refuse(
                value,
                "a while loop body must start with an optional sail_contracts::invariant!(...) and exactly one sail_contracts::decreases!(...)",
            );
            return;
        };
        self.visit_expr(&value.cond);
        for statement in &value.body.stmts[head.statements..] {
            self.visit_stmt(statement);
        }
    }

    fn visit_stmt(&mut self, value: &'ast Stmt) {
        if !matches!(value, Stmt::Local(_) | Stmt::Expr(_, _)) {
            self.refuse(
                value,
                "items and macros in neutral function bodies are unsupported",
            );
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
        ) && !self.widened(value)
        {
            self.refuse(value, "unsupported neutral executable body; calls, effects, loops and macros are not admitted");
        }
        if let Expr::Unary(value) = value
            && !matches!(
                value.op,
                syn::UnOp::Not(_) | syn::UnOp::Neg(_) | syn::UnOp::Deref(_)
            )
        {
            self.refuse(
                value,
                "dereferencing and arbitrary borrowing are unsupported",
            );
        }
        syn::visit::visit_expr(self, value);
    }
}
