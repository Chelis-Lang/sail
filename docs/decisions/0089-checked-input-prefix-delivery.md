# 0089: Share exact input-prefix and solver-delivery operations

- **Status:** proposed
- **Date:** 2026-10-28

## Context

The default v2 key binds the original input bytes, solver identity and mode.
The production key constructor, raw transcript and actual solver-input writes
must be connected to the checked operations without replacing the current
stream parser, held-response ordering or replay-consistency checks.

## Decision

Allow the direct `sail-key` → `sail-acceptance` edge. Keep the actual public
`SolverIdentity` and `Mode` declarations in `sail-key`; capture all their fields
and the opt-in string payload in the Stock driver. A shared factory passes
these fields to the exact v2 encoder and initializes the raw-input owner and
independent delivery seed together.

The acceptance leaf owns raw intervals, incremental framing and acknowledged
full-write accounting. The production SHA adapter and owned solver pipe remain
explicit reviewed primitive boundaries. The complete-input token freezes one
acknowledged stream; it does not establish solver-image or solved-result origin.

Preserve the existing domains, little-endian lengths, input byte order, mode
separation and silence-key framing. Overflow, partial delivery and conflicting
metadata withhold reuse without modifying the original forwarded bytes.

## Consequences

The leaf stays dependency-free. The added edge introduces no cycle or path to
the daemon or verifier. All existing layering edges and isolation checks remain.
The current shared parser, held-response operations and replay-comparison path
remain in place. The normal publication checks still require complete input,
quiet output and observed normal termination.

Encoding injectivity concerns fully framed bytes. Digest collision resistance,
SHA implementation, operating-system writes and launched-image identity remain
explicit trust boundaries. Native differential and added-cost qualification
are required separately from direct Stock proofs. This proposal does not claim
the complete service theorem or authorize merging an unqualified candidate.
