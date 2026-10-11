# Complete-result cache storage ownership

The daemon now accounts for the physical storage retained by cached complete
wire-frame families. Previously, payload lengths hid spare Vec capacity and
eviction refunded the index charge while a socket writer could still own a
selected frame. This resource fix does not change report fields, freshness
validation, proof authority or RPC deadlines.

The existing payload and 16-entry bounds remain. A separate 128 MiB quota
charges actual capacities, Arc owners, key storage and conservative map-node
storage. Selected output retains its complete charged family through output,
even after eviction. Large buffers drop outside the index lock. Quota pressure
returns the complete uncached response. Temporary cold encoding, uncached
caller-owned output and allocator/process RSS are outside this retained-cache
quota.

Four Linux controls pass, covering the original index bounds, spare capacity,
evicted live output and concurrent eviction. Compiled counterfactuals fail the
new assertions when capacity is replaced with length or the selected output
loses its family owner. Strict affected all-target Clippy passes; all 80 daemon
tests pass in 46.594 seconds including compilation. Exact source hashes, raw
log hashes and the separately preserved tested ELF are in the
[machine-readable record](m7-result-frame-ownership.json). Earlier setup and
strict-check failures remain retained with the private raw evidence.

This is local Linux amd64 Docker emulation. It does not establish the original
reader latency threshold or source-engine acceptance. The earlier full Result
p95 failure remains open; no latency improvement is inferred from these tests.
