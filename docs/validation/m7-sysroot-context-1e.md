# Actual pinned sysroot context control

This is a separately frozen, context-only diagnostic against the corrected
`1e8b633` product. It does not execute Cargo, invoke verification, produce a
Current report, or qualify managed acceptance, reader latency, or M7 performance.
The original cold and warm acceptance failures remain recorded in
[m7-sysroot-activation-1e8b633.md](m7-sysroot-activation-1e8b633.md).

The [unselected diagnostic patch](m7-sysroot-context-1e.patch) applies to the
exact `1e8b633572cb680aa27fcebd7dc526ee09278f99` source. Its ignored test calls the
original managed `runner::context` twice. Test-only thread-local hooks record
the existing Lookup/import and natural Prepare responses; they do not change
production decisions, import authority, hash expressions, or daemon code.
An actual sealed product runs the independent daemon. Both contexts discover
Rust and Verus sysroots through the original toolchain probes. No request
contains caller hashes, and no diagnostic code sends Prepare.

The first context retains the original local hash barrier and emits its normal
fill hint only if that barrier succeeds. A separate observation then polls
Lookup, with the unchanged 100 ms whole-RPC budget, for at most 120 seconds from
the natural hint response. This readiness observation belongs only to the
diagnostic; production callers do not wait for construction. It does not
extend the daemon's original 120-second construction cap. The second unchanged
context acquires and retains its original Baseline before Lookup, performs the
original import/barriers, and reports the captured eligible hashes **before**
its local hash loop. Hashes captured by that later loop cannot count as imports.

The control compares the exact context key and complete ordered runtime
identity bytes. Private raw evidence retains discovered roots, request scopes,
wire response categories, any actual snapshot, both identity artifacts,
original caller guard outcomes, phase timers, source/patch/binary/product
hashes, and owned daemon cleanup. The caller is a debug test executable; its
context timings are diagnostic, not release performance measurements.

An absent snapshot is reported only as the observed Prepare acknowledgement,
Lookup response/error, and caller barrier result. The sealed endpoint does not
expose its internal construction refusal reason. An empty Lookup does not
prove a particular memory, traversal, guard, cancellation, or scheduling cause.
The test is ignored and is not added to the default workspace gate.

The authority contract remains [ADR 0031](../decisions/0031-guarded-sysroot-digest-handoff.md)
and the [guarded sysroot guide](../guarded-sysroots.md): only overlapping
independent guards authorize imports; keys, generations, metadata and disk
records do not grant digest authority. Original acceptance and serving bounds
are unchanged.

## Observed result

The single control passed, with normal owned daemon shutdown and unchanged
sealed product hashes. The first context observed an empty Lookup, hashed its
327-file inventory locally, passed its original barrier, and naturally received
`Prepared(true)`. A complete snapshot was observed after 13.965 seconds. The
second unchanged context imported all 327 eligible hashes before its local
hash loop. Both caller guards passed, and the exact context key and complete
ordered runtime identity bytes matched.

The debug caller's first and second contexts took 52.262 and 16.380 seconds.
Their sysroot-hash phases took 18.908 seconds and 1.899 milliseconds; their guard
phases took 22.114 and 10.740 seconds. These observations establish that the
actual pinned inventory handoff works in this scope. They are not a release
speedup, a proof result, or acceptance measurements. Inclusive owned lifecycle
was 83.501 seconds; no forced termination was needed.

The [portable record](m7-sysroot-context-1e.json) includes all phase observations,
raw evidence seals, actual runtime/product/control identities and the initial
diagnostic compile typo. It preserves the distinction between this successful
context control and the original failed managed scenarios.

After the diagnostic closed and its raw data and runnable executable were
sealed, a holder audit inspected 609 processes and 198 descriptors and found
no users of its private compiler target. Only that target's inactive
`debug/incremental` subtree was removed: 1,392,651,477 logical bytes. Dependency
artifacts, fingerprints, frozen binaries, source patches and raw evidence remain.
The [cleanup record](m7-sysroot-context-1e-cleanup.json) does not claim physical
reclaimed storage or affect shared targets or other projects.
