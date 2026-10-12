# Optional result RPC and worker publication diagnostics

Set `SAIL_JOB_RPC_TIMINGS=1` on an owned diagnostic daemon to retain bounded RPC
stage and worker publication metadata. It is off by default. Proof admission,
currentness, output seals, protocol fields, request deadlines and reader checks
are unchanged.
The observation-only key is excluded from CLI submissions and managed compiler
input environment selection, like `SAIL_JOB_EVENTS`.

For an already configured private cache outside verification inputs:

```sh
SAIL_JOB_RPC_TIMINGS=1 sail serve --cache "$SAIL_CACHE_DIR"
```

After normal owned shutdown, read
`$SAIL_CACHE_DIR/projects/jobs/rpc-timings.json` for the explicit `--cache`
command above. Default cache selection instead uses
`$SAIL_CACHE_DIR/results/projects/jobs/rpc-timings.json`. No trace file is written
during a timed RPC. Do not change a running frozen acceptance daemon or collector
to enable diagnostics; start a separate explicitly labelled diagnostic scope.
If that scope uses retained reports, its own continuous freshness owners must
be established independently. A copied report or timing row never admits a proof.

## Metadata and bounds

The ring retains the latest 256 fixed metadata rows, not the first setup polls.
It is preallocated only when enabled and never grows during serving. The initial
allocator capacity is refused if it exceeds 512 rows. Each row contains only
integers, optional integers, booleans and fixed enums; there are no model values,
job IDs, source paths, arguments, environment values or report content. Counts
include overwritten rows and records dropped on lock contention or poison.
Admission uses `try_lock`, so recording cannot wait behind another RPC.

An observation sequence and time relative to daemon start allow ordering. The
export header also records daemon PID and its observation-origin wall time.
Bind these to the owned launch record; a trace left by an earlier service is not
evidence for a later launch. Wall time is metadata, not freshness authority. The
total interval begins after socket acceptance and includes worker dispatch delay;
it cannot measure the client's earlier connect or listener-accept wait. Durations
are integer microseconds, with these boundaries:

| Field | Existing operation observed |
|---|---|
| `dispatch_delay_us` | Accepted socket to its owned request thread starting |
| `read_us` | Complete request-frame read and parse |
| `lookup_us` | Stored job lookup and clone |
| `output_seal_us` | Complete output validation: accumulated owner barriers and/or original full stdout read and exact seal |
| `freshness_us` | Existing current-input validation and classification |
| `frame_key_us` | Complete stored-job serialization and frame-key hash |
| `frame_lookup_us` | Immutable frame cache lookup |
| `encode_us` | Cold complete result parsing and all variant encoding |
| `write_us` | Complete socket output, including its existing error fallback |
| `total_us` | Acceptance to recording after socket output |

The same ring also accepts `worker_publication` rows from the existing FIFO
worker. Their start is the worker beginning execution, not socket acceptance or
job submission. `dispatch_delay_us` therefore has no queue-latency interpretation
on these rows. They contain these additional fixed fields:

| Field | Existing operation observed |
|---|---|
| `child_wait_us` | Existing polling after Running status is saved until the child exit is observed |
| `child_exit_observed_us` | Successful exit observation relative to the daemon diagnostic origin |
| `document_read_us` | Complete existing result-document read and projection construction |
| `freshness_establish_us` | Existing optional freshness-owner establishment before terminal status |
| `status_save_us` | Status-lock acquisition, original terminal mutation and persistence |

For worker rows, `total_us` ends after status persistence and `write_succeeded`
means that persistence succeeded, including an InfrastructureError status. It
does not mean verification succeeded. Cancellation or an early execution error
can leave child/document fields null; failed document reads still record their
duration. No timestamp is inferred from a run ID or retained stdout. There is
no job ID in this private metadata, so overlapping request rows require the
owned launch record and operation discriminator for interpretation. Worker
records compete with RPC records for the same 256-row tail; no second ring,
thread or queue is created.

Freshness establishment remains synchronous before Completed publication.
Neither timings nor a child-exit marker confer Current. The ordinary complete
document, input/runtime checks, cancellation, status errors and FIFO behavior
retain their existing order. Managed runner publication timings are documented
in [managed cold start](managed-cold-start.md).

`frame_hit` includes cached responses reached through original output hashing;
it does not establish guarded-output shortcut admission. The existing computed
`currentness` is also an observation, not cache authority. `frame_bytes` counts the complete selected Result frame, including its
four-byte header. `write_succeeded` means transport success; an Error response can
be written successfully. Missing stages are null for paths that did not execute
them, including nonterminal/older-record fallback. Buffer disposal and other
unmarked work can contribute to `total_us`; stage times are not guaranteed to sum
to the total. Repeated boundaries accumulate rather than replace earlier
intervals, including a failed final owner barrier followed by full fallback.

Large `write_us` values may reflect receiver progress rather than server CPU.
The [complete-result offline control](validation/m7-result-transfer-profile.md)
separates send wall time, own-thread CPU, receiver transfer and full parsing. Its
socket-buffer control did not improve complete client p95 materially; no socket
change is selected, and it is not live-reader acceptance.

Shutdown seals the ring before export and rejects late observations. Export is
outside serving, capped at 256 KiB, uses a new private mode-0600 temporary file
and atomically replaces the final trace. It creates no background thread or
queue. This bound describes owned metadata storage, not RSS or an interruptible
filesystem deadline. A forced exit, contended export, poisoned sink or write
failure may leave diagnostics absent or incomplete; none changes the original
proof result or cleanup error. The total started count also includes request
and worker observations still in flight when the ring was sealed.

The expanded fixed record has a source control requiring at most 512 bytes and
maximum-width serialization of all 256 rows plus a header allowance within the
existing 256 KiB export cap. With at most 512 reserved slots, requested ring
storage is at most 256 KiB. Export bytes are separate transient storage;
allocator metadata and RSS are not these quantities. These new controls remain
uncompiled until the affected source validation runs.

Default operation allocates no rows and takes no stage clocks or trace I/O.
The original seal read, freshness checks, complete preencoded frames and error
paths retain their order. There is no proof/currentness memo, partial response,
new retry or increased timeout.

## Validation scope

Controls cover default-off behavior, bounded tail/order, contention, poisoned
storage, late publication, private export and exact complete socket-response
equivalence for a Cancelled job's Status, null-result Result and stable errors
without launching a worker. It does not exercise a completed full-report frame
or a warm complete-frame cache hit.
Pure environment controls compare selected inputs with diagnostics off/on.
The [qualification record](validation/m7-rpc-stage-recorder.json) binds the
implementation bytes, commands and retained development failures. These host
controls are not a Linux gate or real live-reader performance qualification.

The [original current reader attempt](validation/m7-current-reader-workflow.md)
still failed Result p95 at 394.713 ms, with Rust and retained CLI/MCP unattempted.
The [separate complete-graph offline profile](validation/m7-current-result-components.md)
does not explain that live latency. This diagnostic supplies missing per-RPC
attribution; it does not claim a remedy. The original four readers, 200 samples,
strict p95 below 200 ms and five-second RPC bounds remain required. See
[reader acceptance](reader-acceptance.md), [freshness](freshness.md) and
[the roadmap](ROADMAP.md).

The [one real current-product observation workflow](validation/m7-rpc-stage-workflow.md)
completed genuine Current setup but failed original Python Result p95 at
403.341 ms; Status p95 passed at 147.290 ms. All 200 original admission and
per-call actual verifier checks passed. Shutdown exported only the latest
43 completed Result/Status pairs after cleanup polling displaced earlier rows.
Those complete Result frames were preencoded hits; their write and freshness
intervals are material, with no demonstrated serialization remedy. Rust and
retained integration were unattempted in that fail-fast workflow.

For publication attribution on a newly sealed product, the
[prepared current-product controller](validation/m7-current-publication-controller.md)
uses the repository's current 23-field timing launcher and original repair
fixture. Its pure failure controls provide no new product timing or acceptance
credit. Transport loss remains uncertain; it cannot certify native shutdown.

The [product 730 delivery review](validation/m7-result-delivery-730-review.md)
maps current complete-frame hits, guarded output/fallback, independent freshness
and unchanged Python parsing. Cached socket writes hold no full-graph encoding
or cache lock. Freshness inspection still copies full input path inventories for daemon
consumers that only need revision/complete, and completed Status still reads and
hashes stdout. Immutable snapshot delivery and sharing the existing output-only
owner are conditional designs, not enabled optimizations or latency claims. The original full reader census remains
required, and no additional transport change is selected by that review.
