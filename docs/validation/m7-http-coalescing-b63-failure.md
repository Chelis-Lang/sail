# HTTP coalescing failure and diagnostic scope

The combined `b63af10` workspace run completed 606 of 607 tests successfully.
The original eight-reader HTTP coalescing fixture failed: all callers timed out
instead of returning `Hit`. Solver tests were not attempted. The append-only
measurement is [the failure record](m7-http-coalescing-b63-failure.json).

The configured timeout remains 250 ms, every caller must return a verified
`Hit`, the server delays its reply by 75 ms, and exactly one GET is required.
The failed run did coalesce seven followers into one GET. It received all
285 request-header bytes in its existing single 4096-byte `read`, with no read
error. Acceptance to that read's completion took 87.207 ms. The server sent
its response 249.960 ms after the first lookup began. These wall measurements
do not establish which thread ran, used CPU, or waited during that interval.
The original failure cannot be attributed to machine load from this evidence.

## Read-only implementation audit

The fixture already performs one bounded-buffer header read; it does not read
one byte at a time. The production client builds a GET with an identity-encoding
header and private bearer token. Its pinned reqwest 0.13.5 blocking client sends
that request through an internal channel to its owned single-thread async
runtime; that runtime executes the request and returns the response through a
oneshot channel. Source references are `ClientHandle::execute_request` and
`ClientHandle::new` in reqwest's `src/blocking/client.rs`.

The pinned client defaults to TCP_NODELAY enabled (`Config::default` in
`src/async_impl/client.rs`, propagated by `src/connect.rs`). There is no
established justification for selecting another socket flag, dependency
optimization profile, longer timeout, fixture preparation, or relaxed oracle.
The admission table holds small pending owners, performs no HTTP work, and
all seven followers shared the same pending entry in the failed scope. The
existing helper directly publishes independently verified replies; it does
not wait for the initiating caller to resume before publishing them.

## Prepared observation, not a production change

[The unselected diagnostic patch](m7-http-coalescing-owner-diagnostic.patch)
is based on `d8a59d2`. Production HTTP code is byte-identical to `b63af10` in
these two files before applying that patch. It adds test-only metadata samples
around the unchanged `send`, body read, and fixture request read. Samples
retain the calling thread and this process's reqwest runtime task stat,
schedstat and wait-channel fields, with task birth identities in the raw stat
records. Analysis must refuse a delta when birth identity is missing or changes.
It reads no environment, credentials, request body or report data.

Each sample enumerates at most 64 tasks and reads at most 4096 bytes from each
proc field. It records its own start/end and completeness; the 10 ms traversal
check is a best-effort stop between reads, not a hard bound on kernel file I/O.
At most 16 samples are retained by each recorder. Logging occurs only when the
owned fixture or metrics are dropped. Synchronous observation can change this
fixture's timing, so a successful instrumented run would not qualify the
original gate or explain the historical failure by itself. Every original
assertion, eight-reader barrier, 75 ms sleep and 250 ms deadline is unchanged.
No diagnostic executable, Cargo build or additional test run has started.

The next useful observation distinguishes caller/runtime/server CPU and
runqueue wait across accepted-to-read, send-to-response and body-read phases.
A production change requires evidence of avoidable work. The benchmark and
real-reader workflows remain separate scopes; this failure grants neither
integration nor latency credit.
