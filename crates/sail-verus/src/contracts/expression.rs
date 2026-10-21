use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use super::admission::Predicate;
use super::types::{Struct, Type, qualify};
use super::unsupported;
use crate::Result;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Current,
    Old,
}

pub(super) struct Environment<'a> {
    pub(super) module: &'a [String],
    pub(super) values: BTreeMap<String, Type>,
    pub(super) structs: &'a BTreeMap<String, Struct>,
    pub(super) predicates: &'a BTreeMap<String, Predicate>,
    pub(super) mutable_receiver: bool,
}

pub(super) struct Value {
    pub(super) ty: Type,
    pub(super) expression: TokenStream,
    pub(super) defined: TokenStream,
}

impl Environment<'_> {
    pub(super) fn translate(
        &self,
        expression: &syn::Expr,
        hint: Option<&Type>,
        phase: Phase,
    ) -> Result<Value> {
        match expression {
            syn::Expr::Lit(value) => Self::literal(&value.lit, hint),
            syn::Expr::Path(value) if value.path.segments.len() == 1 => {
                let name = value.path.segments[0].ident.to_string();
                let ty = self
                    .values
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| unsupported("unresolved specification binding"))?;
                let source = if name == "self" && phase == Phase::Old && self.mutable_receiver {
                    quote!(old(self))
                } else {
                    quote!(#value)
                };
                Ok(value_of(ty, source))
            }
            syn::Expr::Field(value) => {
                let base = self.translate(&value.base, None, phase)?;
                let Type::Named(name) = &base.ty else {
                    return Err(unsupported(
                        "field access requires an admitted scalar struct; autoderef is forbidden",
                    ));
                };
                let syn::Member::Named(field) = &value.member else {
                    return Err(unsupported("tuple field projections are unsupported"));
                };
                let ty = self
                    .structs
                    .get(name)
                    .and_then(|s| s.fields.get(&field.to_string()))
                    .cloned()
                    .ok_or_else(|| unsupported("unresolved or non-scalar source field"))?;
                let base_value = base.expression;
                Ok(value_of(ty, quote!((#base_value).#field)))
            }
            syn::Expr::Paren(value) => self.translate(&value.expr, hint, phase),
            syn::Expr::Group(value) => self.translate(&value.expr, hint, phase),
            syn::Expr::Block(value) => {
                let [syn::Stmt::Expr(expression, None)] = value.block.stmts.as_slice() else {
                    return Err(unsupported("unsupported specification block"));
                };
                self.translate(expression, hint, phase)
            }
            syn::Expr::Binary(value) => self.binary(value, hint, phase),
            syn::Expr::Unary(value) => self.unary(value, hint, phase),
            syn::Expr::Call(value) => self.call(value, phase),
            syn::Expr::If(value) => self.conditional(value, hint, phase),
            _ => Err(unsupported("unsupported typed specification expression")),
        }
    }

    fn literal(literal: &syn::Lit, hint: Option<&Type>) -> Result<Value> {
        match literal {
            syn::Lit::Bool(value) => Ok(value_of(Type::Bool, quote!(#value))),
            syn::Lit::Int(value) => {
                let ty = literal_type(value, hint)?;
                let expression = quote!((#value as int));
                let defined = ty.bounds(&expression)?;
                Ok(Value {
                    ty,
                    expression,
                    defined,
                })
            }
            _ => Err(unsupported("non-scalar specification literal")),
        }
    }

    fn infer(&self, expression: &syn::Expr) -> Result<Option<Type>> {
        match expression {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(value),
                ..
            }) if value.suffix().is_empty() => Ok(None),
            syn::Expr::Paren(value) => self.infer(&value.expr),
            syn::Expr::Group(value) => self.infer(&value.expr),
            syn::Expr::Unary(value) => self.infer(&value.expr),
            syn::Expr::Binary(value) if numeric(&value.op) => {
                Ok(self.infer(&value.left)?.or(self.infer(&value.right)?))
            }
            syn::Expr::If(value) => match value.then_branch.stmts.as_slice() {
                [syn::Stmt::Expr(value, None)] => self.infer(value),
                _ => Err(unsupported("unsupported conditional branch")),
            },
            syn::Expr::Block(value) => match value.block.stmts.as_slice() {
                [syn::Stmt::Expr(value, None)] => self.infer(value),
                _ => Err(unsupported("unsupported specification block")),
            },
            _ => self
                .translate(expression, None, Phase::Current)
                .map(|value| Some(value.ty)),
        }
    }

    fn operand_type(
        &self,
        left: &syn::Expr,
        right: &syn::Expr,
        hint: Option<&Type>,
    ) -> Result<Type> {
        Ok(self
            .infer(left)?
            .or(self.infer(right)?)
            .or_else(|| hint.cloned())
            .unwrap_or(Type::Integer {
                signed: true,
                bits: 32,
            }))
    }

    fn binary(&self, source: &syn::ExprBinary, hint: Option<&Type>, phase: Phase) -> Result<Value> {
        if matches!(source.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            return self.logical(source, phase);
        }
        let ty = self.operand_type(
            &source.left,
            &source.right,
            if numeric(&source.op) { hint } else { None },
        )?;
        let left = self.translate(&source.left, Some(&ty), phase)?;
        let right = self.translate(&source.right, Some(&ty), phase)?;
        if left.ty != ty || right.ty != ty || !ty.scalar() {
            return Err(unsupported(
                "specification operand types differ or are non-scalar",
            ));
        }
        let l = left.expression;
        let r = right.expression;
        let ld = left.defined;
        let rd = right.defined;
        let operation = &source.op;
        if numeric(operation) {
            ty.numeric()?;
            if matches!(operation, syn::BinOp::Div(_) | syn::BinOp::Rem(_)) {
                return Err(unsupported(
                    "division and remainder specification lowering needs separately qualified signedness semantics",
                ));
            }
            let expression = quote!((#l) #operation (#r));
            let bounds = ty.bounds(&expression)?;
            return Ok(Value {
                ty,
                expression,
                defined: quote!((#ld) && (#rd) && (#bounds)),
            });
        }
        let expression = if ty == Type::Bool {
            boolean_comparison(operation, &l, &r)?
        } else {
            quote!((#l) #operation (#r))
        };
        Ok(Value {
            ty: Type::Bool,
            expression,
            defined: quote!((#ld) && (#rd)),
        })
    }

    fn logical(&self, source: &syn::ExprBinary, phase: Phase) -> Result<Value> {
        let left = self.translate(&source.left, Some(&Type::Bool), phase)?;
        let right = self.translate(&source.right, Some(&Type::Bool), phase)?;
        if left.ty != Type::Bool || right.ty != Type::Bool {
            return Err(unsupported("Boolean operators need Boolean operands"));
        }
        let l = left.expression;
        let r = right.expression;
        let ld = left.defined;
        let rd = right.defined;
        let operation = &source.op;
        let defined = if matches!(operation, syn::BinOp::And(_)) {
            quote!((#ld) && (!(#l) || (#rd)))
        } else {
            quote!((#ld) && ((#l) || (#rd)))
        };
        Ok(Value {
            ty: Type::Bool,
            expression: quote!((#l) #operation (#r)),
            defined,
        })
    }

    fn unary(&self, source: &syn::ExprUnary, hint: Option<&Type>, phase: Phase) -> Result<Value> {
        if matches!(source.op, syn::UnOp::Neg(_))
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(literal),
                ..
            }) = source.expr.as_ref()
        {
            let ty = literal_type(literal, hint)?;
            let literal = syn::LitInt::new(literal.base10_digits(), literal.span());
            let expression = quote!((-(#literal as int)));
            let defined = ty.bounds(&expression)?;
            return Ok(Value {
                ty,
                expression,
                defined,
            });
        }
        let value = self.translate(&source.expr, hint, phase)?;
        let expression = value.expression;
        let defined = value.defined;
        match &source.op {
            syn::UnOp::Not(_) if value.ty == Type::Bool => Ok(Value {
                ty: Type::Bool,
                expression: quote!(!(#expression)),
                defined,
            }),
            syn::UnOp::Neg(_) if matches!(value.ty, Type::Integer { signed: true, .. }) => {
                let expression = quote!(-(#expression));
                let bound = value.ty.bounds(&expression)?;
                Ok(Value {
                    ty: value.ty,
                    expression,
                    defined: quote!((#defined) && (#bound)),
                })
            }
            _ => Err(unsupported("unsupported scalar unary operation")),
        }
    }

    fn call(&self, source: &syn::ExprCall, phase: Phase) -> Result<Value> {
        let syn::Expr::Path(path) = source.func.as_ref() else {
            return Err(unsupported(
                "specification calls require admitted predicate paths",
            ));
        };
        if path.path.is_ident("old") {
            let argument = source
                .args
                .first()
                .ok_or_else(|| unsupported("old has no entry expression"))?;
            let value = self.translate(argument, None, Phase::Old)?;
            if !value.ty.scalar() {
                return Err(unsupported(
                    "old captures require scalar values, not receiver aliases or structs",
                ));
            }
            // Entry definedness is an explicit assertion, not a stronger precondition.
            return Ok(Value {
                defined: quote!(true),
                ..value
            });
        }
        let name = qualify(self.module, &path.path)?;
        let predicate = self
            .predicates
            .get(&name)
            .ok_or_else(|| unsupported("unresolved requirement predicate"))?;
        if predicate.inputs.len() != source.args.len() {
            return Err(unsupported("predicate argument count differs"));
        }
        let mut arguments = Vec::new();
        let mut definitions = Vec::new();
        for (argument, ty) in source.args.iter().zip(&predicate.inputs) {
            let value = self.translate(argument, Some(ty), phase)?;
            if &value.ty != ty {
                return Err(unsupported("predicate argument type differs"));
            }
            definitions.push(value.defined);
            let expression = value.expression;
            let rust = ty.rust()?;
            arguments.push(if *ty == Type::Bool {
                quote!(#expression)
            } else {
                quote!((#expression) as #rust)
            });
        }
        let mut helper = path.path.clone();
        let last = helper
            .segments
            .last_mut()
            .ok_or_else(|| unsupported("empty predicate path"))?;
        last.ident = format_ident!("__sail_defined_{}", last.ident.unraw());
        let function = &path.path;
        Ok(Value {
            ty: Type::Bool,
            expression: quote!(#function(#(#arguments),*)),
            defined: quote!(true #(&& (#definitions))* && #helper(#(#arguments),*)),
        })
    }

    fn conditional(
        &self,
        source: &syn::ExprIf,
        hint: Option<&Type>,
        phase: Phase,
    ) -> Result<Value> {
        let condition = self.translate(&source.cond, Some(&Type::Bool), phase)?;
        if condition.ty != Type::Bool {
            return Err(unsupported("conditional guard is not Boolean"));
        }
        let [syn::Stmt::Expr(then, None)] = source.then_branch.stmts.as_slice() else {
            return Err(unsupported("unsupported conditional body"));
        };
        let (_, otherwise) = source
            .else_branch
            .as_ref()
            .ok_or_else(|| unsupported("missing specification else branch"))?;
        let ty = self.operand_type(then, otherwise, hint)?;
        let then = self.translate(then, Some(&ty), phase)?;
        let otherwise = self.translate(otherwise, Some(&ty), phase)?;
        if then.ty != otherwise.ty {
            return Err(unsupported("conditional branch types differ"));
        }
        let condition_value = condition.expression;
        let condition_defined = condition.defined;
        let then_value = then.expression;
        let then_defined = then.defined;
        let else_value = otherwise.expression;
        let else_defined = otherwise.defined;
        Ok(Value {
            ty,
            expression: quote!(if #condition_value { #then_value } else { #else_value }),
            defined: quote!((#condition_defined) && if #condition_value { #then_defined } else { #else_defined }),
        })
    }
}

fn value_of(ty: Type, source: TokenStream) -> Value {
    let expression = if matches!(ty, Type::Integer { .. }) {
        quote!((#source as int))
    } else {
        source
    };
    Value {
        ty,
        expression,
        defined: quote!(true),
    }
}

fn literal_type(value: &syn::LitInt, hint: Option<&Type>) -> Result<Type> {
    if value.suffix().is_empty() {
        return Ok(hint.cloned().unwrap_or(Type::Integer {
            signed: true,
            bits: 32,
        }));
    }
    for bits in [8, 16, 32, 64, 128] {
        if value.suffix() == format!("u{bits}") {
            return Ok(Type::Integer {
                signed: false,
                bits,
            });
        }
        if value.suffix() == format!("i{bits}") {
            return Ok(Type::Integer { signed: true, bits });
        }
    }
    Err(unsupported("unsupported integer literal type"))
}

fn numeric(operation: &syn::BinOp) -> bool {
    matches!(
        operation,
        syn::BinOp::Add(_)
            | syn::BinOp::Sub(_)
            | syn::BinOp::Mul(_)
            | syn::BinOp::Div(_)
            | syn::BinOp::Rem(_)
    )
}

fn boolean_comparison(
    operation: &syn::BinOp,
    left: &TokenStream,
    right: &TokenStream,
) -> Result<TokenStream> {
    match operation {
        syn::BinOp::Eq(_) | syn::BinOp::Ne(_) => Ok(quote!((#left) #operation (#right))),
        syn::BinOp::Lt(_) => Ok(quote!(!(#left) && (#right))),
        syn::BinOp::Le(_) => Ok(quote!(!(#left) || (#right))),
        syn::BinOp::Gt(_) => Ok(quote!((#left) && !(#right))),
        syn::BinOp::Ge(_) => Ok(quote!((#left) || !(#right))),
        _ => Err(unsupported("unsupported Boolean comparison")),
    }
}
