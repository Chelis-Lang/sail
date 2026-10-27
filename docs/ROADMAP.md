# Roadmap

Current state and next work. Evidence lives in the linked validation records
and reports; this page holds only status and gates. Status words follow
AGENTS.md: proposed, built, implemented, qualified, selected, accepted.

## Neutral contract service

| Gate | State | Evidence |
|---|---|---|
| Shared neutral source and actual backend helper | Qualified separately | [Lowering](spec/neutral-verus-lowering.md) |
| Negotiated source-only job route and immutable baselines | Cumulative source qualified on native Linux and macOS | [Combined qualification](validation/integration-production.md), [ADR 0070](decisions/0070-neutral-contract-jobs.md) |
| Source-semantic requirement entailment and full requirement tiers | Producer source-bound proof views implemented; independent links belong to Oyster | [Service boundary](spec/neutral-contract-jobs.md) |
| Bounded neutral-source body mutation | Lifecycle qualified; native strong mutants remain Unknown and positive kills unestablished | [Policy and evidence](neutral-mutation.md), [ADR 0074](decisions/0074-bounded-neutral-mutation.md) |
| Separate scalar literal-contract witnesses | Native Linux gates and installed source-owner witness acceptance pass; protected delivery remains separate | [Qualification](validation/scalar-literal-witnesses.md), [ADR 0077](decisions/0077-scalar-literal-contract-witnesses.md) |

## Evidence-acceptance proofs

| Gate | Current state | Remaining work |
|---|---|---|
| Query association, input identity, scope and result authority models | Expanded lifecycle models, composed admission and captured-source direct stock checking pass native and hosted checks; scope is recorded in the [model guide](design/acceptance-proof-checker.md) | Observation adapters, parser/cache-origin refinement and service-wide theorem |
| Production acceptance core | Checkpoint `fe8be9ee` passes native/hosted gates and full fast/standard conformance; bounded timing accepted by owner | remaining stages C–F in the [refinement guide](design/acceptance-production-refinements.md); broader release performance remains separate |
| Acceptance boundary follow-up | Combined whole Stock roots pass 646/1,114, 308/538 and 608/1,030 at `f8b26511`; the full native gate passes at `16938f40`, and all 31 models pass at `ee4e684f`. The draft now also integrates actual output item ordering, original package-selector scanning and the renderer correction, each with separate whole-root/native receipts; [qualification](validation/acceptance-boundary-refinements.md) preserves exact source and limits | All six [deliverables](design/acceptance-boundary-refinements.md) remain open. All 314 query/115 input/247 authority controls and fresh combined native gates, #506/#507 e2e corrections, compiler/process/cache-origin correspondence, conformance, native cost and the service theorem remain required |
| Current-main source reconciliation | The draft retains main through `dca5cd43`, original request/origin guards and build-script copies/directories, and combines parser/cache/service source. Canonical dependency order and parser import guards are restored after native source refusals (#495/#496) | The parser/cache/service predecessor passes its three whole roots and source suites; the new request integration requires fresh qualification. Complete native/macOS, conformance and owner CI policy decision remain. Earlier frozen receipts do not establish current combined acceptance |
| Streaming parser refinement (#371) | Actual outer/raw-owner root passes 524/918; lexical state/head passes 553/951; paired stdout append/controller passes 568/971 under proposed ADR 0086. Native SMT package 25 tests and strict Clippy pass | Full 225-control parser milestone and 278-control combined query union, actual per-item/frame/replay event order, native integration/cost and Session/solver/physical/origin obligations remain |

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
| Default-mode verdict conformance | Main fixes #385, #386 and #393 through isolated settings/options and observed ghost environment; residual #435 stays open | Combined draft conformance and remaining transport transparency |
| Unchanged calls | Done when replay applies: 26 ms against stock's 0.26 s on Anvil on Linux, 60 ms against 0.25 s on macOS (ADR 0082, #402, #437). Otherwise unchanged Anvil about 1.7 s | #314 |
| Tiny edits | Sail still slower than stock by a fraction of a second | #156 |
| Cold runs | Linux at parity or faster; macOS Anvil about 0.4 s slower on 100 s | #278, #232 |
| CI cache reuse | Done: tools identified by content, so restored caches are reused on a new runner (#428); [guide](ci-cache.md) | — |
| Feature set | Done (#423): repair, `explain`, `baseline`, `compare` and MCP supported; fragility, portfolio, team cache and integration APIs experimental | — |
| Release attestations | Workflow and installer check built (#400); first exercised on the first real release | Release |
| Release matrix, deep conformance, walkthroughs | Last complete run on rc.6 / runtime `36d65c3f` archives | Rerun on the release candidate (#282) |
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
| 1. Cold runs and first use (#232) | Shipped vstd proof and replies; pinned Cargo; pipelined native admission (ADR 0061); first-use hashing and daemon start cut (#424, levers 1.5/1.6); solver identity reused under a settled stamp (#417) | macOS Anvil cold (#278); macro-closure compile is most of the remaining first use (#234) |
| 2. Frontend floor (#162) | Profiled; persistent frontend reuse unqualified | Parked after 0.1.0 |
| 3. Solver work on misses (#158, #244) | Longest-first bucket dispatch by a static size estimate shipped in fork commit `ee8dd97d` (runtime `f8589ed4` onward): Vest 10–15% faster at 7–15 threads, no gain on Anvil | Parked after 0.1.0 |
| 4. Incidental edits (#118) | Closure identity, spec-parameter names and trait spellings merged | 15-edit catalogue untimed |
| 5. Managed overhead (#156, #314) | Unchanged-call replay; unit-graph and script-snapshot reuse (#407); build-script receipts for Cargo 1.93–1.99 (ADR 0084) | Setup cost when replay misses; tiny-edit cost |
| 6. Instant revert (#163) | Linux and macOS for qualified profiles; 16 revisions with pinned baseline; `SAIL3146` explains skips | More dependency profiles |
| CI and team caches (#240, #242) | Measured recipe in [CI caching](ci-cache.md); restored caches reused across runners (#428) | GitHub Actions example not yet run on GitHub |

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
