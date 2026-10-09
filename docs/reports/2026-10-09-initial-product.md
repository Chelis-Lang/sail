# Initial-product completion report (2026-10-09)

This report records the handoff audit and the remaining initial-product work.
The initial-product acceptance runs below are complete; fast CI is green.
The foundation and previous caching, watch, MCP and benchmark evidence remain
in `docs/reports/2026-10-08-bootstrap.md` and `docs/benchmarks.md`.

## Owner decisions

- Keep the unknown serving cap at 50%. Canceled, near-limit and effort-less
  unknowns remain unservable; the reason and effort checks still apply.
- Build explicitly selected `accept-first-proof` portfolios. Try unchanged
  baseline first and seeds 1–7 in order, stopping at one complete independent
  proof of the requested scope. Never combine partial successes across seeds.
- Finish the initial product. HTTP shared stores, solver pools, stack-only
  keys and the charter's later research items remain deferred.
- Singular validation is optional and local-only. Long corpus suites, source
  builds and benchmarks stay outside CI.

## Changes

| Area | Change | Decision or evidence |
|---|---|---|
| CI | Always-run completion check rejects skipped, failed or canceled dependencies; require updated branches; all jobs capped at ten minutes | ADR 0013; PR #49 |
| Portfolio | Independent full Verus attempts, reports required for every requested Cargo target, owned targets, full worker-tree cancellation and baseline failure retained when no seed proves | ADR 0020; `docs/divergences.md` |
| Isolation | Explicit mode negotiation in protocol v4; isolated cache keys and disk/memory metadata; no shared publication of portfolio entries | ADR 0020; daemon mode-isolation regression |
| Function output | Exclude unselected timing entries falsely marked successful by pinned Verus; report uncertainty in failing crates with incomplete attribution | VI-0009 reproduction; VI-0003 fallback and persisted-record regressions |
| Fragility | Exact input/transcript/operation join; complete per-configuration proofs required for a function score; missing attribution yields no score | ADR 0018 |
| Measurements | Force fresh verification in exclusively locked, owned Cargo targets; reuse dependencies and preserve caller artifacts | ADR 0018; repeated real MCP requests |
| CLI | `sail bench` delegates to the installed benchmark binary | CLI integration test |
| Diagnostics | Preserve solver stderr; publish only complete silent sessions, with exact-input silence records for solver-free completion | ADR 0022; issue #61 |
| Deep suite | Strict whole-result comparison, explicit instability, narrowly documented normalization, exclusive build ownership | ADR 0019 |
| Singular | Separate source-build identity and local image; runtime hashes validated before execution; tagged deep corpus | ADR 0021 |

## Local acceptance

The combined feature checkout passes all 268 host gate tests. Required GitHub
checks for the core diagnostic fix and complete-run portfolio are green; every
job takes less than four minutes. The Linux CI-image core gate, all 30 core solver tests,
end-to-end comparison, and all three 33-unit guide rounds also pass locally.
The combined Linux-container gate passes all 268 tests and all 35 solver,
fault and portfolio tests. Its unchanged end-to-end retry passes, and all three
33-unit guide rounds match across 311 functions with 3,603 cache entries. One
native debug portfolio test exceeded the unchanged three-minute limit during concurrent source validation; sampling
showed active solver-binary hashing. The same test passed in the Linux debug
build, and in 9.5 s in the native release build with the same assertions and
three-minute limit; no check was changed.

Post-fix standard pass-through, fill and warm each match 158 native macOS
units across 12,583 functions, with zero differences and infrastructure
errors. The warm cache holds 148,427 entries. Only Anvil is skipped for missing
native OpenSSL; its separate Linux-container runs match all three modes across
143 functions, with zero differences or infrastructure errors and 1,478 cache
entries.

The strict, exclusively owned source suite completed all three modes.
Pass-through and fill each report 4,691 same and four confirmed unstable; warm
reports 4,692 same and three unstable. Every mode has zero differences and
zero infrastructure errors across 4,695 cases. Every unstable output
matches one complete observed plain result. There are 58 ignored cases and
five retained plain-test failures; these are not presented as passing proofs.
Normal-thread coverage separately matched 13
selected cases in each mode, including the module-error tests that cannot pass
under the full suite's one-thread setting.

A whole page-table request accepted no proof: baseline and seed 4 each reached
the 3,600-second wall timeout. Seed 4 also emitted errors in other functions;
its allocation success cannot justify a whole-program proof. An independent
`allocate`-only request proved exactly one function with seed 4 and zero errors,
using 448,058,976 of its effective 3,000,000,000 resource units (14.94%). The
function attribute overrides global `--rlimit 20`. The baseline reached its
wall timeout. Complete attempt and solver provenance are retained under
`bench/portfolio/`.

A small complete nonlinear crate proves with seed 4 after baseline exhaustion,
on fresh and warm macOS runs and cold/warm Linux CI-image runs. Its warm proof
hits the cache while the exhausted baseline is re-solved. The fast end-to-end
step checks the reproduction under `corpus/repros/z3-nla-seed-portfolio/`.

Repeated unchanged-source MCP measurements each returned only the requested
function, two measured proofs out of two and score 0. The caller's Cargo target
marker stayed unchanged. A deliberately failing function scored 1 while the
successful function scored 0. MCP store format 2 retires older persisted reports
that could contain uncertain verified claims from incomplete attribution;
current-format results still survive a restart.

The optional Singular source build is complete and runtime identities are
validated. Post-fix pass-through, fill and warm each match all three units
across 606 functions. Verus reports 9 and 27 verified for the integer-ring
examples and 736 for the allocator, all with zero errors. The warm cache holds
8,080 entries. Verus runs natively on macOS and Singular in the pinned Linux
image under emulation; these are correctness results, not Linux speedups.

A combined Linux-container end-to-end attempt stalled during plain Verus's
vstd warm-up before Sail ran. Its raw process snapshots and streams were
retained, and only that owned container was stopped. The unchanged retry
passed default cold/warm verification, JSON and MCP comparisons, and cold/warm seed-4 complete-run portfolio proofs. The
interrupted attempt is not acceptance evidence.
See `corpus/repros/linux-emulation-verus-stall/` for the local observation.

### Superseded runs and defects

Older cache measurements do not establish acceptance of the corrected path.
An earlier strict warm suite found two genuine disagreements from omitted Z3
warnings. PR #62 fixes them; the two affected cases match in all three modes.
The historical broad varying-line classifier and overlapping source-suite run
are superseded. The strict comparator and exclusive OS lock replace them.
An extra failed invocation from a duplicate progress flag is corrected; the
affected example passed all three modes separately, and the new complete
pass-through and fill runs include its corrected invocation. Original reports
are retained rather than rewritten.

Portable summaries, explicit solver hashes, report hashes, complete unstable
case identities and retained plain failures are in
`bench/validation/initial-product-acceptance-2026-10-09.json`. The companion
manifest and generator reproduce the artifact from the raw reports.

## Cost and limits

Portfolio cost data and its generated table are under `bench/portfolio/`.
Three cold/warm pairs per scope include verdict comparisons and cache counters.
These are small selection/protocol probes on the loaded secondary macOS
platform, not Linux reference speedups. Independent failed attempts add cost.
Near-limit unknowns are re-solved even when the portfolio cache is warm.

The cache diagnostic correction was measured on three paired guide fill/warm
rounds. All 396 unit comparisons matched. Median fill changed from 39.6 s to
39.1 s (-1.1%); median warm changed from 29.9 s to 34.9 s (+16.7%). Ranges
overlap under development load, so neither is a stable performance claim.
Data, report hashes and the generator are in `bench/validation/`.

Fragility values measure captured-query replay. They do not establish a
complete independent program proof; only the complete-run portfolio does that.
Ambiguous or missing attribution produces a null score with notes. Multi-crate
Verus log limitations remain documented in VI-0004.

## Deferred research

Read-only HTTP stores, warm solver pools, stack-only keys, warm Verus frontends,
multiple SMT backends, certificates, pre-solving, unsat-core pruning and
instantiation replay are future work. Verus issues are tracked locally; no
upstream reports were filed.
