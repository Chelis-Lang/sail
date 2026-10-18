# Managed visible-output ordering

## Status

Built candidate for #215; unselected. Seven focused production relay/sender
controls, an actual compiler-child cleanup control and release all-target
Clippy pass on macOS. No runtime pin, corpus entry, proof-key algorithm or
reuse requirement changes. Native gates, full E2E, conformance, performance
and the complete macOS host gate remain pending.

The controlled actual-main relay probe on macOS recorded 44 reordered combined
outputs in 96 attempts, while preserving each separate stream. Its producer
wrote a large filtered Cargo JSON record, then a summary, then a Cargo-like
error. This is a fixture reproduction of the relay race, not real Verus
acceptance and not attribution of the original #174 failure. Raw observation
SHA-256: `145ac95ef790cbea0ee9b17e6e73f5f4d15b692b897999ae1d560c22955a3377`.

The proposed [acknowledged transport](../decisions/0055-acknowledged-managed-output.md)
has source tests using the production sender and actual relay with a real
child. They cover both causal stream orders; separate and shared writers;
suppressed human stdout; opaque non-UTF-8 bytes across several frames; a
65,536-byte JSON payload remaining exact in Cargo's metadata capture; and
a broken public sink refusing acknowledgment and cleaning endpoints. These
are actual production transport/relay controls with a Cargo-like child,
not real-project Verus acceptance.

Independent source review identified a compiler-child cleanup hole on a failed
streamed acknowledgment, and interleaving of large writes from concurrent
drivers. The candidate now owns compiler/reader cleanup, serializes complete
emits with a bounded file lock and defers raw Cargo diagnostics through the
final chunk. Preserved non-JSON pipe stdout is deferred through that same
boundary; the concurrency controls commit both pipe streams mid-write and
require each opaque driver emit to stay contiguous. The incomplete-write
deadline is checked on every loop iteration, including busy Cargo JSON;
an actual stalled peer is refused after 30 seconds while its child keeps
producing metadata. No output bytes are normalized or sorted.

The passing focused run uses source blobs `619d5dab` (adapter), `fdb4eecc`
(runner), `5d6a3630` (transport), `c4821480` (concurrent controls) and
`8ce9ef2e` (timeout control), based on `3892881a`. Its eight tests and Clippy
pass; source and all 3,290 protected executable/intermediate/raw paths stay
byte-exact. The command and its children are reaped. Raw result SHA-256:
`c90c892df3a4d9a133d8138d2fe42723b9fd335878455a9b5e50168fe67c2448`.
These results establish the focused control scope only.

## Failed and interrupted attempts

- `3892881a` plus candidate, 2026-10-18, attempt 01: refused before any
  command on disk admission; controls unrun.
- Same base, 2026-10-18, attempt 02: monitor interrupted compilation when a
  normal transient Cargo release path vanished; no unit result.
- Same base, 2026-10-18, attempt 03: five controls pass; the new broken-pipe
  fixture incorrectly required its producer to succeed after the relay closed
  its read pipes. The corrected fixture follows the real caller's kill/wait
  failure path and retains strict NACK and endpoint assertions.
- Same base, 2026-10-18, attempt 04: six relay controls and compiler cleanup
  pass; Clippy reports four source/fixture lint errors. An enclosing guard also
  rejects Cargo updating `target/.rustc_info.json`; all 3,290 other protected
  paths match. This remains a failed attempt. The later recipe prospectively
  permits that regenerable Cargo probe cache, preserving every body check.
- Same base, 2026-10-18, attempt 05: compilation fails on an under-specified
  deferred-buffer type; the explicit `Vec<Vec<u8>>` correction is in the
  passing attempt 06. No unit result from attempt 05.

Acceptance requires real passing/failing/compile-error/unchanged runs through
plain and managed commands, strict combined and separate byte comparisons,
JSON/MCP behavior, closed-pipe handling, all nine E2E phases plus local extra,
the workspace/solver gates and applicable full FAST/STANDARD conformance.
Measure the additional visible-write cost on native Linux before selecting
the implementation. Keep failed attempts visible here and raw originals
outside the repository.
