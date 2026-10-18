# 0055: Acknowledge visible managed-driver output

- **Status:** proposed; focused macOS controls pass, native qualification pending
- **Date:** 2026-10-18

## Context

The managed runner reads Cargo's stdout and stderr separately. A driver can
write its verification summary to stdout and exit before the runner forwards
that summary; Cargo's subsequent compilation error on stderr can overtake it.
An actual unmodified-relay probe reversed 44 of 96 controlled causal writes
without changing either stream's bytes. This is a separate reproduction of
#215, not an explanation of #174's missing original streams.

## Decision

On Unix, give each owned managed invocation a private Unix datagram endpoint
in a newly created directory with mode 0700. Drivers send exact visible bytes
with their original stdout/stderr destination, in bounded chunks, and wait
for an acknowledgment after every chunk. Only the runner's public writer
acknowledges, after writing and flushing. A driver cannot finish its visible
writes and exit before those bytes are delivered.

The same writer drains Cargo's already committed stderr before delivering a
driver frame. This preserves Cargo's preceding status lines without a second
stderr-reader race. Cargo's JSON pipe stays separate: visible chunks never
splice into captured metadata records. Per-driver output files remain the
source of verifier output. Structured callers still suppress human stdout.

Remove the endpoint variable before starting the real compiler or verifier.
It is private managed plumbing, never an input to proof authority. Frames
cannot establish verdicts, metadata, freshness or reuse. A write, flush or
acknowledgment error fails the invocation through the existing SAIL3106 or
SAIL3111 error path; it must not turn a failed delivery into successful output.
Sender socket operations have a 30-second bound. Closing the receiver wakes
and joins its thread, rejects undelivered frames and removes owned endpoints.

An owner-created 0600 lock file serializes each complete visible write across
drivers. Senders open it without following symlinks and acquire its lock with
a bounded try-lock loop; descriptor closure releases it on failure or exit.
The final-chunk marker lets the writer defer raw Cargo diagnostics until that
logical write finishes, preserving contiguous writes across frame boundaries.
A changed sender/stream or 30 seconds without the next chunk fails the run.
Preserved non-JSON pipe stdout is deferred through that same boundary without
delaying Cargo's captured metadata. The incomplete-write deadline applies
even when Cargo's metadata queue stays busy.

## Consequences

This adds local IPC to visible driver writes, so performance remains a
qualification gate. It introduces no daemon, dependency or user setting.
Separate original stdout/stderr bytes and output filtering remain strict;
no sorting, message recognition or timing heuristic reconstructs write order.
Independent Cargo jobs still have their ordinary concurrent ordering.

The implementation is not selected until actual production-sender/relay
controls, macOS host checks, native full E2E and conformance pass. The
original #174 disagreement remains unresolved until a real reproduction
establishes its cause. Research store output replay at Cargo's fresh-artifact
boundary is outside this change's causal-order guarantee.
