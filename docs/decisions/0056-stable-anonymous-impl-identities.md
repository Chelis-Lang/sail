# 0056: Stable identities for concrete trait implementations

- **Status:** proposed; source-only prototype, uncompiled and unselected
- **Date:** 2026-10-18

## Context

[VI-0030](../verus-issues/VI-0030-anonymous-impl-numbering.md) records two actual
accepted-e536 Vest calls differing only by an empty impl insertion. Twenty-three
method paths change and 30 exact solver-input sessions differ, including untouched
CBOR modules. Rustc's sibling impl disambiguator also enters the enclosing
method's closure DefPathHash, so rendering methods differently alone is incomplete.

[ADR 0039](0039-private-verus-fork.md) and
[ADR 0049](0049-stable-closure-names.md) do not already authorize this new impl
symbol renaming. A consistent symbol renaming preserves logical structure but
can change solver hashing and resource use; matching verdict/diagnostic and
performance evidence must precede selection.

## Decision

Prototype only positive, non-marker trait implementations of a zero-argument
nominal Self ADT where Self is the trait reference's only argument. Inherent
impls retain legacy identity. A coherent ground trait reference identifies one
positive implementation; marker and reservation implementations are excluded
because rustc allows overlap. Custom HIR checking completes coherence before
new names are enabled, and ordinary native metadata compilation also checks
coherence. Failed HIR/coherence checking follows the original diagnostic path.

Encode full crate-qualified parent module, Self ADT and trait DefKey paths.
Use no associated-member set or peer inventory. Prototype01 failed independent
static review because native ghost erasure removes private/restricted members
and changes that inventory; its sealed, uncompiled source remains retained.
Unused impl lifetimes, ground predicates, safety, constness and defaultness do
not select another implementation of the same concrete trait reference and
are not eligibility switches or key fields. Generic Self/trait arguments,
unsupported/nested/ambiguous paths, negative impls and over-limit keys remain
legacy. No general Rust type encoder or new exported mapping is introduced.

Use injective full-key hex names `impl&%s2<hex>` in a reserved namespace
disjoint from existing numeric impl names. A 512-byte impl key and 1,024-byte
closure key are explicit bounds; never truncate or partially hash a key.
Members, bodies, spans, source/global order and diagnostic positions do not
enter the key. Adding a member no longer changes the block key. Separate
inherent blocks, including duplicate empty blocks, intentionally retain their
original edit sensitivity.

Producer/native eligibility under arbitrary cfg-driven semantic header or named
hygiene changes remains an explicit source-review obligation before any build;
the narrower member-free rule removes known ghost-member erasure dependence,
not that broader proof requirement. Baseline-valid imports must not gain errors
or silently mix legacy and stable exports. No native inventory mismatch error
is treated as a valid default-mode fix.

Closure identity under an eligible impl uses the same complete parent key and
its existing relative DefKey suffix. All names, equality, hashing and exported
VIR retain the complete selected identity. Closure-local insertion or reordering
remains outside the stability promise. Declaration order retains the old index,
with full identity breaking imported equal-index ties. Other closures retain
both original crate-qualified DefPathHash halves. A matching compiler, vstd and
exports must be built together. A fixed format/version header is checked
before bincode decoding and written before encoding; old, missing, truncated or
mismatched exports request a complete rebuild. Stock/mixed exports are not
accepted. Any future identity/wire policy change must bump this version.

Activate extra semantic queries only after Verus's custom HIR check succeeds.
Refresh pre-check external-item paths and both item views without repeating
attribute classification or diagnostics. Share one per-compiler registry with
all path conversions, including autospec's diagnostic-rename bypass. Do not add
a process-global cache or cache hashes on mutable public PathX values.

The independent paired long-log filename helper from fork `470258b0` is included
with a spinoff-suffix correction: shortening retains the genuine `._NN[_expand]`
index outside the complete-stem digest so Sail preserves obligation identity.
AIR/transcript stems stay paired and bounded. Full encoded function names can
exceed a filesystem component limit; SMT/query contents stay literal.
No SCC change is part of this prototype. No new dependency is introduced.

## Consequences

Names and exported identities become longer, and ClosureId requires shared key
storage instead of Copy. Per-impl queries, retained keys and longer symbols can
cost CPU, memory and SMT bytes; no gain is established. Family-wide foreign
scans have been removed. Inherent and unsupported headers remain edit-sensitive.
The actual witness includes 21 Clone paths and two proof-method paths among its
23 changes; the latter headers and all 30 changed sessions still require matched
candidate classification. This source cannot establish which churn is fixed.

All new units and compiler fixtures remain uncompiled and unrun. Actual
producer/native-rlib/consumer controls cover private/restricted ghost members,
ghost-only and duplicate inherent blocks, unused lifetimes and ground predicates,
full exported/callee path equality, and over-limit legacy policy. Other controls
cover trait method/closure insertion stability, exact complete literal inputs,
inherent/generic/trait-arg legacy scope, complete failed-proof diagnostics,
coherence errors before activation and containing-module/item-map parity. Full
closure key hash/equality/name/serialization/order units and paired spinoff
filename/Sail attribution controls are also unrun. No old assertion has been
weakened to produce an acceptance result.

Independent source review must resolve the header eligibility/coherence boundary
before the first compiler gate. Selection additionally requires a matching
runtime/vstd build, full fork source suite, whole-corpus per-function verdict and
diagnostic conformance, determinism and matched edit-loop measurement retaining
all samples and exact solver work. Complete actual imported closure-type coverage
remains necessary. Sail continues to key exact input bytes; no solver-input
normalization is proposed. The reduced standalone closure fixture is also unrun.
