# 0054: Stable solver names for spec formal parameters

- **Status:** proposed
- **Date:** 2026-10-17

## Context

Renaming a spec function's formal parameter changes the bound variable names in
its definition axioms. Those axioms enter otherwise unchanged callers' solver
inputs. Corrected historical CapybaraKV and Anvil captures show this independently
of the earlier helper-insertion captures: 26 of 111 and 39 of 45 query buckets
respectively differ. The counts of checks after divergence are 208 and 135;
these are not counts of actual solver executions. See
[VI-0023](../verus-issues/VI-0023-spec-parameter-names.md).

[ADR 0039](0039-private-verus-fork.md) permits fork fixes under strict
qualification. [ADR 0049](0049-stable-closure-names.md) covers closure identities,
not this additional binder renaming. Neither historical traces nor source review
establishes a useful gain on the current runtime.

## Decision

Propose lowering each uniquely named spec formal to a reserved solver name based
on its position in the complete signature. Count unused arguments too. Retain
the original source name and span in VIR for diagnostics and the existing
capture-aware parameter-reference rewrite. Preserve ordinary proof/exec formal
names, nested binders, return names and function identities. Recursive formal
copies retain both the original position and their existing recursion depth.
Special `decreases_by`/`recommends_by` proof bodies are inserted into the spec
function's scope; their matching formals use the same positions. This does not
change ordinary callable proof/exec parameter lowering.

Append the new VIR disambiguation variants. Build a matching compiler, vstd,
exports and runtime; do not mix their serialized artifacts with older runtimes.
No Sail query normalization or cache-key change is proposed.

## Consequences

The intended effect is alpha-renaming definition-axiom binders while preserving
logical structure. Selection requires actual counterexample/verdict/diagnostic
comparisons, alias/capture regressions (including recursive copies, shadowing,
unused arguments and generic trait-default callers), determinism and whole-corpus
qualification. All literal solver inputs remain retained; a source rename must
not change an untouched caller's input except log-only source comments.

First measure a narrow paired native screen against genuine cc7 with profiling
off and identical default macro/library origins. Include cold cost and warm
forced-frontend controls, cache observations and every failed sample. Proceed to
full qualification only if the targeted gain exceeds variation. This proposal
changes no selected pin, default or conformance expectation.

The [targeted native screen](../validation/spec-parameter-names.md) now observes
a 24.61% Capy rename median saving across all 18 rows. Its enclosing correctness
matrix fails in an unrelated, mismatched Cargo feature setup before verification;
that failure remains visible. The proposal is not selected, and the remaining
stock comparison and wider qualification gates still apply.
