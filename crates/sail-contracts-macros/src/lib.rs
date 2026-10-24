//! Runtime expansion of the shared, validated neutral contract subset.
#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use sail_contracts_ir::{AttributeKind, ContractFunction, parse_function_with_profile};
use syn::{Expr, ext::IdentExt, fold::Fold};

/// Check a precondition at entry, including release builds.
#[proc_macro_attribute]
pub fn requires(arguments: TokenStream, input: TokenStream) -> TokenStream {
    expand(AttributeKind::Requires, arguments, input)
}

/// Check one guarantee on every normal return, including release builds.
#[proc_macro_attribute]
pub fn ensures(arguments: TokenStream, input: TokenStream) -> TokenStream {
    expand(AttributeKind::Ensures, arguments, input)
}

/// Define a structurally validated pure requirement predicate.
#[proc_macro_attribute]
pub fn requirement(arguments: TokenStream, input: TokenStream) -> TokenStream {
    expand(AttributeKind::Requirement, arguments, input)
}

/// Attach the owner requirement identity to a contracted implementation.
#[proc_macro_attribute]
pub fn satisfies(arguments: TokenStream, input: TokenStream) -> TokenStream {
    expand(AttributeKind::Satisfies, arguments, input)
}

fn expand(kind: AttributeKind, arguments: TokenStream, input: TokenStream) -> TokenStream {
    let result = syn::parse::<syn::ItemFn>(input)
        .and_then(|item| parse_function_with_profile(item, Some((kind, arguments.into())), 2))
        .and_then(runtime);
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

fn runtime(function: ContractFunction) -> syn::Result<proc_macro2::TokenStream> {
    let mut item = function.item;
    let mut checked = Checked::default();
    if function.predicate {
        item.block = Box::new(checked.fold_block(*item.block));
        let inputs = item
            .sig
            .inputs
            .iter()
            .filter_map(|argument| match argument {
                syn::FnArg::Typed(value) => match value.pat.as_ref() {
                    syn::Pat::Ident(pattern) => Some(&pattern.ident),
                    _ => None,
                },
                syn::FnArg::Receiver(_) => None,
            });
        let body = item.block.stmts;
        item.block = Box::new(syn::parse2(quote! {{
            #(let _ = ::sail_contracts::__private::snapshot(&#inputs);)*
            #(#body)*
        }})?);
        let mut entry = item.clone();
        entry.attrs.clear();
        entry.sig.ident = format_ident!("__sail_pure_{}", item.sig.ident.unraw());
        return Ok(quote! { #item #[doc(hidden)] #entry });
    }
    item.attrs.push(syn::parse_quote!(#[allow(unused_parens, reason = "Contract expression grouping is preserved in generated checks.")]));
    let id = function.requirement.as_str();
    let mut preconditions = Vec::new();
    for expression in function.requires {
        let expression = checked.fold_expr(expression);
        preconditions
            .push(quote! { ::sail_contracts::__private::check(#expression, #id, "precondition"); });
    }
    let guarantee = function
        .ensures
        .ok_or_else(|| syn::Error::new_spanned(&item.sig, "SAIL0701: missing ensures"))?;
    let mut guarantee = checked.fold_expr_closure(guarantee);
    let output: syn::Type = match &item.sig.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, value) => *value.clone(),
    };
    if let Some(binding) = guarantee.inputs.first_mut() {
        *binding = syn::Pat::Type(syn::PatType {
            attrs: Vec::new(),
            pat: Box::new(binding.clone()),
            colon_token: syn::token::Colon::default(),
            ty: Box::new(syn::parse_quote!(&#output)),
        });
    }
    if let Some(syn::Pat::Type(binding)) = guarantee.inputs.first()
        && let syn::Pat::Ident(name) = binding.pat.as_ref()
    {
        let binding = &name.ident;
        let body = BorrowedResult {
            name: binding.clone(),
        }
        .fold_expr(*guarantee.body);
        *guarantee.body = syn::parse_quote!({ let _ = #binding; #body });
    }
    let captures = checked.captures.iter().map(|(name, expression)| {
        quote! {
            let #name = ::sail_contracts::__private::snapshot(&(#expression));
        }
    });
    let body = item.block;
    let result = format_ident!(
        "__sail_contract_result",
        span = proc_macro2::Span::mixed_site()
    );
    item.block = Box::new(syn::parse2(quote! {{
        #(#preconditions)*
        #(#captures)*
        let #result = (|| #body)();
        ::sail_contracts::__private::check((#guarantee)(&#result), #id, "postcondition");
        #result
    }})?);
    Ok(quote! { #item })
}

struct BorrowedResult {
    name: syn::Ident,
}
impl Fold for BorrowedResult {
    fn fold_expr(&mut self, expression: Expr) -> Expr {
        if matches!(&expression, Expr::Path(path) if path.path.is_ident(&self.name)) {
            return syn::parse_quote!((*#expression));
        }
        syn::fold::fold_expr(self, expression)
    }
}

#[derive(Default)]
struct Checked {
    captures: Vec<(syn::Ident, Expr)>,
}

impl Fold for Checked {
    fn fold_expr(&mut self, expression: Expr) -> Expr {
        if let Expr::Call(call) = &expression
            && matches!(call.func.as_ref(), Expr::Path(p) if p.path.is_ident("old"))
            && let Some(argument) = call.args.first()
        {
            let value = self.fold_expr(argument.clone());
            let name = format_ident!(
                "__sail_old_{}",
                self.captures.len(),
                span = proc_macro2::Span::mixed_site()
            );
            self.captures.push((name.clone(), value));
            return syn::parse_quote!(#name);
        }
        let expression = syn::fold::fold_expr(self, expression);
        match expression {
            Expr::Paren(value) if matches!(value.expr.as_ref(), Expr::Paren(_)) => *value.expr,
            Expr::Binary(value) => {
                let operation = match value.op {
                    syn::BinOp::Add(_) => "add",
                    syn::BinOp::Sub(_) => "sub",
                    syn::BinOp::Mul(_) => "mul",
                    syn::BinOp::Div(_) => "div",
                    syn::BinOp::Rem(_) => "rem",
                    syn::BinOp::Eq(_) => "eq",
                    syn::BinOp::Ne(_) => "ne",
                    syn::BinOp::Lt(_) => "lt",
                    syn::BinOp::Le(_) => "le",
                    syn::BinOp::Gt(_) => "gt",
                    syn::BinOp::Ge(_) => "ge",
                    _ => return Expr::Binary(value),
                };
                let left = value.left;
                let right = value.right;
                if matches!(value.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
                    let name = format_ident!("{operation}_ref");
                    syn::parse_quote!(::sail_contracts::__private::#name(&(#left), &(#right)))
                } else {
                    let name = format_ident!("{operation}");
                    syn::parse_quote!(::sail_contracts::__private::#name(#left, #right))
                }
            }
            Expr::Unary(value) if matches!(value.op, syn::UnOp::Neg(_)) => {
                if matches!(value.expr.as_ref(), Expr::Lit(literal) if matches!(literal.lit, syn::Lit::Int(_)))
                {
                    return Expr::Unary(value);
                }
                let inner = value.expr;
                syn::parse_quote!(::sail_contracts::__private::neg(#inner))
            }
            Expr::Unary(value) if matches!(value.op, syn::UnOp::Not(_)) => {
                let inner = value.expr;
                syn::parse_quote!(::sail_contracts::__private::not(#inner))
            }
            Expr::Call(mut value) => {
                if let Expr::Path(path) = value.func.as_mut()
                    && let Some(last) = path.path.segments.last_mut()
                {
                    last.ident = format_ident!("__sail_pure_{}", last.ident.unraw());
                }
                Expr::Call(value)
            }
            other => other,
        }
    }
}
