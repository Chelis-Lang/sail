#!/usr/bin/env python3
"""SPIKE: generate deterministic quantifier-free runtime-style queries.

Each file is one single-check script with an explicit logic, in the shapes
runtime clients send (issue #137): shared declarations and named definitions,
enumerated datatypes, guarded requirements, two-copy uniqueness, blocking
clauses, bit-vector identities. Roughly four in five are unsatisfiable by
construction; the solver, not this script, decides each one.
"""
import random
import sys
from pathlib import Path


def lia_chain(r, n):
    xs = [f"x{i}" for i in range(n)]
    out = ["(set-logic QF_LIA)"] + [f"(declare-const {x} Int)" for x in xs]
    for a, b in zip(xs, xs[1:]):
        out.append(f"(assert (< (+ {a} {r.randint(0, 3)}) {b}))")
    out.append(f"(assert (<= {xs[-1]} (+ {xs[0]} {r.randint(0, n)})))")
    return out


def lia_bounds(r, n):
    out = ["(set-logic QF_LIA)"] + [f"(declare-const v{i} Int)" for i in range(n)]
    total = 0
    for i in range(n):
        lo = r.randint(-5, 5)
        total += lo
        out.append(f"(assert (>= v{i} {lo}))")
    s = "(+ " + " ".join(f"v{i}" for i in range(n)) + ")"
    slack = r.choice([-1, -1, -1, -2, 0])
    out.append(f"(assert (<= {s} {total + slack}))")
    return out


def uf_congruence(r, n):
    out = ["(set-logic QF_UF)", "(declare-sort U 0)", "(declare-fun f (U) U)",
           "(declare-fun g (U U) U)"]
    out += [f"(declare-const a{i} U)" for i in range(n)]
    for i in range(n - 1):
        out.append(f"(assert (= a{i} a{i + 1}))")
    i, j = r.sample(range(n), 2)
    depth = r.randint(1, 3)
    l, rr = f"a{i}", f"a{j}"
    for _ in range(depth):
        l, rr = f"(f (g {l} a0))", f"(f (g {rr} a{n - 1}))"
    out.append(f"(assert (not (= {l} {rr})))")
    return out


def uflia_two_copy(r, n):
    out = ["(set-logic QF_UFLIA)", "(declare-fun rate (Int) Int)"]
    for c in ("a", "b"):
        out += [f"(declare-const in_{c} Int)", f"(declare-const out_{c} Int)"]
        out += [f"(define-fun base_{c} () Int (rate in_{c}))"]
        for k in range(n):
            out.append(f"(assert (=> (> in_{c} {k * 10}) (>= out_{c} (+ base_{c} {k}))))")
        out.append(f"(assert (= out_{c} (+ base_{c} (ite (> in_{c} {(n - 1) * 10}) {n - 1} out_{c}))))"
                   if False else f"(assert (= out_{c} (+ (* 2 in_{c}) base_{c})))")
    out.append("(assert (= in_a in_b))")
    out.append("(assert (not (= out_a out_b)))" if r.random() < 0.85
               else "(assert (> out_a 0))")
    return out


def enum_states(r, n):
    names = [f"s{i}" for i in range(n)]
    out = ["(set-logic QF_DT)",
           "(declare-datatypes ((State 0)) ((" + " ".join(f"({x})" for x in names) + ")))",
           "(declare-const cur State)", "(declare-const nxt State)"]
    excluded = names if r.random() < 0.8 else names[:-1]
    for x in excluded:
        out.append(f"(assert (not (= cur {x})))")
    out.append("(assert (= nxt cur))")
    return out


def record_dt(r, n):
    out = ["(set-logic QF_UFDTLIA)",
           "(declare-datatypes ((Req 0)) (((mk (prio Int) (size Int)) (none))))",
           "(declare-const p Req)", "(declare-const q Req)"]
    k = r.randint(1, 9)
    out += ["(assert ((_ is mk) p))", "(assert ((_ is mk) q))",
            f"(assert (= (prio p) (+ (prio q) {k})))",
            "(assert (= (size p) (size q)))",
            "(assert (= p q))" if r.random() < 0.85 else "(assert (not (= p q)))"]
    return out


def bool_blocking(r, n):
    out = ["(set-logic QF_UF)"] + [f"(declare-const b{i} Bool)" for i in range(n)]
    out += [f"(define-fun d{i} () Bool (and b{i} (not b{(i + 1) % n})))" for i in range(n)]
    out.append("(assert (or " + " ".join(f"d{i}" for i in range(n)) + "))")
    # Block assignments one by one; blocking every "some b true" case is unsat.
    if r.random() < 0.8:
        for i in range(n):
            out.append(f"(assert (not b{i}))")
    else:
        for i in range(n - 1):
            out.append(f"(assert (or (not b{i}) b{i + 1}))")
    return out


def bv_identity(r, n):
    w = r.choice([8, 16, 32])
    out = [f"(set-logic QF_BV)", f"(declare-const x (_ BitVec {w}))",
           f"(declare-const y (_ BitVec {w}))"]
    ids = [
        "(= (bvand x y) (bvand y x))",
        "(= (bvor x (bvand x y)) x)",
        "(= (bvxor x x) (bvsub y y))",
        "(= (bvadd x y) (bvadd y x))",
        "(= (bvnot (bvand x y)) (bvor (bvnot x) (bvnot y)))",
        "(= (bvsub (bvadd x y) y) x)",
        "(bvule (bvand x y) x)",
        f"(= (bvshl x (_ bv1 {w})) (bvadd x x))",
    ]
    out.append(f"(assert (not {r.choice(ids)}))")
    return out


def lra_mix(r, n):
    out = ["(set-logic QF_LRA)"] + [f"(declare-const r{i} Real)" for i in range(n)]
    for i in range(n - 1):
        out.append(f"(assert (<= (* 2.0 r{i}) r{i + 1}))")
    out.append("(assert (>= r0 1.0))")
    out.append(f"(assert (< r{n - 1} {2 ** (n - 1)}.0))" if r.random() < 0.85
               else f"(assert (< r{n - 1} {2 ** n}.0))")
    return out


def uf_guarded(r, n):
    out = ["(set-logic QF_UFLIA)", "(declare-fun lim (Int) Int)", "(declare-const t Int)"]
    out += [f"(declare-const g{i} Bool)" for i in range(n)]
    for i in range(n):
        out.append(f"(assert (! (=> g{i} (and (>= t {i * 5}) (< t {i * 5 + 5}))) :named req{i}))")
    i, j = r.sample(range(n), 2)
    out.append(f"(assert g{i})")
    out.append(f"(assert g{j})" if r.random() < 0.85 else f"(assert (not g{j}))")
    out.append("(assert (= (lim t) (lim (+ t 0))))")
    return out


FAMILIES = [lia_chain, lia_bounds, uf_congruence, uflia_two_copy, enum_states,
            record_dt, bool_blocking, bv_identity, lra_mix, uf_guarded]


def main():
    out = Path(sys.argv[1])
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 200
    out.mkdir(parents=True, exist_ok=True)
    r = random.Random(20261109)
    for index in range(count):
        family = FAMILIES[index % len(FAMILIES)]
        lines = family(r, r.randint(3, 12))
        lines.append("(check-sat)")
        (out / f"q{index:03d}-{family.__name__}.smt2").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
