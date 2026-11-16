# 0170: No persistent frontend restoration in v0.3.0

- **Status:** accepted
- **Date:** 2026-11-16

## Context

The v0.3.0 plan requires a bounded go/no-go decision for
[#162](https://github.com/Chelis-Lang/sail/issues/162), subject to
[ADR 0052](0052-warm-frontend-reuse.md). A persistent frontend cache must first
have a complete operation boundary and current-source diagnostic mapping.
Only then can a native key/restore experiment establish whether unchanged
work saves at least 25% of the median edit cycle after its own costs.

The [operation audit](../design/frontend-lowering-boundary.md) identifies
concrete missing inputs and effects. AST-to-SST lowering consumes mutable
context and global call-graph order, writes the function map and emits
current-source diagnostics. Serialized spans omit opaque rustc span data;
Debug output replaces that data with `ANY`. Neither representation is a
complete restorable compiler artifact.

Those obstacles remain present in the current composed fork `e58c3e51`:
the complete `ast_to_sst_crate.rs`, `messages.rs`, `context.rs` and
`rust_verify/src/spans.rs` files are byte-identical to the audited fork
`47fb3be6`. This is source evidence for the continuing boundary gaps, not
an exhaustive transitive dependency proof or an executed native experiment.

Earlier captures do not close those gaps. Twelve literal AST comparisons
include unchanged-input order differences and position-bearing misses;
the later dependency audit also found their patch setup did not select the
fork-built dependencies. The corrected controller has not run natively.
Those observations must not be converted into qualified keys or net savings.

## Decision

Do not implement or select persistent cross-invocation frontend restoration
for v0.3.0. Its correctness prerequisites are incomplete, so the present
candidate is a no-go before performance acceptance. No new key, location
projection, normalized query, restored diagnostic or skipped lowering is
introduced. Existing fresh lowering remains the execution path.

The 25% threshold is unchanged. This decision does not claim that a new
native key/restore benchmark ran or failed that numerical threshold. The
historical M8.0 result and later unqualified capture results retain their
original scopes and failures.

Keep #162 open for future bounded work. Reconsider this disposition only
with a versioned complete operation-input/effect boundary, exact current
location mapping, shadow comparison against fresh outputs, then native net
whole-call measurements and the conformance required by ADR 0052. A source
hash or a Debug match cannot replace any of those obligations.

## Consequences

The v0.3.0 frontend feasibility work has an explicit disposition and is not
a release dependency. Other cold-path work, including first-call recovery,
proc-macro costs and separately admitted vstd reuse, remains independent.
No verdict contract, conformance check, performance baseline, public command
or runtime selection changes. This decision uses the scoped engineering
delegation recorded in charter appendix entry 18; it grants no publication
or performance-acceptance authority.
