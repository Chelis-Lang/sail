# 0078: Captured acceptance models checked by stock Verus

- **Status:** proposed
- **Date:** 2026-10-24

## Context

The evidence-acceptance design separates query association, effective input
identity, requested coverage and result authority. Verifying a copied model
does not verify the production implementation. The first deliverable needs a
small honest boundary and an independent checker before runtime extraction.

## Decision

Keep four model roots and their shared exact context under `proofs/acceptance/`,
outside Cargo's production dependency graph. Use the existing upstream stock
pin in `corpus/toolchain.toml`, directly with its pinned Z3 and `--no-cheating`.
Generate fixed drivers only from captured Git dependency closures. Bind the
full root/function inventory, positive proof counts, deliberately
broken controls, selected Git blobs and validated runtime to every check.

Check full crates; do not expose flags that narrow scope or skip verification.
Conservatively exclude external source-loading constructs until their complete
closure is captured. The Python checker's failure tests join the ordinary
workspace gate. Direct stock proofs run as a separate fast CI job whose success
is required by CI completion, after native duration qualification. The CI image
retains the validated stock archive for offline runtime provenance checking.

## Consequences

No runtime behavior, solver choice, crate dependency or production authority
changes. The models and their explicit assumptions can be reviewed and checked
independently of Sail. Runtime source-sharing or refinement, cache-origin and
filesystem/process observations remain necessary before claiming a production
acceptance proof. The separate CI proof job is implemented; its native duration, packaging and
hosted qualification are recorded separately from production acceptance.

The [checker guide](../design/acceptance-proof-checker.md) owns commands and
limits. The [design](../design/evidence-acceptance-core.md) owns the composed
theorem and remaining delivery stages.
