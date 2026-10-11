# Proposed single compiler-thread authority handoff

Status: design only. No source change, compiler control, build, proof or new
qualification has been run for this proposal. The `0bbc7d378544f972` runtime,
original fourteen-stage failure and separate trace remain immutable.

## Observed cause and supported conclusion

The [separate diagnostic](provenance-diagnostic-result.json) records Imported
binding 7867 (`vstd/laws_eq.rs:268:13: 273:14 (#1756)`). Construction had zero
Foreign files with the original raw hash; pre-focus had one. Both resolver
observations were Delayed. The existing OriginalDescriptor revalidation refused
this as `ChangedSource`. The record immediately precedes focused fresh-store
provenance refusal. This identifies a premature role-classification boundary;
it does not establish the new Foreign file's crate, typed hash, name, stable
identity, length or eligibility. Other full-bucket refusals are not localized.

## Exact current ordering

References below name files in the sealed pinned source, not selected Sail APIs.

| Boundary | Current source | Authority and work |
| --- | --- | --- |
| Original metadata imported | `rust_verify/src/verifier.rs:3259` | `SpanContextX::new` consumes original crate/file metadata. |
| Early optional classification | `rust_verify/src/spans.rs:207` | Captures complete Loaded/Deferred/OriginalDescriptor role against the then-current SourceMap. |
| Frontend HIR→VIR work | `rust_verify/src/verifier.rs:3272` | VIR construction, trait checks and lifetime checks can still access rustc metadata/source information. |
| Natural compiler-thread handoff | `rust_verify/src/verifier.rs:3364` | `tcx` is still available; `sail_focus` starts only after all successful frontend checks. |
| Source inventory and late demotion | `rust_verify/src/sail_engine.rs:813` | Takes early owner and revalidates it against the later SourceMap before any focus worker. |
| Preparation consumption | `rust_verify/src/sail_engine.rs:524` and `:539` | Current bindings are collected from current VIR/seed spans; take/store use the published source owner. |

No take/store is admitted before this source handoff. Re-running the stock
SpanContext constructor would alter raw span allocation/IDs and repeat stock
work; it is excluded. Retaining the whole original metadata HashMap would
introduce unbounded preparation ownership; it is excluded.

## Proposed ownership and authority split

1. At construction, acquire the same process-wide 8 MiB owner and retain only
   bounded immutable original-authority seeds: exact stable crate ID, original
   range, raw expected hash bytes/length and original filename disposition.
   Separately retain the initial **actual resolver recipe**: exact state,
   Delayed path/hash, or Loaded start/end; Busy/None/unsupported seeds remain
   unavailable. This is an observation, not a current-file/eligible-role claim.
   Seeds have **no eligible role** and cannot satisfy provenance. They are not
   a previously failed `State::Unavailable` that will later be promoted.
2. Retain exact supported original filename bytes under that owner. Missing or
   unsupported original filenames and unsupported actual descriptors have
   explicit dispositions. Both original and actual recipe names/overlap are
   charged before copying, without inferring that they are interchangeable. Do not clone an
   unbounded PathBuf/String, load a source file, retain SourceMap/TyCtxt/source
   Arcs, synthesize a hash algorithm, or infer a filename from display text.
3. Immediately before `sail_focus`, on the same successful compiler callback,
   consume the seed owner once. First require the mutable current resolver
   recipe to equal the admitted initial actual observation. Changed path/hash,
   Loaded/None/Busy transition or changed Loaded coordinates refuses; namespace
   growth alone is not a recipe mutation. Then classify using the actual current
   imported resolver, current SourceMap and current `tcx.stable_crate_id` association.
   All existing Loaded, metadata-associated Deferred, and OriginalDescriptor
   refusal checks remain mandatory. A now-present Foreign file must satisfy
   the complete existing metadata-associated role checks; an unrelated crate,
   duplicate/ambiguous owner or unsupported state remains a miss. OriginalDescriptor
   still requires zero equal-raw-hash Foreign files across **all** crates.
4. Perform the same immediate clean validation barrier and publish the one
   immutable owner with the source inventory before current binding collection,
   keys or worker start. No role changes after publication and no reset/retry of
   an unavailable seed or failed classifier. Constructor metadata is frozen
   once; mutable current descriptor/namespace is checked at the handoff.
5. Current exact `(id, data, as_string)` binding and current raw-unit Arc remain
   mandatory. Stock current Reporter/resolver remains the sole loader/hash/
   translation authority; preparation never invokes it to make an owner eligible.

The role-classification time changes, so this requires an ADR amendment and a
new preparation domain with intact old-domain rejection. It is not permission
to weaken zero-Foreign, path, range, hash, hygiene, parent or raw-unit checks.
Initial/pre-focus trace labels must distinguish constructor **seed** observations
from handoff role classification rather than imply an early authority snapshot.

## Physical admission and work

There remains one 8 MiB family throughout seed retention, classification,
validation and Arc handoff. Prefer a fixed pre-reserved record array sized for
its largest final form, with an unclassified seed variant, then in-place
conversion. Charge fixed capacity before reserve; account actual capacity,
original filename, new current name/path and transient old/new string overlap
before each allocation. Drop originals no longer needed physically before
subtracting any internal charge; family admission remains held until the last
physical owner is disposed. Failure/poison/overflow disposes every seed, name,
partially classified record and scratch before releasing admission.

Do not allocate a second 8 MiB family to convert seeds. If separate seed/final
arrays are used instead, their simultaneous actual capacities and all name
copies must fit the **same** 8 MiB allowance before either allocation. A large
legal metadata set may miss rather than exceed this cap. Count/checked arithmetic
and the current SourceMap 4096-file cap precede every scan; original imported
namespace remains immutable and bounded by its constructor count. No arbitrary
metadata query or new source-loader work may be hidden outside the admission.

The existing conservative 500 MiB preparation phase maximum includes one 8 MiB
mapping family. Keeping seed/final/transient allocations inside that family
leaves the maximum unchanged; exact implementation capacity assertions and
physical-drop controls are required before claiming this. Stock working VIR or
source allocations are not newly copied or relabeled as preparation-exempt.

## Typed file-only implementation plan

No implementation below is authorized or present yet. Proposed changes are
limited to `rust_verify/src/{sail_owned_imports,spans,sail_engine,verifier}.rs`,
their existing source/helper test files and the preparation domain in
`vir/src/context.rs`.

- `PendingImportSeeds` exposes no provenance methods. It owns one family storage
  allocation containing fixed immutable original authority and exact initial
  recipe observations. It contains no `State::Loaded/Deferred/OriginalDescriptor`.
- `SpanContextX` stores `Mutex<Option<Result<PendingImportSeeds, CaptureFailure>>>`.
  Constructor admission failure is retained as Failed, never retried. The slot
  is taken once; duplicate handoff sees Missing. If optional focus is absent,
  frontend returns early or compilation is cancelled, dropping SpanContext
  disposes the pending family physically before releasing its single quota.
- A compiler-thread method consumes Pending into classification, using `tcx`
  while it is still live. Borrowed source metadata never escapes this call.
  Classifier failure drops partial final records under the same owner. It cannot
  put a seed back, reset a failure, or inspect a previously eligible role to
  promote it. Success returns the existing immutable `OwnedImportMappings` shape.
- Prefer one fixed record-array storage plus typed Pending/Complete wrappers;
  ownership of its admission moves without release/reacquisition. Actual and
  original recipe strings may be moved into final role data only after exact
  comparison. Any temporary replacement/formatting/Arc header remains inside
  the same charge. Failed state has no retained uncharged children.
- `verifier.rs` performs the one handoff after successful lifetime checks, before
  `sail_focus`; `sail_engine.rs` publishes its validated result together with
  the source inventory. Existing caller arguments/current spans are unchanged.
  Keep the existing final-role classifier and post-classification revalidation
  helper usable by old controls, so their negatives remain intact.
- New domain and observer fields record construction seed versus handoff role.
  The optional trace remains bounded and does not determine eligibility.

The role-clock change is explicit: initial resolver recipes stay exact, while
current Foreign namespace association is first classified after frontend work.
A Foreign file that appears **after** classification is still governed by the
existing late-barrier negative. The observed pre-classification growth is not
an automatically accepted role; all exact classifier checks still apply.

## Mandatory controls before a new build

- Constructor seed cannot answer provenance, cannot be taken twice and retains
  exact original authority without I/O or role classification.
- Actual SourceMap: absent Foreign at construction, valid same-crate Foreign
  inserted by ordinary work before handoff, then one strict Deferred classification.
  This is an eligibility control, not a promised real callback HIT.
- Wrong crate/hash kind/name/stable ID/length, equal-hash Foreign elsewhere,
  duplicate or overlapping ranges, unsupported/missing filename, changed current
  descriptor/hash and Busy/None refuse as existing checks specify. Initial actual
  recipe mutation is tested independently of the immutable original filename.
  Initial Loaded coordinate changes and initially failed seeds cannot pass by
  selecting a different now-valid current owner.
- An invalid original seed or earlier allocation refusal cannot be reset or
  promoted. Current namespace over 4096 refuses before scans.
- Mutation between handoff classification and publication refuses; immutable
  owner is not reclassified after focus begins. The scope of later stock
  diagnostic source loading remains that of existing recipe roles.
- Arc/header, array/string conversion overlap, allocation failures, last-owner
  drop and process-wide admission contention stay under one 8 MiB owner.
- Existing raw-unit, local span hygiene/parent, source/word/key and intact prior
  domain negatives remain. Actual Reporter/replay diagnostic controls remain.
- Full actual VIR/driver/strict controls, fresh ordinary vstd/runtime seal and
  original fourteen stages/600 seconds, strict stock diagnostics, same-input
  oracle and genuine previous-revision FOCUS remain separate mandatory gates.

## Reviewed API and publication boundaries

Here “frontend lowering” means HIR→VIR plus frontend/trait/lifetime checks.
It does not mean fresh `ast_to_sst`/`poly`: those run only after the attempted
preparation lookup (`verifier.rs:2008–2036`). Waiting for fresh SST would defeat
the restoration path. Current span rebinding and ReplayMessages do not depend
on the seed timing.

Pinned rustc 1.98.1 closes the stable-crate allocation question: `TyCtxt::stable_crate_id`
(`rustc_middle/src/ty/context.rs:1168`) reads the local stored field or delegates
to the already-loaded cstore; `cstore_untracked` (`:1398`) borrows that store.
Its implementation (`rustc_metadata/src/rmeta/decoder/cstore_impl.rs:709`) reads
`get_crate_data(cnum).root.stable_crate_id`; `creader.rs:230` borrows the existing
metadata slot. This exact path is not an external query, metadata decode, source
loader or proportional allocation. It assumes cnum belongs to an already-loaded
crate, as SourceMap Foreign cnum does; unsupported association still refuses.
A filename never substitutes for that compiler-owned identity.

For publication coherence, prefer one borrowed `source_map.files()` read guard
for late classification, source inventory construction and final namespace
barrier before publishing SOURCES. Pass the already-borrowed file slice to
helpers rather than acquiring a nested guard or cloning SourceFile Arcs. This
blocks namespace insertion during that transaction without retaining a guard in
worker state. The current resolver recipe is still checked by its bounded mutex
reads; no resolver lock is retained or written. If a concurrent state mutation
invalidates the final barrier, classify failure is not reset or published. All
name/hash inventory buffers remain under their existing source-family budget.
This is a proposed API shape; actual compiler controls must validate it.


SourceMap can still grow later through untouched stock diagnostic resolution.
The handoff does not assert permanent namespace immutability. Prepared diagnostic
recipes resolve against current stock state and current raw bindings, as before;
no source-coordinate attestation is fabricated. Review whether a late actual
SourceMap mutation can affect an authoritative non-recipe field before reuse,
and retain the existing current-source/owner checks accordingly. No implementation
is authorized until this boundary review closes.
