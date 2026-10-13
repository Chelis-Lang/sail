# 0034: MCP response encoding ownership

- **Status:** accepted implementation; aggregate and live-reader acceptance remain separate
- **Date:** 2026-10-11

## Context

The installed MCP server receives complete fresh job-service replies. Previously,
projecting a typed Result response serialized its arbitrary JSON graph into a
second Value. Building the tool result and JSON-RPC envelope cloned that graph
again. Finally, the shared stdout mutex covered JSON encoding as well as writing.
A large response could therefore block an unrelated ping before any output bytes
were ready. This is a concrete ownership and locking problem; the original
Python-first reader failure did not execute MCP and cannot establish its latency
cost. See the [client audit](../validation/m7.md).

## Decision

Consume owned job Result documents and worker tool results when building their
JSON envelopes. Move the existing arbitrary result Value, preserving all fields,
nulls, optional repair evidence and complete models. Keep the public borrowed
`ToolResult::to_json` API; provide a consuming `into_json` form for owners. Keep
both text content blocks and the full structured payload. The complete JSON text
copy is still required for clients that ignore structured content.

Encode the complete JSON-RPC response before acquiring the stdout mutex. Append
one newline, then perform the whole framed write and flush under that mutex.
Frames cannot interleave. Slow output I/O can still delay another writer; this
change removes encoding from that critical section and makes no hard real-time
claim or response-memory bound.

Keep the request's cancellation slot until encoding and publication complete.
Check cancellation before encoding and again under the output mutex. Owned
cleanup removes the slot on every exit, including unwind, only when its cancel
handle still matches: an older request must not remove a later request's slot
after ID reuse. Cancellation cannot retract bytes whose publication has already
started. EOF still waits for owned workers; cancellation notifications remain
processed by the existing server loop.

Every lookup retains fresh transport, version/variant/job identity checks and
source-currentness authority. No client cache budget, RPC deadline, CLI fallback,
verification or solver behavior changes. No dependency edge is added.

## Validation and limits

Actual encoder controls pause a large Serialize operation and serve the server's
ping while it is paused. The old lock-first critical section fails the lock
availability predicate. Other controls cancel during encoding and output-lock
waits, exercise unwind and ID replacement cleanup, compare complete concurrent
frames and compare old/new text and structured JSON bytes. Typed Result controls
include missing and present repair evidence and null/large result graphs.

The [scoped record](../validation/m7.md) retains all actual
test and lint outcomes, including any unchanged original failures. These controls
do not qualify installed proof tools, four-reader latency or M7's whole repair
loop. Those original gates and their bounds remain required.
