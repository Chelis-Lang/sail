//! EXPERIMENTAL in-process refutation prototype (L6 spike; timing only).
//!
//! A deliberately small DPLL(T) over the `lia+uc+bounds` relaxation:
//! - Tseitin clauses over Boolean variables and integer comparison atoms;
//! - unit propagation with chronological backtracking (no learning);
//! - a theory check by Fourier–Motzkin elimination over integer
//!   coefficients, with each derived constraint divided by the gcd of its
//!   coefficients and its bound floored (sound for integers).
//!
//! It answers only "refuted" or "don't know": any overflow, size limit,
//! real or bit-vector operator, or search limit gives up. It is not a
//! product component; it exists to estimate what in-process decision time
//! looks like on the extracted relaxations.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::extract::{Ir, Sort, Var, is_constant};

const MAX_FM_CONSTRAINTS: usize = 4_000;
const MAX_DECISIONS: usize = 2_000;

/// `Σ coef·x ≤ k` with integer coefficients, sorted by variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Le {
    coefs: Vec<(u32, i128)>,
    k: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Le {
    fn normalize(mut self) -> Self {
        self.coefs.retain(|&(_, c)| c != 0);
        let g = self.coefs.iter().fold(0, |g, &(_, c)| gcd(g, c));
        if g > 1 {
            for (_, c) in &mut self.coefs {
                *c /= g;
            }
            self.k = self.k.div_euclid(g);
        }
        self
    }

    fn negated(&self) -> Option<Self> {
        // ¬(Σ ≤ k)  ⇔  Σ ≥ k + 1  ⇔  -Σ ≤ -k - 1   (integers)
        Some(Le {
            coefs: self
                .coefs
                .iter()
                .map(|&(v, c)| c.checked_neg().map(|c| (v, c)))
                .collect::<Option<_>>()?,
            k: self.k.checked_add(1)?.checked_neg()?,
        })
    }
}

/// A linear integer expression.
#[derive(Debug, Clone, Default)]
struct Lin {
    coefs: BTreeMap<u32, i128>,
    c: i128,
}

impl Lin {
    fn scale(&self, f: i128) -> Option<Lin> {
        let mut out = Lin {
            coefs: BTreeMap::new(),
            c: self.c.checked_mul(f)?,
        };
        for (&v, &c) in &self.coefs {
            out.coefs.insert(v, c.checked_mul(f)?);
        }
        Some(out)
    }

    fn add(&mut self, o: &Lin) -> Option<()> {
        self.c = self.c.checked_add(o.c)?;
        for (&v, &c) in &o.coefs {
            let e = self.coefs.entry(v).or_insert(0);
            *e = e.checked_add(c)?;
        }
        Some(())
    }

    /// `self ≤ other` as [`Le`].
    fn le(&self, other: &Lin) -> Option<Le> {
        let mut d = self.clone();
        d.add(&other.scale(-1)?)?;
        Some(
            Le {
                coefs: d.coefs.into_iter().collect(),
                k: d.c.checked_neg()?,
            }
            .normalize(),
        )
    }
}

struct Builder<'a> {
    vars: &'a [Var],
    nvars: usize,
    clauses: Vec<Vec<i32>>,
    atoms: HashMap<usize, Le>,
    atom_ids: HashMap<Le, i32>,
    bool_ids: HashMap<u32, i32>,
    int_ids: HashMap<u32, u32>,
    next_int: u32,
}

impl Builder<'_> {
    fn new_var(&mut self) -> Option<i32> {
        self.nvars += 1;
        i32::try_from(self.nvars).ok()
    }

    fn atom(&mut self, le: Le) -> Option<i32> {
        if let Some(&v) = self.atom_ids.get(&le) {
            return Some(v);
        }
        let v = self.new_var()?;
        self.atoms.insert(self.nvars, le.clone());
        self.atom_ids.insert(le, v);
        Some(v)
    }

    fn fresh_int(&mut self) -> u32 {
        self.next_int += 1;
        u32::MAX - self.next_int
    }

    #[allow(clippy::many_single_char_names)] // Math-style names.
    fn lin(&mut self, ir: &Ir) -> Option<Lin> {
        match ir {
            Ir::Lit(l) => Some(Lin {
                coefs: BTreeMap::new(),
                c: l.parse().ok()?,
            }),
            Ir::Var(v) => {
                if self.vars.get(*v as usize)?.sort != Sort::Int {
                    return None;
                }
                let id = *self.int_ids.entry(*v).or_insert(*v);
                let mut coefs = BTreeMap::new();
                coefs.insert(id, 1);
                Some(Lin { coefs, c: 0 })
            }
            Ir::App(op, args) => match &**op {
                "+" => {
                    let mut acc = Lin::default();
                    for a in args {
                        acc.add(&self.lin(a)?)?;
                    }
                    Some(acc)
                }
                "-" => {
                    let first = self.lin(args.first()?)?;
                    if args.len() == 1 {
                        return first.scale(-1);
                    }
                    let mut acc = first;
                    for a in args.iter().skip(1) {
                        acc.add(&self.lin(a)?.scale(-1)?)?;
                    }
                    Some(acc)
                }
                "*" => {
                    let mut factor: i128 = 1;
                    let mut var: Option<Lin> = None;
                    for a in args {
                        if is_constant(a) {
                            factor = factor.checked_mul(self.lin(a)?.c)?;
                        } else if var.is_none() {
                            var = Some(self.lin(a)?);
                        } else {
                            return None;
                        }
                    }
                    match var {
                        Some(l) => l.scale(factor),
                        None => Some(Lin {
                            coefs: BTreeMap::new(),
                            c: factor,
                        }),
                    }
                }
                "mod" => {
                    // 0 ≤ (mod x m) ≤ |m| - 1 for a literal m ≠ 0.
                    let m = self.lin(args.get(1)?)?.c.checked_abs()?;
                    let z = self.fresh_int();
                    let zl = Lin {
                        coefs: [(z, 1)].into_iter().collect(),
                        c: 0,
                    };
                    let zero = Lin::default();
                    let top = Lin {
                        coefs: BTreeMap::new(),
                        c: m.checked_sub(1)?,
                    };
                    let a = self.atom(zero.le(&zl)?)?;
                    let b = self.atom(zl.le(&top)?)?;
                    self.clauses.push(vec![a]);
                    self.clauses.push(vec![b]);
                    Some(zl)
                }
                "div" | "abs" => {
                    let z = self.fresh_int();
                    Some(Lin {
                        coefs: [(z, 1)].into_iter().collect(),
                        c: 0,
                    })
                }
                "ite" => {
                    let c = self.bool(args.first()?)?;
                    let (a, b) = (self.lin(args.get(1)?)?, self.lin(args.get(2)?)?);
                    let z = self.fresh_int();
                    let zl = Lin {
                        coefs: [(z, 1)].into_iter().collect(),
                        c: 0,
                    };
                    for (guard, val) in [(c, &a), (-c, &b)] {
                        let x = self.atom(zl.le(val)?)?;
                        let y = self.atom(val.le(&zl)?)?;
                        self.clauses.push(vec![-guard, x]);
                        self.clauses.push(vec![-guard, y]);
                    }
                    Some(zl)
                }
                _ => None,
            },
        }
    }

    fn and(&mut self, lits: &[i32]) -> Option<i32> {
        let v = self.new_var()?;
        for &l in lits {
            self.clauses.push(vec![-v, l]);
        }
        let mut big: Vec<i32> = lits.iter().map(|l| -l).collect();
        big.push(v);
        self.clauses.push(big);
        Some(v)
    }

    fn or(&mut self, lits: &[i32]) -> Option<i32> {
        let negs: Vec<i32> = lits.iter().map(|l| -l).collect();
        Some(-self.and(&negs)?)
    }

    fn iff(&mut self, a: i32, b: i32) -> Option<i32> {
        let v = self.new_var()?;
        self.clauses.push(vec![-v, -a, b]);
        self.clauses.push(vec![-v, a, -b]);
        self.clauses.push(vec![v, a, b]);
        self.clauses.push(vec![v, -a, -b]);
        Some(v)
    }

    fn is_bool(&self, ir: &Ir) -> bool {
        match ir {
            Ir::Lit(l) => &**l == "true" || &**l == "false",
            Ir::Var(v) => self
                .vars
                .get(*v as usize)
                .is_some_and(|v| v.sort == Sort::Bool),
            Ir::App(op, args) => match &**op {
                "ite" => args.get(1).is_some_and(|a| self.is_bool(a)),
                "not" | "and" | "or" | "=>" | "xor" | "=" | "distinct" | "<=" | "<" | ">="
                | ">" => true,
                _ => false,
            },
        }
    }

    #[allow(clippy::many_single_char_names)] // Math-style names.
    fn compare(&mut self, op: &str, a: &Ir, b: &Ir) -> Option<i32> {
        let (x, y) = (self.lin(a)?, self.lin(b)?);
        let one = Lin {
            coefs: BTreeMap::new(),
            c: 1,
        };
        let mut x1 = x.clone();
        x1.add(&one)?;
        let mut y1 = y.clone();
        y1.add(&one)?;
        match op {
            "<=" => self.atom(x.le(&y)?),
            "<" => self.atom(x1.le(&y)?),
            ">=" => self.atom(y.le(&x)?),
            ">" => self.atom(y1.le(&x)?),
            "=" => {
                let p = self.atom(x.le(&y)?)?;
                let q = self.atom(y.le(&x)?)?;
                self.and(&[p, q])
            }
            _ => None,
        }
    }

    fn bool(&mut self, ir: &Ir) -> Option<i32> {
        match ir {
            Ir::Lit(l) => {
                let t = self.new_var()?;
                self.clauses.push(vec![t]);
                match &**l {
                    "true" => Some(t),
                    "false" => Some(-t),
                    _ => None,
                }
            }
            Ir::Var(v) => {
                if self.vars.get(*v as usize)?.sort != Sort::Bool {
                    return None;
                }
                if let Some(&b) = self.bool_ids.get(v) {
                    return Some(b);
                }
                let b = self.new_var()?;
                self.bool_ids.insert(*v, b);
                Some(b)
            }
            Ir::App(op, args) => {
                let op: &str = op;
                match op {
                    "not" => Some(-self.bool(args.first()?)?),
                    "and" | "or" => {
                        let lits = args
                            .iter()
                            .map(|a| self.bool(a))
                            .collect::<Option<Vec<_>>>()?;
                        if op == "and" {
                            self.and(&lits)
                        } else {
                            self.or(&lits)
                        }
                    }
                    "=>" => {
                        let mut lits = args
                            .iter()
                            .map(|a| self.bool(a))
                            .collect::<Option<Vec<_>>>()?;
                        let last = lits.pop()?;
                        let mut ors: Vec<i32> = lits.into_iter().map(|l| -l).collect();
                        ors.push(last);
                        self.or(&ors)
                    }
                    "xor" => {
                        let (a, b) = (self.bool(args.first()?)?, self.bool(args.get(1)?)?);
                        Some(-self.iff(a, b)?)
                    }
                    "ite" if self.is_bool(ir) => {
                        let c = self.bool(args.first()?)?;
                        let a = self.bool(args.get(1)?)?;
                        let b = self.bool(args.get(2)?)?;
                        let v = self.new_var()?;
                        self.clauses.push(vec![-c, -a, v]);
                        self.clauses.push(vec![-c, a, -v]);
                        self.clauses.push(vec![c, -b, v]);
                        self.clauses.push(vec![c, b, -v]);
                        Some(v)
                    }
                    "=" | "distinct" => {
                        let first = args.first()?;
                        let mut pairs = Vec::new();
                        if self.is_bool(first) {
                            let lits = args
                                .iter()
                                .map(|a| self.bool(a))
                                .collect::<Option<Vec<_>>>()?;
                            for w in lits.windows(2) {
                                let [a, b] = w else { return None };
                                pairs.push(self.iff(*a, *b)?);
                            }
                        } else {
                            for w in args.windows(2) {
                                let [a, b] = w else { return None };
                                pairs.push(self.compare("=", a, b)?);
                            }
                        }
                        if op == "distinct" && args.len() != 2 {
                            return None;
                        }
                        let all = self.and(&pairs)?;
                        Some(if op == "=" { all } else { -all })
                    }
                    "<=" | "<" | ">=" | ">" => {
                        let mut lits = Vec::new();
                        for w in args.windows(2) {
                            let [a, b] = w else { return None };
                            lits.push(self.compare(op, a, b)?);
                        }
                        match lits.as_slice() {
                            [one] => Some(*one),
                            _ => self.and(&lits),
                        }
                    }
                    _ => None,
                }
            }
        }
    }
}

/// Fourier–Motzkin with integer normalization: `false` if `cs` is
/// infeasible over the integers, `true` if feasible or undecided.
fn feasible(cs: &[Le]) -> bool {
    let mut set: HashSet<Le> = HashSet::new();
    for c in cs {
        if c.coefs.is_empty() {
            if c.k < 0 {
                return false;
            }
            continue;
        }
        set.insert(c.clone());
    }
    loop {
        // Pick the variable with the fewest resolvents.
        let mut counts: HashMap<u32, (usize, usize)> = HashMap::new();
        for c in &set {
            for &(v, k) in &c.coefs {
                let e = counts.entry(v).or_default();
                if k > 0 {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
        }
        let Some((&var, _)) = counts.iter().min_by_key(|(v, (p, n))| (p * n, **v)) else {
            return true;
        };
        let (pos, rest): (Vec<Le>, Vec<Le>) = set
            .drain()
            .partition(|c| c.coefs.iter().any(|&(v, k)| v == var && k > 0));
        let (neg, keep): (Vec<Le>, Vec<Le>) = rest
            .into_iter()
            .partition(|c| c.coefs.iter().any(|&(v, k)| v == var && k < 0));
        set.extend(keep);
        for p in &pos {
            for n in &neg {
                let a = p.coefs.iter().find(|x| x.0 == var).map_or(0, |x| x.1);
                let b = n.coefs.iter().find(|x| x.0 == var).map_or(0, |x| -x.1);
                let combined = (|| {
                    let mut m: BTreeMap<u32, i128> = BTreeMap::new();
                    for (cs, f) in [(&p.coefs, b), (&n.coefs, a)] {
                        for &(v, c) in cs {
                            let cur = m.get(&v).copied().unwrap_or(0);
                            m.insert(v, cur.checked_add(c.checked_mul(f)?)?);
                        }
                    }
                    m.remove(&var);
                    Some(
                        Le {
                            coefs: m.into_iter().filter(|&(_, c)| c != 0).collect(),
                            k: p.k.checked_mul(b)?.checked_add(n.k.checked_mul(a)?)?,
                        }
                        .normalize(),
                    )
                })();
                let Some(c) = combined else { return true };
                if c.coefs.is_empty() {
                    if c.k < 0 {
                        return false;
                    }
                    continue;
                }
                set.insert(c);
                if set.len() > MAX_FM_CONSTRAINTS {
                    return true;
                }
            }
        }
    }
}

struct Solver<'a> {
    clauses: &'a [Vec<i32>],
    atoms: &'a HashMap<usize, Le>,
    assign: Vec<i8>,
    trail: Vec<(i32, bool)>,
}

impl Solver<'_> {
    fn value(&self, lit: i32) -> i8 {
        let v = self
            .assign
            .get(lit.unsigned_abs() as usize)
            .copied()
            .unwrap_or(0);
        if lit < 0 { -v } else { v }
    }

    fn set(&mut self, lit: i32, decision: bool) {
        if let Some(slot) = self.assign.get_mut(lit.unsigned_abs() as usize) {
            *slot = if lit > 0 { 1 } else { -1 };
        }
        self.trail.push((lit, decision));
    }

    /// Unit propagation; `false` on a conflict.
    fn propagate(&mut self) -> bool {
        loop {
            let mut changed = false;
            for c in self.clauses {
                let mut unassigned = None;
                let mut free = 0;
                let mut sat = false;
                for &l in c {
                    match self.value(l) {
                        1 => {
                            sat = true;
                            break;
                        }
                        0 => {
                            free += 1;
                            unassigned = Some(l);
                        }
                        _ => {}
                    }
                }
                if sat {
                    continue;
                }
                match (free, unassigned) {
                    (0, _) => return false,
                    (1, Some(l)) => {
                        self.set(l, false);
                        changed = true;
                    }
                    _ => {}
                }
            }
            if !changed {
                return true;
            }
        }
    }

    fn theory_ok(&self) -> bool {
        let mut cs = Vec::new();
        for (&v, le) in self.atoms {
            match self.assign.get(v).copied().unwrap_or(0) {
                1 => cs.push(le.clone()),
                -1 => match le.negated() {
                    Some(n) => cs.push(n.normalize()),
                    None => return true,
                },
                _ => {}
            }
        }
        feasible(&cs)
    }

    fn backtrack(&mut self) -> bool {
        while let Some((lit, decision)) = self.trail.pop() {
            if let Some(slot) = self.assign.get_mut(lit.unsigned_abs() as usize) {
                *slot = 0;
            }
            if decision {
                self.set(-lit, false);
                return true;
            }
        }
        false
    }

    /// `Some(true)` refuted, `Some(false)` consistent, `None` gave up.
    fn run(&mut self) -> Option<bool> {
        let mut decisions = 0usize;
        loop {
            let ok = self.propagate() && self.theory_ok();
            if !ok {
                if !self.backtrack() {
                    return Some(true);
                }
                continue;
            }
            let open = self
                .clauses
                .iter()
                .find(|c| !c.iter().any(|&l| self.value(l) == 1))
                .and_then(|c| c.iter().find(|&&l| self.value(l) == 0).copied());
            let Some(lit) = open else { return Some(false) };
            decisions += 1;
            if decisions > MAX_DECISIONS {
                return None;
            }
            self.set(lit, true);
        }
    }
}

/// Tries to refute the conjunction. `Some(decisions)` if refuted.
#[must_use]
pub fn refute(conjuncts: &[&Ir], vars: &[Var]) -> Option<usize> {
    let mut b = Builder {
        vars,
        nvars: 0,
        clauses: Vec::new(),
        atoms: HashMap::new(),
        atom_ids: HashMap::new(),
        bool_ids: HashMap::new(),
        int_ids: HashMap::new(),
        next_int: 0,
    };
    for c in conjuncts {
        // A conjunct outside the prototype's reach is dropped: still a
        // relaxation.
        let mark = b.clauses.len();
        match b.bool(c) {
            Some(l) => b.clauses.push(vec![l]),
            None => b.clauses.truncate(mark),
        }
    }
    let mut s = Solver {
        clauses: &b.clauses,
        atoms: &b.atoms,
        assign: vec![0; b.nvars + 1],
        trail: Vec::new(),
    };
    match s.run() {
        Some(true) => Some(s.trail.len()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::extract::Ctx;
    use super::super::term::parse;
    use super::*;

    fn run(decls: &[&str], formula: &str) -> bool {
        let mut c = Ctx::new();
        for d in decls {
            if let Some(t) = parse(d.as_bytes()) {
                c.declare_command(&t);
            }
        }
        let Some(t) = parse(formula.as_bytes()) else {
            return false;
        };
        let tr = c.translate(&t);
        let refs: Vec<&Ir> = tr.conjuncts[0].iter().collect();
        refute(&refs, &c.vars).is_some()
    }

    #[test]
    fn refutes_linear_contradictions_and_only_those() {
        let d = [
            "(declare-const x Int)",
            "(declare-const y Int)",
            "(declare-const b Bool)",
        ];
        assert!(run(&d, "(and (< x y) (< y x))"));
        assert!(run(&d, "(and (< 0 x) (< x 1))"), "integer tightening");
        assert!(run(&d, "(and (or b (> x 5)) (not b) (<= x 5))"));
        assert!(run(&d, "(and (= (* 2 x) (+ (* 2 y) 1)))"), "gcd tightening");
        assert!(!run(&d, "(and (or b (> x 5)) (<= x 5))"));
        assert!(!run(&d, "(and (< x y) (< y (+ x 2)))"));
        assert!(run(&d, "(and (>= (mod x 4) 4))"));
        assert!(run(&d, "(and (= y (ite b 3 5)) (< y 3))"));
    }

    #[test]
    fn fm_handles_chains() {
        let le = |c: &[(u32, i128)], k: i128| Le {
            coefs: c.to_vec(),
            k,
        };
        // x ≤ y, y ≤ z, z ≤ x - 1.
        assert!(!feasible(&[
            le(&[(0, 1), (1, -1)], 0),
            le(&[(1, 1), (2, -1)], 0),
            le(&[(0, -1), (2, 1)], -1),
        ]));
        assert!(feasible(&[
            le(&[(0, 1), (1, -1)], 0),
            le(&[(1, 1), (2, -1)], 0)
        ]));
    }
}
