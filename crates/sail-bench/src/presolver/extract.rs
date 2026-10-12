//! Fragment extraction: a sound relaxation of a check's assertion stack.
//!
//! Every assertion is split into top-level conjuncts and each conjunct is
//! translated into [`Ir`], an expression over the fragment's operators and
//! variables. Two operations relax the stack, and both preserve
//! unsatisfiability (if the relaxation is unsat, so is the original):
//! - *dropping* a conjunct;
//! - *abstracting* a subterm outside the fragment by a fresh variable of the
//!   same sort, shared between syntactically identical closed subterms
//!   (identical closed terms denote the same value).
//!
//! [`Fragment::LiaUcBounds`] also adds ground instances of the pinned
//! prelude's own universally quantified axioms (qid `prelude_*`), found by
//! matching their triggers against the stack's ground terms. An instance of
//! an asserted axiom is entailed by the stack, so adding it is sound too.
//!
//! Only `unsat` of a relaxation says anything about the original check.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::rc::Rc;

use super::term::{Node, T, list, mentions};

/// Canonical-text limit for sharing an abstraction (longer terms get a
/// fresh, unshared variable, which is still sound).
const KEY_LIMIT: usize = 1 << 16;
/// Recursion limit for translation.
const DEPTH_LIMIT: usize = 400;
/// Instances added per check, at most.
const INSTANCE_LIMIT: usize = 20_000;

/// The candidate fragments, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fragment {
    /// Ground conjuncts that are entirely linear integer arithmetic over
    /// declared Int, Real and Bool constants. Everything else is dropped.
    Lia,
    /// `Lia`, plus every other ground or quantified subterm of sort Int,
    /// Real or Bool abstracted as an uninterpreted constant.
    LiaUc,
    /// `LiaUc`, plus ground instances of the prelude's axioms (integer
    /// ranges `uInv`/`iInv`/`uHi`, box/unbox, `Add`/`Sub`/…, `has_type`)
    /// and ground bit-vector operations.
    LiaUcBounds,
}

impl Fragment {
    /// All fragments, weakest first.
    pub const ALL: [Fragment; 3] = [Fragment::Lia, Fragment::LiaUc, Fragment::LiaUcBounds];

    /// The report name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Fragment::Lia => "lia",
            Fragment::LiaUc => "lia+uc",
            Fragment::LiaUcBounds => "lia+uc+bounds",
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    fn abstracts(self) -> bool {
        self != Fragment::Lia
    }

    fn bitvectors(self) -> bool {
        self == Fragment::LiaUcBounds
    }
}

/// A sort, by canonical text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Sort {
    /// `Bool`.
    Bool,
    /// `Int`.
    Int,
    /// `Real`.
    Real,
    /// `(_ BitVec n)`.
    Bv(u32),
    /// Anything else, by canonical text.
    Other(Rc<str>),
}

impl Sort {
    fn of(t: &Node) -> Sort {
        match t {
            Node::Atom(a) => match &**a {
                "Bool" => Sort::Bool,
                "Int" => Sort::Int,
                "Real" => Sort::Real,
                other => Sort::Other(Rc::from(other)),
            },
            Node::List(items) => {
                if let [u, b, w] = items.as_slice()
                    && u.atom() == Some("_")
                    && b.atom() == Some("BitVec")
                    && let Some(w) = w.atom().and_then(|w| w.parse().ok())
                {
                    return Sort::Bv(w);
                }
                Sort::Other(Rc::from(t.text(4096).unwrap_or_default().as_str()))
            }
        }
    }

    /// SMT-LIB text.
    #[must_use]
    pub fn smt(&self) -> String {
        match self {
            Sort::Bool => "Bool".to_owned(),
            Sort::Int => "Int".to_owned(),
            Sort::Real => "Real".to_owned(),
            Sort::Bv(w) => format!("(_ BitVec {w})"),
            Sort::Other(s) => s.to_string(),
        }
    }

    fn in_fragment(&self, frag: Fragment) -> bool {
        match self {
            Sort::Bool | Sort::Int | Sort::Real => true,
            Sort::Bv(_) => frag.bitvectors(),
            Sort::Other(_) => false,
        }
    }
}

/// An expression in the fragment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ir {
    /// A literal (`true`, `12`, `1.5`, `#b01`, `(_ bv3 8)`), as text.
    Lit(Rc<str>),
    /// A variable, by index into [`Ctx::vars`].
    Var(u32),
    /// An operator application; the operator is its SMT-LIB text.
    App(Rc<str>, Vec<Ir>),
}

/// Where a variable came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A declared constant.
    Declared,
    /// An abstracted subterm.
    Abstracted,
}

/// A variable of the relaxation.
#[derive(Debug, Clone)]
pub struct Var {
    /// The name used in emitted SMT-LIB.
    pub name: String,
    /// Its sort.
    pub sort: Sort,
    /// Where it came from.
    pub origin: Origin,
}

#[derive(Debug, Clone)]
struct Decl {
    arity: usize,
    sort: Sort,
}

/// A prelude axiom usable for instantiation.
#[derive(Debug, Clone)]
struct Axiom {
    vars: Vec<Rc<str>>,
    pattern: T,
    body: T,
}

/// One translated assertion.
#[derive(Debug, Clone, Default)]
pub struct Translated {
    /// Kept conjuncts per fragment.
    pub conjuncts: [Vec<Ir>; 3],
    /// Conjuncts the assertion had.
    pub total: usize,
    /// Closed application subterms outside binders (for instantiation).
    pub ground: Vec<T>,
}

#[derive(Debug, Default)]
struct Scope {
    decls: Vec<String>,
    axioms: usize,
}

/// Per-session extraction state: declarations in scope, the variable
/// table, and prelude axioms.
#[derive(Debug, Default)]
pub struct Ctx {
    decls: HashMap<String, Vec<Decl>>,
    /// Every variable created so far.
    pub vars: Vec<Var>,
    keys: HashMap<String, u32>,
    axioms: Vec<Axiom>,
    scopes: Vec<Scope>,
    /// Counter for let-expansion markers, unique across the session so
    /// that two markers never share an abstraction.
    opaque: u32,
}

const BOOL_OPS: &[&str] = &["not", "and", "or", "=>", "xor"];
const CMP_OPS: &[&str] = &["<=", "<", ">=", ">"];
const ARITH_OPS: &[&str] = &["+", "-", "*", "abs", "to_real", "to_int"];
const BV_OPS: &[&str] = &[
    "bvadd", "bvsub", "bvmul", "bvudiv", "bvurem", "bvsdiv", "bvsrem", "bvsmod", "bvand", "bvor",
    "bvxor", "bvnot", "bvneg", "bvshl", "bvlshr", "bvashr", "bvnand", "bvnor", "bvxnor", "concat",
    "bvcomp", "bvult", "bvule", "bvugt", "bvuge", "bvslt", "bvsle", "bvsgt", "bvsge",
];
const BV_INDEXED: &[&str] = &[
    "extract",
    "zero_extend",
    "sign_extend",
    "rotate_left",
    "rotate_right",
    "repeat",
];

fn is_numeral(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_decimal(s: &str) -> bool {
    let mut parts = s.splitn(2, '.');
    matches!((parts.next(), parts.next()), (Some(a), Some(b)) if is_numeral(a) && is_numeral(b))
}

fn bv_literal_width(s: &str) -> Option<u32> {
    if let Some(b) = s.strip_prefix("#b") {
        return u32::try_from(b.len()).ok();
    }
    s.strip_prefix("#x")
        .and_then(|h| u32::try_from(h.len() * 4).ok())
}

/// Whether `ir` is a numeric constant (a literal or a negated literal).
#[must_use]
pub fn is_constant(ir: &Ir) -> bool {
    match ir {
        Ir::Lit(l) => is_numeral(l) || is_decimal(l),
        Ir::App(op, args) if &**op == "-" && args.len() == 1 => {
            args.first().is_some_and(is_constant)
        }
        _ => false,
    }
}

fn is_nonzero_numeral(ir: &Ir) -> bool {
    match ir {
        Ir::Lit(l) => {
            (is_numeral(l) || is_decimal(l)) && l.bytes().any(|b| b.is_ascii_digit() && b != b'0')
        }
        _ => false,
    }
}

/// Splits a formula into conjuncts it entails.
fn conjuncts(t: &T, out: &mut Vec<T>) {
    let Some(items) = t.list() else {
        out.push(t.clone());
        return;
    };
    match (t.head(), items.len()) {
        (Some("and"), _) => {
            for i in items.iter().skip(1) {
                conjuncts(i, out);
            }
        }
        (Some("!"), n) if n >= 2 => {
            if let Some(i) = items.get(1) {
                conjuncts(i, out);
            }
        }
        (Some("not"), 2) => {
            let Some(inner) = items.get(1) else { return };
            let Some(inner_items) = inner.list() else {
                out.push(t.clone());
                return;
            };
            match (inner.head(), inner_items.len()) {
                (Some("not"), 2) => {
                    if let Some(i) = inner_items.get(1) {
                        conjuncts(i, out);
                    }
                }
                (Some("or"), _) => {
                    for i in inner_items.iter().skip(1) {
                        conjuncts(&list(vec![super::term::atom("not"), i.clone()]), out);
                    }
                }
                (Some("=>"), n) if n >= 3 => {
                    for i in inner_items.iter().skip(1).take(n - 2) {
                        conjuncts(i, out);
                    }
                    if let Some(last) = inner_items.last() {
                        conjuncts(&list(vec![super::term::atom("not"), last.clone()]), out);
                    }
                }
                (Some("!"), n) if n >= 2 => {
                    if let Some(i) = inner_items.get(1) {
                        conjuncts(&list(vec![super::term::atom("not"), i.clone()]), out);
                    }
                }
                _ => out.push(t.clone()),
            }
        }
        _ => out.push(t.clone()),
    }
}

fn is_binder(h: Option<&str>) -> bool {
    matches!(h, Some("forall" | "exists" | "let" | "lambda"))
}

/// Collects closed application subterms outside binders and annotations'
/// attributes.
fn collect_ground(t: &T, out: &mut Vec<T>, depth: usize) {
    if depth > DEPTH_LIMIT {
        return;
    }
    let Some(items) = t.list() else { return };
    let h = t.head();
    if is_binder(h) {
        return;
    }
    if h == Some("!") {
        if let Some(i) = items.get(1) {
            collect_ground(i, out, depth + 1);
        }
        return;
    }
    if h.is_some() && h != Some("_") {
        out.push(t.clone());
    }
    for i in items.iter().skip(1) {
        collect_ground(i, out, depth + 1);
    }
}

/// Expands `let` by substitution (sharing subterms). Quantifiers that
/// mention a let-bound name are left for a fresh abstraction by marking
/// them with a unique wrapper. Returns `None` if the budget runs out.
fn expand_lets(t: &T, env: &HashMap<Rc<str>, T>, budget: &mut usize, fresh: &mut u32) -> Option<T> {
    *budget = budget.checked_sub(1)?;
    match &**t {
        Node::Atom(a) => Some(env.get(a).cloned().unwrap_or_else(|| t.clone())),
        Node::List(items) => match t.head() {
            Some("let") => {
                let binds = items.get(1)?.list()?;
                let mut inner = env.clone();
                for b in binds {
                    let [name, value] = b.list()? else {
                        return None;
                    };
                    let v = expand_lets(value, env, budget, fresh)?;
                    inner.insert(Rc::from(name.atom()?), v);
                }
                expand_lets(items.get(2)?, &inner, budget, fresh)
            }
            Some("forall" | "exists" | "lambda") => {
                if env.is_empty() || !mentions(t, &mut |a| env.contains_key(a)) {
                    Some(t.clone())
                } else {
                    *fresh += 1;
                    Some(list(vec![
                        super::term::atom("sail!opaque"),
                        super::term::atom(&fresh.to_string()),
                    ]))
                }
            }
            _ => {
                if env.is_empty() && !mentions(t, &mut |a| a == "let") {
                    return Some(t.clone());
                }
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    out.push(expand_lets(i, env, budget, fresh)?);
                }
                Some(list(out))
            }
        },
    }
}

/// Parses `(forall ((v S)…) (! body :pattern (p) … :qid prelude_…))`.
fn prelude_axioms(t: &T) -> Vec<Axiom> {
    let mut out = Vec::new();
    let Some(items) = t.list() else { return out };
    if t.head() != Some("forall") {
        return out;
    }
    let (Some(vars), Some(annotated)) = (items.get(1).and_then(|v| v.list()), items.get(2)) else {
        return out;
    };
    let Some(vars) = vars
        .iter()
        .map(|v| {
            v.list()
                .and_then(|l| l.first())
                .and_then(|n| n.atom())
                .map(Rc::from)
        })
        .collect::<Option<Vec<Rc<str>>>>()
    else {
        return out;
    };
    let Some(ann) = annotated.list().filter(|_| annotated.head() == Some("!")) else {
        return out;
    };
    let Some(body) = ann.get(1) else { return out };
    let mut patterns = Vec::new();
    let mut prelude = false;
    let mut i = 2;
    while let (Some(k), Some(v)) = (ann.get(i), ann.get(i + 1)) {
        match k.atom() {
            Some(":pattern") => {
                if let Some([p]) = v.list() {
                    patterns.push(p.clone());
                }
            }
            Some(":qid") => prelude = v.atom().is_some_and(|q| q.starts_with("prelude_")),
            _ => {}
        }
        i += 2;
    }
    if prelude {
        for p in patterns {
            out.push(Axiom {
                vars: vars.clone(),
                pattern: p,
                body: body.clone(),
            });
        }
    }
    out
}

#[allow(clippy::many_single_char_names)] // Pattern, ground term, binding.
fn match_pattern(p: &T, g: &T, vars: &[Rc<str>], b: &mut HashMap<Rc<str>, T>) -> bool {
    match (&**p, &**g) {
        (Node::Atom(a), _) if vars.iter().any(|v| v == a) => {
            if let Some(prev) = b.get(a) {
                prev == g
            } else {
                b.insert(a.clone(), g.clone());
                true
            }
        }
        (Node::Atom(a), Node::Atom(c)) => a == c,
        (Node::List(ps), Node::List(gs)) => {
            ps.len() == gs.len() && ps.iter().zip(gs).all(|(p, g)| match_pattern(p, g, vars, b))
        }
        _ => false,
    }
}

/// Substitutes `b` into `t`; `None` if a nested binder could capture.
fn substitute(t: &T, b: &HashMap<Rc<str>, T>) -> Option<T> {
    match &**t {
        Node::Atom(a) => Some(b.get(a).cloned().unwrap_or_else(|| t.clone())),
        Node::List(items) => {
            if is_binder(t.head()) {
                // A nested binder: refuse if it rebinds a substituted name or
                // could capture a name the substituted terms mention.
                let bound: Vec<&str> = items
                    .get(1)
                    .and_then(|v| v.list())
                    .map(|v| {
                        v.iter()
                            .filter_map(|x| x.list().and_then(|l| l.first()).and_then(|n| n.atom()))
                            .collect()
                    })
                    .unwrap_or_default();
                if bound.iter().any(|n| b.contains_key(*n))
                    || b.values().any(|v| mentions(v, &mut |a| bound.contains(&a)))
                {
                    return None;
                }
            }
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                out.push(substitute(i, b)?);
            }
            Some(list(out))
        }
    }
}

impl Ctx {
    /// A fresh context with one (base) scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::default()],
            ..Self::default()
        }
    }

    /// Opens `n` scopes (`push`).
    pub fn push(&mut self, n: usize) {
        for _ in 0..n {
            self.scopes.push(Scope::default());
        }
    }

    /// Closes `n` scopes (`pop`); `false` on underflow.
    pub fn pop(&mut self, n: usize) -> bool {
        for _ in 0..n {
            if self.scopes.len() <= 1 {
                return false;
            }
            let Some(s) = self.scopes.pop() else {
                return false;
            };
            for name in s.decls {
                if let Some(v) = self.decls.get_mut(&name) {
                    v.pop();
                }
            }
            self.axioms
                .truncate(self.axioms.len().saturating_sub(s.axioms));
        }
        true
    }

    fn declare(&mut self, name: &str, arity: usize, sort: Sort) {
        self.decls
            .entry(name.to_owned())
            .or_default()
            .push(Decl { arity, sort });
        if let Some(s) = self.scopes.last_mut() {
            s.decls.push(name.to_owned());
        }
    }

    fn decl(&self, name: &str) -> Option<&Decl> {
        self.decls.get(name).and_then(|v| v.last())
    }

    /// Registers the declarations a command makes.
    pub fn declare_command(&mut self, c: &T) {
        let Some(items) = c.list() else { return };
        match c.head() {
            Some("declare-const") => {
                if let (Some(n), Some(s)) = (items.get(1).and_then(|n| n.atom()), items.get(2)) {
                    self.declare(n, 0, Sort::of(s));
                }
            }
            Some("declare-fun" | "define-fun") => {
                if let (Some(n), Some(a), Some(s)) = (
                    items.get(1).and_then(|n| n.atom()),
                    items.get(2).and_then(|a| a.list()),
                    items.get(3),
                ) {
                    self.declare(n, a.len(), Sort::of(s));
                }
            }
            Some("declare-datatypes") => {
                let (Some(heads), Some(bodies)) = (
                    items.get(1).and_then(|h| h.list()),
                    items.get(2).and_then(|b| b.list()),
                ) else {
                    return;
                };
                for (h, body) in heads.iter().zip(bodies) {
                    let Some([name, arity]) = h.list() else {
                        continue;
                    };
                    if arity.atom() != Some("0") {
                        continue;
                    }
                    let Some(name) = name.atom() else { continue };
                    let dt = Sort::Other(Rc::from(name));
                    for ctor in body.list().unwrap_or_default() {
                        match &**ctor {
                            Node::Atom(c) => {
                                let c = c.to_string();
                                self.declare(&c, 0, dt.clone());
                                self.declare(&format!("is-{c}"), 1, Sort::Bool);
                            }
                            Node::List(parts) => {
                                let Some(c) = parts.first().and_then(|c| c.atom()) else {
                                    continue;
                                };
                                let c = c.to_owned();
                                self.declare(&c, parts.len() - 1, dt.clone());
                                self.declare(&format!("is-{c}"), 1, Sort::Bool);
                                for sel in parts.iter().skip(1) {
                                    if let Some([s, ss]) = sel.list()
                                        && let Some(s) = s.atom()
                                    {
                                        self.declare(s, 1, Sort::of(ss));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Records prelude axioms from an `assert` body.
    pub fn record_axioms(&mut self, body: &T) {
        let found = prelude_axioms(body);
        if let Some(s) = self.scopes.last_mut() {
            s.axioms += found.len();
        }
        self.axioms.extend(found);
    }

    fn var(
        &mut self,
        key: String,
        name: impl FnOnce(usize) -> String,
        sort: Sort,
        origin: Origin,
    ) -> u32 {
        if let Some(&v) = self.keys.get(&key) {
            return v;
        }
        let id = u32::try_from(self.vars.len()).unwrap_or(u32::MAX);
        self.vars.push(Var {
            name: name(self.vars.len()),
            sort,
            origin,
        });
        self.keys.insert(key, id);
        id
    }

    fn fresh(&mut self, sort: Sort) -> u32 {
        let n = self.vars.len();
        self.var(
            format!("f:{n}"),
            |i| format!("sail!a{i}"),
            sort,
            Origin::Abstracted,
        )
    }

    fn abstraction(&mut self, t: &T, sort: Sort) -> u32 {
        // A let-expansion marker is unique by construction; any other closed
        // term is shared by its canonical text.
        match t.text(KEY_LIMIT) {
            Some(text) => {
                let key = format!("a:{}:{text}", sort.smt());
                self.var(key, |i| format!("sail!a{i}"), sort, Origin::Abstracted)
            }
            None => self.fresh(sort),
        }
    }

    /// The sort of a term, as far as it can be determined.
    fn sort_of(&self, t: &T, depth: usize) -> Option<Sort> {
        if depth > DEPTH_LIMIT {
            return None;
        }
        match &**t {
            Node::Atom(a) => {
                let a: &str = a;
                if is_numeral(a) {
                    Some(Sort::Int)
                } else if is_decimal(a) {
                    Some(Sort::Real)
                } else if matches!(a, "true" | "false") {
                    Some(Sort::Bool)
                } else if let Some(w) = bv_literal_width(a) {
                    Some(Sort::Bv(w))
                } else {
                    self.decl(a)
                        .filter(|d| d.arity == 0)
                        .map(|d| d.sort.clone())
                }
            }
            Node::List(items) => {
                let h = t.head();
                let child = |i: usize| items.get(i).and_then(|c| self.sort_of(c, depth + 1));
                match h {
                    Some("!") => child(1),
                    Some("sail!opaque" | "forall" | "exists" | "=" | "distinct") => {
                        Some(Sort::Bool)
                    }
                    Some(op) if BOOL_OPS.contains(&op) || CMP_OPS.contains(&op) => Some(Sort::Bool),
                    Some("ite") => child(2).or_else(|| child(3)),
                    Some("div" | "mod" | "to_int") => Some(Sort::Int),
                    Some("/" | "to_real") => Some(Sort::Real),
                    Some("+" | "-" | "*" | "abs") => Some(child(1).unwrap_or(Sort::Int)),
                    Some(
                        "bvult" | "bvule" | "bvugt" | "bvuge" | "bvslt" | "bvsle" | "bvsgt"
                        | "bvsge",
                    ) => Some(Sort::Bool),
                    Some("as") => items.get(2).map(|s| Sort::of(s)),
                    Some(f) => self
                        .decl(f)
                        .filter(|d| d.arity + 1 == items.len())
                        .map(|d| d.sort.clone()),
                    None => {
                        // `((_ is C) x)` is a tester.
                        let head = items.first()?;
                        if head.list().and_then(|l| l.get(1)).and_then(|a| a.atom()) == Some("is")
                            && head.head() == Some("_")
                        {
                            Some(Sort::Bool)
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }

    /// Translates a term in `frag`, abstracting where the fragment allows.
    fn term(&mut self, t: &T, frag: Fragment, depth: usize) -> Option<Ir> {
        if depth > DEPTH_LIMIT {
            return None;
        }
        if let Some(ir) = self.direct(t, frag, depth) {
            return Some(ir);
        }
        if !frag.abstracts() {
            return None;
        }
        let sort = self.sort_of(t, 0)?;
        if !sort.in_fragment(frag) {
            return None;
        }
        Some(Ir::Var(self.abstraction(t, sort)))
    }

    /// Translates `t` if its top operator is in the fragment.
    fn direct(&mut self, t: &T, frag: Fragment, depth: usize) -> Option<Ir> {
        match &**t {
            Node::Atom(a) => {
                let s: &str = a;
                if is_numeral(s) || is_decimal(s) || matches!(s, "true" | "false") {
                    return Some(Ir::Lit(a.clone()));
                }
                if bv_literal_width(s).is_some() {
                    return frag.bitvectors().then(|| Ir::Lit(a.clone()));
                }
                let d = self.decl(s)?;
                if d.arity != 0 || !d.sort.in_fragment(frag) {
                    return None;
                }
                let sort = d.sort.clone();
                let key = format!("c:{s}:{}", sort.smt());
                Some(Ir::Var(self.var(
                    key,
                    |_| s.to_owned(),
                    sort,
                    Origin::Declared,
                )))
            }
            Node::List(items) => {
                let head = items.first()?;
                if let Some(h) = head.list() {
                    // Indexed: `(_ bvN w)` literals and `((_ extract i j) x)`.
                    if !frag.bitvectors() || head.head() != Some("_") {
                        return None;
                    }
                    let name = h.get(1).and_then(|n| n.atom())?;
                    if !BV_INDEXED.contains(&name) {
                        return None;
                    }
                    let args = self.args(items, frag, depth)?;
                    return Some(Ir::App(Rc::from(head.text(256)?.as_str()), args));
                }
                let h = head.atom()?;
                match h {
                    "!" => self.term(items.get(1)?, frag, depth + 1),
                    "_" => {
                        let ok = frag.bitvectors()
                            && items.len() == 3
                            && items
                                .get(1)
                                .and_then(|b| b.atom())
                                .is_some_and(|b| b.strip_prefix("bv").is_some_and(is_numeral));
                        ok.then(|| Ir::Lit(Rc::from(t.text(256).unwrap_or_default().as_str())))
                    }
                    "ite" | "=" | "distinct" => {
                        let args = self.args(items, frag, depth)?;
                        if h == "ite" && args.len() != 3 || h != "ite" && args.len() < 2 {
                            return None;
                        }
                        Some(Ir::App(Rc::from(h), args))
                    }
                    op if BOOL_OPS.contains(&op) || CMP_OPS.contains(&op) => {
                        Some(Ir::App(Rc::from(h), self.args(items, frag, depth)?))
                    }
                    op if ARITH_OPS.contains(&op) => {
                        let args = self.args(items, frag, depth)?;
                        if op == "*" && args.iter().filter(|a| !is_constant(a)).count() > 1 {
                            return None;
                        }
                        Some(Ir::App(Rc::from(h), args))
                    }
                    "div" | "mod" | "/" => {
                        let args = self.args(items, frag, depth)?;
                        if args.len() != 2 || !args.get(1).is_some_and(is_nonzero_numeral) {
                            return None;
                        }
                        Some(Ir::App(Rc::from(h), args))
                    }
                    op if frag.bitvectors() && BV_OPS.contains(&op) => {
                        Some(Ir::App(Rc::from(h), self.args(items, frag, depth)?))
                    }
                    _ => None,
                }
            }
        }
    }

    fn args(&mut self, items: &[T], frag: Fragment, depth: usize) -> Option<Vec<Ir>> {
        items
            .iter()
            .skip(1)
            .map(|i| self.term(i, frag, depth + 1))
            .collect()
    }

    /// Translates an asserted formula for every fragment.
    pub fn translate(&mut self, body: &T) -> Translated {
        let mut out = Translated::default();
        let mut budget = 200_000usize;
        let expanded = if mentions(body, &mut |a| a == "let") {
            expand_lets(body, &HashMap::new(), &mut budget, &mut self.opaque)
        } else {
            Some(body.clone())
        };
        let Some(body) = expanded else {
            out.total = 1;
            return out;
        };
        let mut parts = Vec::new();
        conjuncts(&body, &mut parts);
        out.total = parts.len();
        for frag in Fragment::ALL {
            for p in &parts {
                if let Some(ir) = self.term(p, frag, 0) {
                    out.conjuncts[frag.index()].push(ir);
                }
            }
        }
        collect_ground(&body, &mut out.ground, 0);
        out
    }

    /// Ground instances of the prelude axioms in scope, triggered by
    /// `ground` and then by the instances' own ground terms (two rounds),
    /// translated as [`Fragment::LiaUcBounds`] conjuncts.
    pub fn instances(&mut self, ground: &[T]) -> (Vec<Ir>, usize) {
        let mut by_head: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in self.axioms.iter().enumerate() {
            if let Some(h) = a.pattern.head() {
                by_head.entry(h.to_owned()).or_default().push(i);
            }
        }
        let mut seen_terms: HashSet<*const Node> = HashSet::new();
        let mut seen_instances: HashSet<String> = HashSet::new();
        let mut out = Vec::new();
        let mut count = 0usize;
        let mut frontier: Vec<T> = ground.to_vec();
        for _round in 0..2 {
            let mut next = Vec::new();
            for g in &frontier {
                if !seen_terms.insert(Rc::as_ptr(g)) {
                    continue;
                }
                let Some(h) = g.head() else { continue };
                let Some(candidates) = by_head.get(h) else {
                    continue;
                };
                for &ai in candidates {
                    let Some(ax) = self.axioms.get(ai).cloned() else {
                        continue;
                    };
                    let mut b = HashMap::new();
                    if !match_pattern(&ax.pattern, g, &ax.vars, &mut b)
                        || ax.vars.iter().any(|v| !b.contains_key(v))
                    {
                        continue;
                    }
                    let Some(inst) = substitute(&ax.body, &b) else {
                        continue;
                    };
                    let Some(key) = inst.text(KEY_LIMIT) else {
                        continue;
                    };
                    if !seen_instances.insert(key) {
                        continue;
                    }
                    count += 1;
                    if count > INSTANCE_LIMIT {
                        return (out, count);
                    }
                    let mut parts = Vec::new();
                    conjuncts(&inst, &mut parts);
                    for p in &parts {
                        if let Some(ir) = self.term(p, Fragment::LiaUcBounds, 0) {
                            out.push(ir);
                        }
                    }
                    collect_ground(&inst, &mut next, 0);
                }
            }
            frontier = next;
        }
        (out, count)
    }
}

/// Writes `ir` as SMT-LIB.
pub fn write_ir(ir: &Ir, vars: &[Var], out: &mut String) {
    match ir {
        Ir::Lit(l) => out.push_str(l),
        Ir::Var(v) => out.push_str(
            vars.get(*v as usize)
                .map_or("sail!undefined", |v| v.name.as_str()),
        ),
        Ir::App(op, args) => {
            out.push('(');
            out.push_str(op);
            for a in args {
                out.push(' ');
                write_ir(a, vars, out);
            }
            out.push(')');
        }
    }
}

/// Collects the variables `ir` uses.
pub fn used_vars(ir: &Ir, out: &mut BTreeSet<u32>) {
    match ir {
        Ir::Lit(_) => {}
        Ir::Var(v) => {
            out.insert(*v);
        }
        Ir::App(_, args) => {
            for a in args {
                used_vars(a, out);
            }
        }
    }
}

/// The oracle query for a relaxation: declarations, named conjuncts,
/// `check-sat` and `get-unsat-core`.
#[must_use]
pub fn emit(conjuncts: &[&Ir], vars: &[Var], rlimit: u64) -> String {
    let mut used = BTreeSet::new();
    for c in conjuncts {
        used_vars(c, &mut used);
    }
    let mut s = String::from("(set-option :produce-unsat-cores true)\n");
    for v in &used {
        if let Some(var) = vars.get(*v as usize) {
            let _ = writeln!(s, "(declare-const {} {})", var.name, var.sort.smt());
        }
    }
    for (i, c) in conjuncts.iter().enumerate() {
        s.push_str("(assert (! ");
        write_ir(c, vars, &mut s);
        let _ = writeln!(s, " :named sail!c{i}))");
    }
    let _ = writeln!(
        s,
        "(set-option :rlimit {rlimit})\n(check-sat)\n(get-unsat-core)"
    );
    s
}

#[cfg(test)]
mod tests {
    use super::super::term::parse;
    use super::*;

    fn ctx_with(decls: &[&str]) -> Ctx {
        let mut c = Ctx::new();
        for d in decls {
            if let Some(t) = parse(d.as_bytes()) {
                c.declare_command(&t);
            }
        }
        c
    }

    fn body(s: &str) -> T {
        parse(s.as_bytes()).unwrap_or_else(|| super::super::term::atom("false"))
    }

    fn show(c: &Ctx, irs: &[Ir]) -> Vec<String> {
        irs.iter()
            .map(|i| {
                let mut s = String::new();
                write_ir(i, &c.vars, &mut s);
                s
            })
            .collect()
    }

    #[test]
    fn splits_negated_goals_and_drops_or_abstracts() {
        let mut c = ctx_with(&[
            "(declare-const x Int)",
            "(declare-const l Bool)",
            "(declare-sort Poly 0)",
            "(declare-fun f (Int) Int)",
            "(declare-fun P (Poly) Bool)",
            "(declare-const p Poly)",
        ]);
        let t = c.translate(&body("(not (=> l (and (P p) (<= (f x) (+ x 1)))))"));
        assert_eq!(t.total, 2);
        // LIA keeps only the label; the goal mentions `P` and `f`.
        assert_eq!(show(&c, &t.conjuncts[0]), ["l"]);
        let uc = show(&c, &t.conjuncts[1]);
        assert_eq!(uc.len(), 2);
        assert!(uc[1].starts_with("(not (and sail!a"));
        assert!(uc[1].contains("(<= sail!a"));
        assert!(uc[1].contains("(+ x 1)"));
    }

    #[test]
    fn shares_identical_closed_terms_and_rejects_nonlinear() {
        let mut c = ctx_with(&[
            "(declare-const x Int)",
            "(declare-const y Int)",
            "(declare-fun f (Int) Int)",
        ]);
        let a = c.translate(&body("(< (f x) 0)"));
        let b = c.translate(&body("(> (f x) (* x y))"));
        let sa = show(&c, &a.conjuncts[1]);
        let sb = show(&c, &b.conjuncts[1]);
        let var_a = sa[0]
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        assert!(sb[0].contains(&var_a), "{sa:?} {sb:?}");
        assert!(b.conjuncts[0].is_empty(), "nonlinear is not LIA");
    }

    #[test]
    fn instantiates_prelude_axioms() {
        let mut c = ctx_with(&[
            "(declare-const x Int)",
            "(declare-fun uHi (Int) Int)",
            "(declare-fun uInv (Int Int) Bool)",
        ]);
        let ax = body(
            "(forall ((bits Int) (i Int)) (! (= (uInv bits i) (and (<= 0 i) (< i (uHi bits)))) :pattern ((uInv bits i)) :qid prelude_u_inv))",
        );
        c.record_axioms(&ax);
        let not_ours =
            body("(forall ((i Int)) (! (= (uHi i) 0) :pattern ((uHi i)) :qid user_axiom))");
        c.record_axioms(&not_ours);
        let g = c.translate(&body("(not (uInv 64 x))"));
        let (inst, n) = c.instances(&g.ground);
        assert_eq!(n, 1);
        let s = show(&c, &inst);
        assert_eq!(s.len(), 1);
        assert!(s[0].contains("(<= 0 x)"), "{s:?}");
    }

    #[test]
    fn scopes_pop_declarations() {
        let mut c = ctx_with(&["(declare-sort Poly 0)"]);
        c.push(1);
        if let Some(t) = parse(b"(declare-const x Int)") {
            c.declare_command(&t);
        }
        assert_eq!(c.translate(&body("(< x 0)")).conjuncts[0].len(), 1);
        assert!(c.pop(1));
        if let Some(t) = parse(b"(declare-const x Poly)") {
            c.declare_command(&t);
        }
        assert_eq!(c.translate(&body("(= x x)")).conjuncts[0].len(), 0);
        assert!(!c.pop(1));
    }

    #[test]
    fn emits_named_conjuncts() {
        let mut c = ctx_with(&["(declare-const x Int)"]);
        let t = c.translate(&body("(and (< x 0) (> x 0))"));
        let refs: Vec<&Ir> = t.conjuncts[0].iter().collect();
        let q = emit(&refs, &c.vars, 1000);
        assert!(q.contains("(declare-const x Int)"));
        assert!(q.contains("(assert (! (< x 0) :named sail!c0))"));
        assert!(q.contains("(check-sat)"));
    }
}
