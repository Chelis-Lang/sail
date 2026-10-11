# Complete Result transfer and parsing control

This bounded offline control profiles a complete 4,313,817-byte framed Result
reconstructed from the genuinely re-established Current baseline in the
[separate retained attempt](m7-rpc-retained-functional.md). Its graph and body
length match that public response, but JSON key order was reconstructed: it is
not captured wire, a fresh authority lookup or live acceptance. The original
[403.341333 ms Result p95 failure](m7-rpc-stage-workflow.md) remains unchanged.

The [source](m7-result-transfer-profile.py) and [all rows and identities](m7-result-transfer-profile.json)
retain eight fixed alternating groups of four clients per condition. Every
condition transfers the same complete frame. The Python receive loop uses the
original `recv`/`bytearray.extend`/`bytes` algorithm and full `json.loads`; complete
graph equality is checked after timing. The raw control parses and validates
all four bodies after the group's transfers, separating transfer progress from
concurrent parsing. Five-second socket timeouts and finite owned group lifetimes
bound fixture I/O; these are separate diagnostic controls, not changed product
deadlines. No actual daemon, leases, proof, service endpoint or collector ran.

| Four-client control, 32 samples each | Complete client p95, ms | Send p95, ms | Send thread CPU p95, ms |
|---|---:|---:|---:|
| Default buffer; parse in clock | 163.252 | 138.353 | 2.408 |
| Larger buffer; parse in clock | 162.326 | 67.130 | 1.704 |
| Default buffer; parse after transfers | 17.530 | 16.791 | 1.891 |
| Larger buffer; parse after transfers | 15.539 | 9.131 | 1.844 |

With parsing in the clock, default transfer p95 was 132.606 ms while its own
client-thread CPU p95 was 6.306 ms. Parse thread CPU p95 was 34.408 ms. When
all parsing moved after the group's transfers, sending p95 fell from 138.353
to 16.791 ms. This demonstrates that client parsing/contention can prolong
producer output in this fixture; server send wall time is not a CPU-copy metric.
It does not establish the cause of every historical live sample, distinguish
hypervisor scheduling, or allow independent quantiles to be added.

The larger requested send buffer was 4 MiB; Linux reported an 8 MiB effective
`SO_SNDBUF`, versus the original 212,992 bytes. It substantially reduced send
wall time but did not materially improve complete client p95 and worsened its
median from 119.092 to 140.370 ms. No socket change is selected. A larger kernel
buffer for up to 32 product clients would also require an explicit resource
ledger; it cannot be treated as free storage outside the retained-frame bound.

The actual server tail already showed complete preencoded hits, negligible
frame selection and no encoding. Another serialization memo is unsupported.
Freshness optimization remains a separately measured path; full transfer and
parsing stay required. No report fields, dependency graph, byte checks, original
collectors or acceptance thresholds changed. See [RPC diagnostics](../result-rpc-diagnostics.md),
[reader acceptance](../reader-acceptance.md), [known issues](../known-issues.md)
and [the roadmap](../ROADMAP.md).
