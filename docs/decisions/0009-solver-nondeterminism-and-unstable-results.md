# 0009: Solver nondeterminism and unstable results

- **Status:** accepted (owner decision, 2026-10-08)
- **Date:** 2026-10-08

## Context

The charter assumed resource limits were deterministic: the same query under
the same limit gives the same answer, which is what makes caching safe (rule
4). `corpus/repros/z3-nla-rlimit-nondeterminism/` shows that this is not quite
true. For nonlinear queries, Z3 4.16.0 spends slightly different effort on
byte-identical input from run to run.

A query that finishes close to its limit can therefore come back `unsat`
(proved) in one run and `unknown` (gave up) in the next. Plain Verus does
this on its own, with no sail involved. For such queries, "the same verdict
as plain Verus, every time" (rule 1) has no fixed meaning.

## Decision

- **Conformance compares against repeated plain runs.** It compares sail
  with plain Verus run on the same machine, with plain Verus run several
  times. A function whose plain outcome varies between those runs is
  classified **unstable**. Unstable functions are reported separately, never
  counted as sail differences, and the fragility census measures how common
  they are.
- **The caching consequences are asymmetric:**
  - **A cached `unsat` is safe to serve.** If Z3 found a proof once, the
    proof exists, however close to the limit it was. sail will then report
    "verified" more consistently than plain Verus does. That is a difference
    in consistency, not in correctness, and it is reported as unstable, not
    hidden.
  - **A cached `unknown` near the limit is the risk.** Serving it would
    freeze a failure that plain Verus sometimes proves. Every cached result
    records the effort it used: the `rlimit-count` delta and the limit in
    force. An `unknown` that finished near its limit is unstable. sail must
    either re-solve it or flag it, and never present it as a settled
    failure. An `unknown` that stopped well short of its limit (incomplete
    quantifier reasoning, for example) is an ordinary cached answer.
- **rlimit consumption never enters a cache key.** It is something Z3
  reports, not something sail gives it. The key binds the rlimit *setting*
  (an option in force, ADR 0004/0008). The consumption is recorded with the
  result and drives the unstable flag.
- **"Near the limit"** starts as a threshold on consumed/limit. It will be
  calibrated from the fragility census before the cache serves `unknown`s;
  until calibrated, every cached `unknown` is treated as unstable.
- **Charter rule 4 is amended** to match (charter appendix, item 2).

## Consequences

- `sail-conformance` gains a repeated-plain-runs mode and an `unstable` unit
  classification, next to `same`, `different` and `infra-error`.
- The ledger (`sail-ledger`) must carry effort and an unstable flag on every
  confirmed result.
- Benchmarks that use the cache must report unstable results as their own
  category.

## Update 2026-10-08

Implemented in `sail-conformance`:
- **Default runs.** One plain run and one sail run per unit.
- **Confirming differences.** When they differ, up to `--confirm-runs`
  further plain runs (default 2) are made.
- **Classification.** A unit is `unstable` when plain Verus disagrees with
  itself on a function verdict, diagnostics or exit code, and sail matches
  one of the plain runs. Otherwise it stays `different`.
- **Up-front repetition.** `--plain-runs N` repeats plain runs up front, for
  the fragility census.
- **rlimit drift** alone never makes a unit unstable: consumption is an
  output, not a verdict.
