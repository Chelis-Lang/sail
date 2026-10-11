# 0033: Exact deferred imported diagnostic locators

- **Status:** proposed; executable candidate qualification pending
- **Date:** 2026-10-11

## Context

[ADR 0030](0030-imported-span-authority.md) proposes a conservative LoadedForeign
authority. Its corrected ordinary candidate passes source tests and strict
checks, but the original qualification fails an exact stock progress-note order
comparison and restores zero preparations. An independent bounded observation
finds the first unavailable imported binding at
`vstd/function.rs:151:20: 151:65 (#0)`: constructor and pre-focus resolver states
are both Delayed. That candidate correctly refuses it. The
[retained evidence](../../engine/verus/candidates/decode-ownership/loaded-import-qualification-failure.json)
does not establish source or product qualification.

Eager stock source loading is not an admissible shortcut: its FileLoader reads
an unbounded String before validation, and SourceMap retains additional text
and indexing allocations until compiler teardown. A successful load ordinarily
creates a local diagnostic file, not the Foreign role required by ADR 0030.

The [closed consumer audit](../../engine/verus/candidates/decode-ownership/deferred-locator-consumer-audit.md)
finds that prepared VIR/SST lowering propagates exact span identity without
resolving imported coordinates or reading deferred source text. Recorded
MessageX diagnostics are replayed through the current stock Reporter and
SpanContext; rendered source snippets and resolver states are not cached.

## Decision

Implement a reviewed **unselected candidate** with a separate DeferredLocator
role. It captures the actual immutable diagnostic recipe, not loaded content:
original stable crate/file/span range, exact original expected hash bytes,
absolute bounded UTF-8 descriptor path, and the uniquely associated current
Foreign metadata identity. Original metadata has no hash-kind tag; preserve its
bytes and bind the Foreign owner's actual typed kind independently, without
claiming the private current loader uses that algorithm.

Only the supported optional interactive ghost callback captures this role.
Capture and pre-focus revalidation require the same actual Delayed descriptor,
exact original authority, complete unique current Foreign association and
checked ranges. No source read, resolver call, hydration or coordinate
construction is introduced. Relative/non-UTF8/oversized paths, missing or
ambiguous owners, changed descriptors, unknown raw owners and initial-to-handoff
Loaded/None/Busy transitions miss.

Move the same admitted owner into the immutable worker snapshot. After handoff
it remains Deferred. Untouched current stock reporting may naturally transition
its own descriptor to Loaded or None; preparation never promotes, resets or
restores that mutable state. Imported decoding still requires the exact current
`(id,data,as_string)` binding and current raw `Arc<()>`. Local raw fields,
hygiene/parent, semantic keys, graph read validation and effects remain exact.

Use `sail-prepared/graph-ledger/import-locator-authority/v6`, with separate
LoadedForeign and DeferredLocator provenance tags. Retain Portable version 3
only while representation is unchanged. Reject intact old-domain artifacts,
including misfiled entries under a new key; never relabel or reseal them.

Extend the existing **single process-wide 8 MiB** family. Pre-admit inline
records, exact path/name capacities, growth/formatting overlap and control
metadata; dispose physical owners before releasing admission. Capture,
revalidation and replay do not create a second family. The conservative
prospective phase maximum remains 500 MiB inside the 512 MiB requirement, not
a process RSS claim. Existing reporter/decode/observer ownership remains charged.

## Consequences

The role can avoid session-coordinate dependence while retaining a current
diagnostic recipe. It does not attest live file bytes, a successful future load
or verified proof. Unsupported descriptors safely fall back to fresh lowering.
The current stock Reporter retains sole authority over loading, expected-hash
validation, offset translation, failure, ordering and rustc emission.

Before a full source build, require an independently reviewed diff and actual
pinned helper controls for no-load capture, exact current bindings, descriptor
and ambiguity negatives, natural post-handoff success/failure, physical/drop
ownership, emissions and old-domain rejection. Then require actual source tests,
strict checks, fresh ordinary vstd/manifest and all original fourteen pipeline
stages, including strict stock stderr, same-input ordered raw SMT/effects and
genuine previous-revision FOCUS under unchanged 600-second bounds. Existing
failures remain separate. This proposed ADR neither selects a source runtime nor
waives real HE/Vest proof, performance or reader acceptance.


## Proposed amendment: original-descriptor-only role

This amendment is reviewed for an unselected file-only implementation; helper
execution and full source/runtime qualification remain pending. The corrected v6 ordinary candidate
passes source controls/strict checks and fresh vstd, but its original fourteen
stages fail at zero hits in revision 0. A separate bounded trace finds a
per-record Delayed refusal in `vstd/function.rs`; its coarse reason does not
distinguish unsupported path from absent Foreign metadata. Both results and
post-run seals are retained in the
[candidate review](../../engine/verus/candidates/deferred-import-locator/dynsend-owner/original-descriptor-role-review.md).

The pinned SpanContext remaining-original loop legitimately creates Delayed
recipes before SourceMap has loaded their Foreign metadata. Propose a distinct
OriginalDescriptorLocator, limited to an exact original metadata filename/hash/
range and the same current Delayed descriptor, with no Foreign raw-hash match
across any current crate. This conservative condition avoids inventing a
CrateNum association when no Foreign owner exists.
Require absolute bounded UTF-8 path identity. Retain original metadata
filename/hash/range once at constructor capture inside the admitted owner;
pre-focus rereads only the mutable current Delayed descriptor and Foreign
namespace against that frozen authority. It does not reread the original table.
Require unchanged descriptor state/path/hash and the same zero-match namespace
condition at handoff. Preserve the existing LoadedForeign and Foreign-associated
DeferredLocator rules and negatives; never promote or relabel one into another.
Current stock Reporter remains the sole source loader, hash checker and offset
translator. The capsule attests only a current recipe, never loaded content, a
loader algorithm, coordinates or a verified result. Exact current raw-unit and
span binding remains mandatory.

A separately tagged v7 preparation domain and unchanged one-family 8 MiB /
prospective 500 MiB phase bounds are required. Original-descriptor filename
association, no-load source APIs, current Reporter/binding, state/namespace/
ambiguity negatives, physical drop and old-domain controls need independent
review and actual execution before a build. All original fourteen stages,
strict raw stock diagnostics and genuine prior-revision FOCUS remain unchanged.
The amendment and candidate remain unselected until executable qualification.

## Proposed amendment: one late authority classification

The OriginalDescriptor candidate's original qualification remains failed.
Its separate bounded trace identifies a recipe that stays Delayed while an
equal-raw-hash Foreign file appears during ordinary frontend work. Early role
classification followed by demotion correctly refuses that change, but precedes
the natural compiler-thread namespace handoff. The trace does not establish the
new Foreign owner's eligibility. See the
[retained cause and candidate](../../engine/verus/candidates/deferred-import-locator/late-handoff/README.md).

Retain immutable bounded original-authority seeds and the exact initial actual
resolver recipe at construction, without an eligible role. Consume those seeds
once after successful HIR-to-VIR/trait/lifetime work, before Sources/current
bindings/keys/workers. Require the current descriptor to equal that initial
recipe, then classify once using the current compiler-owned namespace and all
existing strict role checks. Hold one borrowed SourceMap guard through this
classification, final barrier and publication. Busy, failed, unsupported or
mutated recipes refuse; classification never promotes an earlier failed role.
Duplicate handoff cannot retry a consumed slot. No fresh SST work is required
before lookup, no original metadata is reread and no stock source loader runs.

The completed owner remains immutable. At take/store, exact fresh raw-unit
binding and a borrowed current namespace validate every used imported owner;
late Foreign growth or ambiguity refuses without reclassification. Current
stock diagnostics may naturally load or reject their own recipe, but never
supply cached coordinates, old raw spans or a restored resolver state.

Use the distinct `sail-prepared/graph-ledger/late-import-handoff-authority/v8`
domain with intact v7 rejection. Keep the same single 8 MiB family through seed,
conversion and final ownership; fixed storage, all simultaneous name capacities,
formatting scratch and Arc/control margin are admitted before allocation.
Physical disposal precedes quota release, including failed/unused/cancelled
handoff. Conservative overcharging after seed disposal is permitted; budgets
and the prospective 500 MiB phase maximum do not increase.

The file-only implementation passes forty-three reduced actual pinned API controls,
including twenty-nine unchanged controls. This is not complete source or
runtime qualification. Actual full library/strict checks, new ordinary vstd/
manifest and every original fourteen-stage oracle remain mandatory. This ADR
and runtime stay proposed/unselected until executable qualification.

The subsequent late-handoff qualification exposed a separate parallel-worker
plumbing defect: stock workers have no diagnostic SourceMap, so the new
preparation namespace check refused storage before provenance. The proposed
[v9 preparation-only namespace service](../../engine/verus/candidates/preparation-namespace/README.md)
keeps stock diagnostic routing unchanged and borrows the live map on the compiler
thread through each preparation transaction. Its file-only controls and distinct
ownership arithmetic do not constitute executable runtime qualification.
