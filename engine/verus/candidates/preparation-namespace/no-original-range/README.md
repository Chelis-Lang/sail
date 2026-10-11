# Exact stock absence of an original imported range

This is an **unselected, file-only candidate**. Its new controls have not been
compiled. It combines the independently tested preparation-service retirement
acknowledgement with a separate `NoOriginalRange` authority. Source controls,
fresh ordinary vstd verification, the complete runtime manifest and the original
qualification remain required. The selected runtime is unchanged.

The retained [boundary diagnostic](../boundary-diagnostic/revision0-result.json)
observed three missing-range refusals and a separate `pair-busy` admission miss.
Actual pinned metadata decoding in the
[stock absence control](../../preparation-retirement/absence-control/controls.json)
fully consumed the sealed vstd metadata's 145 original rows. Untouched stock
lookup, unpacking and current-unit rendering returned `None` for all three
observed ranges without calling the loader. The reduced control establishes
this metadata boundary; it does not establish a cache hit, complete diagnostic
equivalence, or the exporting callback's source-file state. See VI-0014 in
`docs/verus-issues/VI-0014-exported-foreign-span-metadata-gap.md`.

The new role accepts only a known crate with a complete constructor inventory,
valid low/high words and no original range containing the low position. Each
original hash/range must match one imported row. Existing duplicate, overlap,
missing-hash and family failure checks remain. Count equality after those checks
establishes the bijection; incomplete or oversized-name inventories offer no
absence authority. The original `record()` refusal is unchanged. A covered
Unavailable record, an unknown crate, malformed raw owner or invalid range
cannot become this role merely because stock rendering returns `None`.

The role supplies no source content, coordinates, loaded owner or loader recipe.
It binds the exact complete original range/hash/OS-filename digest and the actual
current raw unit, packed words, span ID and spelling. Current prepared decoding
still requires the exact current SailSpanKey and rebinds its raw Arc. It bypasses
the old packed-word-only provenance memo, including existing stale entries.
Current SourceMap growth cannot create a row in the immutable imported range
inventory. The borrowed namespace guard and existing work cap remain required
on both direct and parallel preparation paths. Stock Reporter and resolver are
unchanged.

A distinct provenance tag and v10 preparation domain reject intact v9 entries.
The existing current-domain assertion changes accordingly; all earlier negative
assertions remain, and a v9 rejection control is added. No query, diagnostic or
solver normalization is introduced. The original strict stock stderr, same-input
lowering oracle, 600-second compiler limits and genuine previous-revision reuse
checks remain unchanged.

Physical ownership stays within the same 8 MiB family. A fixed optional 32-byte
digest is inline in every Record, so its actual size and reserved Vec capacity
are charged before allocation. Digest computation streams borrowed metadata and
filename bytes; it copies no metadata table or source file. Initial seeds,
classification records and names remain covered by the same owner. Physical
record disposal precedes quota release. The broker keeps its independently
reviewed 16 KiB active plus 64 KiB sticky retirement/queue reserve. The conservative
maximum remains 500 MiB plus 80 KiB, below the unchanged 512 MiB cap.

`identity.json` binds the exact composed full patch, narrow role delta, eight
source files, lockfile and actual stock-control evidence. Full application to
pinned files, application after the positive retirement correction, and reverse
application were checked without changing the native source or launching a
compiler. Existing helper tests are a byte-exact prefix; the prior Deferred
Reporter tests and positive retirement implementation are retained.

New uncompiled controls cover stock absence/current SourceMap growth, incomplete
inventories, complete filename/hash/range identity, namespace overflow, raw-unit
codec rebinding and changed current keys, pending handoff, covered Unavailable
refusal, prepopulated memo sensitivity, and the actual direct/parallel validator
adapter. Existing physical/drop controls and the actual finish-call-removal
counterfactual remain required. None of these planned controls replaces full
runtime or product acceptance.

`run-source-controls.py` prepares the sole complete library/strict invocation in
the closed existing private target. It binds the prior positive retirement
result and exact source, saves the original source before applying only the role
delta, checks the composed full patch in reverse, then runs every VIR/driver
`sail_` control serially and both all-target strict checks. Internal concurrency
is retained. It strips engine selectors and records ordinary scheduling,
resource/actor identity, raw phase logs, source closure and all four prior runtime
seals. Launch remains contingent on the exact composed-source review; no runtime
or proof acceptance is copied from the previous target's test executable.
