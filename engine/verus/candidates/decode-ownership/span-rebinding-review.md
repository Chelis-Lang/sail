# Current-callback span authority review

This is a read-only design audit of the unselected scoped-query candidate
(`scoped-query-private-span.patch`, SHA-256
`1aae80f1cbf3f476d14bb19587f321d5818749a665c847c9b48046f073cc8808`).
It changes no codec, key, comparison, test or selected runtime. The candidate's
previous-revision gate failed after editing task 023 while focusing task 013:
the prepared key matched, but span provenance refused restoration. The precise
first differing span/owner field is still being captured. This audit does not
establish that relocation caused that failure or that a narrower policy is safe.

## Existing authority and consumers

References below are paths within the applied pinned Verus `source/` tree.

| Boundary | Current behavior | Consequence |
|---|---|---|
| `vir/messages.rs:23–85` | `SailSpanKey` is exact `(id, data, as_string)`. Collection rejects two distinct nonempty raw owners for one identity. Decode requires a current binding and clones its raw `Arc`. | Old `SpanData` is never reconstructed from a disk preparation. Missing/ambiguous bindings remain misses. |
| `rust_verify/sail_engine.rs:337–375` | Current bindings come from the fresh pruned crate plus routed seed spans. The bounded decode, exact graph replay and every stored provenance comparison precede effects restoration. | A matching key alone does not authorize reuse. Span validation must finish before any mutation. |
| `rust_verify/spans.rs:251–296` | Local span data packs stable crate ID plus original absolute `lo/hi`; ID is freshly allocated. | Actual local relocation normally also changes the exact identity/key, so changing provenance alone may yield no useful reuse. |
| `rust_verify/spans.rs:210–283` | Imported original positions are mapped through `SpanContext` to the current file range, including hash-checked delayed loading. | Imported global SourceMap coordinates are already translated by the stock diagnostic path. A precise prepared binding must use this real mapping, not a guessed file. |
| `rust_verify/verifier.rs:94,107,2267` | Primary/label rendering and grouped sorting consume current raw spans or the current import resolver. | Rebinding must preserve the current diagnostic locations and ordering, including multi-span behavior. |
| `rust_verify/verifier.rs:2342,2439,2460` | Progress locations also use current raw spans. | Successful wire equality alone cannot prove diagnostic/progress compatibility. |
| `vir/context.rs:219,943` | The private datatype-span seed accessor invalidates active capture. Interpreter/oracle descendants inherit guarded lineage, including clones created before capture. | Registration remains a separate fresh boundary. A new rebinder must not create an untracked lowering read or bypass the clone guard. |

The inspected `ast_to_sst`, poly, trigger and SST-to-AIR paths propagate `Span`
without reading raw Rust `SpanData`. Span IDs and encoded data are not generally
decorative: other fresh frontend/mode/resolution paths use IDs as semantic lookup
keys. This audit proposes no ID or encoded-data normalization. The current
callback remains responsible for Rust type/mode/borrow checks and trait checks.

## Provenance fields and possible relocation

Current local provenance binds raw `lo`, `hi`, hygiene context and optional parent
index, then the sole containing owner tuple `(index, name, start, length, hash,
foreign)`. The SourceOwner `index` and `start` fields are used by this Sail
containment/hash calculation; they are not consumed by the inspected lowering
paths. Their values locate a file within a particular SourceMap. They must still
be checked when selecting a current owner.

A prospective **local** rebinding rule could distinguish relocation from a
changed range only if it establishes a unique identical file owner and equal
file-relative `lo/hi`, with checked bounds. File name, complete source hash,
length and local/foreign classification remain bound. Exact hygiene context and
parent remain bound. Current raw spans must come exclusively from fresh callback
bindings. Moving only `lo`, only `hi`, changing a containing file or altering its
contents remains a refusal. This would be a new equivalence/domain with new
tests; simply removing absolute coordinates from the existing hash is inadequate.

Current **imported** provenance binds the exact two packed words and *every*
foreign owner, including index/start. A foreign file unrelated to that imported
span can therefore invalidate the comparison. Current `SourceOwner` does not
contain the imported stable crate identity or original file range, so it cannot
prove which owner corresponds to those packed words.

A useful imported seam would need an owned, exact mapping captured from the
fresh `SpanContext`: original stable crate/file/range and source hash to the
unique current owner/range. It must preserve the original packed words, file
identity/content, current renderer authority and missing/delayed-load handling.
The mapping must be collected on the proper compiler thread without retaining
`TyCtxt` or a compiler session across edits. Until that complete seam is audited,
the imported path should retain its conservative refusal. Neither keeping only
the packed words nor dropping all foreign owners is justified.

The first mismatch capture should identify Local/Imported raw kind, exact span
identity, primitive raw fields, contributing owner tuples and expected/actual
digests. It must not format `SpanData` on workers; the
[retained worker repro](../../../../corpus/repros/engine-span-data-worker/) covers
that compiler-TLS failure. Diagnostic emission
needs an explicit aggregate bound/key or identity filter: a 1 MiB per-record
limit does not bound repeatedly writing every foreign-owner inventory.

## Required adversarial controls before changing equivalence

Keep the existing primitive-field, source-content, imported-word, memo-cap and
current-binding controls. Add new controls rather than deleting their assertions:

1. Genuine previous-revision FOCUS restoration after the frozen other-module edit;
   a same-invocation FULL restore does not satisfy this requirement.
2. Current-callback raw `Arc` identity after decode, with old and new raw owners
   distinct; every reachable restored SST/effect/message span must be rebound.
3. Unique unchanged owner with a coherent global-coordinate/index relocation;
   changed relative range, source hash/name/length/classification, hygiene or
   parent must still refuse.
4. Missing owner, duplicate indistinguishable owners, overlapping ranges,
   cross-file ranges, boundary/zero-length ambiguity and arithmetic overflow.
5. Imported stable-crate collisions, changed original packed words, incorrect
   original-to-current file mapping, changed relevant foreign contents and
   unresolved/delayed owner lookup. Unrelated-owner relocation can pass only
   under the proven precise dependency mapping.
6. Unknown raw owner types must not be promoted merely because their encoded
   data happens to contain two words.
7. Existing pre-created interpreter/oracle clone escape, destroyed intermediate,
   sibling-isolation and capture-budget failures remain conservative misses.
8. Exact same-input fresh/restored wire **and** effects/grouped diagnostics,
   plus stock verdict/diagnostic checks, stale-line/macro/label cases, and normal
   full compiler completion. Restore fault injections must reach a genuine hit.
9. New-domain rejection of older entries and bounded collection/decode/replay
   ownership within the reviewed aggregate physical preparation budget.

No performance or qualification conclusion follows from this static review.
The selected baseline remains unchanged. Any implementation needs a separately
identified ordinary build and the complete source/real-project acceptance gates.
