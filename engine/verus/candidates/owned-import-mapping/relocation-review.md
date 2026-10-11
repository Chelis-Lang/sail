# Measured import relocation and the current diagnostic boundary

The f833 diagnostic has now established a real **Imported** mismatch. It has not
yet established the selected stock resolver's Loaded/Delayed state. The original
strict progress-note failure and c05 cross-edit failure remain retained. No
selected source, runtime or provenance policy is changed here.

## Actual evidence and its limits

Revision2's first failed current binding is id563, imported words
`[2208441662476345389,316229852143522]`, exact spelling
`vstd/arithmetic/div_mod.rs:519:50:519:56 (#0)`. Its original packed byte range is
73628–73634. Prior expected provenance is
`0849384a493a9a1bf5c7291b96da57c142e578edb3edc00340041ec687b95967`;
current provenance is
`80d85bb04aad4274130455992cfc1a6cf584066170557b8944876b4dd8de6be6`.
B's retained complete delta recomputes all eleven available prior Imported
stores and those two first-mismatch digests. All 454 contributing owners preserve
index/name/length/hash/foreign; only starts change, uniformly by25. The selected
foreign table entry212 moves1502672→1502697, length51086 and source hash unchanged.
This is coordinate relocation, not the unrelated-owner row in the earlier
[handoff](implementation-handoff.md). Source mapping state cannot be inferred
from a display name or an equal-hash foreign table entry.

The additional supplementary API controls use actual SourceMap/Span/Hash types,
the exact extracted VIR Span declaration and unchanged stock `from_raw_span`,
`from_air_span` and `unpack_span`. A real Loaded foreign span shifts25 bytes with
the same file index/hash/stable ID/name and original import words, while current
stock rendering stays exact. A distinct Delayed control already has an equal-hash
Foreign source file but demonstrates that stock still loads the descriptor's
Real file and uses that new range. Initial capture remains Delayed and refuses
promotion. These are API controls, not cache hits or complete proof/diagnostic
qualification. Their first compile failure (trait-object `.is` without the pinned
explicit Any cast) is retained.

## What the current raw binding does and does not authorize

`vir/messages.rs:73–85` requires the exact current `(AstId,data,as_string)` key and
copies its current raw Arc into decoded spans. A genuine imported unit Arc
contains no byte-position authority. It is a type discriminator; the stock
current SpanContext uses the original packed words to select its own resolver
entry. Loaded entry bounds or the exact Delayed descriptor then supply the actual
rendering authority. Unknown raw owners remain unsupported. Root hygiene and
no parent are created by stock import translation, never taken from old SpanData.

Consequently current raw binding prevents old raw locations from escaping, but
cannot replace original hash/range/namespace authority. A coordinate-relocation
candidate must prove the original range→current unique owner→current location
chain without calling the loader. Supporting Delayed requires the separate
state/configuration/side-effect argument in the [timing review](timing-and-domain.md).
No evidence here approves it.

## Lowering and effects consumer audit

The bounded preparation region is fresh Ctx construction followed by
`ast_to_sst_krate`/elaboration/poly. VIR exposes no SourceMap or SpanContext there.
A complete VIR `raw_span` search finds its serialization/rebinding and tests,
not lowering access to foreign SourceFile coordinates. `Span.id` is semantic:
mode/resolution/type-invariant work uses it, and full typed current input binding
must remain. `as_string` is also preserved for printers/debug/fallback diagnostics.

| Prepared component | Exact source dependency preserved |
|---|---|
| SST functions and datatypes | Complete typed IR and span id/data/as_string; raw Arc resolved from current bindings. `poly.rs:1387/1421` transforms/clones typed datatypes, not SourceMap coordinates. |
| NameCtxt export | `def.rs:332–367` name counters and stable-id string map remain exact; no SpanContext/SourceFile lookup. |
| Ordered chosen triggers | `triggers.rs:521–537` clones typed spans and ordered triggers/module data; current span binding is mandatory. |
| Qid map/counts | `sst_to_air.rs:870–905` uses exact friendly function name/count and typed span/trigger clones; neither foreign start nor a resolver state is read. |
| Ensure-predicate map | Function-keyed Boolean state remains fully bound/replayed. No source-file coordinates are consulted. |
| String/byte-string hash maps | Exact typed value/hash/collision state remains bound; no resolver lookup supplies these contents. |
| Graph answers and fresh trait/mode/borrow checks | Existing exact ledger and private unsupported-read boundary remain; no bypass based on imported raw type. |
| Replayed diagnostics | Complete message graph, grouped/immediate emission semantics and current raw bindings remain required. Stock Reporter reads current SpanContext and can cause ordinary lazy source I/O. |

Outside that region, rust_verify Reporter and trait checking can resolve imported
spans; local erasure/debugger/progress sorting uses local raw SpanData. Those
fresh frontend/late-EXEC and reporting paths must remain unchanged. The absence
of a VIR coordinate read does not prove diagnostic equivalence by itself.

## Concrete conservative candidate for review

Capture bounded original range/hash authority at construction, then validate the
current Loaded **Foreign** owner on the compiler thread at pre-focus. Retain one
immutable callback-owned mapping under its8MiB admission. Delayed/None/Busy,
locally loaded fallback, ambiguous original/current/hash owner or unknown raw
type is unavailable. A new provenance domain may encode exact original crate and
packed words, validated original range, complete current typed hash, stable file
ID/name and normalized/unnormalized lengths, while recording/checking the current
mapping bounds for translation. Current file index/start would then be mapping
coordinates, rather than cross-callback semantic equality fields. The exact
current SailSpanKey/raw Arc remains mandatory; local provenance is unchanged.
This policy needs root review before application.

Do not merely omit starts from the old global inventory. Keep every other exact
preparation component and all original content/identity/state negatives. The
mapping must validate checked offsets/bounds, strict unknown-type refusal and
current rendering. Do not globally disable capture for an unrelated unsupported
file: either record its explicit unavailable state without allocating outside
admission, or conservatively miss all preparation and report that limitation.
That choice must not select an ambiguous owner for a supported record.

Use an explicit new preparation-domain key with old-domain intact-misfile/batch
rejection controls. Codec stays unchanged if only provenance digest input changes.
The known8MiB mapping must include records, original authority and revalidation
metadata/Arc ownership; no uncharged duplicate snapshot. Integrate its retained
charge into telemetry and all preparation phase maxima before an ordinary build.
Actual selected state tracing should be bounded/read-only and cannot load files.

Mandatory negatives include changed original words/range, hash algorithm/bytes,
name/stable ID/normalization, namespace, duplicate-content/range ambiguity,
missing metadata, Delayed despite existing Foreign, current raw type/Arc/as_string,
and local hygiene/parent. Positive relocation must preserve current stock lines
and multispans. Then require driver/VIR/strict checks, ordinary vstd, exact same-input
wire/effects/grouped diagnostics and stock failure/late-EXEC/cancel controls,
original600-second genuine previous-revision FOCUS gate, complete HE/Vest proof
and performance catalogues. No qualification or2× claim follows from this review.
