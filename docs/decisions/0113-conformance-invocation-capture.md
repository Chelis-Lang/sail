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
are recorded. A separate run-owned error file records failures even when a
reservation cannot be created and a build script consumes the delegate's
stderr. The loader checks this health record and requires an actual Cargo
compiler job; an earlier discovery probe cannot qualify a failed build. Capture errors and unexplained aggregate stdout fail the run.
This is a harness facility, not a product driver or shim change.

The aggregate stdout must be proved a complete order-preserving interleaving
of the captures' lines, including newline bytes. Torn lines, missing bytes,
extra bytes, unreadable records and a bounded search that cannot decide are
never passes. Compiler-discovery probes are recorded and paired too; their
streams go to Cargo or the invoking build script rather than aggregate stdout.
The pinned Cargo job shape carries JSON diagnostic/artifact flags. Known
version/print/unpretty discovery output is retained separately. Nonempty
stdout with neither supported shape is refused, rather than guessing a subset
that explains the aggregate. Hidden calls that mimic Cargo jobs can cause a
strict refusal; they cannot authorize dropping output.

Pair invocations by package/version, relative manifest, crate/target kind,
binary name, test mode, compilation target, complete arguments and requested
Verus driver channels. This includes the physical leading-space packed-option
variables, their exact separator encoding and all per-package selectors. Named
build-script probes also retain their source/flags/status association. Only
declared source, target, runtime and arm-output path roots become relative in argument
identity; exact original metadata remains recorded.
Repeated identities retain multiplicity and compare as multisets, consuming
each occurrence once. Completion time, process ID and aggregate document
position are never pairing keys.

Managed-arm attribution logs use separate compiler-owned directories, matching
the product driver's target-owned log isolation. The harness adds those options
through the same product packed-channel helper; it never shares one directory
between concurrent compilers. Capture format 2 records and hashes both Cargo's
original requested channels and the child's effective channels. Pairing binds
the original requests; the only controlled supplement is the recorded harness
logging configuration. Exact stdout, stderr and solver inputs are not rewritten.

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
