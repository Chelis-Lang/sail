# Deferred diagnostic locator: independent consumer audit

Status: read-only design review, not an implemented authority role. The selected
runtime is unchanged. The [b46 failure record](loaded-import-qualification-failure.json)
retains the strict NOTE-order mismatch, zero restored preparations and the
separate first-refusal observation. No assertion, source equivalence or budget
is changed by this document.

The observed first refusal is `vstd/function.rs:151:20: 151:65 (#0)`:
original file `406973..411571`, packed span `411214..411259`, constructor and
pre-focus states both **Delayed**. The current LoadedForeign policy correctly
misses. A new role must describe a deferred diagnostic recipe, rather than claim
that this file has loaded source text or current coordinates.

## Pinned audit boundary

This audit covers Verus commit `426d8b01e7ffb910a36c15e1f870ff985bb61eef`
with the reviewed b46 patch
`506679543779f2856899006d7d70420ce85d47cb74c9873a064f97ab82c44984`.
Changed-file identities are retained in
[the build record](loaded-import-test-layout-build.json). Rust source is pinned
to compiler commit `48a229ceaefd4985c50990b14116b6d856af0985` (1.98.1).
References below name paths inside the pinned Verus `source` tree or rustc tree;
they are not paths to a selected Sail implementation.

A complete Rust-source search for `raw_span`, `from_raw_span`, `from_air_span`,
`unpack_span` and `pos_to_extern_source_file` closes the raw/resolver consumer
inventory below. The extra `as_string`, `data` and `id` reads retain their exact
current values; absence of a raw resolver read does not exempt them from keys.

| Consumer | Exact behavior and required boundary |
| --- | --- |
| `vir/src/messages.rs:52–85`, `Span` serialization/deserialization | Persist id/data/as_string; prepared decoding looks up that exact triple in the **current callback** binding map and clones its current raw Arc. It does not decode or construct old SpanData. Missing and conflicting bindings miss. |
| VIR `ast_to_sst*`, `poly`, `triggers`, `interpreter`, `sst_to_air*` | No raw downcast or SpanContext/resolver call. They propagate spans and construct semantic IR/messages. `vir/src/printer.rs` and `def.rs:994` read as_string for logs/debug; modes also reads span id. These fields remain exact. |
| `rust_verify/src/sail_engine.rs:471–515`, current bindings/take/store | Bind the pruned current VIR and current seed spans; verify every portable binding before restoring effects/SST. The proposed role belongs only to imported provenance. Local raw-field checks, full key, graph validation, decode admission and reporter admission remain mandatory. |
| `rust_verify/src/sail_engine.rs:125`, `ReplayMessages::replay` | Dispatches the original Report/ReportAs/ReportNow/ReportAsNow methods and levels to the current reporter. No rendered rustc diagnostic or resolver state is cached. |
| `rust_verify/src/sail_engine.rs:662–679`, recording/focus reporters | Recording forwards the same operation while retaining admitted MessageX ownership. Focus records diagnostic data without resolving locations. A role cannot treat focus observation as evidence of successful stock source loading. |
| `rust_verify/src/verifier.rs:97–117`, stock Reporter | Resolves each primary span and label through the **current** SpanContext and SourceMap, then performs stock multispan filtering/rendering/deduplication. This remains the sole source-location resolver for replayed diagnostics. |
| `rust_verify/src/verifier.rs:2276–2277`, grouped messages | Sorting downcasts raw SpanData, without resolving imported words. An imported current Arc<()> yields None on both arms. Creating a resolved raw span would alter this behavior and is forbidden. |
| `rust_verify/src/verifier.rs:2352/2449/2470`, progress | Downcasts current local function spans. A deferred imported recipe must not introduce SpanData or substitute itself for local function progress. |
| `rust_verify/src/trait_conflicts.rs:354/409/454` | Fresh frontend calls `from_air_span(..., None)`, so it does not initiate a deferred file load. It precedes preparation; its results remain fresh. |
| `rust_verify/src/erase.rs:286/305/334` | Fresh erasure checks local raw spans/id mappings, skipping imported unit spans. Preparation does not replace the erasure input or raw representation. |
| `rust_verify/src/debugger.rs:46/56/101` | Uses local raw spans for current SourceMap line lookup. Deferred imported spans are not new debugger locations. The existing debugger option remains bound in the full preparation key; no new imported-debugger support is claimed. |
| `vir/src/context.rs:943`, private datatype span seeds | An active read invalidates graph capture, including inherited interpreter/fork lineage. The deferred role must not make this unsupported read supported or omit seed binding checks. |

`air::messages::Diagnostics::report_as_multi` defaults to individual report_as
calls. There is no VIR lowering call to report_as_multi in the audited tree.
QueuedReporter has a grouped-message path, and the diagnostic oracle records
that operation independently. Neither is permission to flatten a future new
grouped preparation operation: it must be captured exactly or cause a miss.

## Feasible narrow role, subject to authority review

A DeferredLocator capsule can be an immutable, admitted **recipe identity**.
It must bind the original imported crate and file/span ranges, exact expected
hash bytes and actual descriptor path bytes. Its association with an existing
unique imported owner needs the independently reviewed crate/hash/range/name
authority; a display string is insufficient. Existing foreign typed hash kind
is evidence about that imported owner, not proof of the private SourceMap's
future loader hash algorithm or of disk content at the descriptor path.

At constructor capture and pre-focus handoff, require the same actual Delayed
descriptor. Unknown, None, Busy, ambiguous owners, range overflow, changed
descriptor and an early transition to Loaded decline. Pre-focus must move and
validate the same admitted owner, not create a second uncharged snapshot. No
resolver, canonicalization, file read, source hydration or coordinate translation
is performed to establish the role.

After handoff, the capsule stays Deferred. The current stock resolver may
naturally turn its own mutable descriptor into Loaded or None while emitting
diagnostics. That ordinary transition is not a promotion of capsule authority.
No cache lookup or replay resets the descriptor, writes it into a fresh context,
or restores a prior outcome. Decoded spans still use exact current raw-unit
bindings; local hygiene/parent and raw-kind negatives remain unchanged.

This distinction makes the seam plausible: lowering's semantic output and
stored MessageX data never read the deferred resolver's source text or loaded
coordinates, while all actual diagnostics continue to resolve through the fresh
reporter. It does not yet prove identical rendered diagnostics, resolution side
effects or genuine reuse. Those remain actual controls and original oracles.

The supported ghost callback is `VerifierCallbacksEraseMacro`
(`rust_verify/src/driver.rs:305`). The only explicit custom file_loader assignment
in the audited Verus tree is the separate TCCallbacks trait-check compiler
(`driver.rs:59–62`). Any new callback/loader integration requires its own audit;
do not identify arbitrary loader semantics by a foreign hash or pointer address.
If loader context is included in the capsule's digest, use exact bounded owned
inputs from the supported compiler configuration, not a debug-format guess or
new file IO. Current reporting must still use the current loader.

Use a distinct preparation domain and explicit role tag. An intact old v5 entry
misfiled under a new key must be refused. The policy does not weaken semantic
VIR/context keys, graph reads, body/spec edits or imported packed-word identity.
It authorizes neither a cached rendered snippet nor a full-proof verdict.

## Memory, work and input boundary

Replace or extend records **inside** the existing single process-wide 8 MiB
mapping family. Admit record count × inline size, vector old/new overlap, exact
path/hash/name capacities, Arc/control storage, and initial-to-pre-focus lifetime
before allocation. Immutable per-span lookup should borrow that family; memo,
seed encoding, observer and replay ownership remain separately admitted in the
existing conservative 500 MiB maximum. An additional 8 MiB owner cannot be hidden
outside that maximum. Physical records/paths must drop before quota release on
every failure and compiler teardown.

No source text, SourceFile, SourceMap Arc or TyCtxt is retained by the capsule.
Consequently this role adds no preparation-induced source load or SourceMap
text/line/multibyte allocation. Later stock Reporter allocation remains its
ordinary rendering work, and must not become a preparation-retained owner.

This avoids the concrete eager-load obstacle: rustc
`compiler/rustc_span/src/source_map.rs:246` performs unbounded read_file before
hash/admission; SourceFile text and tables survive mapping drop. Successful
new_source_file can even return an already registered stable-id file unchanged
after reading another String. A capsule must not make any loaded-content claim
from that behavior.

Verus emits dependency info at `verifier.rs:3240`, before SpanContext construction
at `3259`; deferred stock diagnostic IO is not automatically a new dep-info
declaration. Capsule capture must not claim it guards or hashes that path's live
bytes. The role instead preserves current ordinary reporting. Full-report
freshness/compiler seals keep their existing contracts; if a future design caches
source snippets or introduces earlier reads, explicit input/guard ownership is
required before those reads.

## Required actual controls before implementation qualification

1. Real pinned types: exact Delayed descriptor and current unit-span binding;
   capture/handoff performs zero loader calls. Changed path/hash/crate/range,
   byte spelling, missing/duplicate owners, unknown hash kind, raw kind,
   as_string, local hygiene/parent and capacity failure decline before replay.
2. Current rendering: primary and label spans share the same current resolver;
   exact stock successful load and missing/wrong-hash failure have identical
   fresh/cached diagnostics and loader transitions. The capsule never becomes
   Loaded, even when the current descriptor does.
3. Already-present SourceMap stable-id reuse, descriptor remaps, two messages
   sharing one file, mixed local/imported labels, immediate/grouped emission,
   fresh trait-check behavior and progress sorting preserve stock operations.
   Verify no old raw coordinates or SourceMap owner escape from decode/replay.
4. Natural post-handoff Loaded/None transitions and concurrent reporter activity
   preserve current behavior; unsupported/custom transitions conservatively
   miss. Exact original descriptor changes between capture and handoff miss.
5. Adversarial path/count/capacity and drop-race controls retain the 8 MiB family
   and conservative total; unknown platform path encoding declines rather than
   becoming a lossy identity. Old-domain migration rejects intact old entries.
6. Actual source tests/strict Clippy, fresh ordinary build/vstd/manifest and all
   original fourteen gates remain required. Strict stock stderr, same-input raw
   SMT/effects and genuine previous-revision FOCUS remain unchanged, including
   their original 600-second bounds. The existing NOTE-order failure remains a
   failed scope until addressed independently; this role cannot normalize it.

The authority decision needs an ADR explaining the new diagnostic-recipe role
and domain before source selection. Add actual outcomes and runtime identities
to the candidate guide and known-issue evidence; no proposal or safe refusal is
a restored preparation, performance result or product qualification.
