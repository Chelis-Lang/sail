# Cached complete Result transport review

The read-only [source and raw-seal record](m7-cached-result-transport-review.json)
finds no repeated graph serialization or full frame clone on the cached sender
path. `result_frame` validates persisted output and current inputs before selecting
an immutable complete Current, Historical or Unknown frame. Frame selection clones
Arcs; the endpoint writes the existing header and body as one slice. The cache
mutex protects a bounded table, not graph encoding or socket output. Cache misses
and older records still parse and encode the complete response.

All 42 retained Current Result rows from the
[original descriptor reader failure](m7-descriptor-reader-terminal.md) hit encoded
frames and have no encoding interval. For each individual row, subtracting its
socket-output interval from its accepted-to-completion interval gives pre-output
time: median 46.016 ms, p95 63.794 ms, maximum 79.851 ms. This is a same-call
calculation, not subtraction of unrelated quantiles. Socket output p95 is
194.522 ms and includes backpressure and scheduling; the recorder measures no
sender CPU time. These rows are only a retained tail subset and are not joined
to particular client samples.

The original Python client receives all bytes, copies its accumulated bytearray
to bytes and parses the complete JSON graph. It does not use the SDK decoding
cache. The [separate complete transfer control](m7-result-transfer-profile.md)
demonstrates that concurrent client parsing can prolong producer output in that
fixture; it does not establish this live failure's cause. Complete responses,
collector behavior, socket settings and deadlines remain unchanged.

The concrete server work available for independent optimization is the persisted
output-seal validation and runtime/input freshness path. Their authority must
remain continuously guarded. A second encoding memo has no demonstrated work
to remove in these hits. This audit cannot establish that server work explains
the complete 343.142 ms client p95, and does not qualify reader latency or M7.
