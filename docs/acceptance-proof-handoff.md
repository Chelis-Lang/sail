# Acceptance proof delivery handoff

The production acceptance theorem remains open under
[design tracker #368](https://github.com/Chelis-Lang/sail/issues/368).
[PR #378](https://github.com/Chelis-Lang/sail/pull/378) preserves the unfinished
composition checkpoint; it is not a merge candidate as a whole. Component
proofs and selected production refinements do not establish complete service
correctness.

## Current base and preserved work

Actual runtime qualification uses main `8797ccd4`, including the
compiler-read observation fix in #572 and merged Verus fork #13. The final restack uses main `b56889f1`; parser #543 is the immediate parent
at `c04fcee3`, with only inherited installer/docs changes and focused refresh. The
combined native execution is frozen at `00382159`; final qualification edits
preserve all non-Markdown inputs exactly. Preserve schema-two compiler observations,
receipt revision 10, archive revision 5, the durable input-dirty markers and
the complete per-file dep-info rule inventory. The earlier umbrella's input
adapters must not overwrite these fixes.

The preserved published umbrella checkpoint is `0d49bd48`. Its historical
merge-tree comparison against `e9862433` had 41 conflicted files; that count is
not a current-main comparison. Extract bounded feature deltas onto current
main and review their production callers rather than resolving by accepting
whole historical files. The old local branch named `proof/acceptance-followup`
is not the published PR head; obtain the latter from GitHub before comparison.

## Qualified delivery slices

| Slice | Production connection | Completed qualification |
|---|---|---|
| Parser #543 | Native stream splitter and borrowed lexer use the shared checked operations | Parent whole parser 129/204 and all 54 controls pass; frozen corpus/cost and fresh current-main runtime checks pass. The combined context adds four filename functions: 133/208, all 54 controls freshly checked. |
| Deferred reply holding | Preserve the original bound request/reply pair while effort metadata is detached and updated | Whole combined query 295/480 and all 145 ordinary controls pass; actual pairer ordering/stop/finish regressions, native gates, full fast/standard conformance and bounded native demo measured. |
| Dependency filename binding | Check each current decoder proposal against the exact original encoded filename | Whole input 59/96 and all ten controls pass; pathological-name/whole-inventory refusal tests and full native/conformance scope pass. |

The combined matrix has 238 refinement cases and 35 abstract model cases.
The [combined qualification](validation/compiler-filename-refinement.md) records
actual source/runtime identities, exact-source reuse, fresh current-main checks,
timing limits and failed auxiliary attempts. Complete private readback passes.
This is readiness for lead review and hosted checks, not permission to claim
the complete theorem or fresh macOS acceptance.

The native release path avoids allocation just to reuse an optional batch
helper. Reason-unknown/statistics publication retains its existing order; the
native stream is not necessarily a simple source-order FIFO.

The filename predicate refuses empty, NUL, LF and CR names. CR is a legal Unix
filename byte, so selecting that predicate intentionally narrows reusable input
coverage. Refusal must make the inventory incomplete and retain ordinary
verification; it must never drop a path while claiming complete input coverage.
Do not normalize filenames or replace main's per-file rule parser.

## Proof and review discipline

Use direct pinned stock Verus, complete roots, unchanged resources and strict
source/runtime/function-count checks. Retain all inherited functions, contracts,
loop invariants, constructive witnesses and ordinary executable failure
controls. Type errors, resource exhaustion and timeouts earn no negative-control
credit. Review the exact captured source and raw verifier output independently
of the summary report.

The runtime changes in these three slices require the full fast/standard
conformance tier under ADR 0087. A proof-only preparatory edit does not by itself
qualify subsequent production adoption. Keep passed, source-identical reused,
failed and unrun evidence separate. Keep PRs draft until their actual scope, substantive lead review and required
checks are satisfied. Parent #543 lands first; publish the bounded child only
after reviewing its final source and qualification. The native machine is not
needed to review the privately closed receipts.

Main-relative bugs belong in GitHub with precise evidence and duplicate checks.
PR-introduced findings stay in the owning PR or
[findings ledger](design/acceptance-pr-findings.md). Unfinished design work stays
in the design and #368. Jeff merged #557 as an empty-tree change after #572 had already incorporated
its contents. Reconcile #570 against current main and owner direction; proof delivery does
not authorize its closure.

The remaining service theorem still needs parser/session composition, truthful
cache origin, complete compiler-input closure, filesystem/process observations,
original-request correspondence and every public success surface. This delivery
round finishes bounded existing work; it does not assume those obligations away.
