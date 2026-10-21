# 0065: One verification runtime for Oyster

- **Status:** implementation in progress
- **Date:** 2026-10-21

## Context

The owner authorized the Oyster integration critical path alongside the Verus
performance program. Oyster needs raw SMT, temporal checking and contracts
through Sail, without installing or orchestrating separate tools. The earlier
M10 hold until M11 completes is superseded; the existing Verus correctness and
performance contract remains in force.

## Decision

Implement independently qualified vertical slices. The versioned runtime API
lives in `sail-api`; its first implemented executor, `sail-runtime`, runs exact
SMT transcripts through the existing bundled pinned Z3. The CLI resolves the
bundle; the executor validates its hash and version. There is no separate SMT
installation or deployed service. The cache namespace cannot collide with
Verus sessions. The API reports actual capabilities only.

The shim never depends on this executor. Neither the API nor the runtime's
SMT executor depends on Verus. The CLI may depend on both to locate the
existing toolchain. New dependency edges enter the layering gate alongside
their first implementation. Ordinary `sail verify` remains unchanged.

The common input and output use tagged families. Daemon and MCP integration
extend this interface, rather than placing solver commands in the existing
cache protocol or disguising them as Cargo verification jobs. A completed
transcript is not a proof of Rust code: its exact checks and products retain
their backend identity and caller obligation IDs.

Temporal execution links a pinned D.I.V.E. Rust library in a Sail-owned worker
process. The parent enforces deadlines and cancellation by killing and reaping
the worker. No TLC backend, Java execution, or D.I.V.E. CLI wrapper is part of
this route. Neutral contracts share a parser with their runtime checks and
lower to a content-bound shadow project for actual Verus verification. These
are separate slices with their own specifications and acceptance records.

## Correctness and acceptance

Exact input bytes, effective limits, tool hashes and protocol versions bind
cache entries. Missing evidence, malformed transcripts, unsupported syntax,
unknown answers and infrastructure failures remain explicit. Wall deadlines
and cancellation never become cached solver answers. Native backend parity
precedes acceleration; the existing Verus conformance gates remain mandatory.
Private source dependencies are pinned by immutable revision and use a
dedicated Contents-read credential in ephemeral build access. No credential
is stored in source, URLs, images or retained evidence.

The first slice implements only bounded raw-Z3 batches and complete incremental
transcripts. Other backends, persistent session handles, certificate checking,
contract proofs, temporal jobs and the assurance view are not claimed by it.
Full SMT milestone completion still requires cvc5.
