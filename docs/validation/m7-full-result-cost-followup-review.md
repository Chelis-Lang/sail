# Full Result cost follow-up review

The current cached sender has no additional graph-encoding or complete-frame
copy seam supported by the retained evidence. This read-only review binds source
revision `af582d566db112cfc063174aa8d128e05439e6dd`; no compiler, daemon,
collector, proof or Docker lifecycle actor ran. The metadata-publication
candidate remains separate and uncompiled.

The [original descriptor census](m7-descriptor-reader-terminal.md) retains all
200 valid Python samples, with four readers and a complete 4,313,743-byte Result
body. Result p95 343.142 ms fails the unchanged strict 200 ms bound; Status
120.526 ms passes. Rust and subsequent retained scenarios were unattempted.
The latest recorder retained only 42 complete Current Result calls. Every one
hit an encoded frame and had no encoding interval. Frame selection p95 was
0.003 ms, socket-output p95 194.522 ms, and the same-call pre-output calculation
had p95 63.794 ms. See the [prior transport audit](m7-cached-result-transport-review.md)
for raw binding and the same-call calculation. No request identifier joins
these tail rows to the complete client samples. Independent quantiles cannot
be added or subtracted to explain the client p95.

## Source path and remaining measured work

`crates/sail-daemon/src/jobs/store.rs::result_frame` first validates output and
independent input freshness, then selects Current, Historical or Unknown bytes.
`result_cache.rs::Frames::select` clones Arc references. The endpoint in
`crates/sail-daemon/src/jobs.rs` writes that complete existing framed slice;
the cache table mutex is absent from socket output. Misses and old records still
perform complete parsing and encoding. Encoding another cached representation
would remove no work demonstrated in the original hits.

The integrated `output_service.rs::Queue::validate` retains the same charged
output owner across independent freshness, releases output mutexes during that
work, and validates the same published owner afterward. Missing, busy, dirty,
cancelled or substituted coverage repeats the original complete stdout read/SHA
and independent freshness. Its actual live guarded/fallback ratio is not yet
known. A frame hit alone cannot identify guarded output validation.

The original collector in `scripts/m7-job-read-bench.py::call` times connection,
request, complete receive and full `json.loads`. `exact` repeatedly receives
into a bytearray and returns an independent bytes copy. Four executor threads
perform those timed calls; full graph admission checks follow them. The SDK
decoding cache is absent from this Python path. Optimizing Rust SDK decoding
cannot directly improve this historical Python census, and changing its parser,
thread count or timed interval would change the original oracle.

The [complete transfer control](m7-result-transfer-profile.md) isolates a useful
mechanism: concurrent full parsing can prolong producer send wall time. In that
separate fixture, moving parsing after transfers changes send p95 from 138.353
to 16.791 ms, while send-thread CPU remains small. A larger socket buffer lowers
send wall time but barely changes complete client p95 and worsens its median.
This supports treating live write duration as transfer/backpressure/scheduling
time, rather than sender-copy CPU. It does not establish the historical live
cause or justify a buffer change.

## Component gain and resources

C's completed output-overlap component reports p95 19.366167 to 12.713334 ms,
a 6.653 ms difference. It uses a real synthetic FreshnessIndex inspection over
one source file and an 8 MiB generated runtime, plus exact 2,158,309-byte stdout;
it includes no RPC, Python decode, 4.3 MiB wire response or Current proof.
The component passes all input/output checks and preserves mandatory fallback
rechecks. Applying that saving as a hypothetical constant cannot close the
original 143.142 ms miss. Reduced contention may have nonlinear effects in a
live workload, but those effects have not been measured and imply no acceptance.

The daemon retains the existing 128 MiB frame/output family ceiling, including
held writers after eviction and output owners after cell removal. Shared frame
bytes are not duplicated for each writer. This charges requested owned storage;
it is not an allocator-overhead or RSS bound. The API accepts at most 32 MiB per
frame, and its optional decoding family separately bounds retained entries at
128 MiB; returned independently owned graphs and transient decoding are outside
that retention bound. Python's original length guard also bounds each received
wire body, not the heap cost of its decoded graph. A proposed larger kernel
socket buffer would need its own aggregate allocation account for up to 32
endpoint clients; it cannot be hidden inside the retained-frame bound.

No further server-copy candidate is selected by this audit. The meaningful next
integration evidence is the unchanged complete reader census on the combined
product, retaining existing output-guarded/fallback and freshness phases where
available. A new failure can then distinguish repeated fallback from healthy
guarded output and assess the remaining transfer interval. Missing or evicted
timing rows remain unavailable; these source facts and component controls do
not replace that measurement or erase earlier failures.

Audited current source SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `crates/sail-daemon/src/jobs.rs` | `a1601a724b2d8698237666605a0485975a53f56f1f446bd50f012829893f6210` |
| `crates/sail-daemon/src/jobs/store.rs` | `fbeb01b3cf3e2581d4824e9ba9e6e5ab1e5c8d3107ad706f85d9fc25080620c6` |
| `crates/sail-daemon/src/jobs/result_cache.rs` | `1bae07e4cdfaa825edb456b9e9edbe46c834c06c2a88ff48ffaf6132907464c1` |
| `crates/sail-daemon/src/jobs/output_service.rs` | `ce19cc3b6c6b1a5c9b71b391c0fc5b78f6ae25248299c9e1cfedd52d615bd656` |
| `crates/sail-api/src/jobs/decoded.rs` | `bc4f75ebfa4e293a74cb6b1bb63203223310b3a861c30d0a64c2165cd1c3855d` |
| `scripts/m7-job-read-bench.py` | `3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720` |
