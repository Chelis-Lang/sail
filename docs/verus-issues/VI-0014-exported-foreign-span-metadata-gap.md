# VI-0014: Exported spans can lack original source metadata

- **Status:** moot (2026-10-12): it affected only prepared-lowering reuse, which
  rescue lane L7a removed from the engine (ADR 0038). Kept as a record; not
  filed upstream.
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
`ExternalSource::Unneeded` files in `local_files`. This is an early constructor
snapshot, before `construct_vir_crate`; Foreign files follow a separate import
path, and files arriving later are also absent from that snapshot.
`source/rust_verify/src/verifier.rs` exports crate metadata
with `original_files: spans.local_files.clone()`.

An exported span can therefore carry the exporting vstd crate ID without a
corresponding original range in that vstd export. The original SourceFile's
`external_src` state at constructor capture was not observed. Neither Foreign
exclusion nor later file arrival is established as the cause of this instance.
In the retained vstd artifact, the three refused low/high
ranges are `5083697..5083788`, `3783531..3783599` and `3867539..3867615`.
Their compiler spellings refer to core's function and allocator traits and
alloc's boxed implementation. The packed crate word is
`2208441662476345389` in this particular sealed artifact; it is not a general
constant to encode in the implementation.

Stock `pos_to_extern_source_file` searches the immutable imported file table.
`unpack_span` returns `None` if the low position has no containing record;
that branch precedes the optional source loader. The
[read-only metadata lineage](../validation/m7.md)
binds the exact sealed 30,076,214-byte vstd artifact and its 22,312-byte metadata
suffix: all 145 recorded original ranges omit the three positions. That bounded
inspection is separate from the subsequent
[typed stock-resolver control](../validation/m7.md).
That control fully decodes the pinned metadata and executes byte-exact stock
lookup methods in a reduced SourceMap fixture. All three uncovered ranges return
`None` without loading; a real covered descriptor reaches the instrumented
loader once. Unknown crates and covered-but-unavailable records are distinct
negative cases. The original exporting compiler's SourceFile state remains
unobserved. Neither control qualifies a new Sail restoration role.

The current Sail helper labels both an absent crate and an uncovered original
range `UnknownCrate`. The diagnostic label therefore does not establish that a
crate is missing. Truly unknown crates, unsupported raw span forms, invalid
ranges and ambiguous mappings must retain safe misses.

## Reproduction and evidence

The retained [revision-zero diagnostic](../validation/m7.md)
binds the actual compiler, exact failed HumanEval input, original context and
600-second bound. The original assertions remain: its compiler completes in
14.619 seconds, but positive restored-hit admission fails. The diagnostic is
unqualified and does not recover the failed qualification.

The [diagnostic controller](../validation/m7.md)
replays that frozen validation workspace with fresh preparation/oracle output,
without copying proof evidence. The unchanged stock exporter and unpacking
branches above are the source-level reproduction seam. The reduced executable
stock-method fixture is retained separately from the full project run; it does
not reproduce historical source-file arrival or run an entire compiler proof.

A distinct `pair-busy` refusal is also observed at 81,920 retained control bytes.
It concerns compiler-service retirement and is addressed by a separate
[acknowledgement candidate](../validation/m7.md).
That availability fix cannot establish authority for the uncovered ranges.

Any proposed absence role must prove complete immutable constructor coverage,
faithful stock `None` behavior without loading, exact current raw span binding,
unchanged old role/refusal controls and physical bounds. It must then pass the
original same-input lowering oracle, independent stock outcomes and complete
source qualification before selection.
