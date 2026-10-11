# Import capture timing and a guarded integration boundary

This is a static integration proposal for the [inert helper](README.md), plus
one executable pinned-API timing control. It changes no preparation equivalence,
source-engine key, codec, selected runtime or source-loading behaviour. The
[existing source authority review](../decode-ownership/span-rebinding-review.md)
and failed c05 cross-edit gate still stand. The actual first mismatched span and
owner fields remain pending from the separate diagnostic variant.

## What the pinned pipeline actually does

References are relative to the pinned Verus `source/` tree.

| Boundary | Available state and ordinary side effects |
|---|---|
| `verifier.rs:3249`, `spans.rs:84–197` | `SpanContextX::new` receives original imported-file metadata and current `TyCtxt`/SourceMap. It creates Loaded foreign mappings, Delayed path/hash mappings, and missing entries. This precedes VIR construction, trait checking and lifetime checking. |
| `verifier.rs:3265–3368`, `verifier.rs:94/107` | Frontend warnings/errors emitted through the stock Reporter use `from_air_span(..., Some(source_map))`. Imported spans may invoke the ordinary resolver and load a file. Successful frontend work is not a guarantee that every imported span was reported/resolved. |
| `trait_conflicts.rs:354/409/454` | Trait-conflict conversion passes `None` for SourceMap. These calls can use an already Loaded entry but cannot load a Delayed one. |
| `verifier.rs:3380`, `sail_engine.rs:627` | The optional engine freezes its `Sources` inventory just before focus, after frontend checks. Existing files, including any ordinary prior loads, are captured here. No import-mapping authority is currently captured. |
| `sail_engine.rs:636`, `sail_engine.rs:542–546` | Focus uses FocusReporter. It records/emits the message graph without converting imported locations through the stock reporter. Focus therefore does not itself guarantee ordinary lazy source loading. |
| `sail_engine.rs:338–370/382–400` | Current bindings serialize fresh current VIR and seed spans; prepared provenance uses the frozen `Sources`. `take_bucket` and `store_bucket` explicitly ignore their SourceMap parameter. Neither loads/resolves an import. |
| `verifier.rs:3404`, `verifier.rs:94/107` | Full stock reporting can resolve previously Delayed imports after the inventory freeze. That does not refresh the existing frozen `Sources` or its memo. |
| `verifier.rs:3383`, `sail_engine.rs:91` | Callback completion clears Sources and preparation retention. There is no retained compiler session across revisions. |

`spans.rs:230–260` is the only ordinary Delayed-to-Loaded resolver path. It sets
None before `load_file`, checks raw hash bytes, then stores current bounds. The
file added by this fallback is normally a **local** SourceFile, rather than the
Foreign owner supported by the proposal. Treating Loaded as sufficient authority
would therefore be incorrect: the origin, expected hash, crate/range association
and current file must all be checked again.

The new API timing control invokes these unchanged stock methods with a real
file. Early capture performs no I/O; subsequent stock `unpack_span` loads the
file and changes the resolver to Loaded. The early owned snapshot remains
Delayed, and the initial Foreign-only helper refuses a later observation as
NotForeign. This proves that promotion can happen, not that it happens for the
failed real-project binding. It does not construct the complete driver or prove
anything about an actual source preparation hit.

## Smallest conservative integration

If the actual mismatch warrants this seam, first integrate **initial Loaded
Foreign owners only**. Capture their expected original authority in the
constructor while original metadata still exists; keep Delayed/missing/busy
entries unavailable for the whole callback. At the pre-focus Sources boundary,
revalidate each candidate against the current complete file table using the same
stable-crate lookup on the compiler thread. A candidate newly ambiguous after
ordinary frontend work is unavailable. This prevents an early snapshot from
silently ignoring newly added equal-hash current candidates.

No `TyCtxt`, SourceMap reference, SourceFile Arc, resolver lock or raw SpanData
may enter the retained capsule. The lookup closure executes immediately on the
compiler thread. Publish a single immutable admitted owner with Sources only
when both inventory and mapping are complete. Workers may read that owner;
they may not retry resolution or refresh it. Unsupported owners remain misses.
A stock load after this publication cannot promote an unavailable entry for
this callback. Supporting a locally loaded fallback is a separate design:
it lacks the constructor's Foreign `cnum` association and must not be accepted
using a guessed filename.

For a broader future observation at pre-focus, retain a bounded **authority
seed**, rather than cloning the whole original metadata map. A proposed record
would contain stable imported crate, exact original range, up to 32 expected raw
hash bytes and explicit byte count, plus original capture state. Unknown hash
lengths, range/hash ambiguity or overbudget seeds decline preparation. The seed
and resolved copy must share one admission owner; count their simultaneous
capacities before growth. This is a proposal only: the current helper contains
no seed, late observation, refresh or constructor call.

## Delayed descriptor authority is different from Loaded authority

A Delayed resolver already has an exact original authority descriptor: imported
stable crate, original range, expected raw hash bytes and loader path. Copying
that descriptor into a bounded owned record does not eagerly load a file and
must never label it Loaded. The present helper deliberately declines it, so it
may be unable to address a real Delayed mismatch. Do not infer the failed owner
state from the fact that foreign SourceFiles exist elsewhere.

For a future descriptor-only design, the path must be preserved with exact OS
bytes and platform identity, not lossy display text or canonicalized spelling.
Relative paths also require the exact loader working-directory/configuration
context. Compare both original words and the entire exact current SailSpanKey,
including `as_string`. Require a current raw **unit** import binding: unknown
Any owners fail closed. Prepared decode already retrieves the exact current
raw Arc in `vir/messages.rs:73–85`; retain that behaviour. The decoded message
must later be rendered using the **current** SpanContext and SourceMap, with no
copied old raw SpanData or invented filename.

An identical descriptor is not by itself proof of identical future rendering.
The stock resolver observes its mutable state and loader/filesystem at the
actual reporting call. The outcome can be Loaded, hash-mismatch/missing None,
or an already-loaded SourceFile with different normalization. SourceMap's
`load_file` reads through its FileLoader and creates a Real filename using its
path mapping and working directory (`source_map.rs:245–249`); its selected hash
algorithm is an input to SourceMap construction but has no public getter in the
pinned API. Preserve that compiler configuration explicitly rather than infer
it from the original metadata's raw bytes or an unrelated SourceFile.

The smallest descriptor policy would require matching old/current captured
state, descriptor and loader configuration, supported original range, and
complete fresh file-input authority. Delayed↔Loaded/None, busy/poisoned state,
changed path/hash/range/normalization, metadata collapse or unknown loader
configuration is a miss. A record captured as Delayed stays unavailable or
explicitly Delayed even if ordinary stock work later loads it. None cannot be
upgraded using an old descriptor: it may encode a prior failed I/O/hash check.
No call to `resolve` may be added to establish eligibility.

If this policy were later proved, replay could retain the current unit binding
and let stock render a cached **identical diagnostic message** through the same
current resolver that fresh lowering would use. That still requires a separate
argument about diagnostic ordering and loader side effects. Fresh lowering may
emit a diagnostic before prepared storage; replay happens at a different point
unless emission semantics are reproduced. An earlier stock load can affect the
file table and later resolver outcomes. Focus uses a collecting reporter while
full uses the stock reporter. Equal descriptors must not hide that difference.
Require grouped/immediate emission order and full stderr equality, not only
matching message text. Every newly loaded file must enter the managed declared
input/currentness authority, or the run remains conservative Unknown/Historical.

Until those conditions have executable evidence, refuse Delayed provenance or
fall back to fresh lowering. This is not an equivalence approval. Existing
missing/ambiguous binding, changed hash/path/range, exact `as_string`, imported
word and local hygiene/parent controls remain mandatory. Additional real controls
must cover both-Delayed successful load, both-Delayed missing/hash failure,
old-Loaded/current-Delayed and the reverse, prior None after failure, repeated
reporting, changed path remapping/working directory/hash algorithm, source-file
normalization, and stock report timing with exact current locations.

## Domain and binding guards

Any future import-specific provenance is a new, explicit domain. Reject old
preparation-domain entries before decoded effects are applied; preserve the
current codec version unless its representation actually changes. Domain checks
must cover disk and batch entries and have a real old-domain rejection control.
Solver, source patch/runtime, settings and all other exact preparation inputs
remain bound.

A new provenance constructor must begin from the **current** SailSpanBindings
entry and distinguish a real local SpanData from an imported unit owner. Keep
exact SailSpanKey identity, both original imported words and current raw owner
association. Do not turn an arbitrary two-word raw object into an import. Local
lo/hi, SyntaxContext and optional LocalDefIndex checks remain unchanged.

For supported imports, require the exact original stable crate and original
range/hash association, one current foreign owner in that namespace, valid
lo/hi inside the original range, equal normalized lengths, full current typed
hash algorithm/bytes, stable file ID and bounded complete name. Original metadata
stores hash bytes only; retain that ambiguity explicitly. In particular, never
infer SHA-256 versus BLAKE3 from 32 bytes. Preserve stock import rendering's root
SyntaxContext/no-parent semantics and use the exact current raw binding for
all diagnostics. No old raw span is reconstructed.

These guards do **not** yet authorize removing current file index/start or any
other provenance field. If the actual trace establishes session-coordinate
relocation, its proposed semantic boundary must explain the checked translation
into the current raw location and receive exact same-input wire/effects/grouped
message and stock diagnostic controls. Existing tests stay intact; any apparent
conflict with a proposed equivalence must be raised with actual evidence.

## Physical owners and tests required before selection

The inert helper owns one admitted 8 MiB family. A future seed, current snapshot,
Arc header, memo key/value and serialized binding must fit inside that admission
or receive a separately audited phase charge. If initial and late snapshots
coexist, both are charged; replacing one requires physical disposal before
release. A callback's final owner is destroyed before another can take its
charge. The prior 492 MiB preparation phase plus 8 MiB leaves 12 MiB of margin;
this arithmetic is prospective and is not a new aggregate product claim.

The [formatter audit](formatter-audit.json) identifies the pinned Unix Debug
chain. It streams borrowed data; it does not make an unbounded lossy path
String. The String being grown is the bounded destination. Its old/new allocation
headroom remains inside admission. See that record for source identities and
platform limitations; general Debug implementations are not an accepted fallback.

Required real controls include constructor Loaded versus later ordinary
resolution; pre-focus added equal-hash/overlapping owners; same-name files in
different crates; duplicate-content hash collapse; delayed/missing/busy refusal;
wrong typed hash algorithm with equal bytes; normalization; absent owner,
cross-file/invalid range; unchanged local hygiene/parent/imported words; raw-Arc
identity; changed source ranges/diagnostic lines/macros; source table relocation
with exact current diagnostic authority; old-domain rejection; clone/thread
escape; simultaneous seed/snapshot allocations and physical last-owner disposal.
Full driver/VIR/strict checks, ordinary vstd verification, positive and valid-HIT
negative same-input wire controls, complete stock verdict/diagnostic compatibility,
cancel/panic controls, genuine previous-revision FOCUS hits and both real edit
catalogues remain mandatory. No performance or qualification conclusion follows
from the static design or supplementary API tests.
