# Roadmap

Current ordering follows the owner's 2026-11-19 decision, recorded in
[ADR 0174](decisions/0174-v030-native-temporal-scope.md), which amends
[ADR 0109](decisions/0109-oyster-retired-v030-scope.md) and
[ADR 0100](decisions/0100-mutation-first-roadmap.md). The latest stable release
is **0.3.0**, native temporal checking on D.I.V.E. 0.1.0. The next release is
**v0.3.1**, the same native temporal checking on D.I.V.E. 0.2.0, then
**v0.4.0**. Symbolic rechecks (`sail runtime symbolic-check`) are not in
v0.3.1; they are planned with D.I.V.E. 0.3
([#805](https://github.com/Chelis-Lang/sail/issues/805)).

The active execution tracker is
[#712](https://github.com/Chelis-Lang/sail/issues/712), superseding the
historical 0.2.0 handover #609. The owner's 2026-11-11 confirmation that the
whole earlier v0.3.0 program was to be completed is superseded: unfinished
pillars move to [v0.4.0](#after-the-next-release-v040). The continuous #368
proof lane keeps its existing boundary: affected proofs gate changes, but the
complete service-wide theorem is not a release gate.

## Goal

Tell Verus developers and agents what their proofs are worth, and never cost
them time for it: whether their specifications catch plausible implementation
bugs, which proofs are fragile or slow and why, and verification that is never
slower than stock Verus. Preserve default-mode correctness, explicit scope,
provenance and conservative handling of uncertainty. Existing losses to stock
remain engineering work, including cold runs. No baseline is relaxed.

## Next release: v0.3.1

v0.3.1 moves native temporal checking to D.I.V.E. 0.2.0, keeps large
simulations within the response bound, and adds the temporal edit-session,
recheck and replay work on `main`. It changes no Verus verification behavior.

| Item | Gate | Tracker | State |
|---|---|---|---|
| Native temporal checking on D.I.V.E. 0.2.0 | `dive-api`, `dive-ir`, `dive-value` and `dive-trace` resolve from crates.io at exactly 0.2.0; no Git source or `[patch]`; native conformance at the published version | [#807](https://github.com/Chelis-Lang/sail/pull/807) | Pinned to the D.I.V.E. 0.2.0 release candidate `5bb88063` by Git revision; S7 against standard TLC is running. Waits for D.I.V.E. 0.2.0 on crates.io |
| Large simulations within the response bound | A job whose evidence exceeds 16 MiB keeps and replays the traces that fit; `SAIL4216` only when none fits | [#767](https://github.com/Chelis-Lang/sail/issues/767) | Implemented on the release branch |
| `witness-recheck` against an installed Dive | Refuses any Dive release other than the linked one with `SAIL6103` | [Errors](errors.md) | Implemented on the release branch |
| Symbolic rechecks | Not in v0.3.1; need D.I.V.E. 0.3's symbolic worker | [#805](https://github.com/Chelis-Lang/sail/issues/805) | Unlisted in `sail help runtime`; planned with D.I.V.E. 0.3 |
| Publication | Owner approves the exact final candidate | [Releasing](releasing.md), [v0.3.1 milestone](https://github.com/Chelis-Lang/sail/milestone/16) | No candidate |

Unfinished groundwork on `main` (the mutation campaign model, profiling
library modules, the Kani decoder and the cvc5 development backend) ships
unadvertised. No command, MCP tool or JSON field exposes it; the cvc5 backend
needs an undocumented development variable.

Linux aarch64 is outside this program. The owner clarified on 2026-11-11
that it was not requested; no support work is scheduled, and the source-built
oracle proposal in [question 50](open-questions.md) is inactive. The
supported platforms remain Linux x86-64 and macOS on Apple silicon.

## After the next release: v0.4.0

These pillars moved from v0.3.0 (ADR 0174). Each keeps its gate, drop rule and
tracker; their order is not decided. See the
[v0.4.0 milestone](https://github.com/Chelis-Lang/sail/milestone/15).

| Pillar | Gate | Tracker | State |
|---|---|---|---|
| Verus mutation testing | Bounded executable-code mutants in copies through CLI, JSON and MCP together; outcomes and score per [ADR 0111](decisions/0111-mutation-evidence.md), with solver answers verbatim; qualified on real projects on Linux and macOS with actual campaign costs | #140, #141, [#664](https://github.com/Chelis-Lang/sail/issues/664); [brief](design/verus-mutation-release.md), [evidence spike](validation/mutation-evidence.md) | Evidence rule decided; exec-only generator and campaign evidence model on `main`. Engine, CLI, JSON and MCP built on development branches only. Ordinary runtime integration, real-project acceptance, campaign cost/concurrency and Mac remain open |
| Profiling and stability | Name costly quantifiers and unstable proofs from recorded sessions; no change to verdicts or plain `sail verify` bytes | [Health guide](proof-health.md) | Exact session-prefix, prefix-replay and quantifier-count groundwork merged. History, retention, the stability worker and `sail profile` are not merged |
| Never slower than stock; cheaper changed proofs | Every measured row at or under stock on both platforms, or listed as a failed gate; lower cost for an edit in a large module | #158, #232, #558, #560, #288 | Named rows still slower; see the lanes below |
| Reach | Simpler installation and an editor bridge | Installer, Binstall and LSP work on development branches | Not merged |
| Kani backend (headline candidate) | Spike wave with numeric pass and drop rules, then one owner checkpoint; every claim measured against one `cargo kani -j N` with a warm target; results bounded, never verified | [#135](https://github.com/Chelis-Lang/sail/issues/135); [design](design/kani-phase.md), [ADR 0180](decisions/0180-kani-phase-scope.md) | Pilot passed ([record](https://github.com/Chelis-Lang/sail/issues/135#issuecomment-6755845619)). Decoder #739 and input fixtures #743/#755 merged; a qualified single-harness executor and CLI exist on a development branch only. No cache, batch run, corpus or spike record yet Executor and CLI landed unadvertised ([record](validation/kani-cli.md)): single harness, fresh target per run, no cache |
| cvc5 and the SMT-LIB service | cvc5 in the supported SMT runtime with per-solver conformance and unchanged Z3 bytes | #137 | Development backend merged (#741), unadvertised; Mac qualification open |

## Performance and acceptance lanes

| Lane | Current basis | Next outcome |
|---|---|---|
| Changed-proof cost (#158, #244) | Static longest-first dispatch shipped; measured Vest gain, no Anvil gain. Hand-applied prover isolation cut the Anvil sibling edit from 88 s to 54 s with 0 conformance differences, but cost proof headroom; IronKV has no gap worth isolating ([spike](validation/tune-isolation.md)) | `sail tune` dropped for v0.3.0 and `--isolation auto` not built ([spike record](validation/tune-isolation.md)); the performance guide documents the manual technique |
| Mac Anvil cold (#232) | Latest composed cohort: 98.94 s Sail / 96.87 s stock; retained 2.07 s regression. Linux artifact prerequisite fails: unselected source-map fix plus explicit bundled-macro setup passes a bounded artifact bridge; production routing/reuse open ([VI-0043](verus-issues/VI-0043-export-metadata-map-order.md)) | Resolve strict artifact controls, then native paired measurements; retain original rows |
| Frontend feasibility (#162) | v0.3.0 no-go: complete inputs/effects and current diagnostic mapping remain missing ([ADR 0170](decisions/0170-v030-frontend-reuse-no-go.md)); no new performance result | Keep #162 open; revisit only after a complete boundary and fresh-output shadow comparison, then the unchanged 25% native net-gain gate. Never a release dependency |
| First use and full-path overhead (#234, #558, #560) | Shipped vstd, replay and daemon preparation delivered; misses still cost more | Bounded improvements alongside the feature; no selection of experiments without general gains |
| Compiler-output capture (#632, delivered) | Lossless invocation capture and strict original-stream/interleaving controls merged #724; progress-boundary comparison merged #731 ([qualification](validation/conformance-compiler-capture.md)); original aggregate-output failures remain retained | Broad/scaling performance qualification remains #629; no serial substitute or general performance claim |
| Service-wide acceptance proof (#368) | Source-shared parser, classification, persistence and delivery components adopted | Concrete service composition and host/origin refinements; no complete-service theorem claimed |

Performance work covers both native platforms and the broader
[evaluation corpus](performance-evaluation.md). Record whole-call latency, memory,
resource costs and regressions. A component gain or moving work after return does
not alone establish a campaign or product speedup. Every useful edit improvement
also benefits mutation campaigns.

## Backends: Kani, then solvers, Loom and Creusot

Kani ([#135](https://github.com/Chelis-Lang/sail/issues/135)) is the headline
candidate for v0.4.0 ([ADR 0180](decisions/0180-kani-phase-scope.md), amending
ADR 0174). The [design](design/kani-phase.md) gives the argument, the reuse
layers and their expected gains, the correctness contract, the spike wave and
the workstreams. Nothing beyond the decoder and tool pins is on `main`.

| Step | Gate | State |
|---|---|---|
| Spike wave (baseline, cost split, GOTO determinism, history hit rate, admission, integrity, solver choice, std sample, Mac, reach) | One record each with its pass or drop result | Not started |
| Owner checkpoint | Headline claim, fork, Mac and co-shipping pillars decided from the records | Pending the wave |
| Executor and CLI on `main`, batch run, conformance and corpus | Direct-Kani pairs Same; default Verus unchanged | Conformance harness and corpus: fast tier `same` over three rounds, fault arm passes ([record](validation/kani-conformance.md)); x86_64 blocked on Cargo features; standard tier and batch run not yet |
| Reuse: closure receipts, CBMC-boundary shim, signed CI tier | Each layer's ADR and conformance; measured against `cargo kani -j N` | Not started |

Results are bounded evidence, never reported as verified. Existing contract,
stubbing and loop-contract harnesses are run with named qualifiers; Sail
generates no harnesses or contracts. A Kani fork for deterministic output is
permitted when its spike shows the gain.

After Kani, in order: SAT solver identity in the Kani key; Bitwuzla and cvc5
through CBMC's SMT path, alongside cvc5 in the supported SMT runtime
([ADR 0108](decisions/0108-supported-smt-runtime.md), #137);
[Loom](design/loom-after-kani.md) as a spike-gated client of the Kani executor;
[Creusot](design/creusot-after-kani.md) (#133) once cvc5 is a qualified second
solver. Certificates keep one measurement spike recorded on #138 and no product
work. Prusti, kernel provers, sampled-tier integration, cross-backend
portfolios and Dafny remain future options under #146.

## Oyster: retired

The owner abandoned Oyster on 2026-11-09
([ADR 0109](decisions/0109-oyster-retired-v030-scope.md)). Sail plans no Oyster
work. This section was earlier titled "Oyster: shelf completion; Sail surfaces
held".

| Surface | Disposition in v0.3.0 | State |
|---|---|---|
| SMT runtime (`sail runtime`, MCP runtime tools) | Kept, supported (ADR 0108); cvc5 lands here | Supported |
| Evidence tiers and ledger, counterexample decoding, general function attribution | Kept; decoded inputs moved to the MCP `counterexample` tool | Shipped |
| Neutral lowering and shadow crates, neutral source jobs, neutral mutation and literal witness, the `contracts` end-to-end phase, neutral specifications | Removed | Removed; retained 0.2 neutral job records read as `SAIL3161` |
| `sail-contracts`, `sail-contracts-macros` and `sail-contracts-ir` | Removed with the neutral path; Kani function contracts are not adopted as a front end | Removed from the workspace; 0.2.0 stays on crates.io |
| Requirement attribution and `sail assurance` (ADR 0104) | Retire active entry points after the five-function hand join fails its outside-user/CI-demand bar (ADR0148) | Corrected retirement qualifies on native Linux with independent review; final PR CI remains separate |

## Temporal: native checking on D.I.V.E.

The owner decided on 2026-11-18 to fold the temporal extension into the main
workspace and ship native temporal checking in v0.3.0
([ADR 0172](decisions/0172-native-temporal-in-main-workspace.md)). v0.3.0
links D.I.V.E. 0.1.0; v0.3.1 links D.I.V.E. 0.2.0.

| Gate | State |
|---|---|
| `sail-dive` in the main workspace; `sail` links the backend; worker is a `sail-solve` binary | Implemented (ADR 0172) |
| D.I.V.E. 0.2.0 from crates.io | `dive-api`, `dive-ir`, `dive-value` and `dive-trace` must resolve from crates.io at exactly 0.2.0 with no Git source or `[patch]`; the release branch pins the 0.2.0 candidate by Git revision until it is published |
| Genuine temporal properties with an exact store and state/depth limits | Hold, refute with validated counterexamples, or stop with no claim at a limit |
| Native conformance at the published version | Required before merge ([record](validation/native-temporal-dive-0.1.md)) |

Preserve current behavior and regression coverage; do not introduce a TLC
fallback.

## Delivered releases

| Release | Delivered scope | Evidence |
|---|---|---|
| 0.1.0, 2026-10-29 | Cargo verification, managed receipts, exact solver caches, shipped vstd/replies, instant revert, replay, watch/jobs/MCP, repair and evidence commands; native archives | [Notes](release-notes/0.1.0.md), [qualification](validation/release-candidate.md) |
| 0.2.0, 2026-11-04 | Standalone verification/repair/watch/jobs/MCP, daemon preparation, project byte budgets, hosted CI-cache recipe, consumer skill, corrected compiler-input runtime, core improvements and eighteen crates.io packages | [Notes](release-notes/0.2.0.md), [stable qualification](validation/stable-candidate.md), [feature qualification](validation/release-expansion.md) |
| 0.3.0, 2026-11-19 | Native temporal checking on D.I.V.E. 0.1.0 (experimental), proof health, CI reports and the GitHub Action; neutral-contract surfaces removed | [Notes](release-notes/0.3.0.md), [native conformance](validation/native-temporal-dive-0.1.md), [against TLC](validation/sail-dive-vs-tlc.md) |

The final 0.2.0 composition has complete native workflow/mutation captures and
all 1,944 retained paired mutation steps compare Same after the separately
qualified explanation-note correction. Small edits/reverts and Mac Anvil cold
still have slower medians. First/full-path parity is not established. Earlier
failed attempts and source-specific results retain their original scope.
See [performance](performance.md), [known issues](known-issues.md) and the
[backlog reconciliation](reports/2026-11-04-roadmap-review.md).

## Performance parity

The original four-project Linux/macOS parity goal remains open engineering work;
it is not evidence that published 0.2.0 is awaiting release. Historical rc.7
first-unchanged numbers and later component savings retain their original source
and environment. The [feature qualification](validation/release-expansion.md)
records the final composition's slower cases; the
[runtime record](validation/binary-compiler-inputs.md) separately records
full-path misses. Do not replace those rows with cached/replayed timings.

## Lower-priority backlog

<a id="after-010-parked"></a>

The old "After 0.1.0 (parked)" anchor remains for existing links. This is future
work, not a prerequisite chain for mutation testing.

| Work | Tracker or condition |
|---|---|
| More exact vstd profiles and versions | #563 is implemented: Vest's profile and members-only native lookup (CapybaraKV), ADR 0090; staged-candidate and macOS qualification open. Preserve every admission check. A second Verus pin is not adopted (#237: pin cadence only) |
| Remaining incidental edits and restore misses | #118, #595 |
| Prebuilt macro tools | #239; owner question 35 and actual measured gain |
| Broader first-sight preparation and team prefetch | #241, #243; bounded worker already ships |
| Cargo bypass / compiler-boundary fallback | #245, #222; separate authority and cost qualification |
| CI image startup | Observed target met: current-image pull command-window p95 40.2s against 120s; retrospective scope and unresolved historical tails retained ([ADR 0171](decisions/0171-observed-ci-startup-target.md), [record](validation/ci-image-startup.md)); required checks and job budgets unchanged |
| Fragility / portfolio polish | Existing experimental modes; demand-led |
| Broader verification runtime | #146; beyond Kani and cvc5 above |

## Measured and unselected

| Idea | Disposition |
|---|---|
| Rust pre-solver (M9) | Covered 0.1% of solve time; closed |
| Warm solver pools / exact-prefix sessions | Small measured opportunity; no selected pool implementation |
| Stack-only default keys | Model/effort differences; default binds exact process prefix (ADR 0008) |
| Stock spinoff-all / parked Z3 snapshots | Regressions or nonidentical state; unselected |
| Persistent frontend reuse | v0.3.0 no-go before performance acceptance (ADR 0170); #162 remains open for a complete reusable boundary |
| Run-scoped / per-file digest experiments | No general whole-call gain; unselected defaults unchanged |
| General v1 native-artifact store | Disabled under ADR 0053 |

<a id="delivered-milestones"></a>

## Milestones

M0–M6 are delivered. M7 was accepted 2026-10-26. M11 remains the continuous
Verus performance lane; M10 records the broader runtime ambition with explicit
holds. The next-release milestone is v0.3.0 and holds only release blockers;
the pillars moved out of it are in the v0.4.0 milestone (ADR 0174). Genuine
0.2.0 residual work moves to its current lane rather than remaining an
apparent publication blocker.

Active ordinary requirement attribution retirement qualifies on native Linux
with independent approval; historical job/SDK compatibility remains intact
(ADR0148). Final PR CI is separate from local qualification.
