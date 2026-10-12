//! The "tight rational" relaxation of an integer [`Ir`] conjunction.
//!
//! Integer variables become rationals, which a simplex with Farkas
//! certificates decides without branch-and-bound. Each integer comparison
//! atom is replaced by a fresh Boolean `p` with `p ⇒ POS` and `¬p ⇒ NEG`,
//! where `POS`/`NEG` are the atom and its negation tightened for integers
//! (`a < b` becomes `a + 1 ≤ b`, `¬(a ≤ b)` becomes `a ≥ b + 1`, and so on).
//! Every integer model of the original satisfies this with `p` set to the
//! atom's value, so it is a relaxation: `unsat` here implies `unsat` there.
//! Integer-only operators (`div`, `mod`, `to_int`) are abstracted.
//!
//! If the tight relaxation is `unsat`, a pre-solver built on rational
//! simplex plus integer bound tightening can refute the check with a
//! Farkas-style certificate, no integer search needed.

use std::fmt::Write as _;

use super::extract::{Ir, Sort, Var};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Int,
    Real,
}

struct Emitter<'a> {
    vars: &'a [Var],
    decls: String,
    side: String,
    fresh: usize,
}

fn numeral_as_real(l: &str) -> String {
    if l.contains('.') {
        l.to_owned()
    } else {
        format!("{l}.0")
    }
}

impl Emitter<'_> {
    fn kind(&self, ir: &Ir) -> Option<Kind> {
        match ir {
            Ir::Lit(l) if &**l == "true" || &**l == "false" => Some(Kind::Bool),
            Ir::Lit(l) if l.contains('.') => Some(Kind::Real),
            Ir::Lit(l) if l.bytes().all(|b| b.is_ascii_digit()) => Some(Kind::Int),
            Ir::Lit(_) => None,
            Ir::Var(v) => match self.vars.get(*v as usize)?.sort {
                Sort::Bool => Some(Kind::Bool),
                Sort::Int => Some(Kind::Int),
                Sort::Real => Some(Kind::Real),
                _ => None,
            },
            Ir::App(op, args) => match &**op {
                "not" | "and" | "or" | "=>" | "xor" | "=" | "distinct" | "<=" | "<" | ">="
                | ">" => Some(Kind::Bool),
                "ite" => self.kind(args.get(1)?),
                "div" | "mod" | "to_int" => Some(Kind::Int),
                "/" | "to_real" => Some(Kind::Real),
                "+" | "-" | "*" | "abs" => {
                    let mut k = Kind::Int;
                    for a in args {
                        match self.kind(a)? {
                            Kind::Real => k = Kind::Real,
                            Kind::Int => {}
                            Kind::Bool => return None,
                        }
                    }
                    Some(k)
                }
                _ => None,
            },
        }
    }

    fn fresh_real(&mut self) -> String {
        self.fresh += 1;
        let name = format!("sail!r{}", self.fresh);
        let _ = writeln!(self.decls, "(declare-const {name} Real)");
        name
    }

    fn num(&mut self, ir: &Ir) -> Option<String> {
        Some(match ir {
            Ir::Lit(l) => numeral_as_real(l),
            Ir::Var(v) => self.vars.get(*v as usize)?.name.clone(),
            Ir::App(op, args) => match &**op {
                "div" | "mod" | "to_int" => self.fresh_real(),
                "to_real" => self.num(args.first()?)?,
                "abs" => {
                    let x = self.num(args.first()?)?;
                    format!("(ite (>= {x} 0.0) {x} (- {x}))")
                }
                "ite" => format!(
                    "(ite {} {} {})",
                    self.bool(args.first()?)?,
                    self.num(args.get(1)?)?,
                    self.num(args.get(2)?)?
                ),
                "+" | "-" | "*" | "/" => {
                    let mut s = format!("({op}");
                    for a in args {
                        s.push(' ');
                        s.push_str(&self.num(a)?);
                    }
                    s.push(')');
                    s
                }
                _ => return None,
            },
        })
    }

    /// A tightened atom: a fresh `p` with `p ⇒ pos` and `¬p ⇒ neg`.
    fn atom(&mut self, pos: &str, neg: &str) -> String {
        self.fresh += 1;
        let p = format!("sail!t{}", self.fresh);
        let _ = writeln!(self.decls, "(declare-const {p} Bool)");
        let _ = writeln!(self.side, "(assert (=> {p} {pos}))");
        let _ = writeln!(self.side, "(assert (=> (not {p}) {neg}))");
        p
    }

    fn compare(&mut self, op: &str, a: &Ir, b: &Ir) -> Option<String> {
        let ints = self.kind(a)? == Kind::Int && self.kind(b)? == Kind::Int;
        let (x, y) = (self.num(a)?, self.num(b)?);
        if !ints {
            return Some(match op {
                "distinct" => format!("(not (= {x} {y}))"),
                _ => format!("({op} {x} {y})"),
            });
        }
        let le = |l: &str, r: &str| format!("(<= {l} {r})");
        let lt = |l: &str, r: &str| format!("(<= (+ {l} 1.0) {r})");
        let (pos, neg) = match op {
            "<=" => (le(&x, &y), lt(&y, &x)),
            "<" => (lt(&x, &y), le(&y, &x)),
            ">=" => (le(&y, &x), lt(&x, &y)),
            ">" => (lt(&y, &x), le(&x, &y)),
            "=" => (
                format!("(= {x} {y})"),
                format!("(or {} {})", lt(&x, &y), lt(&y, &x)),
            ),
            "distinct" => (
                format!("(or {} {})", lt(&x, &y), lt(&y, &x)),
                format!("(= {x} {y})"),
            ),
            _ => return None,
        };
        Some(self.atom(&pos, &neg))
    }

    fn chain(&mut self, op: &str, args: &[Ir]) -> Option<String> {
        if op == "distinct" {
            let mut parts = Vec::new();
            for (i, a) in args.iter().enumerate() {
                for b in args.iter().skip(i + 1) {
                    parts.push(self.compare("distinct", a, b)?);
                }
            }
            return Some(format!("(and {})", parts.join(" ")));
        }
        let mut parts = Vec::new();
        for w in args.windows(2) {
            let [a, b] = w else { return None };
            parts.push(self.compare(op, a, b)?);
        }
        Some(match parts.as_slice() {
            [one] => one.clone(),
            _ => format!("(and {})", parts.join(" ")),
        })
    }

    fn bool(&mut self, ir: &Ir) -> Option<String> {
        match ir {
            Ir::Lit(l) if &**l == "true" || &**l == "false" => Some(l.to_string()),
            Ir::Var(v) => {
                let var = self.vars.get(*v as usize)?;
                (var.sort == Sort::Bool).then(|| var.name.clone())
            }
            Ir::App(op, args) => match &**op {
                "not" | "and" | "or" | "=>" | "xor" => {
                    let mut s = format!("({op}");
                    for a in args {
                        s.push(' ');
                        s.push_str(&self.bool(a)?);
                    }
                    s.push(')');
                    Some(s)
                }
                "ite" => Some(format!(
                    "(ite {} {} {})",
                    self.bool(args.first()?)?,
                    self.bool(args.get(1)?)?,
                    self.bool(args.get(2)?)?
                )),
                "=" | "distinct" if self.kind(args.first()?)? == Kind::Bool => {
                    let mut s = format!("({op}");
                    for a in args {
                        s.push(' ');
                        s.push_str(&self.bool(a)?);
                    }
                    s.push(')');
                    Some(s)
                }
                "<=" | "<" | ">=" | ">" | "=" | "distinct" => self.chain(op, args),
                _ => None,
            },
            Ir::Lit(_) => None,
        }
    }
}

/// The tight rational relaxation of `conjuncts` as an SMT-LIB query, or
/// `None` if they use operators outside linear arithmetic (bit-vectors).
#[must_use]
pub fn emit(conjuncts: &[&Ir], vars: &[Var], rlimit: u64) -> Option<String> {
    let mut e = Emitter {
        vars,
        decls: String::new(),
        side: String::new(),
        fresh: 0,
    };
    let mut asserts = String::new();
    let mut used = std::collections::BTreeSet::new();
    for c in conjuncts {
        super::extract::used_vars(c, &mut used);
        let b = e.bool(c)?;
        let _ = writeln!(asserts, "(assert {b})");
    }
    let mut s = String::new();
    for v in used {
        let var = vars.get(v as usize)?;
        let sort = match var.sort {
            Sort::Bool => "Bool",
            Sort::Int | Sort::Real => "Real",
            _ => return None,
        };
        let _ = writeln!(s, "(declare-const {} {sort})", var.name);
    }
    s.push_str(&e.decls);
    s.push_str(&e.side);
    s.push_str(&asserts);
    let _ = writeln!(s, "(set-option :rlimit {rlimit})\n(check-sat)");
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::super::extract::Ctx;
    use super::super::term::parse;
    use super::*;

    #[test]
    fn tightens_integer_atoms_in_both_polarities() {
        let mut c = Ctx::new();
        for d in ["(declare-const x Int)", "(declare-const b Bool)"] {
            if let Some(t) = parse(d.as_bytes()) {
                c.declare_command(&t);
            }
        }
        let Some(t) = parse(b"(and (< 0 x) (not (>= x 1)) (=> b (= x 2)))") else {
            unreachable!()
        };
        let tr = c.translate(&t);
        let refs: Vec<&Ir> = tr.conjuncts[0].iter().collect();
        let q = emit(&refs, &c.vars, 100);
        let Some(q) = q else { unreachable!() };
        assert!(q.contains("(declare-const x Real)"));
        assert!(
            q.contains("(assert (=> sail!t1 (<= (+ 0.0 1.0) x)))"),
            "{q}"
        );
        assert!(q.contains("(assert (=> (not sail!t1) (<= x 0.0)))"), "{q}");
        assert!(q.contains("(assert (not sail!t2))"), "{q}");
    }
}
