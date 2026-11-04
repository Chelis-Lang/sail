# Post-0.2.0 roadmap and backlog reconciliation

The owner's 2026-11-04 direction selects Verus-aware mutation testing as the
next-release headline. [ADR 0100](../decisions/0100-mutation-first-roadmap.md)
and the [roadmap](../ROADMAP.md) govern current order. This is a planning and
tracking change, not implementation of the feature or a new qualification run.

## Current work

- #140/#141: bounded executable-code mutation, per-function outcomes and
  CLI/JSON/MCP together. Native Unknown diagnosis starts at
  [#664](https://github.com/Chelis-Lang/sail/issues/664).
- #168: continuous performance, prioritizing changed-proof scheduling/misses
  and Mac Anvil cold; #162 is bounded frontend feasibility, never a release
  dependency. First-use/full-path and remaining useful ideas stay in backlog.
- #135: later real-crate Kani pilot before cvc5, with an explicit go/no-go
  covering actual user value, direct-backend parity and experimental contracts.
- Existing SMT/neutral/assurance integration is maintained, without Oyster SDK
  migration or subset expansion. Oyster has a private design brief and tracker;
  user/problem/session/why requirements must be settled before engineering.
- Temporal packaging waits for D.I.V.E.'s distribution decision. #368 remains
  the independent service-wide acceptance theorem.

## Shipped work and residuals

The audit covers every one of the 63 Sail issues open at its start. Current
issue summaries supersede historical plans while preserving their reports.
Evidence is existing merged code and its source-specific native/installed
qualification; no result is relabeled as a fresh execution.

| Disposition | Issues | Basis and limits |
|---|---|---|
| Delivered feature/maintenance closure | #642, #643, #644, #645, #646, #576 | [Feature qualification](../validation/release-expansion.md), [standalone](../standalone-verification.md), [core matrix](../validation/v020-performance.md); no universal speedup or resource-cost claim |
| Delivered test/runtime correction | #611, #323, #627, #655 | [Changelog](../../CHANGELOG.md), [native integration](../validation/integration-production.md), [source suites](../validation/general-performance.md), [note correction](../validation/mutation-notice-cardinality.md); original failures preserved |
| Delivered bounded measurement/fallback | #562, #622, #561 | [Observer](../validation/benchmark-memory.md), [native-Cargo fallback](../validation/unsupported-cargo-fallback.md); custom toolchains and full-path parity remain limited |
| Documentation corrected | #637 | README distinguishes recoverable fallback from error/unconfirmed evidence; static documentation reconciliation |
| Superseded / not selected | #609, #630 | Old release handover superseded; whole-call experiments unselected rather than disguised as wins; overhead follow-up remains #560 |
| Genuine open correctness-test gap | #632 | Complete lossless parallel compiler-output capture and original parallel comparisons remain unqualified |
| Corrected generator, separate remaining gate | #631 | Missing-prelude defect corrected in #635; valid generated parallel runs exposed #632. #629 retains broad/scaling acceptance |
| Open diagnosis, not a claimed fix | #616, #595, #306, #280 | Fixture hardening or later passing controls do not establish original causes |
| Partial delivery, residual work retained | #558, #560, #563, #118, #241, #252 | Preparation, naming, exact profiles and selected flags are implemented; remaining parity/coverage/resource questions stay explicit |
| Maintained bounded integrations / deferred expansion | #127–#139, #142–#146 | Shipped slices distinguished from broader ambitions; Kani pilot ordering and Oyster/temporal holds recorded individually |

The published 0.2.0 milestone is closed after moving genuine residual work to
its active lane. Its closure asserts neither stock parity nor absence of known
regressions. M7's already accepted repair milestone is also reconciled; its
[qualification](../validation/m7.md) retains its original limits. No new release
version or publication authority is created.

## Remaining correctness and assurance work

Both solver transcript comparators now check splitter completion and retain
incomplete-suffix regressions (#611). Parallel output capture (#632) must retain
all original targets, diagnostics and invocation identity; serial execution and
matching aggregate counts do not replace its acceptance test. Follow-up fixes
need their own runtime/tooling implementation and qualification.

The compound-field lowering correction (#323) has later actual source-service
controls; the early component record's pending status is historical. This does
not justify broadening the admitted source language. Native mutation Unknown
continues to be inconclusive. Acceptance component proofs do not close #368.
