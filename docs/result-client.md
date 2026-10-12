# Complete result decoding in a persistent Rust client

`jobs::Client::new(socket)` now stages decoding reuse for persistent SDK
clients. The first eligible Result response retains its complete validated body
bytes, without cloning its decoded graph. A second byte-identical response is
parsed normally and can promote one independent decoded graph. Later identical
fresh responses can reuse that decoding and return independent complete graphs.
`with_result_decoding_cache(socket)` keeps immediate first-response promotion;
`without_result_decoding_cache(socket)` explicitly disables retention.

## Connect to the jobs service

`Client::new` expects the jobs socket, not the solver-cache socket. Resolve it
with `jobs::configured_socket(&cwd)` to preserve the CLI's `SAIL_CACHE_DIR` and
`SAIL_DAEMON_SOCKET` semantics, including relative settings belonging to that
project directory. If an application already has the cache socket, derive its
job endpoint with `jobs::socket_path(&cache_socket)`.

Within a function returning `std::io::Result`, where `job_id` is the durable
`String` returned by submission:

```rust
use sail_api::jobs;

let cwd = std::env::current_dir()?;
let client = jobs::Client::new(jobs::configured_socket(&cwd)?);
let response = client.call(&jobs::Request::Result { id: job_id })?;
```

These SDK calls do not start a daemon. Start the service with the CLI, for example
`sail jobs list` from the same project directory, before using the client.
Handle `Response::Error` as a service failure even when transport returned `Ok`.
For `Response::Result`, inspect the job lifecycle, currentness and complete result;
a fetched response does not establish a successful proof. A repair also requires
its independent full evidence and `repair.accepted`. `Request::ProofEvents`
returns a bounded proof-journal page and cursor; `Request::Events` reads the
separate target-lifecycle log by byte offset.

## Fresh frames and decoding reuse

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

The [complete fresh-frame cost audit](validation/m7-fresh-frame-hot-cost-audit.md)
separates existing server encoding reuse from client parse/clone costs. Its
offline profiles do not identify the remaining live-service bottleneck or
qualify the 200 ms p95 target; hot encoding already uses complete preencoded
frames after fresh output-seal and currentness validation.

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
200-sample, 200 ms p95 and five-second RPC requirements remain unchanged. Frozen
[`b89546c`](validation/m7-compiler-script-bootstrap/scheduling-recovery.json) passes both
complete collectors and retained CLI/MCP scenarios: Python Result/Status p95
91.227/56.989 ms and Rust 83.867/44.371 ms. Each collector retains 200 full
Current samples while the same selected verifier remains live. The separate
[installed `730a321` workflow](validation/m7-reader-730a321-terminal.md) passes both:
Python Result/Status p95 94.197/59.896 ms and Rust Result/Status p95 67.483/50.273 ms.
The Current Result remains a complete graph above 4 MiB; no reduced payload or
changed deadline supplies this result.
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
same endpoint resolution. The snippet runs inside a function returning
`std::io::Result<()>`; replace the job ID with an existing submitted job. Endpoint
resolution does not start a missing daemon. Submit through CLI/MCP first, then
keep the client across reads:

```rust
use sail_api::jobs::{Client, Request, Response};

let project_dir = std::env::current_dir()?;
let job_socket = sail_api::jobs::configured_socket(&project_dir)?;
let client = Client::new(job_socket);

let job_id = "SUBMITTED_JOB_ID";
// Retain this client: later identical frames may reuse decoding, never authority.
for _ in 0..3 {
    match client.call(&Request::Result { id: job_id.to_owned() })? {
        Response::Result(document) => {
            if document.job.id != job_id {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData, "Result belongs to another job",
                ));
            }
            println!("{:?}: {:?}", document.job.state, document.job.currentness);
        }
        Response::Error(message) => eprintln!("{message}"),
        _ => eprintln!("Unexpected response to a result request"),
    }
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

The actual-client replay also has a raw fresh-frame transfer control. It performs
the same request write and complete bounded frame read, then checks exact bytes
and parses/compares the entire graph after timing. It is a diagnostic rather
than a Client option: no product response uses delayed validation. The replay's
owned fixture accepts and writes one connection at a time; it has no real daemon
or freshness observer. Four-reader transfer costs therefore include that
fixture's scheduling and serialized writes. They do not establish that the
production server has the same bottleneck. Malformed complete JSON still fails
and shuts down/joins the owned producer.

The [component record](validation/m7-result-client-components.json) retains
the exact source/binary/frame identities and all samples. On emulated Linux,
four-thread median allocation/copy was 0.96 ms, whole-body comparison 0.34 ms,
independent complete graph cloning 17.75 ms and typed frame parsing 50.02 ms.
The typed-read p95 also had a 3512.67 ms outlier. These results do not support a
product buffer-pool or unsafe-initialization change. Full-response ownership
and every fresh-frame validation remain intact. The record keeps the separate
raw-transfer replay's variation rather than assigning a live bottleneck from
these component timings.

The [installed b36 transport smoke](validation/m7-installed-mcp-b36.md) passed
on independently frozen production binaries using the unchanged hardened MCP
controller. It checked direct live reads, same-ID state changes, service loss
and restart, notification cancellation, socket cleanup and normal EOF exit.
The worker was stopped before any proof; it does not establish Current-result
latency or the separate real-reader acceptance.
## Automatic retained output validation

The current daemon source enables one optional background constructor for
`managed::GuardedOutput` on Linux. Established continuous namespace and read-lease
coverage can avoid rereading and hashing persisted worker stdout. Both output
barriers bracket the original independent input/runtime freshness check; output
coverage alone cannot make a proof Current. Missing, busy, quota-refused or
invalid coverage uses the complete original read/hash path. If the final output
barrier fails, the daemon repeats that full read/hash and independent freshness
check before serving the complete response.

There is no new user flag. Construction and inspection share the existing
128 MiB frame-retention budget; unavailable quota costs the shortcut. The
[35 affected controls and strict checks](validation/m7-guarded-output-source-controls.md)
pass, including real restored-write fallback and complete frame equivalence.
Frozen `b89546c` passes the [original full reader and retained workflow](validation/m7-compiler-script-bootstrap/scheduling-recovery.json),
with full Current graphs and unchanged p95 limits. The earlier `730a321` product passes the
[original full reader workflow](validation/m7-reader-730a321-terminal.md), including
both collectors and retained CLI/MCP scenarios. See
[ADR 0035](decisions/0035-guarded-retained-output-digests.md) and the
[retained original reader failure](validation/m7-descriptor-reader-terminal.md).

A second, test-only candidate groups file-parent descriptor lookups within one
lease barrier. It retains at most 64 temporary directory descriptors and borrowed
paths, opens each exact child name, and checks the original device/inode and
kernel pathname. It freshly resolves all reused parents before success. Any
unsupported lookup or parent mismatch invokes the complete original absolute
inventory predicate. No descriptor or identity memo survives the barrier, and
production still uses the original path. Namespace, mount generation, helper,
interpreter and both complete kernel inventories are unchanged. A same-inode
hardlink rebound can satisfy the original path predicate; the original outer
namespace observation remains necessary to reject the rename event. The new
control demonstrates that distinction rather than treating path identity as
continuous namespace coverage. Complete-operation measurements must justify any
later selection; these controls give no live reader or latency acceptance.

Established Linux reads avoid decoding the runtime identity sidecars again.
The existing owner retains one boolean instead of another parsed identity tree
or shared owner. Eligibility requires the exact raw
context `.link` and selected `.json` paths in that owner's actual lease bindings,
and is set only after the original final independent identity read and all cold
validation barriers succeed. Canonical aliases or absent bindings use the
original repeated parse. Every read retains the original namespace, mount,
helper, interpreter, complete kernel inventories and input validation, plus
request-entry, generation and pointer checks. This removes a redundant read; it
does not change the original Python collector or establish live latency
acceptance. Non-Linux fallback is unchanged.

The [sidecar component record](validation/m7-runtime-sidecar-components.md)
retains the passing mutation controls, initial Clippy failure and small reduction in complete inspection time. The initial comparison was test-only; the enabled
path uses the same eligibility and complete barriers.

The [enabled-path checks](validation/m7-runtime-sidecar-production.md) retain
the same mutation controls and strict validation after production selection.

The [retained-output overlap change](validation/m7-result-output-overlap-plan.md)
keeps the exact charged output owner alive while releasing its lock during
independent input freshness. It then rechecks the same owner with the original
complete output barrier. Refusal retains the full read/hash fallback. This adds
no flag, partial response or Currentness memo. Native affected controls and
strict checks pass. A separate four-reader component reduced complete output
and fixture-input validation p95 from 19.37 to 12.71 ms. That comparison excludes
RPC transfer and Python parsing. The later
[installed `730a321` workflow](validation/m7-reader-730a321-terminal.md) separately
passes complete Python and Rust reads during verification; it does not isolate
this change's contribution. See the
[complete component scope](validation/m7-result-output-overlap-native.md).
Optional diagnostic rows
distinguish successful guarded output validation from full-hash fallback through
`output_guarded`; `frame_hit` alone does not establish which validation ran.
