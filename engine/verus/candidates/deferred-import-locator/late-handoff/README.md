# Single late compiler-thread import authority handoff

Status: file-only unselected candidate. Forty-three reduced actual pinned API controls
pass (29 unchanged plus 14 new controls). Complete source library tests, strict
checks, ordinary runtime build and qualification have not run for these bytes.
The selected runtime and every earlier sealed candidate remain unchanged.

The original `0bbc` fourteen-stage qualification remains failed: revision zero
verified nine functions with exact stock stderr, but admitted zero preparations.
Its separately granted [bounded diagnostic](../original-descriptor/provenance-diagnostic-result.json)
observed one recipe remain Delayed while equal-raw-hash Foreign metadata grew
from zero files at construction to one before focus. The old OriginalDescriptor
owner correctly refused `ChangedSource`. The trace does not establish that new
file's crate, name, typed hash, stable identity or eligibility. This correction
changes when a role is first classified; it does not accept that observed file
without the existing strict checks.

## Authority and publication

At the original SpanContext constructor, `PendingImportSeeds` retains bounded
immutable original range/hash/filename and an independent initial actual
resolver recipe. It has no provenance API or eligible role. Initial unsupported,
Busy, missing or allocation-refused seeds cannot be reset. Original metadata is
copied once; later mutable descriptors are compared against it, not a resnapshot.

The successful compiler-thread `sail_focus` handoff runs after HIR-to-VIR, trait
and lifetime checks, before Sources inventory, current bindings, keys or workers.
It consumes the slot once and classifies against the actual current namespace.
Fresh SST/poly lowering has not occurred: a preparation lookup still precedes
that work. The exact `tcx.stable_crate_id` path reads existing compiler metadata;
it does not retain TyCtxt, invoke source loading or decode new metadata.

`SpanContextX::sail_with_imports` holds one borrowed SourceMap file guard through
classification, final revalidation and Sources publication. `recipe_unchanged`
returns the actual descriptor mutex guard and keeps it through classification,
rather than cloning or checking a recipe and then rereading it. Failed and
missing slots are consumed without retry; duplicate handoff returns Missing.
All existing LoadedForeign, Foreign-associated Deferred and OriginalDescriptor
path/hash/range/type/crate/uniqueness/zero-Foreign refusals remain mandatory.

After publication the owner is immutable. Take/store hold a fresh borrowed file
guard through exact current-binding collection/decoding and USED-import owner
validation. Captured Foreign index/range/cnum/stable ID/typed hash/name and
uniqueness must still match; an OriginalDescriptor still requires zero matching
Foreign files across all crates. New namespace ambiguity refuses reuse without
reclassifying any role. Names compare by borrowed streaming formatting, without
a new String. Stock diagnostic loading may resolve its own descriptor to Loaded
or None; preparation never restores that state or manufactures coordinates.
Current `(id,data,as_string)` and raw-unit Arc binding remain exact. The untouched
current stock Reporter and resolver remain the sole loader and renderer.

The new preparation domain is
`sail-prepared/graph-ledger/late-import-handoff-authority/v8`. The old current-domain
assertion migrates v7 to v8; its existing negatives remain, and an additional
source-only control rejects an intact v7 artifact. No key, graph, solver or
query normalization is introduced. The optional bounded trace distinguishes
`constructor_resolver_state` from `classification_foreign_hash_matches` and
`authority_phase: late-handoff`; those counts are not constructor observations.
Trace is dormant unless separately requested and never authorizes proof reuse.

## Physical ownership and work

One process-wide 8 MiB admission covers Pending, conversion and Complete.
The fixed record array reserves its largest inline form with checked record
count and byte admission before allocation. A Pending state can coexist with
its final path/name while conversion is on the stack; original and actual recipe
String capacities remain charged while final path/name capacities and bounded
formatting overlap are admitted. The internal byte sum deliberately does not
subtract physically disposed seed names, so conversion can conservatively miss.
There is no second record array or quota family. The existing fixed control
margin covers stack variant replacement, the one Arc header and control values.

Failure and cancellation dispose record/name allocations before family quota
release. Dropping an unconsumed SpanContext slot does the same; the actual
physical-drop pause control proves a racing acquisition still refuses after
physical disposal but before release. SourceMap/TyCtxt/source Arcs are borrowed,
not retained. File count is capped at 4096 before namespace scans. Both constructor
map lengths and total original rows are also capped before per-range searches,
including inconsistent reduced-harness inputs; the immutable imported table
retains its original aggregate bound. Late checks allocate no
new metadata. The prospective phase maximum remains 500 MiB within the 512 MiB
requirement because the same 8 MiB family absorbs all seed/final overlap. This
is explicit ownership arithmetic, not a process RSS measurement or completed
runtime memory qualification.

## Controls and retained scopes

[identity.json](identity.json) binds the full patch, narrow delta, seven source
files, lock and helper artifacts. The full patch is
`engine/verus/candidates/late-import-handoff.patch`; the delta is
[from-original-descriptor.patch](from-original-descriptor.patch). All other
full-patch sections remain byte-exact from the prior complete candidate. Both
routes applied successfully to the six changed pinned-source file snapshots;
whole pinned-source application remains a full-build preflight requirement.

[api-controls.json](api-controls.json) retains the 43-control terminal scope and
all earlier compile failures separately. The harness extracts the unchanged
stock resolver and actual new production handoff method against rustc 1.98.1.
Independent tests ran serially; internal concurrency and original compile/test
bounds (60/30 seconds) stayed unchanged. The fourteen added controls cover:

- Valid late Foreign insertion and exactly one Deferred classification without
  source I/O or a second owner; repeated/missing handoff refuses.
- Initial path/hash/Loaded/None/Busy changes and earlier Budget failure cannot
  be promoted by a newly valid owner.
- Equal-hash Foreign in another crate remains unavailable.
- Pending cancellation and last physical owner disposal retain quota until drop.
- OriginalDescriptor namespace growth and Deferred duplicate-owner growth
  refuse reuse without reclassification.
- The 4097-file/original-row/empty-outer-map guards refuse before scans;
  original metadata stays frozen even
  when the external fixture table changes after capture.
- Untouched actual stock resolver success/failure after handoff preserves the
  immutable Deferred role and fresh raw-unit binding.

The last control exercises the actual resolver and rendered-span API, not full
formatted Reporter stderr or queued diagnostic ordering. The source-only test
file preserves its entire prior module as a byte-exact prefix, then adds these
phase controls. Existing scalar four-method Reporter/ReplayMessages, direct
stock group resolution, raw-binding/hygiene/parent and physical tests remain;
this patch has not yet compiled those complete-source tests. No new actual
sealed-vstd metadata control was executed in this scope; the earlier metadata
result remains independent, not transferred acceptance.

Run the reduced controls with a private output directory:

```sh
python3 engine/verus/candidates/deferred-import-locator/late-handoff/test-api.py \
  --source PINNED_PRIVATE_SOURCE --work-dir PRIVATE_CONTROL_OUTPUT
```

Full actual VIR/driver tests, both all-targets strict checks, fresh ordinary vstd
and a new complete runtime manifest precede the original fourteen-stage oracle.
Its raw stock diagnostics, every same-input checked hit and genuine previous-
revision FOCUS remain required under the original 600-second invocation bound.
No candidate runtime build, qualification retry, hit or performance claim is
part of this helper result. User-facing proof repair remains experimental.
