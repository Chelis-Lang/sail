# Trace-gated owned import implementation handoff

This narrows the [timing/domain design](timing-and-domain.md) into an implementation
sequence. It is **not selected or applied**. The f833 trace must identify the
first real failed span and corresponding stored/current authority. The inert
helper's API tests cannot establish that cause. No preparation comparison changes
until that evidence is reviewed.

## Decide from the measured mismatch

| Actual first delta | Narrow action |
|---|---|
| Local SpanData lo/hi, hygiene/parent, range text or containing source content | Keep exact local provenance and miss. The imported mapping proposal does not apply. Do not reclassify local data as an import merely because its serialized data has two words. |
| Imported raw unit, exact SailSpanKey unchanged; only an unrelated foreign SourceOwner differs | Test a new-domain dependency on the uniquely authoritative selected import owner. Keep every field of that selected owner exact initially, including current index/start. This is the smallest useful change if the selected owner itself is unchanged. |
| Imported raw unit; selected Loaded Foreign owner has changed current index/start but original words/hash/range and complete stable file identity remain unchanged | Separate relocation policy review is required. Prove exact checked original-to-current translation and diagnostic authority; do not delete fields as part of the unrelated-owner change. |
| Imported Delayed, None, Busy, or locally loaded fallback | Initial Loaded-Foreign helper does not apply. Fresh lowering remains the conservative product behaviour. Descriptor-only reuse requires its separate state/loader/report-side-effect proof. |
| Changed selected owner hash/name/normalized range/stable ID/namespace, missing original metadata, duplicate content/ranges or unknown raw owner | Refuse preparation. These are not representational relocation evidence. |
| Trace incomplete or missing retained prior-owner evidence | Preserve failure and improve bounded diagnostic capture; no policy inference from digest inequality alone. |

For an unrelated-owner case, preserve raw first mismatch records and match their
exact SailSpanKey to the stored binding. Establish whether the current SpanContext
is Loaded, Delayed or None for that original crate/range; do not infer it from
another foreign file. Confirm the selected source file independently using the
original metadata and current namespace/hash/range checks. Record every selected
field before describing any field as unchanged.

## First integration, only if the first case requiring it is proved

1. Add the private owned helper under `rust_verify/src/spans.rs`. At the end of
   `SpanContextX::new`, original metadata is still borrowed. Capture only existing
   Loaded Foreign candidates using immediate `tcx.stable_crate_id(cnum)` lookup.
   No `resolve`, `load_file`, `Span::span` or SpanData Debug call is added.
2. Retain one admitted immutable owned mapping, with no compiler borrow or
   SourceFile Arc. Add a private read-only accessor to SpanContext; no generic
   access to its mutable imported resolver map. Disabled/overbudget/unknown
   capture returns preparation unavailable while ordinary verification continues.
3. At the compiler-thread boundary immediately before `sail_focus` freezes
   Sources, revalidate captured candidates against the current complete file
   table and stable crate identities. Added ambiguity declines capture. Do not
   promote constructor Delayed/None/Busy entries. This late check reads stock
   state without performing stock I/O. The current helper must be extended for
   this explicit revalidation; repeating its constructor call would require the
   original metadata that has already been consumed.
4. Attach the same immutable owned admission to Sources for focus/full workers.
   Do not retain SpanContext itself in Sources or create a reference cycle.
   Charge any Arc header/metadata and final-owner lifetime; `finish_cache` alone
   must not release charge while callback/worker owners remain alive. Do not
   create a second snapshot while the first admission is live.
5. Extend imported provenance only for an exact raw unit owner and two original
   words. Lookup the unique original range, validate bounds, then hash the
   **selected authoritative owner** in the new domain. For the unrelated-owner
   case retain current index/start too. Unknown/non-unit/import-unavailable
   branches miss; local provenance and memo inputs remain exact. Update the memo
   key or prove it uniquely binds the immutable mapping within this Sources
   owner; clearing at callback completion remains mandatory.
6. Keep `current_bindings` and `sail_resolve_spans` unchanged: exact
   `(AstId, data, as_string)` lookup gives the current raw Arc. No old raw location
   is restored, and unknown/ambiguous current bindings still fail before effects.
7. Change the explicit preparation dependency domain in `vir/context.rs`, which
   is serialized in both baseline and final bucket-key input. Keep Portable
   version 3 if its representation is unchanged. The source/runtime manifest
   naturally changes too; do not rely only on callers selecting a different
   external context. No migration relabelling or old-entry resealing is allowed.

The selected-owner design cannot be claimed to fix coordinate relocation or
Delayed states. If the measured case is either, stop at the relevant row above.
The function/bucket/SCC order and every other exact lowering dependency remain
unchanged.

## Exact migration and qualification controls

The existing `test-private-span-migration.py` is reusable with genuine old/new
compiled actors and the same caller context. It must preserve old files byte for
byte, reject an intact correctly checksummed old payload deliberately misfiled
under new keys, and require a genuine new-domain warm FOCUS restore. Add a
same-process domain-key control so batch entries cannot bypass the explicit
domain. Do not bump the decoder representation solely to make migration pass.

The first source-control matrix must retain all local hygiene/parent, imported
word, exact as_string/current raw-Arc, missing/ambiguous owner and old-domain
negatives. Add the actual unrelated-owner regression only after its cause is
known: mutate the unrelated foreign table while preserving the selected complete
authority, then require selected import lookup and same-input diagnostic/wire
comparison. Independently mutate every selected field and require a miss. A
future relocation case needs its own shifted-byte-position/source-table tests and
exact current source-line/macros/multispan diagnostics; it is not covered by an
unrelated-owner positive.

Run actual driver/VIR and strict source checks, ordinary vstd build, original
same-input valid-HIT positives and body/order/effects negatives, source failure
and late-EXEC cases, cancel/crash/normal-termination controls, exact stock
verdict/diagnostics and the original 600-second prior-revision test. The
previous-revision **FOCUS** hit must be the first restore after the task023 edit;
a same-invocation FULL restore is insufficient. Complete HE/Vest catalogue and
whole-loop performance gates follow only after these pass. Retain every failed
scope and original runtime; no selected artifact changes before review.

Budget acceptance is separate: one 8 MiB admitted mapping is prospective against
the prior 492 MiB phase maximum. Seeds/revalidation buffers/Arc wrappers/memo and
serialization must be accounted concurrently, with physical final-owner disposal
before quota release. No new ordinary runtime build should begin with a known
aggregate-owner gap. The pinned Unix formatter audit covers only its exact
streaming Debug chain; no arbitrary Debug implementation receives that exemption.
