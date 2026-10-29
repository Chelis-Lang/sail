# Acceptance proof work and PR findings

The [design](evidence-acceptance-core.md) defines the unfinished acceptance
proof. [Issue 368](https://github.com/Chelis-Lang/sail/issues/368) is its single
GitHub work tracker. The component obligations remain in the design and
[production refinement guide](acceptance-production-refinements.md); separate tickets
are not needed for each unfinished component.

Bug filings use current `main` as their baseline. A bug introduced only by a
local change, branch or open PR is fixed there and recorded in that PR or this
ledger. If such a bug reaches main, it needs a main-relative issue. Existing
main bugs get GitHub issues; assign the owner when imminent PR work fixes or
supersedes them so another implementation does not run in parallel.

## Main-relative findings

Baseline: `9dc2038c`.
These nine issues concern implementation present on main. Tooling probes do
not imply an observed wrong verifier verdict or a failure of the existing
registered proof programs.

| Issue | Reported main behavior | Current status |
|---|---|---|
| [419](https://github.com/Chelis-Lang/sail/issues/419) | Explicit environment validation resamples ambient configuration homes | Fixed by merged PR 542; issue closed |
| [432](https://github.com/Chelis-Lang/sail/issues/432) | Source tokenization repeatedly copies the remaining input | Fixed by merged PR 542; issue closed |
| [435](https://github.com/Chelis-Lang/sail/issues/435) | Compiler-visible Sail settings can differ from plain Verus | Owner accepts the bounded integration exception in ADR 0088 / PR 550; no private transport is selected |
| [475](https://github.com/Chelis-Lang/sail/issues/475) | Prepared checkouts share Cargo target fingerprints | Fixed by merged PR 542; issue closed |
| [477](https://github.com/Chelis-Lang/sail/issues/477) | Bounded failure capture selects old command files before producer evidence | Fixed by merged PR 545; issue closed. Collector acceptance does not reproduce the separate original intermittent replay failure |
| [514](https://github.com/Chelis-Lang/sail/issues/514) | The source guard mistakes a `.union(` expression for a declaration | Fixed by merged PR 542; issue closed |
| [532](https://github.com/Chelis-Lang/sail/issues/532) | The reviewed proof-import lexer excludes identifier digits | Fixed by merged PR 542; issue closed |
| [537](https://github.com/Chelis-Lang/sail/issues/537) | Control admission permits a proof-only false assertion with unchanged native behavior | Fixed by merged PR 542; issue closed |
| [540](https://github.com/Chelis-Lang/sail/issues/540) | Full local E2E leaks generated-file source into a stock oracle without its build script | Fixed by merged PR 542; issue closed |

The 514/532 probes extend otherwise-supported source/import syntax; main's
existing 25 captured programs do not contain those cases. Issue 537 has an
actual main source-only inventory/capture reproduction, without a Stock run
or a claim that a historically registered control used the bypass. The
collector source for 477 is byte-identical between main and the failed run.
Native integration and exact proof scope remain in the
[qualification record](../validation/acceptance-production-refinements.md).

## Unmerged PR findings

The following 95 reports concern unmerged implementation, fixtures, local
qualification or support limitations encountered by that work. They are valid
findings; they are not a list of 95 established main-branch regressions. A
failure observed only on a PR does not establish applicability to main.

The former ticket numbers below are stable local finding identifiers. Their
GitHub tickets are closed as PR-only at the owner's request. Full original reports and
all 69 comments are preserved in a managed local Markdown archive; this index
keeps the technical work discoverable in the repository. Closure does not
mean a fix is merged, qualified or no longer required.

PR 378 retains the remaining composition work; PR 543 contains the parser
slice. Each component's design and qualification record owns its actual
source/fix evidence, unsupported primitives and remaining gates. Routine
compile, lint and proof-guidance failures stay in those records rather than
creating new main-bug tickets.

### Query framing and session correspondence

| Finding | Defect or limitation |
|---|---|
| <a id="finding-444"></a> 444 | Private transcript candidate omits actual solver response records |
| <a id="finding-456"></a> 456 | charge complete retained Entry carrier to recording budget |
| <a id="finding-457"></a> 457 | Proof candidate: replay completion does not bind comparisons to retained stdout prefix |
| <a id="finding-458"></a> 458 | test: include option state in existing assembly mutation fixture |
| <a id="finding-468"></a> 468 | test: select full replay mutation from executable body, not ensures |
| <a id="finding-469"></a> 469 | proof-only list witness patterns move values before executable reuse |
| <a id="finding-470"></a> 470 | streaming list kernel fails strict native package lints |
| <a id="finding-480"></a> 480 | Preserve exact splitter inventory through the owned input append wrapper |
| <a id="finding-481"></a> 481 | Correct outer-stream proof expression grouping and sequence index types |
| <a id="finding-482"></a> 482 | Preserve exact outer parser continuation through the clone contract |
| <a id="finding-484"></a> 484 | Retarget native delegation source guards after outer parser extraction |
| <a id="finding-485"></a> 485 | Human-output stdout omission control reaches the default Stock resource limit |
| <a id="finding-487"></a> 487 | use ordinary reveal for nonrecursive lexical proof witnesses |
| <a id="finding-488"></a> 488 | keep shared lexical operations under strict native package lints |
| <a id="finding-490"></a> 490 | use the public item projection in the owned-output proof witness |
| <a id="finding-491"></a> 491 | test: preserve the owned-output partial-frame witness under strict lint |
| <a id="finding-500"></a> 500 | test: distinguish input CR termination from a pending output-line atom |
| <a id="finding-501"></a> 501 | test: use diagnostic empty-item equality in CR fixture |
| <a id="finding-502"></a> 502 | keep closed transcript facts inside the output-owner proof boundary |
| <a id="finding-503"></a> 503 | qualify explicit owned-output transfer syntax under strict Clippy |
| <a id="finding-504"></a> 504 | keep replay initializer lifetime only in its actual test scope |
| <a id="finding-509"></a> 509 | proof: retain original output facets across the actual end wrapper |
| <a id="finding-510"></a> 510 | Full query controls exhaust the unchanged compared-stdout constructive witness |
| <a id="finding-511"></a> 511 | retain the replay-view lifetime across proof syntax erasure |
| <a id="finding-513"></a> 513 | Native forwarder source fixture retains pre-OutputBatch replay advancement selector |
| <a id="finding-517"></a> 517 | candidate shallow value walker ends with unit-typed while expression |
| <a id="finding-518"></a> 518 | candidate command kernel fails strict Clippy on three equivalent idioms |
| <a id="finding-519"></a> 519 | bind replay framing and budget fixture to the actual terminal join |
| <a id="finding-521"></a> 521 | candidate pre-apply capture leaves the native key projection source anchor stale |
| <a id="finding-525"></a> 525 | close candidate frame category comparison correspondence with checked variants |
| <a id="finding-526"></a> 526 | candidate frame interpreter gate rejects proof-only local and match spelling |
| <a id="finding-530"></a> 530 | Candidate option-store native qualification refuses two strict lints |
| <a id="finding-531"></a> 531 | proof: two pre-apply controls exhaust default Stock resources in the combined root |
| <a id="finding-533"></a> 533 | Shared parser candidate drops Token::text must-use annotation |
| <a id="finding-538"></a> 538 | Option-store full-body mutants bypass checked loop obligations |
| <a id="finding-544"></a> 544 | Parser feature omits its internal dependency lock edge |

### Cache state, accounting and origins

| Finding | Defect or limitation |
|---|---|
| <a id="finding-473"></a> 473 | proof: pinned stock lacks actual Arc allocation-identity support |
| <a id="finding-483"></a> 483 | Candidate generation refinement stops before proof/native tests at exec-in-ghost and Clippy errors |
| <a id="finding-486"></a> 486 | Candidate slot charge omits alignment growth in incremental generation metadata |
| <a id="finding-494"></a> 494 | Layout-only source policy accepts alias/runtime substitution under unchanged name counts |
| <a id="finding-498"></a> 498 | Pinned stock Verus rejects actual cache HashMap remove_entry and get_mut operations |
| <a id="finding-499"></a> 499 | Pinned stock Verus cannot observe actual Vec capacity for owned-cache bucket charging |
| <a id="finding-516"></a> 516 | proof: full-body chain touch neighbor-loss control hits default rlimit |
| <a id="finding-520"></a> 520 | test: native LRU budget fixture calibrates itself from production accounting |
| <a id="finding-524"></a> 524 | build: native owned-cache integration fails strict Clippy on shared storage |
| <a id="finding-527"></a> 527 | Pinned stock Verus lacks actual BTreeMap take, owned values and oldest-pop support |
| <a id="finding-536"></a> 536 | Native original-endpoint mutation still constructs removed Inner::default |
| <a id="finding-539"></a> 539 | Native clock source guard accepts destructuring aliases and qualified restart |

### Compiler input and declaration closure

| Finding | Defect or limitation |
|---|---|
| <a id="finding-380"></a> 380 | provider accounting rejects authenticated output completion markers |
| <a id="finding-390"></a> 390 | successful compilation shrinks the receipt declaration domain |
| <a id="finding-410"></a> 410 | Receipt 7 candidate keeps obsolete retained producer-policy revision |
| <a id="finding-438"></a> 438 | consumer accounting accepts orphan declared stream slots |
| <a id="finding-446"></a> 446 | disambiguate merged receipt and proof-context revisions |
| <a id="finding-447"></a> 447 | align bounded source inventory transfer with declaration revisions |
| <a id="finding-507"></a> 507 | Managed ghost-only include stays Unknown after the second accepted verification call |

### Physical observations and process history

| Finding | Defect or limitation |
|---|---|
| <a id="finding-384"></a> 384 | process attempt slots collide across same-run compiler invocations |
| <a id="finding-409"></a> 409 | retained arena depends on removable original producer history |
| <a id="finding-411"></a> 411 | candidate archived attempt validator omitted supported schema checks |
| <a id="finding-425"></a> 425 | solver gate selects an actual compiler phase witness without its required inputs |
| <a id="finding-452"></a> 452 | Proof integration: outer Cargo environment removal requires a test-only command interface |
| <a id="finding-460"></a> 460 | update selected-attempt finish fixture for compiler environment |
| <a id="finding-465"></a> 465 | test: focused failure fixture edits an unselected sidecar instead of its owned attempt |

### Original request and metadata correspondence

| Finding | Defect or limitation |
|---|---|
| <a id="finding-413"></a> 413 | draft unchanged-call integration can skip requested original-request capture |
| <a id="finding-472"></a> 472 | candidate target comparator emits forbidden trait implementation preconditions |
| <a id="finding-479"></a> 479 | request construction candidate has inconsistent source capture bindings |
| <a id="finding-489"></a> 489 | Request dictionary candidate leaves three native fixture compilation errors |
| <a id="finding-492"></a> 492 | Captured worker selector candidate has unwrapped proof witnesses |
| <a id="finding-528"></a> 528 | Unselected raw-metadata refinement adapter imports Error and Result from the wrong crate |
| <a id="finding-529"></a> 529 | Unselected metadata-refinement collision fixture omits fallible expected-target propagation |
| <a id="finding-535"></a> 535 | Additive IEEE proof candidate places mathematical types in a Cargo-erased envelope |
| <a id="finding-541"></a> 541 | proof support: pinned Stock IEEE_EQ/original-bits bridge remains unproved |

### Report authority, replay and presentation

| Finding | Defect or limitation |
|---|---|
| <a id="finding-415"></a> 415 | draft unchanged-call replay bypasses original selected producer history |
| <a id="finding-427"></a> 427 | cached retained-result frames bypass live original-request admission |
| <a id="finding-429"></a> 429 | baseline save rechecks request separately from its admitted project |
| <a id="finding-430"></a> 430 | draft whole-only replay admission blocks required exact partial scopes |
| <a id="finding-449"></a> 449 | refinement report overstates native input source proof coverage |
| <a id="finding-453"></a> 453 | Proof integration completion fixture expects obsolete eleventh E2E result |
| <a id="finding-455"></a> 455 | import Duration for production no-op replay functions |
| <a id="finding-459"></a> 459 | import shared settle constant in native replay fixtures |
| <a id="finding-461"></a> 461 | test: preserve owned legacy reply fixtures under strict lint gate |
| <a id="finding-462"></a> 462 | generic baseline request default adds an unintended deserialize bound |
| <a id="finding-463"></a> 463 | completed-baseline observation fixture exceeds strict line lint |
| <a id="finding-464"></a> 464 | test: baseline view positive assumes currentness without retained readback |
| <a id="finding-471"></a> 471 | cold neutral-contract acceptance loses positive proof to unknown backend currentness |
| <a id="finding-476"></a> 476 | unchanged-call replay cannot record after original-request integration |
| <a id="finding-493"></a> 493 | declare retained lifetimes in checked human-context proof accessor |
| <a id="finding-505"></a> 505 | Whole authority control exhausts unrelated human-output renderer at default resources |
| <a id="finding-506"></a> 506 | Colored Cargo footer prevents unchanged human-output recording |
| <a id="finding-508"></a> 508 | Runtime-control guard counts an erased proof semicolon as executable change |
| <a id="finding-522"></a> 522 | proof: colored renderer always-refusal control exhausts default Stock resources |

### Proof capture and qualification infrastructure

| Finding | Defect or limitation |
|---|---|
| <a id="finding-443"></a> 443 | pinned stock compute interpreter panics on let-bound array view |
| <a id="finding-454"></a> 454 | Proof controls remove loop obligations instead of producing full-scope verification failures |
| <a id="finding-495"></a> 495 | preserve canonical dependency order in composed refinement manifests |
| <a id="finding-496"></a> 496 | retain parser import guards in combined proof runner |
| <a id="finding-497"></a> 497 | use canonical Git for native source equivalence preflight |
