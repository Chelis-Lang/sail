# Independent Loaded-import authority review

This is a static review of proposal commit `5f96e28`, including its inert
`owned-import-mapping/capture.rs` helper and eleven pinned-API controls. It does
not apply a source change, run another proof, approve span equivalence, or
qualify a runtime. The [causal diagnostic](span-provenance-causal-diagnostic.json)
and both original strict failures remain unchanged.

The helper validates a complete original crate/range/hash association against
one current Foreign file. It checks normalized range length, current source
hash bytes, stable crate, unique current bounds and same-crate content-owner
ambiguity. Its checked translation refuses crossing the original file boundary.
Unavailable Delayed, missing and busy records remain unavailable; capture does
not call the stock loader. Records contain owned values rather than compiler
contexts, source-file Arcs or old raw spans. Physical records and names are
destroyed before the single 8 MiB family admission is released. These checks
have no identified static authority blocker in the inert helper.

The future constructor/pre-focus integration still needs the following exact
boundaries:

- Preserve original expected hash/range authority until pre-focus validation.
  A current file hash alone cannot reconstruct original metadata authority.
  Seed, late validation, snapshot, Arc header and any memo belong to the same
  admitted family; a second uncharged snapshot is unsupported.
- Check record storage arithmetic before reservation, followed by an actual
  capacity check. The present fixed Record shape and 4,096-record limit fit the
  family, but adding seed fields must not move the admission check after a
  potentially larger allocation. Include old/new name-growth overlap.
- Accept only a current raw **unit** import, with exact current
  `(id, data, as_string)` binding. An arbitrary non-local Any object containing
  two words does not establish an import. Local raw position, hygiene and parent
  checks remain exact.
- Validate the current original-to-loaded translation on the compiler thread,
  then publish one immutable mapping. Retain typed current hash kind/value,
  stable file ID, full bounded name and both lengths. Current coordinates may
  serve as checked translation values only under an explicitly reviewed new
  domain, with old-domain disk and batch rejection controls.
- Record the actual selected state from this admitted mapping. The first
  mismatch's original words are
  `[2208441662476345389, 316229852143522]`. The f833 trace records contributing
  SourceMap files, not resolver state. Existing Foreign files do not prove
  Loaded, as the real Delayed-plus-existing-Foreign control demonstrates.

The pinned stock constructor in `rust_verify/src/spans.rs:120` selects the
remapping Delayed branch only when `RealFileName::local_path()` is available.
The captured contributor's formatted name has `local: None`; this is a useful
observation to check against the pinned accessor, not a substitute for the
selected resolver state. Capture must retain unsupported states rather than
load a file to establish eligibility.

The consumer boundary supports this narrow investigation. Prepared VIR
lowering/effect paths propagate typed spans and their exact identities; they
have no SourceMap or SpanContext from which to read foreign coordinates.
`vir/messages.rs:73–85` rebinds decoded spans to current raw Arcs. The current
stock Reporter at `rust_verify/src/verifier.rs:94/107` resolves imported words
through the current SpanContext, while local diagnostic sorting at line 2267
uses local raw SpanData. Thus current raw binding is necessary, but it cannot
replace resolver authority. Grouped/immediate diagnostics, fresh frontend and
late EXEC remain independent requirements.

The observed 25-byte relocation identifies why the old broad imported-owner
digest misses. It does not authorize omitting owners or starts from that old
digest. The reviewed candidate must still pass relevant-file/hash-kind/name/
namespace/normalization/range/ambiguity/state negatives, current binding and
local hygiene controls, physical admission/drop controls, all original source
tests, fresh vstd, and the original 600-second previous-revision FOCUS and full
same-input gates. No build is authorized by this review alone.

## Integrated implementation review

The independently inspected private implementation snapshot has complete patch
SHA-256 `31fb3a561a2970023fda7d1593d7a19476fe5899897763a8d7278424dbf1b323`
and delta `c049a63642b8961bf990db4617366d1fe051cb1fb3b218835392afbba0c4d2f4`.
Both patch hashes and all six changed-source hashes matched its identity record.
This records a static review, with no compilation, runtime test or qualification.
Any later export needs its own identity check.

The integrated `rust_verify/src/sail_owned_imports.rs` checks record-count and
`count * size_of<Record> + 1024` before admission/reservation, then checks actual
capacity. Expected original bytes and their length live inline in that record.
Bounded name growth includes both old and new buffers. The owner disposes the
complete record Vec and names before releasing its process-wide admission.
`spans.rs` moves that same owner out of its slot and revalidates it before one
shared Arc publication. Late validation retains no second table, compiler/source
Arc or formatted name.

Current typed hash, stable file ID, namespace, full name and both lengths remain
exact. Original metadata supplies raw hash bytes without an algorithm tag; the
candidate keeps those bytes and separately binds the actual current typed hash.
Initially unsupported states never become eligible, even when ordinary compiler
work later loads their file. Raw unit imports, exact current binding and local
position/hygiene/parent checks remain separate requirements. No additional static
authority or physical-ownership blocker was identified in this snapshot.

The review found and the snapshot corrected generic serializer error-type
mismatches, missing constructor/failure admission telemetry and repeated trace
error publication. Owner peak is sampled immediately after admission, before
allocation can fail; clear preserves the family peak and terminal sampling
includes it. Added controls cover failed admitted capture, typed-hash-kind-only,
stable-ID-only and unnormalized-length changes. These controls are source tests
awaiting actual compilation, rather than reported passes.

The wholly contained 8 MiB family adds to the previously audited 492 MiB maximum:
the prospective total is 500 MiB under the unchanged 512 MiB limit. The family
already includes original authority, record/name capacities, growth and
Arc/control margins; there is no additional seed snapshot to add outside it.
Existing decoded Portable, memo/effects, reporter, graph scratch and observer
overlaps remain charged by their earlier phase argument. Admission telemetry
does not prove transient allocation bounds or compiler RSS.

The bounded trace now includes exact current `as_string`, so a changed imported
crate ID in a new runtime need not be guessed from the old packed words. All
serialization failures stop that trace under the emitter lock; a single bounded
unavailable record replaces the failed frame. One remaining evidence concern is
that tracing occurs before memo lookup, so duplicates can consume the record/byte
cap before the known failing binding. Qualification must retain its actual state
record, or use a separately bounded targeted observation. Exhausted or missing
evidence cannot establish Loaded authority. This concern does not authorize any
provenance, cache-key or original-test change.

The final export closes that observation concern with private selector
`SAIL_ENGINE_IMPORT_TRACE_TARGET=div-mod-relocation`, applied before trace
admission. Complete patch SHA-256 is
`e6f54134fd8478b3b82f3d2513da3a883b70a7f6cd08d2af3985c207abf785ef`;
delta is `dbae5d4f95508219341ecf611ffca399c48c57fba187feee79cc026fdbf31876`.
The exact retained spelling is `vstd/arithmetic/div_mod.rs:519:50: 519:56 (#0)`.
Review caught an earlier selector missing that space; the earlier export stays
retained and the correction adds an explicit no-space negative, alongside
adjacent-range, hygiene, prefix and unknown-selector controls. This selector
filters observations only; it does not affect authority, keys or preparation.
Both final patch hashes and all six source-file hashes were independently
checked again. Static approval permits the authorized private build, with
actual tests, fresh vstd and all original qualification gates still pending.
