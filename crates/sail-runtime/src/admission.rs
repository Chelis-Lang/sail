//! Closed command admission and exact command-to-response matching.

use sail_api::runtime::{Answer, Check, Error, Product, ProductKind, Query};
use sail_smtlib::{
    lex::TokenKind,
    sexpr::{SExpr, SExprKind, parse_all},
};

use crate::invalid;

#[derive(Clone, Debug)]
pub(crate) enum ExpectedKind {
    Check,
    Product(ProductKind, usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Expected {
    pub(crate) command_index: usize,
    pub(crate) kind: ExpectedKind,
}

pub(crate) type Plan = Vec<Expected>;

fn fail(kind: &str, message: impl Into<String>) -> Error {
    invalid(kind, message)
}

fn list(expr: &SExpr) -> Result<&[SExpr], Error> {
    expr.as_list()
        .ok_or_else(|| fail("invalid_script", "expected a command list"))
}

fn atom(expr: &SExpr, src: &[u8]) -> bool {
    matches!(
        expr.kind,
        SExprKind::Atom(TokenKind::Symbol | TokenKind::QuotedSymbol)
    ) && !expr.text(src).is_empty()
}

fn arity(items: &[SExpr], expected: usize) -> Result<(), Error> {
    if items.len() != expected {
        return Err(fail(
            "invalid_script",
            "command has the wrong number of arguments",
        ));
    }
    Ok(())
}

fn option(items: &[SExpr], src: &[u8], limit: u64) -> Result<(), Error> {
    arity(items, 3)?;
    let key = items[1].text(src);
    let value = items[2].text(src);
    let admitted = match key {
        b":print-success" => value == b"false",
        b":produce-models" | b":produce-unsat-cores" => matches!(value, b"true" | b"false"),
        b":smt.random_seed" | b":sat.random_seed" => value == b"0",
        b":rlimit" => {
            std::str::from_utf8(value)
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                == Some(limit)
        }
        _ => false,
    };
    if !admitted {
        return Err(fail(
            "unsupported_option",
            "option is unsupported or conflicts with effective limits or zero seeds",
        ));
    }
    Ok(())
}

/// Admit every command before any backend process or cache access.
#[allow(
    clippy::too_many_lines,
    reason = "One closed command grammar keeps output-producing commands explicit."
)]
pub(crate) fn plan(query: &Query) -> Result<Plan, Error> {
    let src = query.script.as_bytes();
    let commands = parse_all(src).map_err(|e| fail("invalid_script", e.to_string()))?;
    let mut expected = Vec::new();
    let mut depth = 0_u64;
    let mut current = false;
    let mut exited = false;
    for (command_index, command) in commands.iter().enumerate() {
        if exited {
            return Err(fail("invalid_script", "command follows exit"));
        }
        let items = list(command)?;
        let head = items
            .first()
            .ok_or_else(|| fail("invalid_script", "empty command"))?;
        if !matches!(head.kind, SExprKind::Atom(TokenKind::Symbol)) {
            return Err(fail(
                "invalid_script",
                "command head must be an unquoted symbol",
            ));
        }
        let name = head.text(src);
        let product = match name {
            b"check-sat" => {
                arity(items, 1)?;
                current = true;
                Some(ExpectedKind::Check)
            }
            b"check-sat-assuming" => {
                arity(items, 2)?;
                list(&items[1])?;
                current = true;
                Some(ExpectedKind::Check)
            }
            b"get-model" | b"get-unsat-core" => {
                arity(items, 1)?;
                Some(ExpectedKind::Product(
                    if name == b"get-model" {
                        ProductKind::Model
                    } else {
                        ProductKind::UnsatCore
                    },
                    0,
                ))
            }
            b"get-value" => {
                arity(items, 2)?;
                let terms = list(&items[1])?;
                if terms.is_empty() {
                    return Err(fail("invalid_script", "get-value requires terms"));
                }
                Some(ExpectedKind::Product(ProductKind::Values, terms.len()))
            }
            b"get-info" => {
                arity(items, 2)?;
                if items[1].text(src) != b":reason-unknown" {
                    return Err(fail(
                        "unsupported_command",
                        "only reason-unknown is a supported get-info product",
                    ));
                }
                Some(ExpectedKind::Product(ProductKind::ReasonUnknown, 0))
            }
            b"exit" => {
                arity(items, 1)?;
                exited = true;
                None
            }
            b"push" | b"pop" => {
                arity(items, 2)?;
                let count = std::str::from_utf8(items[1].text(src))
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(|| {
                        fail(
                            "invalid_script",
                            "push/pop needs a bounded nonnegative numeral",
                        )
                    })?;
                depth = if name == b"push" {
                    depth.checked_add(count)
                } else {
                    depth.checked_sub(count)
                }
                .filter(|d| *d <= 4096)
                .ok_or_else(|| {
                    fail(
                        "invalid_script",
                        "push/pop depth exceeds its bound or underflows",
                    )
                })?;
                current = false;
                None
            }
            b"set-option" => {
                option(items, src, query.limits.resource_limit)?;
                current = false;
                None
            }
            b"set-logic" => {
                arity(items, 2)?;
                if !atom(&items[1], src) {
                    return Err(fail("invalid_script", "set-logic needs a symbol"));
                }
                current = false;
                None
            }
            b"assert" => {
                arity(items, 2)?;
                current = false;
                None
            }
            b"declare-const" | b"declare-sort" | b"declare-datatype" | b"declare-datatypes"
            | b"define-funs-rec" => {
                arity(items, 3)?;
                current = false;
                None
            }
            b"declare-fun" | b"define-sort" | b"define-const" => {
                arity(items, 4)?;
                current = false;
                None
            }
            b"define-fun" | b"define-fun-rec" => {
                arity(items, 5)?;
                current = false;
                None
            }
            _ => {
                return Err(fail(
                    "unsupported_command",
                    format!("unsupported command {}", String::from_utf8_lossy(name)),
                ));
            }
        };
        if let Some(kind) = product {
            if matches!(kind, ExpectedKind::Product(_, _)) && !current {
                return Err(fail(
                    "invalid_script",
                    "a product requires a preceding check with no intervening state change",
                ));
            }
            expected.push(Expected {
                command_index,
                kind,
            });
        }
    }
    if !expected
        .iter()
        .any(|item| matches!(item.kind, ExpectedKind::Check))
    {
        return Err(fail(
            "invalid_script",
            "transcript requires an actual satisfiability check",
        ));
    }
    Ok(expected)
}

fn product_ok(kind: ProductKind, count: usize, expr: &SExpr, src: &[u8]) -> bool {
    let Some(items) = expr.as_list() else {
        return false;
    };
    match kind {
        ProductKind::Model => items.iter().all(|item| {
            item.as_list().is_some_and(|fields| {
                fields.len() == 5
                    && fields[0].text(src) == b"define-fun"
                    && atom(&fields[1], src)
                    && fields[2].as_list().is_some()
            })
        }),
        ProductKind::Values => {
            items.len() == count
                && items
                    .iter()
                    .all(|item| item.as_list().is_some_and(|pair| pair.len() == 2))
        }
        ProductKind::UnsatCore => items.iter().all(|item| atom(item, src)),
        ProductKind::ReasonUnknown => {
            items.len() == 2
                && items[0].text(src) == b":reason-unknown"
                && matches!(items[1].kind, SExprKind::Atom(TokenKind::String))
        }
    }
}

/// Match the entire original output against the admitted response plan.
pub(crate) fn responses(plan: &Plan, src: &[u8]) -> Result<Vec<Check>, Error> {
    std::str::from_utf8(src).map_err(|_| fail("invalid_response", "solver output is not UTF-8"))?;
    let output = parse_all(src).map_err(|e| fail("invalid_response", e.to_string()))?;
    if output.len() != plan.len() {
        return Err(fail(
            "invalid_response",
            "solver response count differs from admitted command plan",
        ));
    }
    let mut checks: Vec<Check> = Vec::new();
    for (expected, expr) in plan.iter().zip(&output) {
        match expected.kind {
            ExpectedKind::Check => {
                let answer = match expr.text(src) {
                    b"sat" => Answer::Sat,
                    b"unsat" => Answer::Unsat,
                    b"unknown" => Answer::Unknown,
                    _ => {
                        return Err(fail(
                            "invalid_response",
                            "check did not produce a bare satisfiability answer",
                        ));
                    }
                };
                checks.push(Check {
                    command_index: expected.command_index,
                    answer,
                    products: Vec::new(),
                });
            }
            ExpectedKind::Product(kind, count) => {
                let check = checks
                    .last_mut()
                    .ok_or_else(|| fail("invalid_response", "product has no check"))?;
                let answer_ok = match kind {
                    ProductKind::Model | ProductKind::Values => check.answer == Answer::Sat,
                    ProductKind::UnsatCore => check.answer == Answer::Unsat,
                    ProductKind::ReasonUnknown => check.answer == Answer::Unknown,
                };
                if !answer_ok || !product_ok(kind, count, expr, src) {
                    return Err(fail(
                        "invalid_response",
                        "product shape or preceding satisfiability answer is invalid",
                    ));
                }
                check.products.push(Product {
                    command_index: expected.command_index,
                    kind,
                    raw: expr.text(src).to_vec(),
                });
            }
        }
    }
    Ok(checks)
}
