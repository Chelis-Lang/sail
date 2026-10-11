# Original-descriptor diagnostic locator candidate

Status: file-only implementation and controls for independent review. The
selected source runtime and frozen 886b/73bd candidate remain unchanged.
Actual pinned API controls pass 29/29 on the host and 30/30 on Linux, including
the actual sealed-vstd metadata fixture. Full source controls/strict checks,
runtime build, proof and qualification remain pending. The original
fourteen-stage qualification failure and its separate
bounded trace remain failed and diagnostic evidence respectively.

The [reviewed design](../dynsend-owner/original-descriptor-role-review.md) and
[proposed ADR 0033](../../../../../docs/decisions/0033-deferred-import-diagnostic-locators.md)
add a distinct `OriginalDescriptorLocator` role. It stores the exact original
metadata filename/hash/range once at construction. Handoff rereads only the
mutable current Delayed descriptor and Foreign namespace against that frozen
original authority. A zero matching Foreign raw hash across all current crates
is required at both barriers; no absent CrateNum, hash kind, current owner,
coordinate or loaded-content claim is fabricated. Existing LoadedForeign and
Foreign-associated DeferredLocator policies and refusals stay intact.

The role uses a distinct imported provenance tag and v7 preparation domain.
Current `(id,data,as_string)`/raw-unit rebinding and untouched current stock
Reporter resolution remain mandatory. Early Loaded/None/busy or changed
path/hash, new Foreign association, unsupported paths and original filename
mismatch refuse it. Diagnostic fields now distinguish path/original-filename
refusal and actual Foreign raw-hash counts; observation remains bounded and
cannot change proof diagnostics or eligibility.

The same process-wide 8 MiB owner admits enlarged fixed Record capacity before
allocation, plus exact path capacity before copying; it disposes all physical
owners before releasing quota. No second family, source loading or proportional
new serialization buffer is introduced. The reviewed prospective phase maximum
remains 500 MiB within 512 MiB. This is preparation ownership arithmetic, not a
process RSS measurement or completed runtime physical-budget qualification.

`from-886b.patch` is the file-only production/test delta;
`engine/verus/candidates/original-descriptor-locator.patch` is the complete
unselected patch from the pinned commit. `identity.json` binds
its bytes and source files. The old rust_verify and API helper test modules are
byte-exact prefixes of the extended modules. The existing vir/context assertion
of the current domain intentionally changes v6 to v7; its original negatives
remain, with a new intact-v6 identity negative. No check is removed or weakened.

`test-api.py` retains actual pinned SourceMap/resolver controls. Added controls
cover original filename absence/mismatch, early descriptor changes, zero-Foreign
namespace at both barriers, path refusal, sole current stock load success/
failure/repeat and physical disposal versus a racing acquisition. Source-only
controls additionally exercise distinct production provenance tags and actual
Reporter/ReplayMessages diagnostic fields. The Reporter scope is scalar four
methods/levels/primary/labels plus direct stock grouped calls in both arms;
it does not establish complete formatted stderr, queued ordering, or prepared
group replay equivalence. The original full raw diagnostic oracle remains required.

`metadata-input.py` bounds the complete sealed vstd input to 128 MiB, suffix to
8 MiB, candidate work to 64, records to 4096, hash bytes to 32 and UTF-8 paths
to 4096 before allocation in the actual serde decoder. It requires the exact
sealed SHA, one full-consumption metadata suffix and the retained selected
original hash/range. Four Python malformed-length/schema controls pass. This
preflight alone is neither a generic VIR decoder nor compiler authority.

With `--vstd-metadata` and exact pinned `--deps`, the harness preserves the
actual `CrateMetadata` and `FileStartEndPos` derives, decodes that bounded suffix
with pinned bincode_next, and injects the unchanged stock remaining-original
constructor loop. The control then checks the actual selected original recipe
in an **empty current Foreign namespace**, with no FileLoader reads. Retain
input/suffix/selected-filename SHA and the vstd seal before and after. This may
establish a legal recipe from actual metadata; it does not establish the exact
historical failed callback's namespace or infer its path from display text.
The actual metadata/type control passed: 145 original records, a 22,312-byte
fully consumed suffix, and the selected 121-byte absolute UTF-8/NUL-free original
filename. Its path is retained by SHA, never guessed from display spelling.
[api-controls.json](api-controls.json) binds input/suffix/filename and actual
compiler/library/fingerprint hashes; before/post sealed vstd SHA is equal.
The selected actual metadata recipe is eligible with zero loader reads in this
controlled empty Foreign namespace. The historical namespace remains unknown.

Two pre-rustc setup failures remain recorded: raw span crate-id words initially
exhausted the candidate cap; fixed bounded metadata-header nomination preserves
all limits. The selector then incorrectly assumed identical Cargo profiles
across packages; actual root/transitive dependency fingerprint IDs now select
the compatible libraries and record each distinct profile/features without
guessing or trial pairs. These corrections change only the diagnostic harness.
The successful preflight scanned 34,584 raw markers and nominated one candidate.

Current imported HashMap/Vec work is bounded by the original constructor
aggregate: `SpanContextX` writes/builds/sorts this private table only before
capture and Arc publication, then revalidation borrows the same immutable
table. Only its ExternSourceInfo mutex values can change after publication.
SourceMap can grow independently, so the new 4096-file guard precedes any
current descriptor/Foreign scan and has a real 4097-file negative control.

Before a new full source build, require independent review and actual pinned
helper/metadata/source controls. Source strict checks, fresh ordinary vstd,
complete new runtime manifest and the original fourteen-stage/600-second
qualification—including strict stock stderr, same-input oracle and genuine
previous-revision FOCUS—remain mandatory. The candidate is unselected.
