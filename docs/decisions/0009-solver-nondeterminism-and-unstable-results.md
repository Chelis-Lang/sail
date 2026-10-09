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

## Update 2026-10-08: effort is recorded

Every `check-sat` cache entry now records, in its `effort` metadata:
- `limit`: the `:rlimit` in force;
- `used`: the `rlimit-count` delta between the statistics replies around
  the check;
- `reason`: the `reason-unknown` reply, for `unknown` answers.

`unknown` answers are now stored, so the census has data, but they are still
never served. In the toy fixture, the `unknown` answers that come from
failing proofs report `(incomplete quantifiers)` or
`(incomplete (theory arithmetic))` and use about 0.02% of their limit, far
from it. Calibrating the near-limit threshold from the fragility census
decides when `unknown` answers like these may be served.

## Update 2026-10-08: serving far-from-limit `unknown` (owner decision)

**Census.** A fast-tier fill stored 3,929 `unsat` and 97 `unknown` answers.
- All 97 `unknown` answers report `(incomplete quantifiers)`.
- Their effort is at most 0.0089% of the 30M limit (median 0.0048%).
- The rlimit jitter seen in the nonlinear repro is a few units.

**The owner approved the policy.** A cached `unknown` is served only when
both of these hold:
- its recorded reason is not resource exhaustion (`canceled`, or anything
  mentioning resources, rlimit, timeout or memory);
- it used at most 50% of a non-zero limit.

Every other `unknown` is re-solved, as is any `unknown` with no recorded
effort.

**Implementation.**
- The daemon appends the entry's effort to `GET` hits as an optional trailer.
  Older clients ignore it.
- The shim decides. If a daemon sends no trailer, the shim refuses to serve
  the `unknown`.

## Update 2026-10-09: fragility census and a calibrated threshold (recommendation)

The fragility census (ADR 0018) gives the calibration data this ADR asked
for. **This update recommends a threshold; it does not change the serving
policy.** The owner decides.

**Census.**
- **Fast tier, 147 units.** That is 763 solver sessions and 4,086 checks,
  each re-run under 8 configurations: the unmodified session and seeds 1
  to 7 (`bench/results/fragility-census-fast-arm64-macos-devload-2026-10-08.json`).
- **Run twice.** The two runs agreed on every reported check and every
  class count. Their distributions differed only in the sixth digit of
  the means, which is the known nonlinear `rlimit` jitter.
- **Stability.** 3,986 checks were proved under every configuration, 97 were
  proved under none, and 3 were **unstable**: proved under some
  configurations and not others. All 3 are in `human-eval-verus`, and plain
  Verus (the unmodified session) proves all 3.
- **Unknowns with a recorded reason.** There were 776 such answers (check
  times configuration), on 97 checks.
  - Every one reports `(incomplete quantifiers)`.
  - Each used at most **0.0089%** of its limit (median 0.0048%).
  - None of these checks was proved under any configuration.
- **Every answer flip was a resource exhaustion.**
  - The 4 `unknown` answers in the 3 unstable checks each used exactly 100%
    of the limit. The other configurations proved those checks using 6% to
    99% of it:
    - limit 30M: proved at 6–53%, exhausted under seed 3;
    - limit 30M: proved at 10–99% (82% unmodified), exhausted under seed 3;
    - limit 600M: proved at 6–43%, exhausted under seeds 1 and 3.
  - **`page-table`'s `allocate` (VI-0002) behaves the same way.** It was
    measured alone (`--verify-function`). It exhausted its 3,000M limit
    under 7 of 8 configurations, and seed 4 proved it using 448M (15%).
- **Seeds change effort a lot.** For the same check, the most effort over
  the least effort across seeds is 1.0 at the median, 2.3 at p99 and
  **37×** at most (`guide/iterators.rs` #3, always proved).

**What this says about the threshold.**
1. **The reason test decided every case; the 50% threshold decided none.**
   Every answer that flipped was an exhaustion, which the reason test
   refuses. Every non-exhaustion `unknown` sat more than 5,000× below 50%.
2. **Unknowns cluster at the two extremes.** Non-exhaustion unknowns sit near
   0% of the limit and never flip. Exhaustions sit at 100%. Nothing in the
   corpus sits in between.
3. **The threshold is a backstop** for a non-resource `unknown` whose effort
   is high enough that a different search could have reached a proof. Seeds
   move a check's effort by up to 37×, so an `unknown` that used a fraction
   `f` of its limit could plausibly use up to `37f` under another search.
   Keeping that below the limit needs `f < 1/37 ≈ 2.7%`.

**Recommendation.** Serve a cached `unknown` only if both of these hold:
- its reason is not resource exhaustion (unchanged);
- it used **at most 1% of a non-zero limit**, down from 50%.

The effects:
- **Same answers served on this corpus.** The largest non-exhaustion
  `unknown` is 0.0089% of the limit, 112× below 1%.
- **Stays inside the worst observed seed spread.** A 37× swing from 1%
  still reaches only 37% of the limit.
- **The error is always on the safe side.** Refusing to serve costs only
  a re-solve, which gives the answer the pinned Z3 gives. It never costs a
  wrong answer.

The 1% figure comes from the fast tier, on one machine, with 8 seeds.
Recalibrate if either of these happens:
- the standard-tier census (anvil, ironkv, page-table in full) finds a
  non-exhaustion `unknown` above 0.1% of its limit;
- a census finds a seed spread above 100×.

**Two related observations.**
- **Plain Verus proves a check at 82% of its limit** in human-eval (the 30M
  check proved at 10–99%). One of the eight configurations exhausted it.
  Serving its cached `unsat` is safe and already allowed, and the census
  reports it as unstable.
- **The census itself knows no `reason-unknown` for 4 `unknown` answers.**
  A replay sends only the captured follow-ups, and Verus asks for a reason
  only after a non-`unsat` answer. The census therefore treats an `unknown`
  that reached the limit as an exhaustion, whatever is on record. The
  shim's policy does not need this rule: it serves only entries recorded
  with a reason.
