//! Separate quantifier-free scalar literal-contract witnesses.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::admission::{admit, identity, signature};
use super::expression::{Environment, Phase};
use super::types::Type;
use super::unsupported;

mod concrete;
pub use concrete::validate;
use quote::ToTokens as _;
use sail_api::literal_witness::{POLICY, Plan, Prepared, VERSION};
use sail_api::mutation::{Candidate, digest};

/// Prepare opt-in witness queries over the original deterministic literal inventory.
/// Each SAT query witnesses a defined unchanged postcondition violation under all
/// original preconditions. This constructs no verifier verdict or mutation kill.
///
/// # Errors
/// Rejects invalid source or inventory budgets. Unsupported candidates remain
/// explicit in the returned plan, never silently omitted.
pub fn prepare(input: &sail_api::contracts::Submission, budget: usize) -> crate::Result<Plan> {
    let inventory = super::mutation::plan(input, budget)?;
    let mut result = Plan {
        version: VERSION,
        policy: POLICY.into(),
        baseline: input.clone(),
        max_mutants: budget,
        prepared: Vec::new(),
        unsupported: inventory.unsupported,
        omitted: inventory.omitted,
    };
    for candidate in inventory.candidates {
        match candidate_query(&candidate) {
            Ok(query) => result.prepared.push(query),
            Err(error) => result
                .unsupported
                .push(format!("{}: {error}", candidate.id)),
        }
    }
    result.unsupported.sort();
    Ok(result)
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep source admission and exact query binding together."
)]
fn candidate_query(candidate: &Candidate) -> crate::Result<Prepared> {
    let input = &candidate.source;
    let admitted = admit(&input.source, input.profile)?;
    let source = admitted
        .model
        .functions
        .iter()
        .find(|f| format!("{}::{}", input.crate_name, identity(f)) == candidate.function)
        .ok_or_else(|| unsupported("literal witness target identity is missing"))?;
    let (owner, inputs, output) = signature(source, &admitted.structs, input.profile)?;
    if owner.is_some() || inputs.iter().any(|(_, ty)| !primitive(ty)) || !primitive(&output) {
        return Err(unsupported(
            "literal witnesses require a free function with owned Boolean or fixed-width integer inputs and result",
        ));
    }
    let [syn::Stmt::Expr(body, None)] = source.function.item.block.stmts.as_slice() else {
        return Err(unsupported(
            "literal witness body is not one exact scalar literal",
        ));
    };
    if !matches!(body, syn::Expr::Lit(_))
        || body.to_token_stream().to_string() != candidate.replacement
    {
        return Err(unsupported(
            "literal witness body differs from the inventory replacement",
        ));
    }
    let guarantee = source
        .function
        .ensures
        .as_ref()
        .ok_or_else(|| unsupported("missing guarantee"))?;
    let Some(syn::Pat::Ident(result)) = guarantee.inputs.first() else {
        return Err(unsupported("missing guarantee result binding"));
    };
    if inputs.iter().any(|(name, _)| result.ident == name.as_str()) {
        return Err(unsupported(
            "literal witness result binding must differ from input names",
        ));
    }
    let mut environment = Environment {
        profile: input.profile,
        module: &source.module,
        values: inputs.iter().cloned().collect(),
        structs: &admitted.structs,
        predicates: &admitted.predicates,
        mutable_receiver: false,
    };
    environment
        .values
        .insert(result.ident.to_string(), output.clone());
    let mut bindings = inputs
        .iter()
        .enumerate()
        .map(|(i, (name, _))| (name.clone(), format!("x{i}")))
        .collect::<BTreeMap<_, _>>();
    let recorded_inputs = bindings
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let literal = environment.translate(body, Some(&output), Phase::Current)?;
    let mut encoder = Encoder {
        values: bindings.clone(),
    };
    bindings.insert(
        result.ident.to_string(),
        encoder.tokens(literal.expression)?,
    );
    encoder.values = bindings;
    let mut script = String::from("(set-logic QF_NIA)\n");
    for (i, (_, ty)) in inputs.iter().enumerate() {
        let sort = if *ty == Type::Bool { "Bool" } else { "Int" };
        let _ = writeln!(script, "(declare-const x{i} {sort})");
        if let Type::Integer { signed, bits } = ty {
            let (minimum, maximum) = bounds(*signed, *bits);
            let _ = writeln!(
                script,
                "(assert (and (<= {minimum} x{i}) (<= x{i} {maximum})))"
            );
        }
    }
    // Predicates are deliberately refused until their separate definitions and
    // definedness functions have an independently qualified translation.
    let mut assertions = Vec::new();
    for precondition in &source.function.requires {
        let translated = environment.translate(precondition, Some(&Type::Bool), Phase::Current)?;
        assertions.push(encoder.tokens(translated.defined)?);
        assertions.push(encoder.tokens(translated.expression)?);
    }
    let post = environment.translate(&guarantee.body, Some(&Type::Bool), Phase::Post)?;
    // Do not manufacture a witness from undefined checked arithmetic.
    assertions.push(encoder.tokens(post.defined)?);
    assertions.push(format!("(not {})", encoder.tokens(post.expression)?));
    // old() is pure for this immutable input subset, but its original entry
    // definedness is a separate obligation and must hold for every witness.
    let mut captures = Captures::default();
    syn::visit::Visit::visit_expr(&mut captures, &guarantee.body);
    for capture in captures.expressions {
        let value = environment.translate(&capture, None, Phase::Current)?;
        assertions.push(encoder.tokens(value.defined)?);
    }
    for assertion in assertions {
        let _ = writeln!(script, "(assert {assertion})");
    }
    script.push_str("(check-sat)\n");
    if script.len() > sail_api::runtime::MAX_INPUT_BYTES {
        return Err(unsupported(
            "literal witness query exceeds retained input bound",
        ));
    }
    let query_sha256 = digest(script.as_bytes());
    let query = sail_api::runtime::Query {
        obligation_id: format!("{POLICY}:{}:{query_sha256}", candidate.id),
        requirement_ids: vec![candidate.requirement_id.clone()],
        backend: "z3".into(),
        script,
        limits: sail_api::runtime::Limits::default(),
    };
    // Full original Verus lowering independently rechecks all contract typing.
    super::lower_profile(&input.source, &input.crate_name, input.profile)?;
    Ok(Prepared {
        version: VERSION,
        policy: POLICY.into(),
        candidate: candidate.clone(),
        query_sha256,
        inputs: recorded_inputs,
        query,
    })
}

fn primitive(ty: &Type) -> bool {
    matches!(ty, Type::Bool | Type::Integer { .. })
}

fn bounds(signed: bool, bits: u32) -> (String, String) {
    if signed {
        let magnitude = 1_u128 << (bits - 1);
        (format!("(- {magnitude})"), (magnitude - 1).to_string())
    } else {
        (
            "0".into(),
            if bits == 128 {
                u128::MAX
            } else {
                (1_u128 << bits) - 1
            }
            .to_string(),
        )
    }
}

struct Encoder {
    values: BTreeMap<String, String>,
}
impl Encoder {
    fn tokens(&self, tokens: proc_macro2::TokenStream) -> crate::Result<String> {
        self.expression(&syn::parse2(tokens).map_err(super::syntax)?)
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive small generated-expression translation fails closed."
    )]
    fn expression(&self, source: &syn::Expr) -> crate::Result<String> {
        match source {
            syn::Expr::Paren(value) => self.expression(&value.expr),
            syn::Expr::Group(value) => self.expression(&value.expr),
            syn::Expr::Block(value) => {
                let [syn::Stmt::Expr(value, None)] = value.block.stmts.as_slice() else {
                    return Err(unsupported("unsupported witness block"));
                };
                self.expression(value)
            }
            syn::Expr::Lit(value) => match &value.lit {
                syn::Lit::Bool(value) => Ok(value.value.to_string()),
                syn::Lit::Int(value) => Ok(value.base10_digits().into()),
                _ => Err(unsupported("unsupported witness literal")),
            },
            syn::Expr::Cast(value) if matches!(value.ty.as_ref(), syn::Type::Path(path) if path.path.is_ident("int")) => {
                self.expression(&value.expr)
            }
            syn::Expr::Path(value) if value.path.segments.len() == 1 => self
                .values
                .get(&value.path.segments[0].ident.to_string())
                .cloned()
                .ok_or_else(|| unsupported("unbound witness symbol")),
            syn::Expr::Path(value) if value.path.segments.len() == 2 => {
                let ty = value.path.segments[0].ident.to_string();
                let edge = value.path.segments[1].ident.to_string();
                for bits in [8, 16, 32, 64, 128] {
                    for signed in [false, true] {
                        if ty == format!("{}{bits}", if signed { 'i' } else { 'u' }) {
                            let (min, max) = bounds(signed, bits);
                            return match edge.as_str() {
                                "MIN" => Ok(min),
                                "MAX" => Ok(max),
                                _ => Err(unsupported("unsupported witness bound")),
                            };
                        }
                    }
                }
                Err(unsupported("unsupported witness path"))
            }
            syn::Expr::Unary(value) => {
                let op = match value.op {
                    syn::UnOp::Not(_) => "not",
                    syn::UnOp::Neg(_) => "-",
                    _ => return Err(unsupported("unsupported witness unary operation")),
                };
                Ok(format!("({op} {})", self.expression(&value.expr)?))
            }
            syn::Expr::Binary(value) => {
                let op = match value.op {
                    syn::BinOp::And(_) => "and",
                    syn::BinOp::Or(_) => "or",
                    syn::BinOp::Eq(_) => "=",
                    syn::BinOp::Ne(_) => "distinct",
                    syn::BinOp::Lt(_) => "<",
                    syn::BinOp::Le(_) => "<=",
                    syn::BinOp::Gt(_) => ">",
                    syn::BinOp::Ge(_) => ">=",
                    syn::BinOp::Add(_) => "+",
                    syn::BinOp::Sub(_) => "-",
                    syn::BinOp::Mul(_) => "*",
                    _ => return Err(unsupported("unsupported witness binary operation")),
                };
                Ok(format!(
                    "({op} {} {})",
                    self.expression(&value.left)?,
                    self.expression(&value.right)?
                ))
            }
            syn::Expr::If(value) => {
                let [syn::Stmt::Expr(then, None)] = value.then_branch.stmts.as_slice() else {
                    return Err(unsupported("unsupported witness conditional"));
                };
                let (_, otherwise) = value
                    .else_branch
                    .as_ref()
                    .ok_or_else(|| unsupported("missing witness else branch"))?;
                Ok(format!(
                    "(ite {} {} {})",
                    self.expression(&value.cond)?,
                    self.expression(then)?,
                    self.expression(otherwise)?
                ))
            }
            _ => Err(unsupported(
                "witness translation currently excludes predicate calls, references and nominal values",
            )),
        }
    }
}

#[derive(Default)]
struct Captures {
    expressions: Vec<syn::Expr>,
}
impl<'ast> syn::visit::Visit<'ast> for Captures {
    fn visit_expr_call(&mut self, value: &'ast syn::ExprCall) {
        if matches!(value.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("old")) {
            self.expressions.extend(value.args.iter().cloned());
        }
        syn::visit::visit_expr_call(self, value);
    }
}
