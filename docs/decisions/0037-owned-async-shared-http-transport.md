# 0037: Drive shared HTTP requests without blocking adapters

- **Status:** rejected after the bounded comparison
- **Date:** 2026-10-11

## Context

The original `b63af10` eight-reader fixture failed at its unchanged 250 ms
shared lookup deadline, despite coalescing seven followers into one GET.
The server completed its existing single header read 87.207 ms after accepting
the connection. [The failure](../validation/m7.md)
does not establish the cause of that interval.

One separately instrumented fixture passed, and measured a 32.722 ms increase
in the reqwest runtime CPU counter across send samples, alongside 3.187 ms on
the helper. These non-atomic observations do not explain the failed gate.
Pinned reqwest 0.13.5's blocking API introduces a request channel, response
oneshot and blocking body adapter around its async client. Removing those
adapters is a concrete implementation candidate; a comparison must count work
on both helper and I/O threads, rather than claim savings from moving it.

## Decision

The candidate uses the same reqwest async client and options, with one owned
current-thread Tokio I/O runtime. An existing bounded lookup helper polls its
request and body futures through `Handle::block_on`; the I/O owner continuously
drives spawned connection tasks and timers. No request-channel or blocking
body adapter is introduced by Sail. Clones share the client and runtime owner.

Client construction remains on the setup or lookup helper, with fallible
thread and runtime construction and one bounded ready channel. A caller already
entered into an async runtime receives `SAIL0650` before construction or
`block_on`, rather than reaching Tokio's nested-runtime panic. The I/O thread
captures the builder, ready sender and stop notification only; it cannot own
or drop its `Owner`, preventing self-join. Final owner disposal signals stop
and joins that I/O thread. Runtime destruction happens on the owner thread,
not inside a caller's unrelated runtime. Publication releases the snapshot,
cancellation and constructor locks before disposing of retired transport.

Each transport has one I/O thread and a blocking pool capped at one worker.
This replaces Tokio's default maximum of 512 blocking workers for that runtime.
HTTPS uses the same resolver; queued DNS remains subject to the same request
timeout and shared fallback. The service still caps lookup helpers at 64 and
serializes construction. These are per-service/per-transport bounds, not a
process-wide bound on arbitrary public clients created by an application.

Preserve the endpoint policy, exact URL and bearer token, CA bundle validation,
platform TLS roots and refresh behavior, no proxy, no redirect, identity
encoding, HTTP status/refusal handling, MAX+1 retained-body limit and every
independent signed-entry check. The client still has its original configured
whole-request/connect timeout, and the caller retains its original absolute
lookup deadline, coalescing, negative cache and backoff. There is no new retry.

## Consequences

The [unselected source patch](../validation/m7.md)
passed 39 affected HTTP security, admission, refresh and lifecycle controls.
Both once-only instrumented eight-reader fixtures passed every original
assertion. However, combined helper and I/O CPU across send through body rose
from 66.264 ms to 84.160 ms, and lookup through publication rose from
185.680 ms to 222.422 ms. Body wall time fell from 5.047 ms to 0.884 ms;
that isolated saving did not improve the complete operation.

The [comparison record](../validation/m7.md) retains
the first strict lint failure and corrected direct library/unit/HTTP-integration
lint passes. This is not a complete Cargo workspace gate. Metadata sampling
is synchronous and non-atomic; one sequential pair cannot establish a timing
distribution or explain the original failed gate. Production retains the
blocking transport. There is no proof, reader benchmark or acceptance credit.

Like the former blocking client, final disposal can wait for a kernel-blocked
DNS worker. It is not a hard shutdown deadline or cancellable DNS guarantee.
The daemon keeps that disposal off foreground status and publication locks;
no detached reaper or accumulating unowned runtime is added. Ordinary client
clones keep the runtime alive through in-flight calls. Unsupported nested
synchronous use refuses safely; an async application must use a blocking
helper for this synchronous public API.

No crate dependency edge, third-party version, solver identity, wire schema,
proof authority or cache key changes. Production selection requires review,
meaningful original controls and measured complete-operation improvement.

## Remaining attribution limit

The existing baseline samples locate 19.615 ms of I/O-owner CPU before
server acceptance, 25.455 ms while the server completes its header read,
and 12.061 ms after server send before the helper receives the response.
The fixed server sleep interval adds no measured I/O CPU. These are observed
intervals, not function profiles. Pinned Hyper's HTTP/1 dispatch, header
encoding and flush path is a candidate for narrower attribution; the records
do not establish a redundant operation or justify another transport rewrite.
Server-side samples do not observe helper CPU, so the candidate's smaller
I/O-thread counters cannot be counted as total savings.

The diagnostic archive was subsequently recovered and checked against its
existing complete-byte seal and 148-member inventory. The
[recovery record](../validation/m7.md) preserves
the earlier failed transfers. Recovery adds no experiment, conformance or
performance acceptance credit; the candidate remains unselected.
