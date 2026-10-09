# 0018: Fragility measurement

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter's initial scope includes an opt-in seed portfolio that reports
how fragile a proof is, and the fragility census is a charter workload.
ADR 0009 also leaves the near-limit threshold for serving cached `unknown`
answers to be calibrated "from the fragility census".

What already existed:
- `sail-bench seeds` replayed captured sessions under 8 seeds and measured
  answer and effort stability (`docs/survey/solver-behavior.md`).
- The pieces it used were private to `sail-bench`: transcript splitting and
  pairing, seed injection, a fresh-process solver runner, trace discovery,
  statistics and a work queue.
- `sail-portfolio` and `sail-solver` were placeholders.

Constraints:
- Measurement must never change a verdict or reach a cache key (charter
  rules 2, 5 and 8).
- Mariposa's mutations (prior-art §3.1) are assertion shuffling, symbol
  renaming and reseeding. Shuffling found the most instability. Renaming
  and reordering are exactly the normalizations rule 5 forbids by default.
- The solver's input carries no function names. Function attribution
  from Verus's AIR logs and solver-op structure was built concurrently
  (ADR 0014: `sail-verus`, `sail-ledger`, `sail verify --output json`).

## Decision

### Shared machinery moves into the crates the charter names

- **`sail-solver::process`:** runs the pinned Z3 in a fresh process on an
  exact input, with solver identity (version and binary SHA-256). Its
  wall-clock guard is a safety net and is reported as an infrastructure
  failure, never as an answer. Error code `SAIL5001`.
- **`sail-portfolio`** gains:
  - `transcript`: splitting and pairing;
  - `sessions`: trace discovery;
  - `stats` and `parallel`: shared helpers;
  - `config`: the configurations, applied to exact bytes;
  - `fragility`: measurement, summaries and rendering.

  Error codes are `SAIL51xx`.
- **`sail-bench` re-exports** the moved modules under their old paths, so
  `seeds`, `history` and `profile` are unchanged.
- **Layering** (`xtask/src/layering.rs`) gains two edges:
  - `sail-bench → sail-portfolio`, for the census;
  - `sail-cli → sail-portfolio`, for `sail fragility`.

  A new structural test forbids `sail-shim`, `sail-key`, `sail-cache`,
  `sail-session` and `sail-api` from reaching `sail-portfolio`. Mutated
  sessions therefore cannot feed anything that derives, stores or serves a
  key. `sail-portfolio` itself does not depend on `sail-key` or
  `sail-cache`.

### What a configuration is

- **Baseline (seed 0):** the captured bytes, unmodified. Z3's default seed
  is 0, so this is what plain Verus ran.
- **Reseed `s`:** inserts `(set-option :smt.random_seed s)` and
  `(set-option :sat.random_seed s)` just after Verus's option prelude, as
  `sail-bench seeds` always has.
- **Shuffle `k` (opt-in, `--shuffles K`):** permutes each maximal run of two
  or more consecutive `assert` commands, using a fixed SplitMix64
  permutation seeded by `k`. All other bytes stay where they were.
  - **Why this preserves meaning.** A run has no command in between, so its
    asserts share one scope and see the same declarations, all made before
    the run. A check asks about the conjunction of the scope's assertions,
    and conjunction does not depend on order.
  - **What never moves.** `:named` asserts (a later assert may use the
    name), and every `push`, `pop`, declaration, definition, option and
    check. The scopes, declarations and options in force at each check are
    exactly as captured.
  - Only Z3's search may change. Measuring that is the purpose.
- **Renaming is not implemented.** A consistent α-renaming needs a full
  symbol table: sorts, functions, constants, bound variables, patterns,
  and the `:qid`/`:skolemid` attributes that name Z3's skolems. Verus also
  reads `%%location_label%%` names back out of models. Getting any of this
  wrong changes the query, so renaming waits for a parser-backed
  implementation with Z3 as the oracle (rule 5).

### What is measured

- **One fresh process per (session, configuration).** Each process replays
  the whole session. Z3 reads commands in order, so every check is measured
  after its exact captured prefix, and one process covers every check.
- **Caveat: later commands are the captured ones.** If a configuration
  changes an earlier answer, Verus might have sent something different
  afterwards. The later checks are still measured on the prefix plain Verus
  produced.
- **Per check:**
  - the answer under each configuration, and their distribution;
  - the number and fraction of configurations that prove it (`unsat`);
  - the `rlimit-count` delta under each configuration, as min, median, max
    and coefficient of variation;
  - the `:rlimit` in force, the largest share of it used, and the margin
    (`1 - max_used / limit`);
  - a **stability** class:
    - `stable-proved`: every configuration proves it;
    - `stable-unproved`: no configuration proves it;
    - `unstable`: some do and some do not;
    - `incomplete`: some configuration gave no paired answer, which is an
      infrastructure failure and never an answer;
  - a **score** in `[0, 1]`: the larger of answer disagreement
    (`2·min(p, 1−p)` for proved fraction `p`) and effort pressure
    (`min(1, max_used/limit)`).
- **Summaries:**
  - distributions of CV, max/min ratio, share of limit used and score;
  - a histogram of share of limit used;
  - the `unknown` answers: their reasons, the share of limit they used, and
    whether another configuration proved the check (ADR 0009's calibration
    data);
  - the baseline replay compared with the capture.
- **Unknowns with no reason on record.** A replay sends only the captured
  follow-ups, and Verus asks for `reason-unknown` only after a non-`unsat`
  answer. A configuration that fails a check the capture proved therefore
  has no reason on record. Such an `unknown` counts as an exhaustion when
  its delta reached the limit. It never counts as a non-exhaustion give-up.

### Function attribution (minimal, until ADR 0014's attribution is wired in)

- **Results are keyed by (session, check).** A check gets a function name
  only when Verus's `--time` breakdown for the run lists **exactly one**
  function that used solver effort (`rlimit > 0`), for example under
  `--verify-function`. Verus also lists the functions it skipped there,
  with an `rlimit` of 0. Otherwise the check stays unattributed. A wrong
  name would be worse than none.
- **Why the obvious heuristics fail.** On `examples/assertions.rs`:
  - the per-query `req%`/`ens%` declarations name callees, not the function
    being checked;
  - the breakdown's order is not the session's order;
  - several functions have equal `rlimit` totals.
- **The census therefore reports** per check, per session and per corpus
  unit (one source file for the examples), plus per function where
  attributed.
- **Next step.** Feed ADR 0014's attribution (AIR `;; <Kind> <function>`
  comments mapped to solver ops) into `CheckFragility::function`. Then
  fill the reserved `fragility` field of `--output json` and the MCP
  `fragility` tool. The measurement itself does not change: it already
  keys every check by session and index, which is what that attribution
  maps.

### Surfaces

- **`sail fragility`** (`sail-cli`; user-facing, as the charter's
  "fragility measurement" feature):
  - Usage: `[--seeds N] [--shuffles K] [--jobs J] [--json] [--keep-traces DIR | --traces DIR] [--z3 PATH] [--focus] [cargo verus verify args…]`.
  - It runs `cargo verus verify` once, through `sail-z3` in pure
    pass-through with tracing: no daemon, no cache. With `--focus` it runs
    `cargo verus focus` instead, which partial verification needs. It adds
    `--output-json --time` after `--`.
  - `--output json` is accepted as a synonym for `--json`. The MCP
    `fragility` tool (ADR 0016) runs `sail fragility --output json` with
    `verify`'s arguments.
  - Verus selectors after `--` (`--verify-module`, `--verify-only-module`,
    `--verify-function`, `--verify-root`) switch to `cargo verus focus`,
    as `--focus` does.
  - Example for one function:
    `sail fragility --focus -- --verify-root --verify-function max`.
  - If Cargo considers the crate up to date, Verus does not run and
    nothing is captured. The error says so.
  - It then measures and prints a summary, or the whole report with
    `--json`.
  - It never reads or writes the cache, so it cannot affect `sail verify`'s
    verdicts or keys.
- **`sail-bench fragility`** (the census, over corpus units):
  - Usage: `[--tier T] [--project P] [--filter S] [--seeds N] [--shuffles K] [--jobs J] [--verus-arg A]… [--name NAME]`.
  - It writes `bench/results/fragility-census-*.json`.
  - `sail-bench report` renders those files as sections of
    `docs/benchmarks.md`.
  - It is **local only**: it takes minutes on the fast tier and longer
    elsewhere, so it is never a CI job (AGENTS.md).
- **Tests.**
  - Unit tests run in the fast gate.
  - `crates/sail-portfolio/tests/real_solver.rs` measures the toy fixture
    under 3 seeds, and checks that shuffles are accepted by the pinned Z3.
    It takes seconds, in the `solver` step (`SAIL_TEST_Z3`).

## Consequences

- **The census is reproducible from one command per selection.** It records
  machine, commit, solver hash and configurations with its results.
- **ADR 0009's threshold now has data.** See ADR 0009's 2026-10-08 census
  update for the recommendation. The serving policy is unchanged until the
  owner decides.
- **An opt-in "accept a proof from any configuration" mode** is still to be
  built. It needs its own key namespace and an entry in
  `docs/divergences.md` (rule 8). This ADR adds measurement only.
- **Shuffling is available but off by default.** Its configurations are
  labelled in every report.

## Function reports (2026-10-09)

`--measure-fragility` is an explicit, local-only verification option. It captures
baseline sessions in pass-through mode and joins their full input SHA-256 and
ordered check sequence to AIR/transcript obligations. No function name is guessed
from timing or the number of positive-effort functions. Advisory checks are not
counted as primary proof obligations. A function score requires every primary
obligation to be covered and every replay to be complete; missing evidence stays
null with notes. Count a configuration as proving the function only if all its
primary obligations prove together. Keep the schema's fraction-not-proving score
separate from the census's existing effort-pressure and disagreement scores.

These are captured-prefix replays, not independent seeded Verus proofs. They do
not change the baseline verdict. The complete-run portfolio remains the only mode
that selects a different seeded proof. Ordinary verification runs no replay.
The fragility MCP response also carries an attributed verification report when
capturing fresh runs; trace-only measurements cannot invent verification results.
