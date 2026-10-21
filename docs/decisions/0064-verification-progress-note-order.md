# 0064: Accept verification-progress note ordering

- **Status:** accepted (owner, 2026-10-21)
- **Date:** 2026-10-21

## Context

Stock Verus prints filtered verification bucket-start notes in worker arrival
order (VI-0012). Sail's fork already emits these notes in queue order. Native
Anvil controls on stock 7d82994 reproduce different orders for identical inputs,
while per-function verdicts, diagnostics and exit codes agree. The earlier
strict standard comparison remains a failed attempt, rather than a passing
result retroactively.

## Decision

Permit reordering of complete standalone `note: verifying module ...` progress
blocks, including function-qualified blocks. Require the exact same bytes and
multiplicity. Only blocks in one uninterrupted run may reorder; a diagnostic
message, summary, other note or other output ends that run. Unterminated blocks
and text inside diagnostic messages retain their existing exact comparison.

Everything else retains its contract: complete diagnostic messages and counts
under ADR 0042, other stderr in order, non-JSON stdout, document counts,
per-function verdicts and exit codes. The harness records an accepted VI-0012
entry separately from failures when the progress ordering differs. An accepted
progress difference cannot turn a verdict or error difference into a match.

## Consequences

Conformance can recognise deterministic fork progress despite stock scheduling.
The shared stderr reader applies this contract to corpus and deep-rvt checks.
This changes only comparison of observed progress; it changes no runtime,
SMT input, solver key or reported verdict. Changed, missing or repeated notes
remain differences. The charter appendix and divergence registry record the
owner-approved exception.
