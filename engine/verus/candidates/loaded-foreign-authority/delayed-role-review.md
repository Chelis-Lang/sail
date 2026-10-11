# Delayed import authority after the actual first refusal

Status: read-only design review. No authority policy, preparation domain, source
patch or selected runtime changes are approved by this document. The `50667954`
candidate remains unqualified. Its original exact note-order failure, zero hits
and the separate first-refusal observation remain retained.

The first observed refusal is the imported `vstd/function.rs:151:20: 151:65
(#0)` binding. Its constructor and pre-focus descriptor are both Delayed,
original file range `406973..411571`, span range `411214..411259`. The candidate
correctly declines this state. Moving the existing LoadedForeign capture later
does not establish a usable authority role.

## Exact pinned stock path

Verus `source/rust_verify/src/spans.rs` routes imported raw-unit spans through
`SpanContextX::from_air_span` and `unpack_span`. With the current compiler's
SourceMap, `pos_to_extern_source_file_resolve` changes a Delayed descriptor to
None, calls the stock `SourceMap::load_file(filename)`, and changes it to Loaded
only if the loaded file's raw source-hash bytes match the descriptor hash.
Stock then translates original lo/hi offsets into that file's current range,
with root hygiene and no parent. These are stock operations, not a proposed
shortcut for the preparation codec.

The actual diagnostics consumer is `Reporter::report_as` in
`source/rust_verify/src/verifier.rs`: it resolves each message span and label
through the **current** SpanContext and SourceMap. A current raw `Arc<()>`
alone therefore does not carry a complete diagnostic location. A future policy
must preserve the current binding, its exact id/data/as_string, and its current
resolver authority.

Rustc 1.98.1 `compiler/rustc_span/src/source_map.rs` defines `load_file` as a
full `FileLoader::read_file` followed by `new_source_file`. The latter uses
`StableSourceFileId::from_filename_in_current_crate`. In
`compiler/rustc_span/src/lib.rs`, `SourceFile::new` builds a current-crate file
with `src: Some`, `external_src: Unneeded`, and `cnum: LOCAL_CRATE`. The
separate imported-file constructor uses `Foreign` and the imported crate
identity. Thus an ordinary successful Delayed load generally creates a local
**diagnostic** file; it does not establish the current LoadedForeign role.
This is a source audit. A real pinned-type loader control has not yet been run.

## Option A: separately scoped loaded diagnostic Local

A reviewed new role would need all of the following:

- Retain the original imported crate, original range, expected raw hash bytes,
  and the exact actual Delayed loader descriptor before stock resolution.
- Observe the **actual** stock successful transition to Loaded. Bind the
  resulting current file's typed hash kind/bytes, complete name, StableSourceFileId,
  current-crate role, normalized/unnormalized lengths and actual current range.
  Reject missing, ambiguous, hash-mismatched or incorrectly ranged owners.
- Preserve current raw-unit rebinding and all local hygiene/parent and imported
  word negatives. Use a distinct domain; old v5 entries cannot restore under
  the new role. Never relabel the original Delayed state as already Loaded.
- Prove source-input/guard coverage before any additional file read, and retain
  source-file memory ownership through its actual compiler lifetime.

The current stock API has a concrete physical-bound obstacle: `read_file`
allocates the complete String **before** source-hash, size or normalization
validation. A later metadata/size check does not bound that allocation or stop
a concurrent growing, replaced or mapped file. There is no public bounded read
parameter on `SourceMap::load_file`; the installed FileLoader is private to
SourceMap. A direct filesystem limited read followed by `new_source_file` would
bypass the stock loader's semantics, including custom loaders, and is not an
equivalent implementation by inspection.

Introduced SourceFile retention also includes the source String, line and
multibyte vectors, normalization records and SourceMap lookup/table ownership.
It survives dropping the owned mapping. Those optional preparation allocations
cannot simply be reclassified as ordinary compiler working IR. The same 8 MiB
family and prospective 500 MiB overlap require pre-read protection/admission,
all dynamic capacities and a charge owner that outlives the actual SourceMap
allocations. Neither a post-check nor the existing mapping Drop supplies that
proof. No bounded loaded-Local implementation is proposed until this is closed.

## Option B: exact deferred locator capsule, without loading

A narrower alternative for review is a distinct **Delayed descriptor** role.
It would not claim current loaded source text or coordinates. An owned bounded
capsule could bind the exact selected original range/crate, expected hash bytes,
actual filename/path-resolution context, and observed descriptor state; cached
spans would still be rebound to current exact raw-unit bindings. The current
stock reporter would retain sole authority to load, validate, translate and
render later diagnostics. Initial or current None/Busy/ambiguous states would
miss, and a state transition would conservatively miss until independently
covered.

This avoids added SourceMap source allocations, but is **not approved** merely
because its storage can fit the existing 8 MiB family. The closed consumer audit
must establish that preparation's non-span lowering/effects never consume the
deferred loader state or source text. It must also establish that no cached
diagnostic bypasses current descriptor resolution. Exact expected-hash/path
identity and current raw binding cannot be replaced by a display-name guess.
The original metadata table stores hash bytes without hash-kind information;
ambiguous kinds or equal-content owners still require explicit refusal.

## Controls before any new runtime qualification

Retain the original failures. Exercise actual pinned rustc types for successful
and failed stock loads, current local versus foreign roles, missing paths,
changed hash/path/range, duplicate-content/range ambiguity, and concurrent
growth/replacement/mmap. For any new role, preserve old-domain rejection and
all existing imported-word, raw-kind, hygiene, parent, missing-owner and
as_string negatives. Verify current multi-span diagnostics and loader failure
behavior, grouped/immediate emission, actual same-input ordered wire/effects,
and a genuine previous-revision focused restore under the original 600-second
gate. No state observation or safe miss is a cache-hit or performance result.
