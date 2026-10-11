# Current Result component diagnosis

The separate offline control does **not explain** the original live Result p95
of 394.713 ms or qualify reader performance. It identifies material full JSON
decode cost and small isolated report-read, hash and transfer costs. No additional
encoding memo is justified: the server already selects a preencoded immutable
frame after fresh seal and currentness validation.

The [complete record](m7-current-result-components.json) contains all component
samples, thread CPU observations, input hashes and the secondary older Rust
profiler's precise scope. The [original failed workflow](m7-current-reader-workflow.md)
and its 200 live samples remain unchanged. No proof, service endpoint, collector
or acceptance workflow was repeated.

## Reconstruction and boundaries

The original Python collector saved parsed public JSON and response length; it
did not save wire bytes. Wrapping the saved complete public Result in its
version/body envelope and compact UTF-8 JSON produces exactly the observed
4,312,245-byte body. Its members follow the saved sorted graph, rather than the
original wire order. Both complete full reports, vstd, all functions, diagnostics,
provenance and wrapper fields remain present and equal. This is explicitly a
**reconstruction**, not captured wire.

The [bounded component tool](m7-current-result-components.py) uses the original
frozen Python `exact()` and `json.loads`, verifies complete graph equality outside
each timer, and performs 32 samples per component with one or four workers. Its
socketpair fixture owns and joins every writer, uses the original five-second
socket timeout and closes both endpoints on success or failure. It contains no
daemon, verifier or freshness authority. The filesystem input is a byte-sealed
copy of the original 2,157,555-byte worker stdout, not a current proof claim.

| Isolated component | One worker p95 ms | Four workers p95 ms |
|---|---:|---:|
| Full Python JSON decode | 49.289 | 90.352 |
| UTF-8 decode | 0.215 | 0.197 |
| Producer file read | 0.126 | 0.775 |
| Complete producer SHA-256 | 4.099 | 3.819 |
| Producer read and SHA-256 | 4.438 | 4.328 |
| Full frame transfer and original exact read | 2.408 | 10.357 |
| Transfer, exact read and full JSON decode | 27.365 | 121.181 |

These are separate waves; their percentiles cannot be added or subtracted to
explain the live workload. The differing decode distributions also show why a
single isolated component is not a live acceptance proxy. No host availability,
load or upstream cause is inferred from these observations.

The existing older Rust component executable checked the same complete graph
after typed reencoding: four-worker p95 was 116.783 ms for typed parse, 6.448 ms
for response clone, 9.973 ms for encoding and 0.000333 ms for an Arc selection.
It is a secondary offline control, not a measurement of the current SDK or the
unattempted original Rust collector. Its source and binary hashes are retained
separately.

## Remaining server attribution

For the frozen current product, both Status and Result read and validate the
same persisted output seal and run fresh currentness checks. The immutable
Result cache key binds the completed stored job, including its output seal;
currentness selects a complete preencoded variant. A cache hit performs no
function-graph serialization. The original saved workflow did not record cache
hits or separate freshness/write durations, so this audit cannot claim that
every original Result was a hit or assign the remaining latency to one stage.

The next diagnostic adds optional, bounded, metadata-only RPC stage observations
without report payloads, input changes or default tracing I/O. A retained-input
fixture must establish its own continuous guards and remains diagnostic only.
The original four readers, 200 samples, strict 200 ms p95 and five-second RPC
bounds remain required. See [reader acceptance](../reader-acceptance.md),
[freshness](../freshness.md) and [the roadmap](../ROADMAP.md).
