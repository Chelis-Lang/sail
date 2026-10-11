# 0036: Managed compiler artifact families

- **Status:** implemented; whole-product validation pending
- **Date:** 2026-10-11

## Context

Before compiler families, the managed runner placed Cargo's target directory
beneath the complete proof context. That context binds the source engine request,
including the chosen function. A different focus selector therefore chose a
different dependency build tree, even with identical compiler, engine, solver,
configuration and arguments. The rebuilt source-engine repair fixture exhausted its original total
budget; a separate retained diagnostic exhausted the same budget in dependency
compilation before submitting a missing selector. These observations establish
avoidable target separation in the code, not the cause or cost of each timeout.

## Decision

Keep the existing proof-context serialization and digest unchanged. Add a
separate, domain-tagged compiler-artifact digest over the same exact arguments,
engine request, runtime identity and byte-exact environment, clearing only the
engine request's function field. Keep stock and source-engine requests distinct.
Compiler artifacts and executable aliases live in this family. Runtime sidecars,
source observations, proof receipts, committed reports and runs stay under the
complete proof context. Local build-script observations also remain there; an
optional family export certifies ordinary generated-output contracts for reuse,
never proof receipts or Currentness.

The optional daemon sysroot hint selection also uses a distinct versioned domain
and the same borrowed function-independent request view. This changes selection
only. Each caller still creates its own namespace/kernel Baseline before import,
checks exact scope and content identities, validates at the original barriers,
and falls back to original local hashing on refusal or uncertainty. No caller
digest owner or proof context is shared. The 256 KiB streamed selection bound
remains unchanged; request buffers are borrowed rather than cloned.

A nonblocking family ownership lock accompanies the existing proof-context lock
through package cleaning, Cargo invocation, artifact validation and receipt
publication. Acquisition always proceeds from proof context to compiler family.
A busy family returns the existing SAIL3104 error. No process waits for another
family owner or extends a verification deadline.

Ordinary Cargo freshness controls dependency reuse. Every selected repair
package is still forced fresh. A skipped verification artifact without an intact
receipt for the exact proof context follows the existing clean-and-reverify path.
An old receipt for another selector cannot establish Current. No compiler, solver,
source engine, selected package, proof result or freshness guard is substituted.

## Consequences

Changing only a source-engine focus selector can reuse ordinary dependency
artifacts. The new artifact namespace has a one-time cold setup and serializes
focus contexts that share a compiler family. Other request fields, arguments,
runtime bytes and environment changes isolate families. Source and Cargo
configuration changes retain their existing content and Cargo freshness guards.

A compiler family can reuse an ordinary script contract only after independently
rehashing all declared files and environment, matching the exact package ID and
manifest in current metadata, and checking Cargo's actual `output` file bytes and
parsed declarations under that family's target. The export binds the compiler
family and a checksum, and is limited to a 1 MiB encoded envelope. Linux reads
use nonblocking, no-follow regular-file opens; unsupported platforms, malformed,
missing, incomplete, changed or oversized records take fresh script cleaning.
These bounds cover the new envelope and each output file read, not total managed
memory or RSS. Existing local Cargo observation loaders retain their original
loading behavior.

The exclusive owner consumes the export before any cleaning or invocation.
Failed, interrupted and non-Current runs cannot leave an earlier generation
certifying rewritten output. An absent generation forces cleaning even when an
older private context has observations. Its old declaration paths remain only
as uncovered watch roots, so the original declaration-establishment transition
can acquire the necessary inventory; they grant no output reuse. A fresh export
is atomically published only after the original successful Current transaction.

A second full contract check runs after the original native owners and monitored
snapshot are live, before Cargo can reuse output. Any newly invalid package is
cleaned. This closes a restored-mtime change between preliminary checks and
watch acquisition. Every distinct output unit for a package is retained in local
observations and checked; any invalid unit disqualifies the entire package. A
valid package version cannot hide another invalid version with the same name.

Cargo-injected or proof-specific declared environment variables conservatively
refuse sharing. The one audited exception is `RUSTC_BOOTSTRAP=1`, set by the
pinned Cargo-Verus and explicitly bound by the managed child command; old
parent-value observations become stale. No process environment is mutated.
All other actual environment and tool changes retain family isolation. The
compiler identity includes the actual Sail executable bytes/path; an older
supported CLI cannot write into a newer binary's family.

These observations have the same local trusted-artifact boundary as existing
managed Cargo artifacts and proof receipts. The checksum detects accidental
corruption; it is not authentication. A writer able to fabricate both private
metadata and arbitrary Cargo output is outside this boundary. Generated OUT_DIR
artifacts retain Cargo's original integrity/freshness rules, with exclusive
Sail mutation and export invalidation. No historical path list alone certifies
current output, no proof observations are transferred, and first-run incomplete
declarations remain incomplete.

The [retained diagnostic](../validation/m7-compiler-script-observations.md)
shows a repeated dependency build in the same family after selector change. It
does not prove the cause of the earlier whole-loop timeout. Product and unchanged
benchmark validation remain required; no fixture or acceptance bound changes.
