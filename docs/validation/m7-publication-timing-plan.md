# Publication timing instrumentation

Status: instrumentation source `db4c65f` passed strict Clippy and its new
controls in a complete workspace run, which failed the original shared HTTP
coalescing deadline (611 of 612 tests passed). The original pinned-solver suite
passed separately, 41/41. The [failed complete scope](../../bench/validation/m7-linux-publication-timing-core-db4c65f-failure-2026-10-11.json)
remains failed. An [instrumented diagnostic product](../../bench/validation/m7-linux-publication-timing-diagnostic-product-db4c65f-2026-10-11.json)
has built and installed, explicitly without qualification credit. The
[original repair timing run](../../bench/validation/m7-linux-publication-timing-original-repair-db4c65f-failure-2026-10-11.json)
fails at 120.138 seconds during the missing-selector foreground case; later
regression and MCP scenarios are unattempted. Cleanup reports no errors, and a
subsequent owned-path process/descriptor census finds no holders. These
observations do not replace the retained original repair failure or qualify
performance.

## Measured publication seam

Two surviving completed worker records spend 4.881 and 4.731 seconds in
independent freshness establishment after child exit, versus 0.361 and
0.725 milliseconds parsing the document. Status persistence costs 27.581 and
5.157 milliseconds. Managed result, receipt and compiler-family publication
records are also milliseconds. The latest-256 ring overwrites 534 of 790
started records, so the cold worker's publication interval is unavailable;
cancelled-worker fields remain absent rather than fabricated. All four bounded
managed setup files and the ring are retained with their exact hashes.

This isolates a substantial post-exit cost in the observed jobs. It does not
prove which operation inside freshness causes it or generalize to another host.
The borrowed-document candidate removes a correct duplicate parse, but these
measurements defer it as a repair-deadline solution. The next core work targets
self-invalidating metadata publication and independent freshness construction,
preserving synchronous readiness, exact runtime/input validation and Current
classification. No timing observation grants proof or completion authority.

The original a983ab2 repair fixture exhausted its unchanged 120-second total
budget at the last late-exec poll. Earlier successful, missing-selector and
Current negative cases completed. Retained late-worker stdout contains a full
Current negative document, but its presence during cleanup does not timestamp
emission or persisted terminal status before the deadline. Completed jobs show
roughly 3.17–3.47 seconds between runner wall duration and recorded job duration;
those intervals do not identify one causal block. No retry or new proof actor
is part of this source-only scope.

## Existing sinks and authority

The managed `SAIL_JOB_EVENTS` sink gains result, guarded receipt and ordinary
compiler-family publication labels. Its existing 64 files/process, 64 rows/file,
16 KiB/file, exclusive mode-0600 creation and fail-safe writes remain unchanged.
The recorded result wall duration is still sampled before publication.

The daemon `SAIL_JOB_RPC_TIMINGS=1` sink gains a fixed worker operation and five
optional numeric fields in its existing latest-256 ring, capacity refused above
512. The child marker records successful `try_wait` observation relative to the
daemon origin. Existing read_document, optional establish_freshness and status
save blocks are timed without replacing their results. Status timing includes
its original jobs-lock acquisition. Freshness remains before Completed and FIFO
dispatch is unchanged. A cancelled or early-error path may omit later fields.
Shutdown-only private export remains capped at 256 KiB; contention drops
metadata rather than waiting. No job identifiers, paths, environment values,
report contents, new runtime hashing, background work or second queue are added.
The timing row's persisted flag cannot authorize a proof or Current.

## Controls and next execution

New source controls check fixed publication labels and retained error behavior;
disabled worker observations; error-path stage presence without fabricated
freshness; fixed record size; and worst-width serialization of a full ring
within the existing export cap. Existing default-off, contention, poison,
exhaustion, private output and complete-frame controls remain intact.

After independent code review and a sole compilation slot grant, affected
commands are:

```sh
cargo test -p sail-verus --lib managed::diagnostics -- --test-threads=1
cargo test -p sail-daemon --lib jobs::rpc_diagnostics -- --test-threads=1
cargo clippy -p sail-verus -p sail-daemon --all-targets -- -D warnings
```

Actual normal/error worker integration, the whole workspace gate and installed
product must validate the new call sites separately. Any first compile or
strict failure is retained, without preemptive lint-policy changes. A subsequent
owner-granted diagnostic may enable the existing sinks on a fresh owned daemon
while keeping the original fixture's 120/30/5 limits and correctness assertions.
Trace loss or overwriting is an unavailable observation, not a cause. None of
these component controls alone qualifies the original product fixture or the
repair benchmark.

See [daemon field boundaries](../result-rpc-diagnostics.md) and
[managed publication intervals](../managed-cold-start.md). Moving freshness
construction off the critical path requires a separate authority and lifecycle
review: publishing Completed before optional construction can make the first
Result/Status classify Unknown. This instrumentation makes no such change.
