# Loaded foreign import authority candidate

**Private implementation, proposed and unqualified.** The selected installer,
source patch and runtime pins are unchanged. No compiler or proof was run for
this candidate. Its identity is in [identity.json](identity.json); its complete
patch is [loaded-foreign-authority.patch](../loaded-foreign-authority.patch).

The f833 diagnostic established that all454 foreign contributors retain their
identity and content while their session byte starts shift25. The first failed
binding is Imported id563, words `[2208441662476345389,316229852143522]`, displayed
as `vstd/arithmetic/div_mod.rs:519:50: 519:56 (#0)`. Its old/current provenance was
recomputed from complete inventories. **Loaded/Delayed state was not captured**
in that evidence. The implementation must record the actual state before a
relocation hit can qualify. The earlier strict immediate-progress stderr failure,
c05 prior-revision failure and all original qualification assertions remain.
See [the causal review](../owned-import-mapping/relocation-review.md) and
[proposed ADR0030](../../../../docs/decisions/0030-imported-span-authority.md).

## Implementation boundary

`SpanContextX::new` captures once, only with `SAIL_ENGINE=interactive` and the
existing `SAIL_ENGINE_FUNCTION` selector present. It preserves original stable
crate/range/raw hash bytes while the original metadata still exists. One admitted
owner holds only primitives, typed source identity and bounded complete names.
It keeps no compiler borrow, SourceMap/SourceFile Arc, resolver lock, old raw span
or duplicate metadata snapshot.

Before focus, `sail_take_loaded_imports` moves that same owner out of SpanContext
and revalidates initial Loaded entries against the current resolver and complete
source table. It checks exact original ranges, unchanged resolver bounds, current
file index/range, Foreign state and constructor namespace/cnum, content hash,
full name, stable source identity, normalized and unnormalized lengths, and
newly introduced equal-content/current-range ambiguity. Initially Delayed,
None or Busy entries stay unavailable even if ordinary compiler work has since
loaded them. Their *actual* pre-focus state is observed separately. Local fallback,
unknown source and any changed/ambiguous authority remain misses. No loader call
or stock state mutation is added.

For a genuine raw unit import and exactly two packed words, provenance binds the
original namespace/words/range/raw expected hash plus the selected complete
current typed hash (algorithm and bytes), full name, stable file identity and both
lengths. Original metadata supplies no algorithm tag; the implementation preserves
its exact raw bytes and adds the actual current typed tag without inventing an
original kind. StableSourceFileId's complete pinned Hash128 is streamed through
its typed Hash implementation into SHA256, without Debug/SpanData/TLS or a heap
string. Current index/start/end are checked translation coordinates, not
cross-callback equality fields. The immutable SourceMap entries and stock Loaded
resolver do not subsequently change their bounds; appended files cannot replace
those entries. Stock Delayed loading remains outside this eligibility policy.

`current_bindings`, guarded span decode and all graph/effect/SST/key components
remain unchanged. Every decoded span requires the exact current `(id,data,
as_string)` binding and receives its **current** raw Arc. Unknown raw Any types
cannot enter the import policy. Local lo/hi/hygiene/parent/file provenance remains
exact. The new preparation domain is
`sail-prepared/graph-ledger/loaded-foreign-authority/v5`; Portable version3 is
unchanged. Old v4 and earlier entries miss, with no migration/resealing.

## Physical ownership

One process-wide8MiB admission is acquired before table allocation. Record count
is capped at4096 and `count * size_of<Record> +1024` is checked **before** reserve;
actual Vec capacity is checked afterward. The1024-byte margin includes the
owner/Arc/control metadata and possible final-header disposal overlap. Names
are capped at4096 bytes using the pinned audited streaming formatter. Before
formatting, remaining space covers old/new string buffers and fixed scratch;
all retained capacities are charged. The original expected hash lives inline in
the same record, not an extra seed. Late validation writes a borrowed name
comparator and allocates no snapshot or table.

The same admitted family moves constructor→pre-focus→one Arc shared by Sources.
Records and their names are physically disposed before admission release. No
clone/record escape API exists in production. `retained_charge` includes the
whole8MiB while physically owned, including constructor ownership before Sources
and any remaining Arc lifetime. Sampling immediately after admission and a
preserved family peak cover failed capture and missing/ambiguous selection too;
clear cannot report zero for an already admitted family. The prior audited preparation maximum492MiB
therefore becomes a **prospective500MiB**, not an RSS measurement or qualified
bound. Ordinary compiler working IR and opt-in fresh/restored oracle arms retain
their separately reported working-memory scope.

## Read-only state evidence

Set private `SAIL_ENGINE_IMPORT_TRACE=1` for source qualification. The existing
source `SAIL_ENGINE_EVENTS` sidecar receives `import-authority-state` frames during
focus, including the exact current id/words/as_string (spelling capped at4096 bytes),
initial/pre-focus resolver states,
original and observed bounds, expected bytes, selected typed identity and checked
translated range. Unsupported states are explicit; they are never promoted from
filename evidence. Observations do not authorize cache reuse or proof success.

The trace is serialized under the existing emitter mutex with borrowed fields,
no JSON Value or copied name. It permits4096 records,32KiB per record and4MiB
aggregate published bytes including LF, reserving512 bytes for its single
unavailable replacement. Queued callers cannot publish after the sticky stop.
It uses the existing bounded observer owner/writer; it adds no heap budget.
Absent tracing leaves this diagnostic path inactive. Supplementary qualification
can set `SAIL_ENGINE_IMPORT_TRACE_TARGET=div-mod-relocation` to observe only the
exact spelling `vstd/arithmetic/div_mod.rs:519:50: 519:56 (#0)` before record
admission. This fixed display filter supplies no authority and makes no filename
inference: actual typed state and packed words are still recorded. Unknown target
tokens skip all observations; omitted target preserves the generic trace. A
source regression rejects adjacent ranges, hygiene spellings, prefixed filenames
and unknown tokens. The filtered trace has the same caps and honest unavailable
behavior; it is separate from the unchanged fourteen qualification scripts.

## Required checks before any activation

Existing memo assertions are preserved with an explicit small **test-only** typed
mapping fixture; that fixture makes no acquisition or physical-budget claim.
Actual-source tests retain all eleven pinned API controls and add late-state,
original-hash/new ambiguity, real Loaded relocation/content/name/length and
unknown-type/word/state, typed hash-kind/stable-ID-only and admission-peak
controls. Domain tests retain the old dependency-shape
assertion and add v4→v5 rejection. **These new tests have not been compiled.**

Run actual driver/VIR tests and strict checks, ordinary fresh vstd build, intact
old-domain/misfile controls, all frozen14 source qualification stages with their
original600-second limits, exact same-input ordered wire/effects/grouped
messages, stock verdict/diagnostics including failed-focus/late EXEC, cancellation
and normal termination. Retain the original strict progress-note failure if it
recurs. State evidence must show the actual failing words are eligible Loaded
Foreign before describing a relocation as fixed. Genuine first FOCUS restore
from the previous task023 revision is required; same-invocation FULL reuse is
insufficient. Complete HE/Vest and whole-loop2×/reader gates remain independent.

The first static filter draft omitted the space in the retained display spelling.
Review caught this before any compiler or runtime build. The selector and its
positive control now use the exact retained string; the no-space spelling is an
explicit negative. No trimming or normalization is used.
