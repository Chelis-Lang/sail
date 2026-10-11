# Fresh preparation provenance refusal: independent review

The original fourteen-stage qualification of the unselected `886b9b83` patch,
`73bd8aa61b18ca12` runtime stopped in revision 0 of the previous-revision-focus
stage. The saved summary contains **zero hits, zero same-callback checks and
zero focused restores**. The assertion text `unchecked actual hit` also covers
this zero-hit case; it does not establish that a nonzero hit escaped checking.
Stock and engine both returned 0, verified 9 functions with no errors, and
passed the exact 234-byte stderr and typed outcome comparisons preceding that
assertion. Revisions 1 and 2 were not reached. This is not cross-edit evidence.

This review used existing records and frozen code only. It launched no compiler,
proof, retry or changed acceptance controller. The selected engine remains
unchanged. The original failure and complete closure belong to the separate
qualification-failure record; this review does not replace them.

## Where admission stopped

The event stream contains four `prepared-unavailable` records with
`stage: provenance`: one during focus and three during full verification.
There are no prepared hits or restores. In the
[frozen source patch](../../deferred-import-locator-dynsend-owner.patch),
`store_bucket` emits this event when provenance for a collected span returns
`None`, before `write_disk` or retained batch-entry publication. Therefore the
failure is fresh preparation admission, rather than demonstrated rejection of
an existing prepared entry by decoding or graph replay.

The collection serializes SST, the complete `SailEffectsView`, and recorded
lowering messages. In particular, the effect view contains the span-bearing
`func_sst_map`; a rejected binding need not belong to the selected function.
Graph-read frames at this point are already encoded byte vectors, not Span
objects newly visited by this collection. Successful initial serialization
and span collection precede the reported provenance refusal.

## Span forms and refusal boundaries

| Current raw owner | Provenance path | Safe refusal or support |
| --- | --- | --- |
| `SpanData`, including crate `air_no_span` with empty `data` | Local raw fields | Binds lo/hi, hygiene and parent; nonzero ranges require exactly one containing current SourceOwner. |
| DUMMY `SpanData`, lo=hi=0 | Local raw fields | Still binds hygiene and parent; bypasses source-file containment. Empty packed data does not turn it into an import. |
| Unit `()`, exactly two packed words | Imported authority | Requires successful complete capture, unique original range, and supported LoadedForeign or DeferredLocator role. |
| Unit `()`, empty/malformed packed data | Unsupported representation | Refuses before the imported-state trace. AIR parser text labels can have this form. Their presence in this failed collection is not established. |
| Any other raw type | Unsupported owner | Refuses; having two packed words is insufficient authority. |

The stock `air_no_span` initializer uses the crate HIR inner span and
`to_raw_span`, which stores actual `SpanData`; it is not a unit/empty-data
import. The ordinary VIR imported span decoder produces the unit owner.

The [owned mapping helper](../capture.rs) has two distinct refusal scopes.
A whole-capture error is retained as the Sources import `Err`, so every imported
span refuses even if its own descriptor would otherwise be valid. Missing or
ambiguous original hash/range authority, invalid/current overlapping ranges,
wrong loaded role/hash/crate, duplicate content ownership and budget/allocation
failures can take this route. A Delayed descriptor with unsupported path bytes
or no matching current Foreign file instead becomes an unavailable individual
record. The pre-focus barrier can also invalidate a previously supported record
when resolver state, descriptor or current source identity changes. These
checks can refuse the very first unchanged callback; an edit is not required.
None is identified as the actual cause by the saved four stage-only events.

## Existing controls and remaining evidence

The actual source controls cover long local spans on workers, non-root hygiene,
parent changes, current owner/position changes, malformed imported words,
unknown raw owners, exact current-Arc decoding, and single-file Deferred recipe
and Reporter replay cases. The single-file controls do not establish admission
of the complete actual vstd descriptor inventory. Focused missing controls are
DUMMY and crate no-location spans in a complete portable collection, unit text
labels, and a composite mapping with a valid selected descriptor plus an
unrelated unavailable or globally ambiguous descriptor. Any added controls
must retain the original negative assertions and distinguish family-wide
refusal from per-record refusal.

No opt-in import-state trace was recorded by the original controller. Its
bounded dormant trace only runs after a valid imported provenance key and only
in focus. Thus an eligible trace record is not automatically the first refused
binding, and no trace can also mean Local or unsupported raw/data refusal.
Before changing authority, correlate a trace with the actual refused binding
or retain that attribution as unavailable. Do not infer a Deferred-specific
cause from the coarse event or the approximately 10 MiB retained peak.

The qualification assertions, original 600-second bounds, full diagnostic
comparison, same-input wire/effect oracle and previous-revision focused-hit
requirement remain unchanged. No source authority narrowing, budget change,
normalization, activation or performance qualification follows from this audit.

## Retained record identities

- Patch: `886b9b830eda4a4128fa861557427cbeabae92c170988b123d3bcb271749f508`.
- Stage summary: `6f5d44faafd4d8779df25f331c67dee3f5f1475f6328271f07e80782acdfa06d`.
- Original event stream: `68e0ea031a3f7dc9db92033e00d793f4e2bd36066a3602bbd19c1ccc8c5aaa0f`, 14,624 bytes.
- Saved engine verifier identity: `f05539c5753153f1d46b773645bb3f9a73e0091fab53dbb0598a16ab6e740dd7`.

These are scope-specific saved identities, not a new executable qualification.
