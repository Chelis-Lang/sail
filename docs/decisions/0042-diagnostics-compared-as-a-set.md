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

## Implementation (2026-10-13)

`sail_conformance::diagnostics::compare_stderr` serves both the corpus arms
and the `rust_verify_test` runner (ADR 0019).
- **A message** is a column-0 header (`error`, `warning`, `note`, `help` or
  `failure-note`, optionally with a code in brackets, then `: `) whose next
  line is a `-->` location, up to and including the blank line that rustc's
  renderer prints after each diagnostic. Spanned child notes inside a
  diagnostic stay with it. A header without a location (`error: aborting
  due to 16 previous errors`, a progress note) is not a message, so it stays
  in the ordered text.
- **The ordered text** is everything else, with one placeholder line per
  run of consecutive messages, so a summary that moves before the messages
  is still a difference.
- **Same-module order is not enforced.** Module identity cannot be
  recovered from a message: its location names a file and line, inline
  modules share a file (`debug_expand.rs` has its root module and `M4` in
  one file), a location can point into another file such as `vstd`, and
  `--output-json` gives no function spans to map a location to a module. A
  rule based on files would reject the reorderings this ADR accepts. The
  comparison is therefore a plain multiset, and a defect that only reorders
  messages within one module would not be reported. Every message's bytes
  and count, and all other text, remain exact.
- **The `rust_verify_test` runner no longer forces `--num-threads 1`**: with
  the multiset comparison, Verus's default threads lost nothing on the full
  suite and cover three more tests fully (ADR 0019, update of 2026-10-13).

## Incidental source-test rerun paths (owner, 2026-10-21)

For the exact source-hash-bound dispatch query regression, ignore only the six
random tempfile characters in its printed SMT-log rerun commands. Preserve
the command format, thread count, path surroundings, diagnostics, query-byte
assertions and outcomes. This is test scaffolding, not a verifier diagnostic.
