# 0080: The cache checks response framing with the SMT-LIB splitter

- **Status:** proposed
- **Date:** 2026-10-26

## Context

A stored reply must stand for exactly one solver response. The cache's
`shape_ok` and the shim's plausibility check looked only at a reply's prefix
or emptiness, so an entry holding two concatenated response forms (#379)
passed both and added a response to the client's stream. The response
splitter in `sail-smtlib` already defines the unit a live session counts
responses in.

## Decision

`sail-cache` may depend on `sail-smtlib` directly. `shape_ok` requires
`sail_smtlib::split::is_single_response`, as do the shim's read-side check and
the daemon's PUT handler, so one definition of a response frame applies at
write time, read time and maintenance.

## Consequences

`sail-smtlib` has no internal dependencies and was already reachable from
`sail-cache` through `sail-key`; the layering table gains one direct edge.
Replies that are not one complete frame are refused (a miss, then a replay
into the pinned solver), never served.
