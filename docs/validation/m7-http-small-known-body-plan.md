# Small known HTTP body candidate

Status: file-only, unselected and uncompiled. This change does not qualify the
original eight-reader 250 ms control, establish a latency cause or replace its
retained failure. No compiler, server, verification or Docker lifecycle actor
has run for this candidate.

## Boundary and allocation

For a successful, unencoded response, use `Response::content_length()` only as
the pinned transport's exact body size hint. If it is at most 8192 bytes, keep
the existing fixed 8192-byte Vec allocation, resize within that capacity and
`read_exact` the complete framed body. This removes the additional blocking Read
used solely to observe EOF. Partial reads still loop until the entire body is
received or fail; zero length is the already-complete empty body.

Larger and unknown lengths retain the original `MAX_ENTRY_BYTES + 1` capped
stream. The early known-oversize refusal and final received-byte limit remain
unchanged. Status/authentication, Content-Encoding refusal, redirects, TLS,
client pooling, whole-request timeout and signed-entry validation are untouched.
No server-sized large allocation, async client, collector or extra full-body
copy is introduced. The small path retains exactly one application Vec with
8192 capacity; the larger streaming path retains its existing growth behavior.
This is an application-buffer statement, not an RSS or transport-buffer bound.

## Pinned implementation argument

The [reqwest 0.13.5 response API](https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.Response.html#method.content_length)
exposes body length rather than trusting the raw header directly. Its async
implementation calls `Body::size_hint().exact()`. The blocking Read performs a
timed wait for the body read. Hyper 1.12.0 HTTP1's Length decoder reads no more
than its remaining count and rejects EOF before that count reaches zero. Its
connection marks the body complete when the last data frame exhausts the
decoder; dispatch closes the body channel after that frame without reading
additional socket bytes to establish this message boundary.

Malformed or conflicting Content-Length headers are rejected by Hyper's header
parser. Transfer-Encoding takes precedence; chunked and close-delimited lengths
produce no exact size hint and retain the streaming route. The workspace's
reqwest default features are off, with only blocking and rustls-no-provider
selected; its lock has neither HTTP2 nor decompression dependencies. Existing
Content-Encoding refusal remains before either body route. Changes to these
pins or features require rechecking this argument, not treating the raw header
as a new independent authority.

The initially considered `Response::bytes()` route is rejected here. Pinned
reqwest collects into http-body-util's BufList, retaining all nonempty frame
buffers before flattening; conversion to the public Vec adds a copy. A bounded
payload alone does not preserve the existing allocation envelope under extreme
fragmentation. This candidate uses no collector.

The adjacent source record binds selected dependency source files to actual
registry archives, verifies archive checksums against Cargo.lock and verifies
each selected source byte hash against its archive member. It does not replace
the eventual actual compiled feature/layout controls.

## Required controls

New source-only unit controls count real Read calls: small complete bodies have
no EOF read, zero length has no reads, their Vec capacity stays 8192, and short
input fails UnexpectedEof. Returning to the old EOF stream would fail that count
assertion. Large and unknown lengths must still read EOF. These are mechanism
controls, not elapsed-time qualification.

New loopback controls cover exact zero/small/8192/8193 bodies, small truncation
with the original interrupted-response classification, invalid/conflicting
lengths, chunked bytes, and a missing final byte exceeding the unchanged request
timeout. Existing known-oversize, unknown 13000-byte, abrupt termination,
encoding, redirect, TLS, signature/trust and eight-reader controls are unchanged.
Their original deadlines and assertions remain mandatory. New server threads
are joined and fixtures removed before outcome assertions.

After independent review and an explicit sole-slot grant, compile the affected
remote unit controls and complete HTTP integration suite, then strict all-target
checks. Preserve any first failure and actual tested images. The exact original
eight-reader case and its baseline must be measured before selecting this
candidate; no speed claim follows from eliminating one possible EOF Read.
