# 0036: Managed compiler artifact families

- **Status:** proposed
- **Date:** 2026-10-11

## Context

The managed runner currently places Cargo's target directory beneath the complete
proof context. That context binds the source engine request, including the chosen
function. A different focus selector therefore chooses a different dependency
build tree, even with identical compiler, engine, solver, configuration and
arguments. The rebuilt source-engine repair fixture exhausted its original total
budget; a separate retained diagnostic exhausted the same budget in dependency
compilation before submitting a missing selector. These observations establish
avoidable target separation in the code, not the cause or cost of each timeout.

## Decision

Keep the existing proof-context serialization and digest unchanged. Add a
separate, domain-tagged compiler-artifact digest over the same exact arguments,
engine request, runtime identity and byte-exact environment, clearing only the
engine request's function field. Keep stock and source-engine requests distinct.
Compiler artifacts and executable aliases live in this family. Runtime sidecars,
source/build-script observations, proof receipts, committed reports and runs stay
under the complete proof context.

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

Every build script without this proof context's declared-input observation is
cleaned before its output can be reused. This extends the existing missing-local-
script rule to registry packages as well. Proof-specific runtime paths remain
visible to build scripts, so declared environment dependencies may correctly
rebuild. This change does not promise
reuse in those cases, receipt transfer or a latency improvement before actual
product and unchanged benchmark validation. The original repair fixture limits,
source-engine qualification and conformance oracles remain required.
