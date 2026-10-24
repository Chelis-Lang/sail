use std::collections::BTreeSet;

use syn::{BinOp, Expr, Lit, Stmt, UnOp};

use crate::unsupported;

/// The expression's evaluation phase and permitted source bindings.
#[derive(Clone, Debug)]
pub struct ExpressionContext {
    /// Names bound by the signature or postcondition closure.
    pub bindings: BTreeSet<String>,
    /// Whether scalar prestate captures are allowed.
    pub old: bool,
    /// Whether calls to structurally checked requirement predicates are allowed.
    pub predicate_calls: bool,
}

/// Validate a supported, effect-free scalar specification expression.
///
/// Calls still require whole-source resolution through [`crate::parse_source`].
/// # Errors
/// Returns `SAIL0701` for unsupported syntax or unbound source names.
pub fn validate_expression(expression: &Expr, context: &ExpressionContext) -> syn::Result<()> {
    validate(expression, context, 0)
}

fn validate(expression: &Expr, context: &ExpressionContext, depth: usize) -> syn::Result<()> {
    if depth > 128 {
        return Err(unsupported(
            expression,
            "contract expression exceeds 128 levels",
        ));
    }
    let recurse = |value| validate(value, context, depth + 1);
    match expression {
        Expr::Lit(value) if matches!(value.lit, Lit::Bool(_) | Lit::Int(_)) => Ok(()),
        Expr::Path(value)
            if value.qself.is_none()
                && value.path.segments.len() == 1
                && context
                    .bindings
                    .contains(&value.path.segments[0].ident.to_string()) =>
        {
            Ok(())
        }
        Expr::Path(value)
            if value.qself.is_none()
                && value.path.segments.len() >= 2
                && value.path.segments.iter().all(|s| s.arguments.is_empty()) =>
        {
            Ok(())
        }
        Expr::Field(value) => recurse(&value.base),
        Expr::Paren(value) => recurse(&value.expr),
        Expr::Group(value) => recurse(&value.expr),
        Expr::Unary(value) if matches!(value.op, UnOp::Not(_) | UnOp::Neg(_) | UnOp::Deref(_)) => {
            recurse(&value.expr)
        }
        Expr::Binary(value)
            if matches!(
                value.op,
                BinOp::Add(_)
                    | BinOp::Sub(_)
                    | BinOp::Mul(_)
                    | BinOp::Div(_)
                    | BinOp::Rem(_)
                    | BinOp::And(_)
                    | BinOp::Or(_)
                    | BinOp::Eq(_)
                    | BinOp::Ne(_)
                    | BinOp::Lt(_)
                    | BinOp::Le(_)
                    | BinOp::Gt(_)
                    | BinOp::Ge(_)
            ) =>
        {
            recurse(&value.left)?;
            recurse(&value.right)
        }
        Expr::If(value) => {
            recurse(&value.cond)?;
            block(&value.then_branch, context, depth + 1)?;
            match &value.else_branch {
                Some((_, branch)) => recurse(branch),
                None => Err(unsupported(
                    value,
                    "a specification conditional needs an else branch",
                )),
            }
        }
        Expr::Block(value) if value.label.is_none() => block(&value.block, context, depth + 1),
        Expr::Call(value) => call(value, context, depth),
        _ => Err(unsupported(
            expression,
            "unsupported or effectful contract expression",
        )),
    }
}

fn block(value: &syn::Block, context: &ExpressionContext, depth: usize) -> syn::Result<()> {
    match value.stmts.as_slice() {
        [Stmt::Expr(expression, None)] => validate(expression, context, depth + 1),
        _ => Err(unsupported(
            value,
            "a predicate block must contain one pure result expression",
        )),
    }
}

fn call(value: &syn::ExprCall, context: &ExpressionContext, depth: usize) -> syn::Result<()> {
    let Expr::Path(path) = value.func.as_ref() else {
        return Err(unsupported(
            value,
            "only named requirement predicate calls are supported",
        ));
    };
    if path.qself.is_some() || path.path.segments.iter().any(|s| !s.arguments.is_empty()) {
        return Err(unsupported(
            value,
            "generic predicate calls are unsupported",
        ));
    }
    if path.path.is_ident("old") {
        if !context.old || value.args.len() != 1 {
            return Err(unsupported(
                value,
                "old needs one argument in a postcondition",
            ));
        }
        let entry = ExpressionContext {
            old: false,
            predicate_calls: false,
            ..context.clone()
        };
        for argument in &value.args {
            validate(argument, &entry, depth + 1)?;
        }
        return Ok(());
    }
    if !context.predicate_calls {
        return Err(unsupported(
            value,
            "requirement predicate bodies and old captures cannot call code",
        ));
    }
    for argument in &value.args {
        validate(argument, context, depth + 1)?;
    }
    Ok(())
}
