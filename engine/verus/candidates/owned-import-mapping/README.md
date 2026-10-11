# Owned import mapping: inert proposal

This proposal changes no selected runtime, source-engine key, codec, comparison,
binding, loader or proof verdict. The [reviewable source patch](../owned-import-mapping.patch)
adds an **uncalled** private helper and unit tests to the pinned fork. It is not
a correction for the failed c05 cross-edit gate: B's exact first mismatch trace
must establish whether the affected span is Local or Imported and which field
changed. The existing [authority review](../decode-ownership/span-rebinding-review.md)
and every prior failure remain applicable. The [relocation review](relocation-review.md)
now records the measured Imported coordinate shift and the narrower consumer/API
evidence; selected Loaded/Delayed mapping state remains unobserved.

The [actual API controls](api-controls.json) exercise the proposal against pinned
Rust source-map types and unchanged extracted stock resolver methods. They are
helper tests, not a complete driver/source build, proof oracle, runtime
qualification or performance result. Initial API/fixture compilation failures
are retained in that record.

## Precise stock seam

These references are within the pinned Verus `source/` tree:

| Source | Authority and lifetime |
|---|---|
| `rust_verify/verifier.rs:3249` | `SpanContextX::new` receives the current `TyCtxt`, current compiler SourceMap and owned original-file metadata. This is on the compiler callback before VIR construction and before focus workers. |
| `rust_verify/spans.rs:84–197` | Constructor matches foreign SourceFiles by `tcx.stable_crate_id(source_file.cnum)` and original source-hash bytes, then associates original and current normalized ranges. Original metadata is not retained afterward. |
| `rust_verify/spans.rs:42–57` | Each imported file owns original `BytePos` bounds and an `Arc<Mutex<Loaded / Delayed / None>>`. `Loaded` holds current bounds; `Delayed` holds a path and expected hash bytes. |
| `rust_verify/spans.rs:230–260` | The ordinary resolver can change `Delayed` to `None`, perform `load_file`, compare the loaded hash, then publish `Loaded`. Calling it solely to improve cache eligibility would change I/O and diagnostic state. |
| `rust_verify/spans.rs:265–284` | Imported data is exactly two words: stable crate ID and original packed lo/hi. Translation uses the selected original/current ranges; the reconstructed diagnostic Span has root hygiene and no parent. |
| `rust_verify/spans.rs:106`; `rust_verify/import_export.rs:12` | Original-file metadata is keyed only by raw source-hash bytes. Equal-content files can overwrite one another. Neither filename uniqueness nor original hash algorithm can be inferred from that map. |

The actual compiler is Rust 1.98.1, commit
`48a229ceaefd4985c50990b14116b6d856af0985`. Its source references are relative
to the installed Rust source root:

| Rust source | Exact available data |
|---|---|
| `compiler/rustc_span/src/lib.rs:1743–1875` | `SourceFileHash` is Copy and exposes `kind` plus `hash_bytes()`. MD5/SHA-1/SHA-256/BLAKE3 are distinct typed algorithms; a 32-byte original key cannot distinguish the last two. |
| `compiler/rustc_span/src/lib.rs:1921–1990` | SourceFile exposes immutable name, hash, normalized and unnormalized lengths, stable ID and importing `cnum`. Rust metadata deliberately omits session-global `start_pos`. |
| `compiler/rustc_span/src/lib.rs:2128–2190` | `StableSourceFileId` hashes filename and exporting stable crate identity. Source content is hashed before newline/BOM normalization; normalized ranges must remain separate from that hash. |
| `compiler/rustc_span/src/source_map.rs:280–311` | `files()` returns a monotonic vector read guard: existing indices remain valid during the same callback. New files are separated by one byte, including empty files. |
| `compiler/rustc_span/src/source_map.rs:355` | `new_imported_source_file` installs a foreign SourceFile with full hash kind, stable ID, normalized metadata and `cnum`. The helper tests use this real API. |
| `compiler/rustc_data_structures/src/sync/freeze.rs:73–87` | `external_src.borrow()` takes a shared read guard; it does not call `freeze()` or change lazy-loading state. |

## Proposed capture, without a reuse policy

The future capture point is at the end of `SpanContextX::new`, while
`original_crate_files` is still borrowed. The constructor would pass its current
`tcx.stable_crate_id(cnum).as_u64()` lookup into the helper; the closure is used
immediately and never stored. This avoids retaining `TyCtxt`, SourceMap or a
compiler session. The proposal contains no call at this point.

For each original imported range, capture copies only the existing resolver's
primitive bounds under `try_lock`. Delayed, missing or busy entries retain an
explicit unavailable state. They are never eagerly loaded or promoted after
capture. The initial supported Loaded path requires a **foreign** current
SourceFile, exact stable crate association, unique original range/hash, equal
normalized lengths, exact original hash bytes and a unique current range. It
also rejects multiple equal-hash current files in the same imported crate.
Structural overlap, malformed endpoints and wrong SourceMaps fail closed.
Locally loaded fallback files are deliberately unsupported in this first slice.

Owned records contain original crate/range, current file index/range, normalized
and unnormalized lengths, complete bounded FileName Debug spelling, actual typed
SourceFileHash and StableSourceFileId. There is no source text, old raw SpanData,
source-file Arc, compiler borrow or shared resolver mutex. Capturing these fields
does **not** establish that any of them can be omitted from existing provenance.

An eventual precise imported provenance could consult this authoritative mapping
for the **exact current** `SailSpanKey` binding. Such a change would require a new
domain and complete oracles; this patch implements none of it. Local SpanData
would keep exact lo/hi, hygiene and parent checks. Raw unit imports must be
distinguished explicitly from local and unknown raw owners; two packed words
alone do not establish import authority. The existing compiler's root/no-parent
import rendering must remain unchanged. No old raw span would be reconstructed.

## Ownership and limits

The prototype reserves one process-wide 8 MiB admission before allocation. At
most 4096 entries/source files are examined. It allocates one fixed-capacity
record vector, bounded complete names of at most 4096 bytes, and headroom for
name growth/formatter temporaries. No growable hash index, source/file clone or
per-record callback is retained. Read-only record access prevents escaping a
mutable vector that could bypass admission. Every partial failure destroys its
allocated records before release. Last-owner Drop explicitly disposes the vector
and all strings **before** releasing the admission counter; a racing real-type
control pauses at that boundary.

The snapshot would live through focus/full in the same callback and then be
destroyed. A future shared Arc wrapper must stay under this admission and the
closed callback lifetime. Its charge must be added to source preparation
telemetry and every phase budget. The prior candidate's 492 MiB phase bound plus
this 8 MiB reservation leaves 12 MiB below 512 MiB before any additional wrapper,
serialization or mapping infrastructure. Those additional owners must be audited
inside the remaining margin. **This proposal establishes no aggregate 512 MiB
product claim** and has not been integrated into Sources or prepared storage.

There are no promises about capture speed. The bounded prototype uses exact
scans; any index optimization would need equivalent namespace/hash/ambiguity
tests. Hash-map import iteration is sorted only in this private lookup inventory,
not in the stock graph, bucket, function, command or solver order.

The [trace-gated implementation handoff](implementation-handoff.md) identifies
the exact causal choices, narrow source seams and migration controls. It is
unapplied and awaits the first real mismatch.

The [timing and domain review](timing-and-domain.md) distinguishes constructor
Loaded owners from later ordinary resolution and exact Delayed descriptors. The
[pinned formatter audit](formatter-audit.json) records the actual Unix streaming
Debug chain; it grants no arbitrary formatter or aggregate memory exemption.

## Controls and remaining gates

The API tests cover loaded foreign mapping against the exact stock resolver,
worker-owned values, delayed/missing/busy states with no source-map growth,
namespace and hash mismatch, duplicate original ranges/hashes and equal-content
current files, boundaries/cross-file spans, wrong SourceMap, observed coherent
coordinate relocation, normalization, admission refusal and physical disposal
before release. The relocation control checks what Rust's API does; it does not
declare two preparations equivalent or claim a cross-edit hit.

Existing real local hygiene/parent, imported-word, missing/ambiguous current
binding, stale-line/macro, raw-Arc identity and interpreter/oracle clone controls
must remain. Missing/delayed owners may never pass via a fallback filename guess.
Any future application still needs the actual mismatch trace, complete driver/VIR
and strict source checks, ordinary vstd verification, exact same-input wire,
effects and grouped diagnostics, stock verdict/diagnostic checks, cancellation,
and genuine previous-revision FOCUS restoration. Real HE/Vest edit-catalogue
qualification and the 2× complete-loop gate remain outstanding.

To reproduce just the supplementary API controls:

```sh
python3 engine/verus/candidates/owned-import-mapping/test-api.py \
  --source "$PINNED_VERUS_SOURCE" --work-dir target/owned-import-mapping
```

The harness uses the actual 1.98.1 private compiler types, extracts stock private
resolver methods unchanged, and records source/script/binary identities and raw
compiler/test output. The reduced fixture does not construct a real `TyCtxt` or
compile the complete Verus driver. No shared Cargo target or selected runtime
is used.
