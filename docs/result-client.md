# Complete result decoding in a persistent Rust client

`jobs::Client::with_result_decoding_cache(socket)` opts a persistent SDK client
into retaining one previously decoded Result response. `Client::new(socket)`
keeps ordinary single-use decoding behavior. Every call still
opens its own connection, sends its request, and receives the complete fresh
frame under the existing five-second I/O deadlines. Its framing limit remains
32 MiB. Only an exact match of the entire received body to an already validated
version-one frame reuses decoding. Changed job identifiers, currentness, errors,
arguments, results, member ordering or any other body bytes use the ordinary
parser. No report field, dependency result, counterexample or freshness check is
removed. Truncated, oversized, incompatible, duplicate-envelope and trailing
input follow the original framing/parser rules.

Keep an opted-in Client, or clones of that Client, across calls to benefit. Each clone
shares the entry; each returned Response owns an independent complete graph.
A short mutex only takes/replaces an Arc. Byte comparison, graph cloning,
parsing and destruction occur outside it; contention falls back to correct
ordinary parsing. `Debug` prints the socket and a compact storage count, never
cached frame bytes or model contents. Existing CLI and MCP commands create
fresh clients or subprocesses: this change does not establish a cross-call
speedup for those paths. First-result retention adds an independent full graph
clone; isolated measurements found a cold-client regression. That cost is why
single-use calls retain the original uncached constructor.

## Retained ownership bound

Eligible Result frames are at most 8 MiB. A per-client-family budget limits all live
retained entries shared by that Client and its clones, including an old entry
held by an in-flight caller, to
128 MiB of conservatively charged requested storage. Replacing the one current
entry does not release an older entry's reservation until its last Arc drops.
The count includes raw bytes, Entry/Arc inline storage, boxes, String/PathBuf and
Vec capacities, enum slots, nested schema descendants and map nodes. Returned
caller-owned graphs, temporary parsing/clone buffers and allocator fragmentation
are outside this retention bound; it is not an RSS guarantee or a process-wide limit across
unrelated clients. Endpoint tables must bound their client families separately. Unknown ownership,
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

The original Python and Rust reader collectors and their four-reader,
200-sample, 200 ms p95 and five-second RPC requirements remain unchanged.
Offline decoding or cloning improvements do not qualify those gates or the
M7 edit-loop performance target. See [freshness](freshness.md),
[the repair workflow](proof-repair-walkthrough.md) and
[the roadmap](ROADMAP.md) for the separate product contracts.
