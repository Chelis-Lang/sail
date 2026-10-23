# 0054: Stable solver names for spec formal parameters

- **Status:** accepted
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

Lower each uniquely named spec formal to a reserved solver name based
on its position in the complete signature. Count unused arguments too. Retain
the original source name and span in VIR for diagnostics and the existing
capture-aware parameter-reference rewrite. Preserve ordinary proof/exec formal
names, nested binders, return names and function identities. Recursive formal
copies retain both the original position and their existing recursion depth.
Special `decreases_by`/`recommends_by` proof bodies are inserted into the spec
function's scope; their matching formals use the same positions. This does not
change ordinary callable proof/exec parameter lowering.

The 2026-10-23 correction includes complete signature size as well as position
in the reserved name, counting unused arguments. A captured macOS HumanEval
mutation exposed a proof-search regression from position-only names in earlier
termination queries; the later proof query itself was unchanged. Signature-size
qualification preserves source-rename stability and separates those contexts.
See [VI-0034](../verus-issues/VI-0034-spec-formal-solver-context.md) and the
[new runtime qualification](../validation/spec-signature-runtime.md). This does
not relax any functional comparison or introduce Sail-side normalization.

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
full qualification only if the targeted gain exceeds variation. The qualified matching platform pins select this change without changing
Sail query handling or any conformance expectation.

The [qualification record](../validation/spec-parameter-names.md) preserves
both the earlier standalone screen and the fresh combined-runtime measurements.
The combined runtime lowers the Capy rename median by 22.63% across three
rotated rounds, with all 18 commands and their input/restoration snapshots
retained. Both fork actors match stock's 1,871 vstd and 675 Capy function records
and diagnostics. Literal solver inputs remain different from stock; this is
not an SMT-byte equality claim. The Anvil rename median falls 59.49% across
three paired rounds, with cold and comment controls nearly unchanged. The later
stock comparison matches all 143 functions, 144 proved backend checks and 45
scope notes in its focused module; the whole project is not qualified by that
partial result. All 4,714 fork source-suite comparisons, the independent complete
final inventory audit and three-run FAST determinism pass; the original enclosing
failures stay recorded. All four DEEP modes match 159 active units and
12,726 functions each, with only the two documented optional Singular skips;
final runtime, source and command/checker guards pass. Both platform archives
and independent review are complete. Current required PR checks gate merging
the matching runtime pins.
