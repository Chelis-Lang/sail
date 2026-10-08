# Z3 4.16.0: nondeterministic rlimit consumption on a nonlinear query

**Found:** 2026-10-08, while investigating rlimit drift in the first
fast-tier conformance run.

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

**Machine:** macOS 26 arm64, Apple M3.

## What happens

`query.smt2` is the exact byte stream plain Verus sent to one Z3 process
when verifying `examples/imo_1988_6.rs` from the Verus repository, at the
pinned commit, with `--crate-type=lib`. It is a spun-off
`by(nonlinear_arith)` query: one `check-sat` under
`(set-option :smt.arith.solver 6)` and `(set-option :rlimit 30000000)`.

Fed to the pinned Z3 repeatedly, the input is byte-identical every time, and
so is the answer (`unsat`). The `rlimit-count` reported after the check is
not:

```
$ ./repro.sh <pinned z3> 20
   5 rlimit-counts: 26688 26706 41645 | answers: unsat
  15 rlimit-counts: 26688 26706 41647 | answers: unsat
```

Within Verus, this shows up as per-function `rlimit` in `--output-json
--time` varying between runs of plain Verus. In 6 runs, `vieta_jump` reported
11218 five times and 11212 once. In 10 traced runs, all 15 solver processes
received byte-identical input every time. Three of the 14 nonlinear spinoff
processes reported varying `rlimit-count`. The main process, which uses
`smt.arith.solver 2`, did not.

## Why it matters

- **Charter rule 4** assumes resource limits are deterministic for a given
  solver and input. For Z3 4.16.0's nonlinear solver this holds only
  approximately: consumption varies by a few units. A query whose
  consumption is close to its limit could therefore get different answers in
  different runs, *in plain Verus*. The cause is not identified yet; timer-
  or memory-dependent code in the nonlinear arithmetic path is a candidate.
- **Conformance** must not treat rlimit drift as a sail difference. The
  harness reports it separately (ADR 0006).
- **Caching.** A cached answer is still an answer the pinned solver gave on
  that exact query. But the claim "the same verdict as plain Verus, every
  time" needs a precise meaning for queries whose plain-Verus outcome is
  itself not stable. Recorded in `docs/open-questions.md`.
