# 0037: Drive shared HTTP requests without blocking adapters

- **Status:** rejected: the candidate cost more CPU and lookup time than the blocking transport it would replace
- **Date:** 2026-10-11

## Context

The original `b63af10` eight-reader fixture failed at its unchanged 250 ms
shared lookup deadline, despite coalescing seven followers into one GET.
The server completed its existing single header read 87.207 ms after accepting
the connection. [The failure](../validation/m7.md)
does not establish the cause of that interval.

Pinned reqwest 0.13.5's blocking API introduces a request channel, response
oneshot and blocking body adapter around its async client. Removing those
adapters was a concrete implementation candidate.

## Decision

The candidate used the same reqwest async client and options, with one owned
current-thread Tokio I/O runtime polled through `Handle::block_on` from the
existing bounded lookup helper. Endpoint policy, TLS, timeouts, body limits,
coalescing, backoff and every signed-entry check were unchanged.

It was not selected. On the same instrumented eight-reader fixtures, combined
helper and I/O CPU across send through body rose from 66.264 ms to 84.160 ms,
and lookup through publication rose from 185.680 ms to 222.422 ms. Production
keeps the blocking transport, and no code from the candidate remains.

## Consequences

None: no dependency edge, wire schema, proof authority or cache key changed.
The full comparison is preserved in the `archive/m7-contracts-f7103bf` tag.
