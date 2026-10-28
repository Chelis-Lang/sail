# Acceptance proof delivery handoff

The production acceptance theorem remains open under
[design tracker #368](https://github.com/Chelis-Lang/sail/issues/368).
[PR #378](https://github.com/Chelis-Lang/sail/pull/378) preserves the unfinished
composition checkpoint; it is not a merge candidate as a whole. Component
proofs and selected production refinements do not establish complete service
correctness.

## Current base and preserved work

Resumed delivery uses main `e9862433`, including the compiler-read observation
fix in #572 and Verus fork #13. Preserve schema-two compiler observations,
receipt revision 10, archive revision 5, the durable input-dirty markers and
the complete per-file dep-info rule inventory. The earlier umbrella's input
adapters must not overwrite these fixes.

The published umbrella head is `0d49bd48`. Its merge-tree comparison against
`e9862433` has 41 conflicted files. Extract bounded feature deltas onto current
main and review their production callers rather than resolving by accepting
whole historical files. The old local branch named `proof/acceptance-followup`
is not the published PR head; obtain the latter from GitHub before comparison.

## Active delivery slices

| Slice | Production connection | Required qualification |
|---|---|---|
| Parser #543 | Native stream splitter and borrowed lexer use the shared checked operations | Preserve all 54 parser controls; resolve the ordinary-control resource failure; then full required native, conformance and cost gates |
| Deferred reply holding | Preserve the original bound request/reply pair while effort metadata is detached and updated | Whole query root, original held-payload controls, actual pairer ordering/stop/finish regressions, native gates, full fast/standard conformance and bounded cost |
| Dependency filename binding | Check each current decoder proposal against the exact original encoded filename | Whole input root, six ordinary controls, pathological-name and conservative refusal regressions, native gates and full fast/standard conformance |

These are source work in progress, not passing qualification results. Avoid
new runtime allocation in the pairer's release path merely to reuse an optional
batch helper. Keep reason-unknown/statistics publication order as implemented;
the native stream is not necessarily a simple source-order FIFO.

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
failed and unrun evidence separate. Leave all PRs draft until their actual
scope and required gates are satisfied.

Main-relative bugs belong in GitHub with precise evidence and duplicate checks.
PR-introduced findings stay in the owning PR or
[findings ledger](design/acceptance-pr-findings.md). Unfinished design work stays
in the design and #368. Jeff merged #557 as an empty-tree change after #572 had already incorporated
its contents. PR #570 remains open pending owner direction; proof delivery
does not authorize its closure.

The remaining service theorem still needs parser/session composition, truthful
cache origin, complete compiler-input closure, filesystem/process observations,
original-request correspondence and every public success surface. This delivery
round finishes bounded existing work; it does not assume those obligations away.
