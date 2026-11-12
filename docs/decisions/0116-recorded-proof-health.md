# 0116: Recorded proof health without inferred stability

- **Status:** accepted for implementation; product qualification pending
- **Date:** 2026-11-12

## Context

Schema-v2 reports already retain per-check effort, requested limits, answer
provenance and unknown reasons when attribution succeeds. A whole-function
timing cannot supply missing per-check margin, and one successful run cannot
measure stability. Workspaces require the complete Cargo package/target/kinds
identity, including same-named libraries and binaries. ADR 0009 sets the
existing strict greater-than-half threshold; #719 supplies its overflow-safe
ledger comparison.

## Decision

Add optional schema-v2 function health measurements derived only from original
obligations. Distinguish missing attribution, an explicitly empty inventory,
missing check fields, unlimited limits and finite observed fractions. Rank
resource exhaustion, near-limit checks and exact finite consumption with u128
cross products. Keep actual verdicts, provisional establishment, provenance
and explicitly measured fragility unchanged. No new verdict or evidence tier
is introduced, and no renamed/session replay is inferred.

Ship `sail health`, JSON and MCP `proof_health` over one bounded read-only view
of completed requested targets. Default to 20 functions, accept 1–1000 and
state omissions. Keep package/name/kinds/function identity exact. Retained jobs
respect service currentness; result files preserve only recorded currentness.
An immutable baseline or selected reference job stays a separate view with
its own context, revision and original evidence. No combined score or proof
comparison is inferred across contexts.

Old schema-v2 documents can omit the new health member. Views recompute health
from original checks rather than trusting a possibly stale derived member.
The existing required-field schema test stays intact; new controls separately
establish optional whole-member compatibility, required measurement children
and strict unknown-field rejection. No dependency or layering edge changes.

## Consequences

The first health view needs no retained solver sessions and performs no new
verification. Unattributed functions remain visibly unmeasured. Dependency-only
targets are outside the bounded requested-target view. Backend reports still
retain their original per-target evidence and diagnostics.

Session retention and its shim/fault/conformance gates, renaming and stability
experiments, fragility graduation and full-prefix quantifier profiling remain
separate work. Qualification requires native schema/coverage/boundary controls,
CLI/file/MCP agreement, retained baseline/currentness controls, independent
review, the API factory assembly binding review and applicable repository gates.
No runtime selection or publication follows from accepting this decision.
