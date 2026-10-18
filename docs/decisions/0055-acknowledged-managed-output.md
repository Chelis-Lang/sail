# 0055: Acknowledge final managed stdout through Cargo

- **Status:** proposed; focused macOS correctness passes, complete gates pending, unselected
- **Date:** 2026-10-18

## Context

The managed runner reads Cargo's stdout and stderr separately. A driver can
write its verification summary and exit before the runner forwards that summary;
Cargo's subsequent error can overtake it. An actual main-relay fixture reversed
44 of 96 controlled causal writes while preserving each separate stream. This
reproduces #215; it does not establish the cause of #174's missing streams.

The first candidate (`8766ff71`) sent stdout and stderr payloads directly to the
runner over local IPC. Its Cargo-like fixtures passed, but an actual Cargo
control exposed a correctness defect: Cargo renders and filters rustc's JSON
stderr, so forwarding those raw bytes directly bypasses the original behavior.
That candidate is rejected. A barrier on stderr also cannot prove that earlier
stdout has passed Cargo's separate buffering.

## Decision

Keep all user bytes on the original Cargo pipes. Stderr is unchanged, including
Cargo's diagnostic rendering, filtering, record ordering and later status lines.
Only each driver's final stdout emission has a completion barrier. Every current
adapter branch emits its rebuilt stdout once and then returns.

On Unix, the managed runner creates a private 0700 directory, an acknowledgment
socket and a fresh random capability in a 0600 regular file. A driver acquires
an owner-created 0600 regular lock with a bounded try-lock loop, writes its
unchanged final stdout through the original pipe, completes an unterminated
final record with Cargo's observed EOF newline, then appends a capability marker
on that same stdout pipe, flushes and waits for a private acknowledgment.
The framing uses printable ASCII: an actual Cargo control strips record/unit
separator control characters, so such delimiters cannot carry the capability
unchanged. The socket carries no payload. Native compiler and verifier children do not
inherit the endpoint variable.

After Cargo processes a complete stdout record, the runner removes only this
run's exact terminal marker. It processes the original record using the same
Cargo-JSON filtering and human-output suppression as before. Only after writing
and flushing the public output, or deliberately suppressing it for a structured
caller, does it acknowledge. A failed public write drops the completion and
sends NACK. The driver cannot exit successfully before this completion.

Actual Cargo controls show that a final unterminated stdout prefix receives a
newline at EOF. The sender supplies that newline before a separate marker record,
so a valid JSON final record remains independently parseable by Cargo. The marker
contributes no public bytes. Actual matched controls must establish identical
captured and public output for newline-terminated and unterminated valid JSON,
plain partial text and multiline output containing a large JSON record. Those
expanded controls pass in focused macOS attempt 10; complete gates remain pending. A malformed current-capability marker
fails with SAIL3106. Foreign marker-like text stays literal. The sender refuses
an exact current-capability collision before writing any payload.

The lock and private acknowledgment have a 30-second bound. Closing the receiver
rejects remaining owned clients and removes its endpoint and held-identity
files. On Unix, the Cargo readers use nonblocking reads and a bounded poll with an
explicit stop flag. On a relay error they stop and join without awaiting EOF
from a descendant that retained a descriptor. Existing callers retain Cargo
kill/wait and outer process-group ownership; the transport creates no nested
process group and claims no universal descendant cancellation. Compiler failure
cleanup remains in the adapter's existing owned-child and stdout-reader guard. The marker never establishes metadata, a verdict,
freshness or proof reuse; the private environment variable remains excluded
from proof inputs.

## Consequences

The guarantee is narrow: final driver stdout is delivered before that driver
exits and permits its subsequent Cargo error. It does not reconstruct a global
order between stdout and stderr or independently concurrent Cargo jobs. Store
replay at Cargo's fresh-artifact boundary remains outside this guarantee.

There is no daemon, dependency, user setting, output normalization or binary
rewrite. User bytes continue through Cargo's original processing. Actual Cargo
with the production sender/relay, broken-pipe/NACK/timeout controls, both-platform
checks, full E2E and conformance, and native overhead measurement must pass
before selecting the implementation. The original #174 failure stays unresolved.
