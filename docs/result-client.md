# Complete result decoding in a persistent Rust client

`jobs::Client::new(socket)` now stages decoding reuse for persistent SDK
clients. The first eligible Result response retains its complete validated body
bytes, without cloning its decoded graph. A second byte-identical response is
parsed normally and can promote one independent decoded graph. Later identical
fresh responses can reuse that decoding and return independent complete graphs.
`with_result_decoding_cache(socket)` keeps immediate first-response promotion;
`without_result_decoding_cache(socket)` explicitly disables retention.

Every call still opens its own connection, sends its request, and receives the
complete fresh frame under the existing five-second I/O deadlines. Its framing
limit remains 32 MiB. Only an exact match of the entire received body to an
already validated version-one frame reuses decoding. Changed job identifiers,
currentness, errors, arguments, results, member ordering or any other body bytes
use the ordinary parser. No report field, dependency result, counterexample or
freshness check is removed. Truncated, oversized, incompatible,
duplicate-envelope and trailing input follow the original framing/parser rules.

Each call captures its candidate generation at request entry, before connection,
negotiation or network I/O. Overlapping cold requests cannot accidentally promote
an entry published during their own reads. Promotion has a nonblocking,
generation-bound claim: other callers parse normally rather than waiting. A
late promotion cannot replace a newer candidate. Failed ownership accounting or
budget admission leaves the correct result uncached. An owned promotion claim
is released on failure, so a later fresh RPC can try once after resource pressure
clears; there is no wait or retry inside the refused call. Successful publication
changes generation, preventing delayed old admissions from promoting again.

Keep the Client or clones across calls to benefit. Clones and rebound endpoints
share one family; each returned Response owns an independent complete graph.
A short mutex only takes/replaces an Arc. Byte comparison, graph cloning,
parsing and destruction occur outside it; contention falls back to correct
ordinary parsing. `Debug` prints the socket and a compact storage count, never
cached frame bytes or model contents. Installed MCP durable read-only tools use
a bounded persistent client family; status/result and counterexample/cached-result
lookups authorize every read through a complete fresh service reply. Single-shot
CLI processes cannot reuse a prior process's graph. Writes and blocking
verification polling retain their existing lifecycle rules. This change does
not by itself establish MCP or live-reader latency acceptance.

## Cancellable reads

`Client::call_cancellable(request, cancelled)` uses one owned, joined monitor
and an absolute five-second transport interval, including capability negotiation
and fragmented frames. Socket2 0.6.5 provides safe nonblocking Unix connection
setup, using short bounded connect/poll attempts and dropping each timed-out
socket before a pre-RPC retry. Cancellation and the shared remaining budget are
checked between attempts; a successful socket must also have an actual peer.
A full listener backlog therefore cannot hold a blocking `UnixStream::connect`
before the monitor exists. No request bytes are sent during these attempts.
[Socket2's connect contract](https://docs.rs/socket2/0.6.5/socket2/struct.Socket.html#method.connect_timeout)
binds the implementation; ordinary `call` connection behavior is unchanged.
Cancellation shuts down both socket directions; normal
completion wakes the monitor immediately. Every exit joins it, and a response
arriving after cancellation or expiry is refused, including a cached identical
body. The callback must return promptly. Kernel operations and OS scheduling
are not interruptible wall-clock guarantees. Ordinary `Client::call` keeps its
existing per-I/O five-second limits.

`CancellableError::Connect` means the initial connection failed before any RPC.
A startup path may fall back only for a missing or refused endpoint in that
category. `Operation` covers all errors after connecting, including cancellation,
expiry and negotiation: retrying through another transport after these errors
could duplicate work. `io_error()` preserves the original error kind and message.
Cancellation is `Interrupted` (`SAIL0804`); whole-transport expiry is `TimedOut`
(`SAIL0805`). Monitor creation failure or a cancellation callback unwind is
reported as `SAIL0806`; a failed monitor closes its socket before its owner joins.
The monitor uses fallible thread creation before sending the RPC. Installed MCP
durable read-only tools use this API. Missing/refused pre-RPC connections may
fall back to the existing CLI startup path; operations already started are never
retried. Transport/adversarial tests cover the integration, while live MCP
latency acceptance remains unmeasured.

## Retained ownership bound

Eligible Result frames are at most 8 MiB. A per-client-family budget limits all live
retained entries shared by that Client and its clones, including an old entry
held by an in-flight caller, to
128 MiB of conservatively charged requested storage. Replacing the one current
entry does not release an older entry's reservation until its last Arc drops.
The count includes each frame-only candidate and decoded entry, raw bytes,
Entry/Arc inline storage, boxes, String/PathBuf and
Vec capacities, enum slots, nested schema descendants and map nodes. Returned
caller-owned graphs, temporary parsing/clone buffers and allocator fragmentation
are outside this retention bound; it is not an RSS guarantee or a process-wide limit across
unrelated clients. Endpoint tables must bound their client families separately.
`client.with_socket(socket)` rebinds an endpoint while sharing the same optional
cache and budget; use this for a bounded multi-endpoint table instead of creating
an unrelated 128 MiB family per endpoint. Evicted endpoints' in-flight entries
remain charged. The new endpoint still supplies the complete fresh reply before
any exact-body comparison. Unknown ownership,
arithmetic overflow or an exhausted budget leaves the correct result uncached.

The audit binds the pinned Rust 1.99 BTree implementation and serde_json 1.0.151
with the workspace's default/std features. Its BTree nodes have eleven key/value
slots and twelve child slots, with at least five entries in each non-root node.
Charging 1024 bytes per possible node, plus all nested key/value capacities,
bounds the audited 64-bit String/Value layout. The pinned implementation is
inspected against the [Rust BTree node source](https://doc.rust-lang.org/src/alloc/collections/btree/node.rs.html).
Exhaustive schema destructuring makes newly added fields require ownership
accounting. The actual retained clone is recounted before insertion. Runtime
layout guards refuse retention for unaudited map/number layouts, including
`preserve_order` and arbitrary-precision variants; those clients still decode
normally. A toolchain, schema or serde_json feature change requires revisiting
this storage audit.

## Validation and limits

Adversarial tests cover entire-frame identity, independent returned ownership,
changed metadata/currentness/errors/ordering, malformed and incomplete frames,
concurrent different replies, overbudget retention, old entry lifetimes,
unaudited layouts and compact Debug output. The optional
`result_client_profile` example replays a captured full frame through actual
Client calls and compares the complete decoded graph outside each timed wave.
This is an offline socket replay, not a proof or live-reader acceptance.
Its owned producer bounds accept and socket I/O, observes a stop signal on
normal completion or any warmup/validation failure, and is joined before return.
The rejected `result_stream_profile` experiment is diagnostic only; its owned
producer also closes and joins on malformed-frame errors. Neither example is a
product transport option or a replacement for the frozen reader collectors.

The original Python and Rust reader collectors and their four-reader,
200-sample, 200 ms p95 and five-second RPC requirements remain unchanged.
Offline decoding or cloning improvements do not qualify those gates or the
M7 edit-loop performance target. See [freshness](freshness.md),
[the repair workflow](proof-repair-walkthrough.md) and
[the roadmap](ROADMAP.md) for the separate product contracts.

The [retained offline profile](validation/m7-result-client-profile.json) includes
both the first automatic-retention experiment and the explicit opt-in version.
The first experiment's cold four-reader p95 was 438.93 ms versus 171.88 ms for
ordinary parsing, motivating the new staged default instead of an eager graph
clone on the first response. In the
explicit experiment, the warm four-reader median was 45.61 ms and maximum/p95
77.68 ms; the ordinary four-reader median was 56.23 ms with a 503.48 ms outlier.
Eight samples under emulation are diagnostic rather than a stable reference or
live acceptance. The full raw samples, complete graph comparison, binary/source
bundle hashes, cold costs and rejected slower streaming alternative are kept.

## Calling the SDK

Pass the durable **job** socket, rather than the SMT-cache socket. A configured
cache endpoint can be converted with `jobs::socket_path`; the CLI and MCP use the
same endpoint resolution. Keep the client across reads:

```rust
use sail_api::jobs::{Client, Request, Response};

let client = Client::new(job_socket);
let response = client.call(&Request::Result { id: job_id.clone() })?;
if let Response::Result(document) = response {
    println!("{:?}: {:?}", document.job.state, document.job.currentness);
}
```

The returned lifecycle and currentness are separate from the proof outcome.
Inspect the complete report or repair acceptance evidence before treating a
function as verified. A cached decoding does not turn provisional, historical
or unknown evidence into a confirmed result.

Connection and I/O failures return `io::Error`. Framing rejects an oversized
body with `SAIL0801` and an incompatible envelope version with `SAIL0802`;
malformed JSON and server errors are returned without substituting a prior
result. The ordinary `call` method retains its existing five-second **per-I/O**
timeouts; it does not promise a five-second total across arbitrary fragmented
reads. Use `call_cancellable` for the joined cancellation and absolute transport
budget described above. Installed MCP uses that path; live acceptance remains
a separate gate.

The [cancellable transport controls](validation/m7-cancellable-client.json)
record the original large-request readiness failures alongside the final ten-case
macOS and Linux results. The original 8 MiB public request, two-second fixture
readiness check and full response contract remain intact. The additional bounded
send-buffer control exercises the same production exchange path; it does not
replace large-request serialization coverage. These controls qualify transport
ownership and cancellation behavior, not the live-reader latency gates.

The [original prepared reader attempt](validation/m7-reader-prepared-terminal.md)
failed the unchanged Python gate despite genuinely established Current full
proofs. All 200 samples are retained; Rust was not reached. Staged SDK cold,
second-response and warm controls must be recorded separately and cannot turn
that failed run into a pass. The frozen Python collector remains untouched.

The [staged default controls](validation/m7-staged-result-client.json) retain
all cold, second-response and warm samples for the final recovery fix. The
complete 4.315 MB captured frame includes every dependency and both full-report
copies. Four-reader medians in milliseconds were:

| Offline environment | Uncached | Default cold | Second response | Default warm |
| --- | ---: | ---: | ---: | ---: |
| Native macOS arm64, developer load | 91.19 | 108.30 | 152.96 | 48.07 |
| Emulated Linux amd64, developer load | 280.17 | 195.83 | 229.64 | 126.33 |

These eight-sample controls show the second-response promotion cost and a warm
median improvement; they do not establish a stable reference speedup. The final
emulated warm p95 was 209.76 ms, above the separate 200 ms live-reader threshold.
The first final-source Linux release replay reused an older binary because
archived source mtimes predated its prior compilation. That replay remains
recorded as stale provenance. A forced package-only release rebuild produced
the independently hashed final binary used for the table. Final API tests and
strict Clippy passed on both platforms.

The `result_frame_profile --frame CAPTURED_FRAME` component diagnostic separates
safe zero-initialized body allocation/copy, exact comparison against a distinct
complete captured body, full typed parsing and independent Response cloning.
Every output is checked after its timing wave; allocation/copy refuses oversized,
truncated or trailing input and comparison controls detect a changed final byte.
It retains all eight samples per stage at one and four threads. These in-memory
components exclude Unix transfer, negotiation and server freshness work, so their
timings cannot be subtracted from a separate socket run to attribute live latency.
The producer and captured full response remain unchanged in the actual-client
replay. This diagnostic does not introduce a product buffer pool, alter frame
validation or reduce returned ownership.
