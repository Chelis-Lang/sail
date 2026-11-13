# Roadmap

Current ordering follows the owner's 2026-11-09 decision, recorded in
[ADR 0109](decisions/0109-oyster-retired-v030-scope.md), which amends
[ADR 0100](decisions/0100-mutation-first-roadmap.md). The latest stable release
is **0.2.0**. The next release is **v0.3.0**; it has no publication date, and
final publication requires the owner's approval of its exact candidate.

The active execution tracker is
[#712](https://github.com/Chelis-Lang/sail/issues/712), superseding the
historical 0.2.0 handover #609. The owner confirmed on 2026-11-11 that the
whole v0.3.0 program, including flagged defects and qualification gaps, is to
be completed through candidate staging. The continuous #368 proof lane keeps
its existing boundary: affected proofs gate changes, but the complete
service-wide theorem is not a v0.3.0 release gate.

## Goal

Tell Verus developers and agents what their proofs are worth, and never cost
them time for it: whether their specifications catch plausible implementation
bugs, which proofs are fragile or slow and why, and verification that is never
slower than stock Verus. Preserve default-mode correctness, explicit scope,
provenance and conservative handling of uncertainty. Existing losses to stock
remain engineering work, including cold runs. No baseline is relaxed.

## Next release: v0.3.0

Each item starts with a short spike that has a pass/fail rule and a drop rule.
Work that fails its spike is closed with a short record and not carried.
v0.3.0 will remove API published in 0.2.0, so it is a breaking release.

| Pillar | Gate | Tracker | State |
|---|---|---|---|
| Verus mutation testing (headline) | Bounded executable-code mutants in copies through CLI, JSON and MCP together; outcomes and score per [ADR 0111](decisions/0111-mutation-evidence.md), with solver answers verbatim; qualified on real projects on Linux and macOS with actual campaign costs | #140, #141, [#664](https://github.com/Chelis-Lang/sail/issues/664); [brief](design/verus-mutation-release.md), [evidence spike](validation/mutation-evidence.md), [neutral evidence](validation/neutral-mutation.md) | Evidence rule decided (ADR 0111); exec-only generator delivered in #705; campaign engine and user interfaces not built |
| Proof health and profiling | Rank functions by resource margin and stability, and name costly quantifiers, from recorded sessions; no change to verdicts or plain `sail verify` bytes | Experimental fragility engine exists; [health guide](proof-health.md) | Health fields, CLI/JSON/MCP and retained comparisons implemented in the working candidate; qualification pending. Session retention, stability worker and full-prefix profiling remain unbuilt |
| Never slower than stock; cheaper changed proofs | Every measured row at or under stock on both platforms, or listed as a failed gate; lower cost for an edit in a large module | #158, #244, #232, #558, #560 | Named rows still slower; see the lanes below |
| Reach | GitHub Action, simpler installation, editor bridge | [CI-cache recipe](ci-cache.md) shipped in 0.2.0; [GitHub Action](github-action.md) delivered in #702 | Installer distribution and editor bridge not built; installed Mac Action qualification remains |
| Backends | Kani as an experimental backend for hand-written harnesses, with `arrayvec` 0.7.8 as the pilot fixture and direct-Kani comparison; then cvc5 in the supported SMT runtime with per-solver conformance and unchanged Z3 bytes | [#135](https://github.com/Chelis-Lang/sail/issues/135), #137 | Kani pilot passed ([record](https://github.com/Chelis-Lang/sail/issues/135#issuecomment-6755845619)) and Kani is selected as experimental; the bounded-observation decoder passes native Linux component gates and independent review on a development branch; executor/CLI and broader backend qualification remain pending. cvc5 follows it |
| Neutral-contract removal | Archive tag, #664 cases ported to plain Verus, #683 and #696 reproduced on ordinary jobs; then removal, including all three contract crates, with every 0.2.0 item listed in the changelog | [ADR 0109](decisions/0109-oyster-retired-v030-scope.md) | Removed, including all three contract crates; the last implementation is tagged `archive/neutral-contracts-final` and the published 0.2.0 crates are not yanked. #664's retained case is plain Verus in [`corpus/repros/neutral-contract-unknown`](../corpus/repros/neutral-contract-unknown/README.md). #683 and #696 were reproduced on ordinary jobs (2026-11-09 spike comments on each issue); their fixes are tracked there |
| Publication | Owner approves the exact final candidate | [Releasing](releasing.md) | No candidate |

The headline answers: **my program verifies, but would my specifications catch
plausible bugs?** Survivors are actionable observations about specification
strength, not proof of adequacy or vacuity. Existing benchmark mutation
catalogues and neutral literal campaigns are groundwork, not the general
product. Expected cache savings must be measured across complete campaigns,
including setup and unsuccessful work. See the
[next-release milestone](https://github.com/Chelis-Lang/sail/milestone/14).

Linux aarch64 is outside this program. The owner clarified on 2026-11-11
that it was not requested; no support work is scheduled, and the source-built
oracle proposal in [question 50](open-questions.md) is inactive. The
supported platforms remain Linux x86-64 and macOS on Apple silicon.

## Performance and acceptance lanes

| Lane | Current basis | Next outcome |
|---|---|---|
| Changed-proof cost (#158, #244) | Static longest-first dispatch shipped; measured Vest gain, no Anvil gain. Hand-applied prover isolation cut the Anvil sibling edit from 88 s to 54 s with 0 conformance differences, but cost proof headroom; IronKV has no gap worth isolating ([spike](validation/tune-isolation.md)) | `sail tune` dropped for v0.3.0 and `--isolation auto` not built ([spike record](validation/tune-isolation.md)); the performance guide documents the manual technique |
| Mac Anvil cold (#232) | Latest composed cohort: 98.94 s Sail / 96.87 s stock; retained 2.07 s regression | Attribute and fix with native paired measurements; retain original rows |
| Frontend feasibility (#162) | Persistent reuse unqualified; incomplete inputs and diagnostic mappings identified | Bounded go/no-go for complete inputs, current diagnostics and net whole-call benefit; never a release dependency |
| First use and full-path overhead (#234, #558, #560) | Shipped vstd, replay and daemon preparation delivered; misses still cost more | Bounded improvements alongside the feature; no selection of experiments without general gains |
| Correctness-test gaps (#632) | Trailing-output comparison fixed in 0.2.0 (#611); parallel verifier-output capture remains incomplete | Lossless per-compiler capture and original parallel controls; no serial substitute or weaker comparison |
| Service-wide acceptance proof (#368) | Source-shared parser, classification, persistence and delivery components adopted | Concrete service composition and host/origin refinements; no complete-service theorem claimed |

Performance work covers both native platforms and the broader
[evaluation corpus](performance-evaluation.md). Record whole-call latency, memory,
resource costs and regressions. A component gain or moving work after return does
not alone establish a campaign or product speedup. Every useful edit improvement
also benefits mutation campaigns.

## Backends: Kani, then cvc5

Kani ([#135](https://github.com/Chelis-Lang/sail/issues/135)) ships in v0.3.0
as an experimental backend, by the owner's choice. It runs harnesses the user
writes by hand; `arrayvec` 0.7.8, a pinned ordinary Rust crate, is the pilot
fixture. Qualification compares direct Kani with the Sail path, exercises
concrete failures and retains exact tool identities, bounds, unsupported cases
and costs. Results are bounded evidence, never reported as verified. Kani
function contracts are not adopted as a front end.

cvc5 follows Kani inside v0.3.0. It lands only in the supported
SMT runtime ([ADR 0108](decisions/0108-supported-smt-runtime.md)), with solver
identity in every key. Certificates get one measurement spike recorded on
#138 and no product work. Creusot, Prusti, kernel provers, sampled-tier
integration, cross-backend portfolios and Dafny remain future options under
#146.

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
| Requirement attribution and `sail assurance` (ADR 0104) | Kept experimental for now; keep-or-remove is decided when `sail mutate` exists | Experimental, present |

## Temporal: behind a default-off flag

Superseding the earlier hold: on 2026-11-06 the Oyster owner resumed Oyster
for shelf completion, covering its SDK migration to the published 0.2.0
crates and D.I.V.E. work in Sail behind a default-off flag
([ADR 0101](decisions/0101-dive-behind-a-default-off-flag.md)).

| Gate | State |
|---|---|
| Default builds, crates.io and public archives link no temporal backend | Enforced by `cargo xtask check-layering` and the CLI tests |
| Extension build and tests at the pinned D.I.V.E. revision | Local tier, `ci/local.sh extension` |
| Internal D.I.V.E.-enabled `sail` | `ci/run.sh extension-install DIR` |
| Public temporal packaging | Waits for D.I.V.E.'s public distribution; flip queued as a draft pull request |

Preserve current behavior and regression coverage; do not introduce a TLC
fallback.

## Delivered releases

| Release | Delivered scope | Evidence |
|---|---|---|
| 0.1.0, 2026-10-29 | Cargo verification, managed receipts, exact solver caches, shipped vstd/replies, instant revert, replay, watch/jobs/MCP, repair and evidence commands; native archives | [Notes](release-notes/0.1.0.md), [qualification](validation/release-candidate.md) |
| 0.2.0, 2026-11-04 | Standalone verification/repair/watch/jobs/MCP, daemon preparation, project byte budgets, hosted CI-cache recipe, consumer skill, corrected compiler-input runtime, core improvements and eighteen crates.io packages | [Notes](release-notes/0.2.0.md), [stable qualification](validation/stable-candidate.md), [feature qualification](validation/release-expansion.md) |

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
| CI image startup | #195; retain required checks and job budgets |
| Fragility / portfolio polish | Existing experimental modes; demand-led |
| Broader verification runtime | #146; beyond Kani and cvc5 above |

## Measured and unselected

| Idea | Disposition |
|---|---|
| Rust pre-solver (M9) | Covered 0.1% of solve time; closed |
| Warm solver pools / exact-prefix sessions | Small measured opportunity; no selected pool implementation |
| Stack-only default keys | Model/effort differences; default binds exact process prefix (ADR 0008) |
| Stock spinoff-all / parked Z3 snapshots | Regressions or nonidentical state; unselected |
| Persistent frontend reuse | No complete reusable operation boundary; bounded feasibility only |
| Run-scoped / per-file digest experiments | No general whole-call gain; unselected defaults unchanged |
| General v1 native-artifact store | Disabled under ADR 0053 |

<a id="delivered-milestones"></a>

## Milestones

M0–M6 are delivered. M7 was accepted 2026-10-26. M11 remains the continuous
Verus performance lane; M10 records the broader runtime ambition with explicit
holds. The next-release milestone is v0.3.0. Genuine 0.2.0 residual work moves
to its current lane rather than remaining an apparent publication blocker.
