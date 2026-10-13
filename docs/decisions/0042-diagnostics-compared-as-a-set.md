# 0042: Diagnostics are compared as a set of complete messages

- **Status:** accepted (owner decision, 2026-10-13)
- **Date:** 2026-10-13
- **Amends:** how ADR 0009's conformance compares diagnostics; charter rule 1 itself is unchanged

## Context

Conformance compared Sail's stderr with plain Verus's byte for byte. Stock
Verus prints each module's errors when that module's checks finish, so the
order of messages from different modules depends on timing (VI-0007). On
`debug_expand.rs` plain Verus printed the same order in 19 of 19 runs on one
host, while Sail with a warm cache, whose answers arrive instantly, printed
the root module's error first in 1 of 7 runs. The 14 messages were identical
in both (`corpus/repros/sail-cache-warm-error-order/`).

Charter rule 1 asks for the same diagnostics as plain Verus. The owner
decided that what users need from that is the same errors with the same text
and locations; the order in which messages from different modules are
printed is not part of it.

## Decision

- Conformance splits stderr into complete diagnostic messages (each from its
  header line to the next header or the summary) and compares the two sides
  as multisets. Every message must be present on both sides with identical
  bytes, the same number of times.
- Everything that is not a module's diagnostic message is still compared
  byte for byte and in order: the summary lines, notes, exit code and every
  per-function verdict.
- Messages from the same module keep their relative order in the comparison.
- The end-to-end checks that compare `sail verify` with `cargo verus verify`
  on a single-module project stay byte for byte.
- A difference in the set of messages, or in any message's bytes, is still a
  difference. Nothing is normalised inside a message.

## Consequences

- The VI-0007 timing variation no longer fails conformance, in either the
  default or the engine arm.
- A defect that drops, duplicates or alters a message is still caught.
- The comparison is implemented once, in `sail-conformance`, with tests for
  reordered, missing, duplicated and altered messages.
