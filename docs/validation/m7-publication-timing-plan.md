# Publication timing instrumentation

Status: source-only implementation; new controls, strict checks, installed
product and diagnostic fixture have not run. This record does not replace the
retained original repair failure or qualify performance.

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
