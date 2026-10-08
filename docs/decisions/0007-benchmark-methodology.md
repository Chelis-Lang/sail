# 0007: Benchmark methodology (cold workload and baseline)

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter defines usefulness as a measured claim, set against a baseline
of plain Verus on the same machine, and adds a rule: every benchmark run also
runs the conformance check. Results are published in a generated
`docs/benchmarks.md`. Timing is noisy:
- plain Verus's per-function rlimit varies slightly between runs
  (`corpus/repros/z3-nla-rlimit-nondeterminism/`);
- other load on the machine distorts wall times.

## Decision

- **Cold workload.** `sail-bench cold` runs each unit for N rounds (default
  3). Each round runs plain Verus and Verus through `sail-z3`, one after the
  other, alternating which goes first to cancel out OS file-cache effects.
  Units run one at a time.
- **What is recorded per side and round:**
  - wall time;
  - user plus system CPU time of the process tree;
  - peak RSS of the largest single process (`/usr/bin/time`);
  - the solver time Verus reports (`--time`).
- **Conformance every round.** Each round compares the two runs with
  `sail_conformance::compare`. Any round that differs is a bug, reported in
  the results; it is never averaged away.
- **Where results go.** Results are written to `bench/results/<name>.json`
  (schema v1). These files record the machine, the pinned versions and the
  sail commit. `sail-bench report` renders every result file into
  `docs/benchmarks.md`, which is never edited by hand.
- **Reporting rules.** Reported figures are medians. Published baselines are
  taken on an otherwise idle machine. A result taken under load must say so
  in its name, and is not published as a baseline.

## Consequences

- The M3 baseline doubles as the pass-through overhead measurement the
  charter asks for in the cold workload.
- Later workloads plug into the same file format with a different `workload`
  value: warm, single-function edits, history replay, agent loops.
