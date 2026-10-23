# Roadmap

Current state and next work. Evidence lives in the linked validation records
and reports; this page holds only status and gates. Status words follow
AGENTS.md: proposed, built, implemented, qualified, selected, accepted.

## Goal

The first public release must be clearly faster than plain `cargo verus
verify` (stock Verus) in normal use. Most scenarios should be faster; Sail
may add only modest overhead on runs that are already tiny and cannot be
optimized, and must never make a run genuinely slower, cold runs included
(owner direction 2026-10-20, superseding the fixed 5% line of
[question 32](open-questions.md)). Verdicts, diagnostics and exit codes stay
identical to stock. Sail's Verus fork is an accepted way to get speed (ADR
0039, ADR 0043). The release is a CLI with Linux and macOS binaries. M10
(multi-backend verification runtime, Oyster) is outside this release.

Milestones: the GitHub milestone **0.1.0** holds the release blockers. M11
is *Verus core and performance*, with its ordered backlog
[#168](https://github.com/Chelis-Lang/sail/issues/168); new performance
issues are filed under M11 and listed in #168. M10 is the multi-backend
verification runtime (#146).

### Current standing against stock Verus

Both exact installed consumers complete all 31 matrix rows and all four
three-round mutation cohorts: 651 matching comparisons per platform, zero
false verified or functional differences. Complete edit-loop speedups are
1.13–2.62× on Linux and 1.38–2.84× on Mac. Runtime 7d9d5410 fixes the Mac
HumanEval proof regression found by that suite (#338). Mac cold Anvil and
Vest, plus unchanged/tiny-edit overhead, keep release acceptance open.
Current numbers and limits are in [performance](performance.md).

## 0.1.0

Nothing is published and repository visibility is unchanged. Nothing goes to
crates.io until the owner's rollout; the release is open source under the
MIT License with macOS and Linux binaries on GitHub releases. The last draft
candidate, rc.5, was not accepted (#184, #187, #189); the next is rc.6.

**Scope** (owner, 2026-10-19), frozen on `release/0.1.0` from `fc29023a`:
Later Oyster integration on main is outside these rc.6 artifacts.

- Verified vstd shipped in release archives as a compiler-boundary proof
  bundle, served only after exact admission (#249,
  [ADR 0048](decisions/0048-shipped-dependency-build.md)); any mismatch
  verifies vstd normally.
- Fork runtime `7d9d5410` on upstream Verus 0.2026.10.18.7d82994 (Rust
  1.99.1), Linux and macOS (#267, #338).
- Proof coexistence, so shipped and locally verified vstd proofs share one
  cache.
- Managed builds use the engine's pinned Cargo
  ([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md)).
- Instant revert on Linux and macOS (#224, #269).

| Gate | Status | Next |
|---|---|---|
| Runtime rebased onto upstream 7d82994, Linux and macOS | Done; corrected 7d9d5410 manifests selected (#338) | — |
| Shipped default/100 proofs and signed replies | Both platform factories and public Vest proof/undo controls pass | — |
| Consumer gates and conformance | Full native gate, solver, E2E+extra, PR/full fast and standard pass | — |
| Mac host gate | Pass: 981 tests, clippy clean | — |
| Determinism | Mac: 320 stable query streams; strict output 319/320, one existing resource-count variation | Recorded in runtime qualification |
| Realistic edit-loop suite (#284) | Pass: four full three-round cohorts per platform, 651 matching comparisons each | — |
| Release matrices | Complete: 31/31 selected comparisons valid per platform, corrected Vest subsets retained separately | Review slower rows |
| Cold runs never slower than stock | Linux cold rows at parity or faster; Mac Anvil adds 2.86 s, Vest adds 0.74 s | #314, #278, #232 |
| Sub-second overhead modest | Open: unchanged calls and tiny edits | #156, #314 |
| Deep corpus and fork source suite | Four corpus arms pass; fork direct/cache-fill/cache-warm each 4,901 Same; stock-only control has four unstable debug-order cases | — |
| Actual archives, clean README and Claude Code | Both frozen archives pass first proof, edit, actual repair and real MCP with 17 tools | — |
| Performance/docs review and rc.6 draft | Qualified artifacts staged for private rc.6 review; performance acceptance remains open | Owner review of cold and unchanged costs |
| Owner review and publication | Not started | — |

**Merged on main for 0.1.0** (implemented; release qualification is the
gate table above): MCP in Claude Code, agent-sized replies, honest counts,
macOS managed reuse, install and first run, release builds with the engine
runtime, fork runtime releases, third-party notices, managed target cleanup,
`LICENSE`/`CONTRIBUTING.md`, `sail new`, output schema v2 (ADR 0044),
managed output order and script facts, and managed verification by default
([ADR 0046](decisions/0046-managed-verification-by-default.md)). Out of
0.1.0: single-file projects (#164), multi-backend verification (M10).

## Performance program

Ordered by release-goal blocking first (no scenario slower than stock),
then value × tractability. Track 1 is the release blocker; track 2 is the
largest edit-loop prize and the only one that moves all five projects.
"Fork" means the lever changes Sail's Verus. Prizes are measured unless
marked "estimate"; "unmeasured" means no evidence exists.
New performance issues are filed under M11 and ordered in #168.

### 1. Cold runs and pre-caching (release blocker)

Where cold time goes: a fresh small project compiles its proc-macro and
build-script crates (proc-macro2, quote, verus_syn, Verus macro crates),
then verifies vstd (2,061 checks, 366 solver sessions), then the user crate.
Cold was 0.97–1.01× on the 2026-10-15 corpus (removed pass-through mode)
and is 0.86–0.88× on the latest measured managed builds; the regression is
unattributed (lever 1.1). On Vest, overhead fixes (1.1, 1.3, 1.4, EXEC skip)
can reach parity at best; only vstd coverage for Vest's pin (1.8–1.9; 1.7 covers only the runtime's own vstd)
turns cold into a win, because Vest pins vstd 2026-09-06 and misses the
shipped bundle.

| # | Lever | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|---|
| 1.1 | Cold-gap attribution, re-baseline and owner | Required for the goal: about 12 s on Vest cold, 4 s on small | Attributed on the 7d82994 runtime; after the #232 fixes, small project +22.5% → +8.2%, public-profile Vest +12.6% → +4.8% (three rounds, [record](validation/cold-first-run.md)) | Owner decision on native-admission waits taken: pipelined admission accepted and selected (ADR 0061); 1.6 (daemon overlap) remains | No | #232 |
| 1.2 | Shipped verified vstd + pinned Cargo 1.98.1 | Fresh project 31.78 → 14.23 s vs stock (2.23×; pre-join candidate screen) | Merged (#249, ADR 0058) | 0.1.0 gate table | No | #161 |
| 1.2b | Pinned toolchain and rust-src fetched at install, in parallel with the archive download | How the 1.2 pin arrives; unmeasured share of 14.23 s | Proposed | rust-src split is part of 1.5's profile; then fetch during install (days) | No | #233 |
| 1.3 | Read the solver binary once per run (or a content-addressed private copy) | 2–3.3 s per fresh project, 0.5–0.9 s per Vest edit | Proposed; #219 recovered 1–3% | Per-run identity reuse (1–2 days); a private copy needs an ADR on same-path replacement | No | #156 |
| 1.4 | Take instant-revert retention off the cold path | Was +2.65 s on macOS Vest cold (`006ee5d0`, +6.6% vs stock) | Done: closure-scoped source check (#289) and pipelined native admission (ADR 0061). Vest cold: Linux 69.91 s vs stock 71.45 s, macOS +4.3%; returned A about 1.1 s ([record](validation/cold-first-run.md#pipelined-native-admission-adr-0061)). Skipping retention on a first run is not viable (#285) | None | Yes on Vest | #163, #278 |
| 1.5 | Profile the 9.3–14.2 s residual first use | Unmeasured, likely 0–3 s; fixed waits alone could be up to about 4 s per first run; decides 1.10 | Proposed | Control: stock `cargo build --timings` of only the macro closure (never timed). Then per-phase timestamps: native compile, rust-src setup, admission hashing (662 policy files, 128 source maps), fixed 2 s waits (0.5 day). Candidate cuts: per-machine content-memoized policy-archive attestation, readiness signals in place of the fixed arrival and graph-capture waits, parallel hashing (1–3 days) | No | #234 |
| 1.6 | Overlap daemon start and solver-identity hashing with Cargo's native compile | Unmeasured; pure cold cut, needs no think time | Proposed | `SAIL_JOB_EVENTS` timeline of a fresh project to show the serial section, then overlap (1–2 days) | No | #235 |
| 1.7 | Ship vstd SMT replies as signed entries in the release archive ([ADR 0062](decisions/0062-shipped-vstd-solver-replies.md), accepted) for the pinned vstd | Measured: component miss 31.76 → 29.93 s against stock's 29.82 s (+6.5% → +0.4%); lock-file and loader-variable misses 7–10% faster than stock; bundle hit unchanged; 20,197 of 20,197 vstd lookups hit ([record](validation/vstd-miss-replies.md#shipped-replies-implementation)) | Built (PR #287); Linux measured, macOS unmeasured | Measure on the macOS release stage | No | #236 |
| 1.8 | Compiler-boundary local reuse as the default fallback | 39.84 → 9.34 s per new demo project (Sail vs Sail); worktree and Vest unmeasured, and Vest's vstd pin likely misses | Built behind a research flag; #222 fix built, qualification pending | Land the #222 fix; Vest hit/miss from 1.1; then qualify as default | No | #161, #222 |
| 1.9 | More vstd versions (Vest pins 2026-09-06) | About 26 s per Vest-class worktree; corpus gain about 0 today | Proposed | Can start from 1.1's Vest hit/miss: fill-once local store keyed by `Cargo.lock` checksum (about 1 week), then published bundles (about 1 week) | No | #237 |
| 1.10 | Release-built macro crates as pinned tools | Plausibly 1.5–3 s fresh project (estimate) for projects matching the runtime's pinned vstd and macros; v1's 1.28 s is the upper bound. At opt-level 3 also 0.6–2 s per warm edit (one sample) | Proposed; blocked on an owner parity decision under ADR 0053 (#187: a broken user `cc` would no longer break Sail) | After 1.5: re-run the opt-level 3 macro A/B with ≥3 cold and ≥6 warm samples per arm on Vest and IronKV (0.5 day); then serve exact-match macro units from the archive, bound by digest (4–6 days). On Mac, requires qualifying the VI-0026 stable `@rpath` install-name fix (branch `perf/vstd-macos-repro`) | Yes | #239 |
| 1.11 | Seed cold runs from a team or CI cache | Cold minus most solver time (Anvil solver about 85.7 s of 124 s), still paying cold compile and frontend; unmeasured | Infrastructure implemented (ADR 0017, 0025); never measured on real projects | One seeded fresh-clone run on Anvil and CapybaraKV (about 1 day, no code); baseline from 1.1 | No | #240 |
| 1.12 | Background prebuild at `sail new` or first sight (the ordinary managed Cargo build, run early) | Hides up to the residual first use with think time; 0 without | Proposed; independent of 1.8 | Time `sail new` → first verify with a realistic agent think-time trace (days). Background pre-verification on a miss waits for 1.8 to be default-safe | No | #241 |
| 1.13 | User CI cache recipe | Unmeasured; plausibly the all-hits floor on unchanged code (charter success criterion) | Proposed | One measured GitHub Actions example persisting the Sail cache or reading a team store (days) | No | #242 |
| 1.14 | Bulk prefetch from the team store | Unmeasured | Proposed | Only after 1.11 shows remote latency matters | No | #243 |
| 1.15 | Refuse the shipped proof on its sysroot shape before hashing; read the runtime once per run | Measured: toolchain-component miss 36.68 → 35.64 s (+3.8% → +0.8% vs stock), hit −0.29 s, lock miss −0.36 s; `sail new` on that toolchain 1.12 → 0.08 s ([record](validation/shipped-miss-hashing.md)) | Built (this change). The drivers' per-compile launcher/compiler rehash (about 0.7 GB per miss) stays: ADR 0050 rules out timestamp evidence | Owner decision only if the driver rehash should change | No | #286 |

Also a cold contributor: EXEC-pass skip (track 2 step 2) saves about 3 s on
Vest cold as well as every edit, about a quarter of the Vest cold gap.

Not pursued: v1 unit-copy store (blocked; unsound as designed, #187, ADR
0053); proc macros at opt-level 3 built locally (cold Vest about 73 → 85 s,
one sample; resampled inside 1.10).

### 2. Frontend floor (#162)

The all-hits floor (IronKV 5.6 s to Anvil 38.2 s) caps every edit-loop
lever and is the only lever that moves all five corpus projects. Status:
the per-bucket memo key is blocked; the cheap steps are untried
([warm-frontend report](reports/2026-10-16-warm-frontend.md),
[validation](validation/warm-frontend.md)).

| Step | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| 1. Re-profile frontend stages at 31 and 8 threads | Decides step 3 | Proposed | Fork timers branch `m11-8/frontend-timers` (1 day plus one native-host day) | Yes | #162 |
| 2. Batch A/B of cheap cuts: EXEC-pass skip or metadata-only (3.04 s of a Vest cycle; also a cold cut), the unmeasured pruning branches `perf/prune-raw-lookups`, `research/prune-shared-indexes` and `perf/prune-traversal-memo`, and small lowering (SCC/AIR/autospec) branches | Estimated 10–20% off the floor | Proposed | One batched screen (2–3 days) | Yes | #162 |
| 3. Position-independent VIR key and per-bucket memo ([ADR 0052](decisions/0052-warm-frontend-reuse.md), proposed) | Estimated 25–45% off the floor; Vest all-hits 1.6× → about 2.1–2.4× | Blocked: raw key matches 0/200 Vest buckets on any edit (span offsets, unstable `external_fns` order) | Commit only if step 1 shows lowering ≥25% of the floor (3–5 weeks) | Yes | #162 |
| Cargo bypass when only target sources change | ≤1.72 s (7.1%) of a Vest cycle, upper bound (rescue stage profile¹) | Untried; constrained by ADR 0041 | After step 1 | No | #245 |

Terms: VIR is Verus's intermediate representation; a bucket is the unit
Verus sends to one solver session; spinoff runs a function in its own solver
process; rlimit is the solver's resource limit. See the warm-frontend report.
Merged: the selection-family fix (#220, [record](validation/managed-selection-family.md)).

### 3. Solver work on misses (#158 and throughput)

The large solver prizes are mostly Anvil: one `resource_match` bucket (9
checks, about 66 s) is re-solved on any edit, and solver time is 85.7 s of
Anvil's 124 s cold run. Verus dispatches buckets FIFO with spinoff buckets
last; cost ordering and heavy-bucket splitting were never tried. Estimates
below come from the [real-project report](reports/2026-10-15-real-project-performance.md)
("Where the time goes" and the miss-cost table). Kill rule for the whole
track: stop if the heavy-bucket experiment recovers under 10 s.

| Lever | Prize (estimate) | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Heavy-bucket experiment: `#[verifier::spinoff_prover]` on Anvil's 1–3 heaviest functions (measurement only) | Anvil body edits 88–104 → about 44–50 s best case; 0 if one query holds the cost | Proposed; answers #158 and throughput together | Anvil cold timeline plus per-check split, then the annotation A/B (1–2 days) | No | #158 |
| Cost-ordered (longest-first) bucket dispatch and automatic spinoff of heavy functions | Anvil cold and spec-wide 0–35%, 0–5% elsewhere (jointly) | Deferred until the experiment | Order from recorded cost hints (50–100 lines in `verifier.rs`; queries stay byte-identical). Auto-spinoff changes rlimits, so an opt-in mode, ADR and divergence entry | Yes | #244 |
| Edit-triggered live-prefix preparation | Anvil edits 88–104 → about 45–55 s with ≥54 s think time; 0 without | Blocked on [question 31](open-questions.md) and scheduling ([ADR 0051](decisions/0051-exact-solver-snapshots.md), proposed) | Fallback if the experiment fails: decide question 31 (about 1 day), then 1–2 weeks | No | #158 |

[Solver-snapshot report](reports/2026-10-16-solver-snapshots.md),
[validation](validation/solver-snapshots.md).

### 4. Incidental-edit robustness (#118)

Keeps edits that change nothing logically at the floor. Merged (qualified
per record): closure identity (#221, [record](validation/closure-full-identity.md)),
spec-parameter names (#223, [record](validation/spec-parameter-names.md)).

| Item | Prize | Status | Next step (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Trait function spellings | Vest empty-impl edit 45.09 → 33.96 s | Merged in fork runtime `f8589ed4` | — | Yes | #118 |
| 15-edit catalogue | Finds remaining leaks; mostly insurance | Prepared, never timed ([catalogue](validation/incidental-edits.md)) | One machine, 5–7 h, no new code, plus one Anvil row | No | #118 |

### 5. Managed overhead (#156)

Small-project edits and unchanged runs are sub-second but slower than stock
(standing table). Merged (qualified per record): #186, #194, #219, #225,
#226 ([managed overhead](validation/managed-overhead.md)); solver identity
(#217) added cost back ([record](validation/solver-identity.md)). Rule: skip
Sail-side items under 100 ms unless the sub-second threshold decision makes
them gating.

| Item | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Retention cost gate, concurrent revision snapshots, byte-compared driver aliases | Small edit 2.55 → 1.04 s, revert 1.72 → 1.01 s, unchanged 0.223 → 0.215 s (0.474 → 0.240 s with sail on another filesystem) | Built (branch `perf/small-overhead`) | Review and merge; remaining ≈0.25 s per edit is itemized in the [record](validation/managed-overhead.md#small-project-overhead-156) | No | #156 |
| Per-run solver identity | See 1.3 | Proposed | Lever 1.3 | No | #156 |
| Attribute the Vest unchanged gap (0.54 vs 0.23 s) | Up to 0.31 s per unchanged Vest run | Proposed | `SAIL_JOB_EVENTS` breakdown, tied to 1.3 and 1.4 (0.5 day) | No | #156 |
| Fork attribution sidecar | Up to 0.8 s on JSON/MCP Vest runs only | Unfunded unless MCP latency becomes a priority | — | Yes | #156 |

### 6. Instant revert breadth (#163)

Merged for the Linux public alloc/std profile (#224,
[record](validation/instant-revert.md), [guide](instant-revert.md));
qualified on Linux and macOS on `f8589ed4`. Remaining: macOS and other
dependency profiles, and lever 1.4 (its cold and cache cost, about 519 MB).

## Owner and lead decisions pending

| Decision | Who | Blocks |
|---|---|---|
| Closure-name determinism amendments to ADRs 0049 and 0056 (#247), accepted by the lead | Owner review | — |
| Prebuilt macro crates under ADR 0053 parity ([question 35](open-questions.md)) | Owner | 1.10 |
| Private solver copy on same-path replacement (ADR) | Lead | 1.3 variant |
| Question 31, continuation timing oracle | Owner | Live-prefix preparation |
| ADR 0048 status: research path → release default | Lead | 0.1.0 |
| ADRs 0051, 0052 still proposed | Lead | Track 2 step 3, track 3 |
| ADR 0046 managed by default: implemented, awaiting review | Lead | 0.1.0 |
| M7 milestone acceptance | Lead review | History only |

## Measured and closed

| Idea | Evidence |
|---|---|
| Rust pre-solver (M9, a planned Rust-side pre-solving milestone) | Covers 0.1% of solve time (ADR 0038; rescue lane¹) |
| Warm solver pools | A warm fast-tier run starts a solver for 26 of 1,526 sessions |
| Warm exact-prefix sessions | Save 9–19 ms per HumanEval cycle, 107 ms per Vest cycle ([ADR 0040](decisions/0040-warm-solver-sessions.md), do not build) |
| vstd-preloaded spare process | 0.19 s on Vest (0.8%; rescue stage profile¹) |
| Separate verify-pass cache | No gain; rustc incremental already on (rescue lane¹) |
| Stack-only keys as default | Identical answers but rlimit deltas up to 17× (ADR 0008); default stays full-prefix ([question 8](open-questions.md)) |
| Stock `-V spinoff-all` | Floor 1.7–1.8× worse on CapybaraKV and Vest (a little on IronKV), cold 1.4–1.8× slower, one IronKV verdict change ([report](reports/2026-10-15-real-project-performance.md)) |
| Parked Z3 fork snapshots | Not state-identical: Z3 atfork timer state, `num-allocs` differ ([report](reports/2026-10-16-solver-snapshots.md)) |
| Fork `--time` per-function report | Within noise, 25.77 vs 25.89 s (ADR 0047) |
| Read-only pruning traversal | −0.7% Vest, +0.3% IronKV, inconclusive ([validation](validation/warm-frontend.md)) |
| CI image slimming (#198) | 9% smaller, no latency gain ([record](validation/ci-image-startup.md)) |
| `sail verify --incremental` | Recompiled about 18 crates every run (#157, fixed by #170) |

Never run and not funded: Z3 version or option sweeps (stock refuses other
Z3 versions), unsat-core pruning. Reopened: "a warm team cache does not help
cold runs" was never measured on real projects (lever 1.11).

¹ Rescue-lane reports live in tag `archive/rescue-m7-98f6292`
(`docs/reports/rescue/`), cited by ADR 0038.

## Delivered history

| Milestone | State | Evidence |
|---|---|---|
| M0–M4 foundation: workspace, survey, pass-through shim, corpus, conformance, baseline | Done 2026-10-08 | [bootstrap report](reports/2026-10-08-bootstrap.md) |
| M5 initial product | Done 2026-10-09 | [report](reports/2026-10-09-initial-product.md) |
| M6 live workspace verification, jobs/watch/MCP, HTTP team cache | Implemented and accepted locally | [report](reports/2026-10-10-live-workspace.md) |
| M7 proof engine and repair workflow | Every gate has native evidence; speed accepted at 2.14× HumanEval, 1.82× Vest vs M6 (ADR 0038 amendment); milestone acceptance awaits lead review | [M7 validation](validation/m7.md) |
| Fork as default runtime (ADR 0043) | Selected | [record](validation/fork-default.md) |
| M11 Verus core and performance | In progress; ordered backlog #168 | [milestone](https://github.com/Chelis-Lang/sail/issues/168), [managed cold start](managed-cold-start.md) |

Caching, CLI, agent ergonomics, benchmarks, corpus tiers and CI from the
charter's initial scope are done (ADRs 0009, 0011, 0013–0021, 0025).
The M11 reliability items in #168 are closed.

## Later

- **M10 verification runtime (Oyster integration authorized alongside M11).** One runtime and ledger for
  Rust, SMT-LIB and TLA+ jobs ([scope](scope/verification-runtime.md), #146).
  Interfaces (#127–#129), then the SMT-LIB service (#137), then backends
  (#130–#136), certificates (#138), D.I.V.E. TLA+ jobs (#139), mutation
  (#140, #141), selection (#142), assurance view (#143), library mode (#144),
  Dafny evaluation (#145). Every backend passes conformance before any
  acceleration.
- **Single-file projects** (#164).
- **CI image pull time** (#195): developer CI only, not a user claim.
- **History-tolerant serving and a named stack-only mode** (ADR 0040 point
  3, ADR 0008): opt-in divergence only; authorized in principle and
  deferred ([question 8](open-questions.md)).
- **Per-module `rust_to_vir` / GHOST typeck reuse:** the only route to
  halving the floor; uncharted compiler work, not counted on.
- **GHOST incremental build / render-once variants:** source possibly lost;
  check `archive/retired-ghost-incremental-build-source-829c328e` before
  treating them as recoverable; unmeasured.
- **Function-level speculation in `sail watch`:** provisional tier defined
  (ADR 0016) but never produced; shares the scheduling hook with live-prefix
  preparation.

| Charter roadmap item | Where it is now |
|---|---|
| Multiple SMT backends, portable queries | M10: #137, #142 |
| Alethe and Carcara certificates | M10: #138 |
| Other verifiers | M10: #130–#136; Dafny evaluation #145 |
| Warm Verus frontend | Performance program track 2, #162 |
| Rust pre-solver (M9) | Closed (ADR 0038) |
| Unsat-core pruning, instantiation replay | Not funded |
| Warm solver sessions, stack-key mode | Closed for default (ADR 0040, ADR 0008) |
