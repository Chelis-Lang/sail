//! Qualify deterministic operators from pinned modules before native binding.
use super::Failure;
use crate::Outcome;
use dive_ir::{Expr, Ir, OpRef, StdOp, StdSupport, Symbol};
use std::collections::BTreeSet;

pub(super) fn qualified_bundle(display: &str) -> bool {
    matches!(
        display,
        "<bundled:community-modules>/FiniteSetsExt.tla"
            | "<bundled:community-modules>/Folds.tla"
            | "<bundled:community-modules>/Functions.tla"
    )
}

fn refused(reason: &str) -> Failure {
    (Outcome::Unsupported, format!("SAIL4202: {reason}"))
}

fn disallowed(op: StdOp) -> bool {
    let qualified = match op.module() {
        "Naturals" | "Integers" | "Sequences" | "FiniteSets" | "Bags" | "Folds" | "Functions"
        | "FiniteSetsExt" => true,
        "TLC" => matches!(
            op,
            StdOp::TlcMakeFcn | StdOp::TlcCombineFcn | StdOp::TlcPermutations
        ),
        _ => false,
    };
    !qualified || op.impure() || matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
}

/// Every use, including operator arguments, substitutions, ASSUMEs and unused
/// definitions, is checked before the binder can evaluate a startup expression.
/// An original native declaration in embedded TLC has no executable call here.
/// The pinned binder skips native declarations when precomputing zero-arity
/// definitions. CFG redirections, which could change that dispatch, are refused.
pub(super) fn admit(ir: &Ir, report: &dive_api::ConfigReport) -> Result<BTreeSet<String>, Failure> {
    let mut forbidden = BTreeSet::new();
    for def in ir.defs() {
        if let Some(op) = def.std
            && disallowed(op)
        {
            let owner = ir.module(def.module);
            let embedded = op.module() == "TLC"
                && def.source.is_none()
                && def.origin.instance.is_none()
                && owner.name == "TLC"
                && owner.standard
                && def
                    .origin
                    .file
                    .and_then(|id| ir.files().get(id.index()))
                    .is_some_and(|f| f.standard && f.display == "<standard>/TLC.tla");
            if !embedded {
                return Err(refused("unqualified native declaration"));
            }
            forbidden.insert(def.name.clone());
        }
    }
    let blocked = |id| ir.def(id).std.is_some_and(disallowed);
    for node in ir.nodes() {
        if node
            .effects
            .intersects(dive_ir::Effects::IMPURE.union(dive_ir::Effects::LIVE_TLCGET))
            || matches!(&node.expr, Expr::Def(id) if blocked(*id))
            || node.expr.op_refs().into_iter().any(|op| match op {
                OpRef::Std(op) => disallowed(*op),
                OpRef::Def(id) => blocked(*id),
                _ => false,
            })
        {
            return Err(refused("unqualified native operation in temporal source"));
        }
    }

    let cfg = report
        .config()
        .ok_or_else(|| refused("unqualified TLC configuration"))?;
    let root = ir.module(ir.root());
    for (name, symbol) in &root.context {
        if matches!(symbol, Symbol::Def(id) if blocked(*id)) {
            forbidden.insert(name.clone());
        }
    }
    if cfg.assignments().count() != cfg.constants.len()
        || cfg.assignments().any(|a| {
            !a.args.is_empty()
                || forbidden.contains(&a.name.text)
                || !root.context.iter().any(|(name, symbol)| {
                    name == &a.name.text
                        && matches!(symbol, Symbol::Decl(id)
                        if ir.decl(*id).constant.is_some() && ir.decl(*id).arity == 0)
                })
        })
    {
        return Err(refused(
            "TLC admission requires direct literal assignments to declared constants",
        ));
    }
    for role in [
        &cfg.init,
        &cfg.next,
        &cfg.specification,
        &cfg.view,
        &cfg.symmetry,
        &cfg.alias,
        &cfg.periodic,
        &cfg.rl_reward,
    ]
    .into_iter()
    .flatten()
    .chain(&cfg.invariants)
    .chain(&cfg.properties)
    .chain(&cfg.constraints)
    .chain(&cfg.action_constraints)
    .chain(&cfg.postconditions)
    .chain(&cfg.possible)
    {
        if forbidden.contains(&role.text)
            || ir
                .root_def(&role.text)
                .is_none_or(|id| ir.def(id).std.is_some())
        {
            return Err(refused("TLC configuration selects an unqualified role"));
        }
    }
    Ok(forbidden)
}
