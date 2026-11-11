# 0113: Compare complete compiler invocations in Cargo conformance

- **Status:** accepted
- **Date:** 2026-11-11

## Context

VI-0039 and #632 establish that concurrent Cargo-Verus stdout is an
interleaving of compiler streams, and even serial Cargo can change their
completion order. Document-position pairing cannot establish per-function
conformance for this profile. The existing strict failures remain evidence;
serializing the command or dropping malformed fragments does not qualify it.

## Decision

The differential harness stages an unchanged copy of each arm's Cargo-Verus
frontend beside a recording `verus` delegate. The delegate runs the original
driver, forwards both streams, and retains their exact bytes, arguments,
package/target identity, status and SHA-256 digests. It reserves an incomplete
record before spawning and completes it only after both streams and status
are recorded. Capture errors and unexplained aggregate stdout fail the run.
This is a harness facility, not a product driver or shim change.

The aggregate stdout must be proved a complete order-preserving interleaving
of the captures' lines, including newline bytes. Torn lines, missing bytes,
extra bytes, unreadable records and a bounded search that cannot decide are
never passes. Compiler-discovery probes are recorded and paired too; their
streams go to Cargo or the invoking build script rather than aggregate stdout.

Pair invocations by package/version, relative manifest, crate/target kind,
binary name, test mode and compilation target. Discovery probes additionally
bind their arguments. Only declared source, target and runtime path roots
become relative in probe identity; exact original metadata remains recorded.
Repeated identities retain multiplicity and compare as multisets, consuming
each occurrence once. Completion time, process ID and aggregate document
position are never pairing keys.

Every ordinary invocation uses the existing structured stdout and per-function
oracle, with byte-equal stderr and exact non-JSON stdout. Probes compare both
streams byte for byte and retain their exit statuses. The aggregate exit and
complete diagnostic oracle remain checked. Plain confirmation repetitions use
the same oracle under ADR 0009; invalid captures cannot become instability.
Engine seal checks and displayed totals use all captured documents.

Add the existing workspace `sha2` dependency to `sail-conformance` for capture
integrity. No new package, internal layering edge, runtime dependency or solver
identity is introduced. The parser's exact lock binding must be independently
reviewed for this dependency-only lock edge before it advances.

## Consequences

Parallel conformance can qualify complete compiler results without changing
compiler output or the user's concurrency. The tee must be built/shipped with
the contributor harness, including prebuilt end-to-end archives. Direct Verus
units retain their original comparison. This does not repair Cargo's public
output framing, establish parallel scaling performance, or accept any verdict
or diagnostic divergence. Linux/Mac controls, real parallel tears, injected
capture failures, fast/standard corpus qualification and independent review
remain implementation gates, not consequences of accepting this design.
