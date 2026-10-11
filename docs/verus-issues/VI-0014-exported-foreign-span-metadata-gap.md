# VI-0014: Exported foreign spans can lack original source metadata

- **Status:** open; observed locally, not filed upstream.
- **Verus:** pinned `0.2026.10.04.426d8b0`, source
  `426d8b01e7ffb910a36c15e1f870ff985bb61eef`, Rust 1.98.1.
- **Found:** 2026-10-11 in the optional M7 source-engine qualification.
- **Impact:** conservative prepared-lowering reuse refuses these imported spans.
  The observed stock and source-engine full proofs still agree: nine verified,
  zero errors and exact diagnostics. No incorrect proof result is established.
- **Workaround:** none selected. Missing source ranges must not be fabricated,
  loaded on demand or interpreted as coordinate authority.

## Observed coverage boundary

Pinned `source/rust_verify/src/spans.rs` packs every rustc span as the exporting
local crate ID and its original low/high positions. The constructor puts only
`ExternalSource::Unneeded` files in `local_files`; Foreign files follow a
separate import path. `source/rust_verify/src/verifier.rs` exports crate metadata
with `original_files: spans.local_files.clone()`.

Consequently a span that originally points into a Foreign core/alloc file can
carry the exporting vstd crate ID without a corresponding original range in
that vstd export. In the retained vstd artifact, the three refused low/high
ranges are `5083697..5083788`, `3783531..3783599` and `3867539..3867615`.
Their compiler spellings refer to core's function and allocator traits and
alloc's boxed implementation. The packed crate word is
`2208441662476345389` in this particular sealed artifact; it is not a general
constant to encode in the implementation.

Stock `pos_to_extern_source_file` searches the immutable imported file table.
`unpack_span` returns `None` if the low position has no containing record;
that branch precedes the optional source loader. A separate executable stock
control and typed metadata lineage check are being prepared. Source inspection
alone does not qualify a new Sail restoration role.

The current Sail helper labels both an absent crate and an uncovered original
range `UnknownCrate`. The diagnostic label therefore does not establish that a
crate is missing. Truly unknown crates, unsupported raw span forms, invalid
ranges and ambiguous mappings must retain safe misses.

## Reproduction and evidence

The retained [revision-zero diagnostic](../../engine/verus/candidates/preparation-namespace/boundary-diagnostic/revision0-result.json)
binds the actual compiler, exact failed HumanEval input, original context and
600-second bound. The original assertions remain: its compiler completes in
14.619 seconds, but positive restored-hit admission fails. The diagnostic is
unqualified and does not recover the failed qualification.

The [diagnostic controller](../../engine/verus/candidates/preparation-namespace/boundary-diagnostic/run-diagnostic.py)
replays that frozen validation workspace with fresh preparation/oracle output,
without copying proof evidence. The unchanged stock exporter and unpacking
branches above are the source-level reproduction seam. A standalone minimal
compiler fixture is still pending; the retained project run is not presented
as one.

A distinct `pair-busy` refusal is also observed at 81,920 retained control bytes.
It concerns compiler-service retirement and is addressed by a separate
[acknowledgement candidate](../../engine/verus/candidates/preparation-retirement/README.md).
That availability fix cannot establish authority for the uncovered ranges.

Any proposed absence role must prove complete immutable constructor coverage,
faithful stock `None` behavior without loading, exact current raw span binding,
unchanged old role/refusal controls and physical bounds. It must then pass the
original same-input lowering oracle, independent stock outcomes and complete
source qualification before selection.
