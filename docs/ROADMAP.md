# Roadmap

Current ordering follows the owner's 2026-11-04 direction, recorded in
[ADR 0100](decisions/0100-mutation-first-roadmap.md). The latest stable release
is **0.2.0**. The next release has no selected version or publication date;
final publication requires the owner's approval of its exact candidate.

## Goal

Make Verus verification faster and help developers and agents discover whether
their specifications catch plausible implementation bugs. Preserve default-mode
correctness, explicit scope, provenance and conservative handling of uncertainty.
Most verification scenarios should be faster than stock Verus; existing losses
remain engineering work, including cold runs. No baseline is relaxed.

## Next release: Verus-aware mutation testing

The headline feature answers: **my program verifies, but would my specifications
catch plausible bugs?** It serves existing Verus projects independently of
Oyster, neutral contracts and additional backends.

| Work | Required outcome | Tracker |
|---|---|---|
| Native outcome diagnosis first | Reproduce existing literal-mutant Unknown cases, distinguish verifier rejection from independently confirmed counterexamples, and qualify useful killed/survived/inconclusive observations without relabeling Unknown | #140, [#664](https://github.com/Chelis-Lang/sail/issues/664); [existing evidence](validation/neutral-mutation.md) |
| Verus-aware mutation | Bounded executable-code changes in copies; preserve specifications, proof/ghost code and original user files | #140 |
| Campaigns and reports | Per-function inventory, budgets and killed/survived/inconclusive outcomes through CLI, JSON and MCP together; omitted/unsupported work stays visible | #141 |
| Native qualification | Passing baselines, plausible bugs, survivors, Unknown, cancellation and limits; real projects on Linux/macOS; unchanged-contract evidence and actual campaign costs | #140, #141 |

Survivors are actionable observations about specification strength, not proof of
adequacy or vacuity. Existing benchmark mutation catalogues and neutral literal
campaigns are groundwork, not the general product. Expected cache savings must
be measured across complete campaigns, including setup and unsuccessful work.
See the [feature brief](design/verus-mutation-release.md) and
[next-release milestone](https://github.com/Chelis-Lang/sail/milestone/13).

## Alongside: performance and acceptance

| Lane | Current basis | Next outcome |
|---|---|---|
| Changed-proof cost (#158, #244) | Static longest-first dispatch shipped; measured Vest gain, no Anvil gain | Profile critical paths; evaluate recorded-cost scheduling and useful miss reductions on broader edit workloads |
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

## Then: Kani pilot before cvc5

[#135](https://github.com/Chelis-Lang/sail/issues/135) starts with an executable
pilot on a real pinned ordinary Rust crate. Compare direct Kani with the proposed
Sail path, exercise concrete failures and retain exact tool identities, bounds,
unsupported cases and costs. Assess the experimental function-contract interface
in the go/no-go decision before committing to a production backend. The pilot
must not depend on widening Oyster's neutral-contract subset.

cvc5, certificates, Creusot, Prusti, kernel provers, sampled-tier integration,
cross-backend portfolios and Dafny remain future options under #146. They are
not parallel commitments for the next release.

## Oyster: design first, engineering on hold

Maintain and test the experimental raw SMT, neutral contracts, source jobs and
assurance reads already consumed by Oyster. Do not expand those surfaces, widen
the neutral subset or migrate Oyster's SDK while the design is unsettled.

The next Oyster deliverable is a short design document answering who the user
is, their present problem, one successful end-to-end session, and why a
requirements layer adds value beyond direct specifications, contracts and
mutation feedback. The owner's supplied colleague feedback is the deciding
input. Design resolution precedes renewed integration engineering.

## Temporal: wait for distribution

The native D.I.V.E. extension exists internally; public Sail archives have no
temporal backend (ADR 0083). Hold packaging and expansion until D.I.V.E.'s
private-source distribution decision is made. Preserve current behavior and
regression coverage; do not introduce a TLC fallback.

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
| More exact vstd profiles and versions | #563, #237; preserve every admission check |
| Remaining incidental edits and restore misses | #118, #595 |
| Prebuilt macro tools | #239; owner question 35 and actual measured gain |
| Broader first-sight preparation and team prefetch | #241, #243; bounded worker already ships |
| Cargo bypass / compiler-boundary fallback | #245, #222; separate authority and cost qualification |
| CI image startup | #195; retain required checks and job budgets |
| Fragility / portfolio polish | Existing experimental modes; demand-led |
| Broader verification runtime | #146; design/distribution holds above |

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
holds. The next-release mutation milestone is feature-based until the owner
selects a version. Genuine 0.2.0 residual work moves to its current lane rather
than remaining an apparent publication blocker.
