# Optional result RPC stage diagnostics

Set `SAIL_JOB_RPC_TIMINGS=1` on an owned diagnostic daemon to retain bounded RPC
stage metadata. It is off by default. Neither setting changes proof admission,
currentness, output seals, protocol fields, request deadlines or reader checks.
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
| `output_seal_us` | Bounded full stdout read and exact seal validation |
| `freshness_us` | Existing current-input validation and classification |
| `frame_key_us` | Complete stored-job serialization and frame-key hash |
| `frame_lookup_us` | Immutable frame cache lookup |
| `encode_us` | Cold complete result parsing and all variant encoding |
| `write_us` | Complete socket output, including its existing error fallback |
| `total_us` | Acceptance to recording after socket output |

`frame_hit` and the existing computed `currentness` are observations, not cache
authority. `frame_bytes` counts the complete selected Result frame, including its
four-byte header. `write_succeeded` means transport success; an Error response can
be written successfully. Missing stages are null for paths that did not execute
them, including nonterminal/older-record fallback. Buffer disposal and other
unmarked work can contribute to `total_us`; stage times are not guaranteed to sum
to the total.

Shutdown seals the ring before export and rejects late observations. Export is
outside serving, capped at 256 KiB, uses a new private mode-0600 temporary file
and atomically replaces the final trace. It creates no background thread or
queue. This bound describes owned metadata storage, not RSS or an interruptible
filesystem deadline. A forced exit, contended export, poisoned sink or write
failure may leave diagnostics absent or incomplete; none changes the original
proof result or cleanup error. The total started count also includes requests
still in flight when the ring was sealed.

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
