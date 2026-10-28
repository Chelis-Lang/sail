# Roadmap

Current state of main and the next work. This page holds status and gates
only; [performance](performance.md) has the numbers,
[known issues](known-issues.md) the open defects and limits.

## Goal

The first public release must be clearly faster than plain `cargo verus
verify` (stock Verus) in normal use. Most scenarios should be faster; Sail
may add only modest overhead on runs that are already tiny, and must never
make a run genuinely slower, cold runs included (owner direction
2026-10-20). Verdicts, diagnostics and exit codes stay identical to stock.
Sail's Verus fork is an accepted way to get speed (ADR 0039, ADR 0043). The
product is a CLI with Linux and macOS binaries. Final release composition is
the owner's decision.

Milestones: **0.1.0** holds the release blockers. M11 (*Verus core and
performance*) holds performance work, ordered in
[#168](https://github.com/Chelis-Lang/sail/issues/168). M10 is the
multi-backend verification runtime (#146).

## 0.1.0

Nothing is published and nothing goes to crates.io until the owner's
rollout. The release is MIT-licensed, with macOS and Linux archives on GitHub
releases. It follows main; the earlier private rc.6 is only a measurement
baseline.

| Gate | State | Next |
|---|---|---|
| Fork runtime | `36d65c3f` on upstream Verus 0.2026.10.18.7d82994 (Rust 1.99.1), Linux and macOS; selected | — |
| Shipped vstd proof and solver replies | Done (#249, #287; ADR 0048, ADR 0062) | — |
| Default output identical to stock | Done: human stderr is byte-identical by default; Sail notes only with `SAIL_EXPLAIN=1` or in JSON (#391) | — |
| Default-mode verdict conformance | Done: #385, #386, #393 fixed. Sail's named settings stay compiler-visible as an accepted exception (ADR 0088, #435) | — |
| Proof-reuse input coverage | Open: replay can return an old report after a ghost compiler input changes (#552, fix in PR #554); reuse loses effective compiler-environment bindings (#551); a forced `--release` run fails with `SAIL3107` (#553) | Fix, with regressions |
| Unchanged calls | Later unchanged calls replay in 0.03–0.09 s against stock's 0.20–0.26 s (Linux Anvil 26 ms; macOS `dca5cd43` 0.04–0.09 s; ADR 0082, #402, #437). The first unchanged call and calls that miss replay are slower | [Performance parity](#performance-parity) |
| Tiny edits | One-line edits faster than stock on all four real projects (macOS `dca5cd43`); tiny-project edits still slower | [Performance parity](#performance-parity) |
| Cold runs | Linux at parity or faster; macOS Anvil 0.7 s slower on 100 s, ranges not overlapping (#466) | #232 |
| CI cache reuse | Done: tools identified by content, so restored caches are reused on a new runner (#428); [guide](ci-cache.md) | — |
| Feature set | Done (#423): repair, `explain`, `baseline`, `compare` and MCP supported; fragility, portfolio, team cache and integration APIs experimental | — |
| Release attestations | Workflow and installer check built (#400); first exercised on the first real release | Release |
| Release matrix, deep conformance, walkthroughs | Last complete run on rc.6 / runtime `36d65c3f` archives; release-candidate qualification not started | Rerun on the release candidate (#282) |
| Owner review and publication | Not started | Owner |

**On main for 0.1.0:** managed verification by default (ADR 0046), the
pinned Cargo (ADR 0058), shipped verified vstd and signed vstd replies,
instant revert with 16 retained revisions and a pinned baseline (#405),
unchanged-call replay (ADR 0082) with solver-identity reuse (#417),
per-target revision scoping (#395, #414), single-frame cached replies with
replay checked against served bytes and fail-closed journals (#392),
read-back currentness that includes the runtime (#389), setup cuts (#407,
#424), watch fixes (#420), the guided installer, output schema v2,
`sail new`, MCP, proof repair and the evidence commands. D.I.V.E. temporal
jobs are not in 0.1.0; they build only in the extension workspace (ADR 0083).

## Performance tracks

| Track | State | Open work |
|---|---|---|
| 1. Cold runs and first use (#232) | Shipped vstd proof and replies; pinned Cargo; pipelined native admission (ADR 0061); first-use hashing and daemon start cut (#424, levers 1.5/1.6); solver identity reused under a settled stamp (#417) | macOS Anvil cold, 0.7 s (#232); macro-closure compile is most of the remaining first use (#234) |
| 2. Frontend floor (#162) | Profiled; persistent frontend reuse unqualified | Parked after 0.1.0 |
| 3. Solver work on misses (#158, #244) | Longest-first bucket dispatch by a static size estimate shipped in fork commit `ee8dd97d` (runtime `f8589ed4` onward): Vest 10–15% faster at 7–15 threads, no gain on Anvil | Parked after 0.1.0 |
| 4. Incidental edits (#118) | Closure identity, spec-parameter names and trait spellings merged | 15-edit catalogue untimed |
| 5. Managed overhead | Unchanged-call replay; unit-graph and script-snapshot reuse (#407); build-script receipts for Cargo 1.93–1.99 (ADR 0084) | [Performance parity](#performance-parity) |
| 6. Instant revert (#163) | Linux and macOS for qualified profiles; 16 revisions with pinned baseline; `SAIL3146` explains skips | More dependency profiles |
| CI and team caches (#240, #242) | Measured recipe in [CI caching](ci-cache.md); restored caches reused across runners (#428) | GitHub Actions example not yet run on GitHub |

## Performance parity

Rows still slower than stock. Direction: spike each row's tractability first
(what it costs and what would remove it), then fix in the order the spikes
justify. Numbers are in [performance](performance.md). Benchmarks do not yet
record peak memory (#562).

| Row | Measured, Sail against stock | Issue |
|---|---|---|
| First unchanged call after a verification (full path, records replay) | macOS `dca5cd43`: Anvil 2.07 s against 0.30 s; Vest 0.79 s against 0.26 s | #558 |
| MCP and job calls never replay (ADR 0082) | Every unchanged call pays the full path | #559 |
| Unchanged call that misses replay (`touch`, recent ctime, other arguments) | macOS `dca5cd43`: Anvil 1.59 s against 0.24 s | #560 |
| Unchanged `SAIL3112` setups | Anvil 0.90 s with no proof reuse ([record](validation/unsupported-cargo-fallback.md)), stock about 0.3 s | #561 |
| Tiny-project edits | About 0.4 s (Linux) and 0.7 s (macOS) more, `e51e4660` matrix | #560 |
| macOS Anvil cold | 100.2 s against 99.5 s, ranges not overlapping (#466) | #232 |

## After 0.1.0 (parked)

| Item | Issue |
|---|---|
| Acceptance integration draft and its staged input-coverage fixes | #378 |
| Service-wide acceptance theorem and refinements | #368, #371–#376 |
| Recorded-cost dispatch hints in the fork | #244 |
| Heavy-bucket split and opt-in automatic spinoff | #244, #158 |
| Edit-triggered live-prefix preparation (question 31, ADR 0051) | #158 |
| EXEC-pass skip | #162 |
| Warm frontend: position-independent VIR key (ADR 0052) | #162 |
| Prebuilt macro crates as pinned tools (question 35) | #239 |
| More vstd versions | #237 |
| Fragility hardening | — |
| Portfolio tidy | — |
| Compiler-boundary local reuse as default fallback | #222 |
| Background prebuild; bulk prefetch from a team store | #241, #243 |
| Cargo bypass when only target sources change | #245 |
| Single-file projects | #164 |
| CI image pull time | #195 |
| M10 verification runtime | #146 |

## Integration (Oyster and neutral contracts)

These are experimental integration APIs, not part of the supported 0.1.0
command set.

| Slice | State | Remaining |
|---|---|---|
| Raw SMT shared runtime | Implemented | Consumer qualification at its selected SDK |
| Neutral source jobs and baselines | Implemented ([ADR 0070](decisions/0070-neutral-contract-jobs.md)) | Broader source profiles |
| Requirement assurance | Source-bound proof views implemented | Independent links belong to Oyster |
| Bounded neutral-source mutation | Implemented ([ADR 0074](decisions/0074-bounded-neutral-mutation.md)) | Positive native kills unestablished |
| Scalar literal-contract witnesses | Implemented ([ADR 0077](decisions/0077-scalar-literal-contract-witnesses.md)) | — |
| Native D.I.V.E. temporal worker | Built only by the extension workspace (ADR 0083) | Not in 0.1.0 |
| Neutral lowering version two (#323) | Implemented | Qualification before lifting the source-job hold |

## Decisions pending

| Decision | Who | Blocks |
|---|---|---|
| Release composition and publication | Owner | 0.1.0 |
| Prebuilt macro crates under ADR 0053 parity ([question 35](open-questions.md)) | Owner | #239 |
| Question 31, continuation timing oracle | Owner | Live-prefix preparation |
| ADR 0048 status: research path to release default | Lead | — |
| ADRs 0051, 0052 still proposed | Lead | Tracks 2 and 3 |

## Measured and closed

| Idea | Result |
|---|---|
| Rust pre-solver (M9) | Covers 0.1% of solve time (ADR 0038) |
| Warm solver pools | A warm fast-tier run starts a solver for 26 of 1,526 sessions |
| Warm exact-prefix sessions | 9–19 ms per HumanEval cycle, 107 ms per Vest cycle ([ADR 0040](decisions/0040-warm-solver-sessions.md), not built) |
| Stack-only keys as default | Identical answers, rlimit deltas up to 17× (ADR 0008); default stays full-prefix |
| Stock `-V spinoff-all` | Floor 1.7–1.8× worse, cold 1.4–1.8× slower, one verdict change ([report](reports/2026-10-15-real-project-performance.md)) |
| Parked Z3 fork snapshots | Not state-identical ([report](reports/2026-10-16-solver-snapshots.md)) |
| Read-only pruning traversal | Inconclusive ([validation](validation/warm-frontend.md)) |
| v1 native-artifact store | Disabled: unsound as designed (#187, ADR 0053) |

Not funded: Z3 version or option sweeps, unsat-core pruning, instantiation
replay.

## Delivered milestones

| Milestone | State |
|---|---|
| M0–M4 foundation: workspace, survey, shim, corpus, conformance | Done 2026-10-08 |
| M5 initial product | Done 2026-10-09 |
| M6 live workspace verification, jobs, watch, MCP, HTTP team cache | Done |
| M7 proof engine and repair workflow | Accepted 2026-10-26 ([validation](validation/m7.md)) |
| Fork as default runtime (ADR 0043) | Selected |
| M11 Verus core and performance | In progress (#168) |
