use std::collections::BTreeMap;

use num_bigint::BigInt;
use sail_api::literal_witness::{Plan, Validated, model_query};
use sail_api::output::ItfValue;
use sail_api::runtime::{Answer, Outcome, ProductKind, QueryResult};

use super::super::admission::{admit, identity, signature};
use super::super::expression::{Environment, Phase};
use super::super::types::Type;
use super::super::unsupported;

/// Regenerate the complete source-bound plan and independently evaluate one
/// complete concrete SAT assignment against its original scalar contracts.
/// This authenticates relationships, not independent backend execution.
/// # Errors
/// Rejects changed plans, incomplete/mismatched responses, undefined arithmetic,
/// out-of-range inputs, false preconditions and satisfied postconditions.
#[allow(
    clippy::too_many_lines,
    reason = "Keep original source, model and independent evaluation checks together."
)]
pub fn validate(plan: &Plan, candidate_id: &str, result: &QueryResult) -> crate::Result<Validated> {
    plan.validate().map_err(unsupported)?;
    if super::prepare(&plan.baseline, plan.max_mutants)? != *plan {
        return Err(unsupported(
            "retained literal witness plan differs from original-source regeneration",
        ));
    }
    let prepared = plan
        .prepared
        .iter()
        .find(|p| p.candidate.id == candidate_id)
        .ok_or_else(|| unsupported("literal witness candidate is absent"))?;
    let query = model_query(prepared);
    let commands = sail_smtlib::sexpr::parse_all(query.script.as_bytes())
        .map_err(|e| unsupported(e.to_string()))?;
    let check_index = commands
        .len()
        .checked_sub(2)
        .ok_or_else(|| unsupported("missing check command"))?;
    if result.obligation_id != query.obligation_id
        || result.requirement_ids != query.requirement_ids
        || result.script_sha256 != sail_api::mutation::digest(query.script.as_bytes())
        || result.backend != query.backend
        || result.limits != query.limits
        || result.outcome != Outcome::Completed
        || result.error.is_some()
        || result.solver.as_ref().is_none_or(|s| {
            s.backend != "z3"
                || s.version.is_empty()
                || s.sha256.len() != 64
                || !s.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        })
        || result.checks.len() != 1
        || result.stdout.len() > result.limits.output_bytes
        || result.stderr.len() > result.limits.output_bytes
    {
        return Err(unsupported(
            "literal witness model response identity or execution differs",
        ));
    }
    let check = &result.checks[0];
    if check.answer != Answer::Sat
        || check.command_index != check_index
        || check.products.len() != 1
        || check.products[0].kind != ProductKind::Values
        || check.products[0].command_index != check_index + 1
    {
        return Err(unsupported(
            "literal witness requires one complete SAT scalar assignment",
        ));
    }
    let raw = &check.products[0].raw;
    let stdout =
        sail_smtlib::sexpr::parse_all(&result.stdout).map_err(|e| unsupported(e.to_string()))?;
    if stdout.len() != 2
        || stdout[0].text(&result.stdout) != b"sat"
        || stdout[1].text(&result.stdout).trim_ascii() != raw.trim_ascii()
    {
        return Err(unsupported(
            "literal witness model product differs from original stdout",
        ));
    }
    let response = sail_smtlib::sexpr::parse_all(raw).map_err(|e| unsupported(e.to_string()))?;
    let pairs = response
        .first()
        .filter(|_| response.len() == 1)
        .and_then(sail_smtlib::sexpr::SExpr::as_list)
        .ok_or_else(|| unsupported("literal witness values are not one list"))?;
    if pairs.len() != prepared.inputs.len() + 1 {
        return Err(unsupported("literal witness assignment is incomplete"));
    }
    let input = &prepared.candidate.source;
    let admitted = admit(&input.source, input.profile)?;
    let source = admitted
        .model
        .functions
        .iter()
        .find(|f| format!("{}::{}", input.crate_name, identity(f)) == prepared.candidate.function)
        .ok_or_else(|| unsupported("literal witness target is missing"))?;
    let (_, signature_inputs, output) = signature(source, &admitted.structs, input.profile)?;
    let types = signature_inputs.iter().cloned().collect::<BTreeMap<_, _>>();
    let mut values = BTreeMap::new();
    let mut recorded = Vec::new();
    for (index, (name, symbol)) in prepared.inputs.iter().enumerate() {
        let pair = pairs[index]
            .as_list()
            .filter(|p| p.len() == 2)
            .ok_or_else(|| unsupported("invalid witness scalar pair"))?;
        if pair[0].text(raw) != symbol.as_bytes() {
            return Err(unsupported("literal witness input identity differs"));
        }
        let ty = types
            .get(name)
            .ok_or_else(|| unsupported("literal witness input type missing"))?;
        let text =
            std::str::from_utf8(pair[1].text(raw)).map_err(|e| unsupported(e.to_string()))?;
        if text.len() > 48 {
            return Err(unsupported(
                "literal witness scalar exceeds fixed-width bounds",
            ));
        }
        let scalar = decode_scalar(ty, &pair[1], raw)?;
        let value = match &scalar {
            ItfValue::Bool(v) if *ty == Type::Bool => Value::Bool(*v),
            ItfValue::BigInt { bigint } if matches!(ty, Type::Integer { .. }) => {
                Value::Int(integer(bigint)?)
            }
            _ => {
                return Err(unsupported(
                    "literal witness input scalar cannot be decoded",
                ));
            }
        };
        if let (Type::Integer { signed, bits }, Value::Int(v)) = (ty, &value) {
            let (minimum, maximum) = bounds(*signed, *bits);
            if v < &minimum || v > &maximum {
                return Err(unsupported(
                    "literal witness input is outside its source type",
                ));
            }
        }
        values.insert(name.clone(), value);
        recorded.push((name.clone(), scalar));
    }
    let sentinel = pairs[prepared.inputs.len()]
        .as_list()
        .filter(|p| p.len() == 2)
        .ok_or_else(|| unsupported("missing witness sentinel"))?;
    if sentinel[0].text(raw) != b"true" || sentinel[1].text(raw) != b"true" {
        return Err(unsupported("invalid witness sentinel"));
    }
    let guarantee = source
        .function
        .ensures
        .as_ref()
        .ok_or_else(|| unsupported("missing witness guarantee"))?;
    let Some(syn::Pat::Ident(result_binding)) = guarantee.inputs.first() else {
        return Err(unsupported("missing result binding"));
    };
    let mut environment = Environment {
        profile: input.profile,
        module: &source.module,
        values: types,
        structs: &admitted.structs,
        predicates: &admitted.predicates,
        mutable_receiver: false,
    };
    environment
        .values
        .insert(result_binding.ident.to_string(), output.clone());
    let [syn::Stmt::Expr(body, None)] = source.function.item.block.stmts.as_slice() else {
        return Err(unsupported("literal body is missing"));
    };
    let literal = environment.translate(body, Some(&output), Phase::Current)?;
    let concrete = evaluate_tokens(literal.expression, &values)?;
    values.insert(result_binding.ident.to_string(), concrete);
    for expression in &source.function.requires {
        let value = environment.translate(expression, Some(&Type::Bool), Phase::Current)?;
        require(
            evaluate_tokens(value.defined, &values)?.boolean()?
                && evaluate_tokens(value.expression, &values)?.boolean()?,
            "literal witness precondition is false or undefined",
        )?;
    }
    let mut captures = super::Captures::default();
    syn::visit::Visit::visit_expr(&mut captures, &guarantee.body);
    for capture in captures.expressions {
        let value = environment.translate(&capture, None, Phase::Current)?;
        require(
            evaluate_tokens(value.defined, &values)?.boolean()?,
            "literal witness entry capture is undefined",
        )?;
    }
    let post = environment.translate(&guarantee.body, Some(&Type::Bool), Phase::Post)?;
    require(
        evaluate_tokens(post.defined, &values)?.boolean()?,
        "literal witness postcondition is undefined",
    )?;
    require(
        !evaluate_tokens(post.expression, &values)?.boolean()?,
        "literal witness satisfies the unchanged postcondition",
    )?;
    Ok(Validated {
        prepared: prepared.clone(),
        model_result: result.clone(),
        values: recorded,
    })
}

fn decode_scalar(
    ty: &Type,
    expression: &sail_smtlib::sexpr::SExpr,
    bytes: &[u8],
) -> crate::Result<ItfValue> {
    use sail_smtlib::{lex::TokenKind, sexpr::SExprKind};
    if *ty == Type::Bool {
        if matches!(&expression.kind, SExprKind::Atom(TokenKind::Symbol)) {
            return match expression.text(bytes) {
                b"true" => Ok(ItfValue::Bool(true)),
                b"false" => Ok(ItfValue::Bool(false)),
                _ => Err(unsupported("literal witness Boolean term is malformed")),
            };
        }
        return Err(unsupported("literal witness Boolean term is not an atom"));
    }
    if !matches!(ty, Type::Integer { .. }) {
        return Err(unsupported("literal witness scalar type is unsupported"));
    }
    if let Some(parts) = expression.as_list() {
        let [operator, magnitude] = parts else {
            return Err(unsupported(
                "literal witness negative integer term has invalid arity",
            ));
        };
        if !matches!(&operator.kind, SExprKind::Atom(TokenKind::Symbol))
            || operator.text(bytes) != b"-"
        {
            return Err(unsupported(
                "literal witness integer term is not unary minus",
            ));
        }
        let magnitude = numeral(magnitude, bytes)?;
        return Ok(ItfValue::BigInt {
            bigint: if magnitude == "0" {
                "0".into()
            } else {
                format!("-{magnitude}")
            },
        });
    }
    Ok(ItfValue::BigInt {
        bigint: numeral(expression, bytes)?.into(),
    })
}

fn numeral<'a>(expression: &sail_smtlib::sexpr::SExpr, bytes: &'a [u8]) -> crate::Result<&'a str> {
    use sail_smtlib::{lex::TokenKind, sexpr::SExprKind};
    if !matches!(&expression.kind, SExprKind::Atom(TokenKind::Numeral)) {
        return Err(unsupported(
            "literal witness integer magnitude is not an SMT numeral",
        ));
    }
    let text = std::str::from_utf8(expression.text(bytes))
        .map_err(|error| unsupported(error.to_string()))?;
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(unsupported(
            "literal witness integer magnitude is not a canonical SMT numeral",
        ));
    }
    Ok(text)
}

fn require(condition: bool, message: &str) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(unsupported(message))
    }
}
fn integer(text: &str) -> crate::Result<BigInt> {
    text.parse()
        .map_err(|e| unsupported(format!("invalid witness integer: {e}")))
}
fn bounds(signed: bool, bits: u32) -> (BigInt, BigInt) {
    let magnitude = BigInt::from(1_u8) << if signed { bits - 1 } else { bits };
    if signed {
        (-&magnitude, &magnitude - 1_u8)
    } else {
        (BigInt::from(0_u8), magnitude - 1_u8)
    }
}
#[derive(Clone, PartialEq, Eq)]
enum Value {
    Bool(bool),
    Int(BigInt),
}
impl Value {
    fn boolean(self) -> crate::Result<bool> {
        match self {
            Self::Bool(v) => Ok(v),
            Self::Int(_) => Err(unsupported("witness Boolean type differs")),
        }
    }
    fn integer(self) -> crate::Result<BigInt> {
        match self {
            Self::Int(v) => Ok(v),
            Self::Bool(_) => Err(unsupported("witness integer type differs")),
        }
    }
}
fn evaluate_tokens(
    tokens: proc_macro2::TokenStream,
    values: &BTreeMap<String, Value>,
) -> crate::Result<Value> {
    evaluate(&syn::parse2(tokens).map_err(super::super::syntax)?, values)
}
#[allow(
    clippy::too_many_lines,
    reason = "Independent concrete evaluator preserves short-circuit and checked definedness without SMT parsing."
)]
fn evaluate(expression: &syn::Expr, values: &BTreeMap<String, Value>) -> crate::Result<Value> {
    match expression {
        syn::Expr::Paren(v) => evaluate(&v.expr, values),
        syn::Expr::Group(v) => evaluate(&v.expr, values),
        syn::Expr::Block(v) => {
            let [syn::Stmt::Expr(e, None)] = v.block.stmts.as_slice() else {
                return Err(unsupported("invalid witness evaluation block"));
            };
            evaluate(e, values)
        }
        syn::Expr::Lit(v) => match &v.lit {
            syn::Lit::Bool(v) => Ok(Value::Bool(v.value)),
            syn::Lit::Int(v) => integer(v.base10_digits()).map(Value::Int),
            _ => Err(unsupported("unsupported witness evaluation literal")),
        },
        syn::Expr::Cast(v) if matches!(v.ty.as_ref(),syn::Type::Path(p) if p.path.is_ident("int")) => {
            evaluate(&v.expr, values)
        }
        syn::Expr::Path(v) if v.path.segments.len() == 1 => values
            .get(&v.path.segments[0].ident.to_string())
            .cloned()
            .ok_or_else(|| unsupported("unbound witness evaluation symbol")),
        syn::Expr::Path(v) if v.path.segments.len() == 2 => {
            let ty = v.path.segments[0].ident.to_string();
            let edge = v.path.segments[1].ident.to_string();
            for bits in [8, 16, 32, 64, 128] {
                for signed in [false, true] {
                    if ty == format!("{}{bits}", if signed { 'i' } else { 'u' }) {
                        let (min, max) = bounds(signed, bits);
                        return match edge.as_str() {
                            "MIN" => Ok(Value::Int(min)),
                            "MAX" => Ok(Value::Int(max)),
                            _ => Err(unsupported("unsupported witness evaluation bound")),
                        };
                    }
                }
            }
            Err(unsupported("unsupported witness evaluation path"))
        }
        syn::Expr::Unary(v) => match v.op {
            syn::UnOp::Not(_) => Ok(Value::Bool(!evaluate(&v.expr, values)?.boolean()?)),
            syn::UnOp::Neg(_) => Ok(Value::Int(-evaluate(&v.expr, values)?.integer()?)),
            _ => Err(unsupported(
                "unsupported witness evaluation unary operation",
            )),
        },
        syn::Expr::If(v) => {
            if evaluate(&v.cond, values)?.boolean()? {
                let [syn::Stmt::Expr(e, None)] = v.then_branch.stmts.as_slice() else {
                    return Err(unsupported("invalid witness evaluation conditional"));
                };
                evaluate(e, values)
            } else {
                evaluate(
                    &v.else_branch
                        .as_ref()
                        .ok_or_else(|| unsupported("missing witness evaluation else"))?
                        .1,
                    values,
                )
            }
        }
        syn::Expr::Binary(v) => {
            let left = evaluate(&v.left, values)?;
            if matches!(v.op, syn::BinOp::And(_)) {
                return Ok(Value::Bool(
                    left.boolean()? && evaluate(&v.right, values)?.boolean()?,
                ));
            }
            if matches!(v.op, syn::BinOp::Or(_)) {
                return Ok(Value::Bool(
                    left.boolean()? || evaluate(&v.right, values)?.boolean()?,
                ));
            }
            let right = evaluate(&v.right, values)?;
            match v.op {
                syn::BinOp::Eq(_) => Ok(Value::Bool(left == right)),
                syn::BinOp::Ne(_) => Ok(Value::Bool(left != right)),
                syn::BinOp::Add(_) => Ok(Value::Int(left.integer()? + right.integer()?)),
                syn::BinOp::Sub(_) => Ok(Value::Int(left.integer()? - right.integer()?)),
                syn::BinOp::Mul(_) => Ok(Value::Int(left.integer()? * right.integer()?)),
                syn::BinOp::Lt(_) => Ok(Value::Bool(left.integer()? < right.integer()?)),
                syn::BinOp::Le(_) => Ok(Value::Bool(left.integer()? <= right.integer()?)),
                syn::BinOp::Gt(_) => Ok(Value::Bool(left.integer()? > right.integer()?)),
                syn::BinOp::Ge(_) => Ok(Value::Bool(left.integer()? >= right.integer()?)),
                _ => Err(unsupported(
                    "unsupported witness evaluation binary operation",
                )),
            }
        }
        _ => Err(unsupported("unsupported witness evaluation expression")),
    }
}
