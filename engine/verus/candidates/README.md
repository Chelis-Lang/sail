# Exact graph-read candidate

This candidate has compiled library checks, VIR unit tests and an ordinary Linux
source build, but no qualified runtime. The build verified 2,060 vstd obligations
with zero errors; its exact manifest and artifacts are retained in
`qualifications/2459-linux-build.json`. Runtime source oracles and product
acceptance remain pending. It does not change
`interactive.patch`, the installer or any qualified runtime. See ADR 0028.

Apply `graph-ledger.patch` to a separate checkout of the primary pinned Verus
commit. Do not apply it on top of `interactive.patch`: the candidate is a complete
patch including that baseline. Before building, run:

```sh
python3 engine/verus/candidates/audit-graph-ledger.py --source VARIANT/source
```

After building the candidate in a separate source variant, run the seven ledger tests:

```sh
cargo test --release -p vir --lib sail_graph::tests
```

Use the variant's pinned source toolchain and normal source-build environment.
Then run the existing source tests and the local-only exact oracle:

```sh
python3 engine/verus/candidates/test-graph-ledger.py \
  --engine VARIANT/verus --stock STOCK/verus --z3 PINNED/z3 --out OUTPUT
```

The oracle uses the same input paths and compares original full diagnostics,
function outcomes and solver-input bytes without normalization. It retains
unchanged-callback, body-edit, stale-span, corrupt/misfiled/oversized checkpoint
checks. Cancellation and real-project catalogue gates remain separately
mandatory. Neither the scripts' presence nor static coverage is a runtime pass.

`reporter-ownership.patch` is a narrow overlay for the currently qualified
baseline or the previous graph candidate. It disposes diagnostic message vectors
and all cache-owned nested Arc references before releasing their process-wide
quota. It is not an installer-selected patch. `graph-ledger.patch` already
includes the overlay; do not apply it a second time. Its blocked-destructor
regression exercises both recording and focus reporters, while a separate Arc
lifetime check preserves compiler-owned references.

The allocation-counter destructor audit covers graph Recording, Ledger and final
Capture ownership plus diagnostic RecordingReporter and FocusReporter. The only
other decrement is the preparation miss counter, which owns no allocation.
Span-collection guards restore callback-local state and have no quota to release;
serialized entry/provenance/source storage uses explicit global gates or direct
ownership accounting, rather than destructor counters. Compiler-owned message
clones are working state outside cache ownership; removing cache references does
not invalidate those clones.
