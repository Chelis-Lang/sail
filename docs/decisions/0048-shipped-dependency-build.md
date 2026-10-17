# 0048: Reuse vstd at its compiler boundary after native prerequisites

- **Status:** proposed; implementation prototype, disabled by default
- **Date:** 2026-10-17
- **Issue:** [#161](https://github.com/Chelis-Lang/sail/issues/161)
- **Extends:** [ADR 0053](0053-disable-unqualified-native-store.md)

## Context

The owner requires recovery of the first-project speed lost when the native
artifact store was quarantined. Copying native Cargo units before a build can
hide a changed native compiler's failure (#187). Hashing a few tool names does
not establish every input to arbitrary native compilation.

The earlier shipped-vstd implementation already records compiler outputs,
source bindings, exact linked-library bytes and nonlocal proof provenance.
Its replay of native libraries, proc macros and build-script executables has
the same unresolved boundary. That path must not be enabled.

## Decision

Prototype a narrower path in the Cargo compiler wrapper. Every ordinary Rust
crate, proc macro and build script runs through the actual compiler before any
cache lookup. Only the pinned, nonprimary vstd library dependency may look up
its own recorded outputs, after Cargo has finished its native prerequisites.
The lookup binds the actual fresh `--extern` libraries and adjacent VIR files,
all compiler options, runtime and installed sysroot identity, actual solver
SHA-256, source files and declared compiler environment.

Only a complete successful verification document can publish a vstd entry.
Replayed proofs keep their original establishing invocation and distinguish
release builds from machine-local verification through later receipt reuse. Native output files and Cargo fingerprints are never placed by this
path. The v1 machine-local store stays disabled.

The prototype requires `SAIL_VSTD_BOUNDARY_RESEARCH=1`. Engine, focused and
loader-variable runs decline it. Schema 6 and a distinct bundle kind reject
the earlier all-unit bundles. Source-path remapping is confined to the named
experiment and uses the previously built fork support; this change selects
no runtime or release asset.

A separate `SAIL_VSTD_BOUNDARY_LOCAL=<directory>` research mode binds both the
literal and canonical registry root into each key. It never remaps registry
sources and may retain those literal paths in outputs because reuse cannot
cross registry roots. It still rejects outputs naming the old target directory,
executes native prerequisites and requires a complete successful proof before
publication. Misses compile and may populate this separate cache; an absent
release manifest does not disable observation. Local entries report the actual
original invocation, never a fictitious release-built proof.

The first native local screen established an actual hit: 8.456 s against
37.892 s without the boundary cache, with fresh native prerequisites in both
projects. Replacing the private native compiler still failed before verification.
This single observation motivates the narrower source/provenance work below;
it is not qualification of that earlier unrestricted first-fill model.

The next source requires an explicit embedded policy for the 15 official
resolved packages and 662 archive file hashes, plus their exact default feature
sets. It checks actual sources independently of mutable Cargo checksum metadata;
foreign sources, patches, versions, features and macro providers decline reuse.
Optional native observations surround the real compiler, preserving inherited
streams and its status. A successful admitted library invocation can establish
only its declared, initially absent output files, bound to source/feature,
actual Verus launcher/embedded compiler driver, generated inputs and this run's
native dependency observations. Cargo's leading rustc path is a discarded
compatibility argument, not the executing compiler. External dependency search
and incremental directories decline observation; ordinary owned target paths
remain supported.
First-fill and serving both require those current-run output records. The key
also hashes actual runtime artifacts, rather than trusting their manifest alone.
Unsupported wrapper/compiler/backend/source shapes compile without establishing
reuse. These checks are necessary input provenance, not a completed audit of
all compiler builtin, span, macro or diagnostic inputs.

This is not a correctness qualification. The remaining questions are whether
fresh native prerequisites have repeatable bytes, whether the pinned macro
and compiler input model is complete, and whether source/artifact mutation
during compilation can invalidate publication. No source-level key is assumed
equivalent to exact solver input without qualification. Missing, changed or
damaged inputs must run the actual compiler and verifier.

The next publication check compares compiler-retained source-map checksums
against independent official vstd BLAKE3/size constants. This does not change
compiler options or add a product dependency. It rejects changed compiler reads
even if the filesystem is restored before publication, but it does not attest
arbitrary proc-macro tracked files or mapped native artifacts. Those remaining
input boundaries still block default selection.

## Consequences

A successful replacement would retain fresh native compilation failures while
avoiding vstd's repeated verification. Native prerequisite compilation remains
part of the first-project cost. If fresh dependency bytes differ, the lookup
must miss. Three experimental fresh-project pairs now show a 4.23× median gain,
but the compiler-input boundary and full parity remain unqualified. The native changed-compiler reproduction,
per-function verdict and diagnostic parity, corruption and mutation refusals,
both platforms and measured first-project cost gate any default enablement.

See [the implementation record](../validation/vstd-compile-boundary.md).
