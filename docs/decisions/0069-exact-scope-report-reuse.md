# 0069: Reuse successful reports only within their exact partial scope

- **Status:** implemented; bounded native qualification passes, final release qualification pending
- **Date:** 2026-10-21

## Decision

An unchanged partial verification may reuse its successful report for the exact
ordered invocation arguments. It never establishes a whole-target proof. Keep
Cargo invocation and every existing receipt check: successful verifier result,
complete observed report, sealed publication, full input revision, environment,
runtime, dependency bytes, artifact identities and contents, and current inputs.
Do not normalize selectors, options or SMT bytes.

Tag such receipts with the complete partial invocation arguments. Ordinary
whole-proof validators refuse every tagged receipt; only the selected target's
exact context and argument list may consume it. Shared dependency proofs and
instant-revert archives refuse tagged receipts. Both fresh and reused partial
results have `complete: false`. Cached partial reports use the additive
`scope_reuse` argument list; existing `reused` and `reused_targets` fields
retain their whole-proof meaning. Explicit force-fresh still runs verification.
Missing, corrupt, stale or mismatched requested-scope reports force fresh
verification. Cargo-Verus focus compiles dependencies without verifying them;
an unverified dependency has no report or proof to require or reuse. Whole
verification retains its existing dependency-report requirement.
The existing focused cleanup regression remains a force-fresh test.

Receipt version 4 records this distinction; older receipts require fresh
establishment. This amends ADR 0023 only to permit exact-scope report reuse,
while its prohibition on partial results becoming whole-target proof receipts
remains intact.

## Consequences

Anvil's unchanged partial call currently takes 39.53 seconds against stock's
0.32 seconds (#314). Avoiding redundant verification addresses the real loss
without presenting unverified functions as verified. The bounded installed correction removes repeated verification: unchanged
Anvil calls take 1.89–2.05 seconds with zero verifier invocations. Stock's
0.287-second baseline still exposes overhead; final paired measurements and
release acceptance remain pending. See [validation](../validation/exact-scope-reuse.md).
