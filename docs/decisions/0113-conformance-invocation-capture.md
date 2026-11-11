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
identity; exact original metadata remains recorded. Rooted values have typed
namespace/prefix/suffix identities, distinct from every literal argument. The
suffix retains exact separators and dot components; string placeholders or
path-component normalization cannot collapse a literal into a rooted path.
The same rule covers argv, manifest/target fields and each packed channel.
Repeated identities retain multiplicity and compare as multisets, consuming
each occurrence once. Completion time, process ID and aggregate document
position are never pairing keys.

Managed-arm attribution logs use separate compiler-owned directories, matching
the product driver's target-owned log isolation. The harness adds those options
through the same product packed-channel helper; it never shares one directory
between concurrent compilers. Capture format 3 records and hashes both Cargo's
original requested channels and the child's effective channels. Pairing binds
the original requests; the only controlled supplement is the recorded harness
logging configuration. Exact stdout, stderr and solver inputs are not rewritten.

Every compiler command carrying `-` captures and forwards raw stdin, records
its bytes and digest, and binds that digest into pairing. Input failure or
incomplete input is a capture failure, never a matched backend rejection.
Known hidden `--emit=metadata -o -` probes retain binary stdout verbatim; only
forwarded compiler JSON requires UTF-8.

Native autocfg probes use a random 16-hex-digit instance name and a monotonic
ordinal ([autocfg 1.5.1 source](https://github.com/cuviper/autocfg/blob/1.5.1/src/lib.rs)).
For the exact non-verifying, stdin-fed, library/LLVM-IR shape, pairing binds
package/target, ordinal, complete source bytes and every remaining argument/
packed channel. It treats only that recorded instance label as opaque. The
actual command, input, output and solver queries are never rewritten. Verified
packages and arbitrary names have no such rule. Ambiguous instance labels
within one arm fail rather than matching as an unordered multiset.

Every ordinary invocation uses the existing structured stdout and per-function
oracle, with byte-equal stderr and exact non-JSON stdout. Probes retain their exit statuses and byte-exact stderr. Their plain stdout
stays byte-exact. A selected-package `-vV` call can print Rust verbose text
followed by ordinary Verus JSON: its adjacent text boundaries remain byte-exact
and that JSON uses the same existing structured oracle, including every
verification-result and function field. Recorded fork/version/timing metadata
is treated exactly as it is for ordinary compiler JSON, not as a verdict.
This corrects the initial implementation's overstrict probe-byte rule to the
handover's requested structured-stdout comparison; no existing oracle field or
accepted-divergence predicate is removed. The aggregate exit and
complete diagnostic oracle remain checked. Plain confirmation repetitions use
the same oracle under ADR 0009; invalid captures cannot become instability.
Displayed totals include every captured Verus JSON document, including the
structured suffix of version probes. Engine target-seal checks use actual
Cargo compiler jobs; a hidden discovery probe cannot create a requested target.

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
