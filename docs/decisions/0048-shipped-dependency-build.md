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
loader-variable runs decline it. Schema 9 and a distinct bundle kind reject
the earlier all-unit bundles. Source-path remapping is confined to the named
experiment and uses the previously built fork support; this change selects
no runtime or release asset.

On macOS, admitted native proc macros in the experiment receive one explicit
linker option to set `LC_ID_DYLIB` to `@rpath/` plus their exact Cargo output
basename. Ordinary linking embeds the absolute target directory, preventing
strict fresh-project artifact equality ([VI-0026](../verus-issues/VI-0026-macos-native-install-name.md)).
The additional literal compiler tokens enter the native input key before the
actual child observation and are rechecked at completion. Source, library,
solver and output bytes remain exact; no existing binary, query or response
is normalized. Linux and nonexperimental builds receive no option. A minimal
pinned-compiler control establishes identical native bodies and working macro
consumers. Actual Mac fill, fresh consumer, unchanged receipts and loader
controls pass with complete native-body equality. Adversarial and wider runtime
qualification remain required before selection; no Mac timing is claimed.

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

Every cache refusal, including corruption, must leave ordinary compiler output
unchanged. The optional research trace retains the reason; the prototype's
earlier extra corruption warning is removed. Normal compilation and verification
still decide the result after the refusal.

This is not a correctness qualification. The remaining questions are whether
fresh native prerequisites have repeatable bytes, whether the pinned macro
and compiler input model is complete, and whether source/artifact mutation
during compilation can invalidate publication. No source-level key is assumed
equivalent to exact solver input without qualification. Missing, changed or
damaged inputs must run the actual compiler and verifier.

The publication check requires distinct GHOST and EXEC source-map
observations from the real compiler, compared against independent official vstd
BLAKE3/size constants. Dep-info alone can describe a later compiler phase or
replace a source-map hash with an extra tracked-file reread. This does not change
compiler options or add a product dependency. It rejects changed compiler reads
even if the filesystem is restored before publication, but it does not attest
arbitrary proc-macro tracked files or mapped native artifacts.

A controlled native-library substitution subsequently produces a different
vstd metadata/VIR pair under the original key and a requested-project metadata
import error (#222). Its proof-canary control remains failed. The successor
therefore binds consumption by copying exactly builtin metadata and the two
admitted macro libraries into a fresh private directory, verifying those bytes
against current-run native provenance and forwarding those exact paths to the
compiler. No hardlinks or native Cargo output replay are used. Copies remain
intact through publication; old schema-7 records are rejected. Additional native
graphs and adjacent VIR still decline. The actual Linux dylibs depend only on
platform runtime libraries. Both-platform loader checks, literal parity and
protected mutation controls still block default selection.

## Native producer consumption: proposed successor

Copying vstd's direct inputs does not establish how those inputs were built.
An admitted macro source or transitive native library could be replaced during
its compiler invocation and restored before an endpoint hash check. The
schema-9 successor therefore requires successful compiler-owned source-map
observations for each native producer as well. The separate native protocol
binds the actual child PID, run/key nonce, source checksums and original sizes,
and tracked environment. It writes only after ordinary compilation returns.
The existing two-pass vstd protocol remains separate and unchanged.

Native compilation receives independent private copies of the complete owned
`-L dependency` artifact graph and explicit externs. Every copied input must
match a qualified current-run producer. Cargo's metadata-ready pipeline can
precede a producer's final output, so the observer waits for completion and
recaptures the graph before starting the dependent. Copies remain owned until
completion. Current-run producer records retain the exact completion document
and its digest; legacy records cannot establish a successor input.

The closed official policy now includes independently generated compiler
BLAKE3/size constants for all 662 existing SHA-256-checked archive files.
An `include_str!` input reported as both a source-map file and an extra tracked
file is admitted only by its retained compiler source checksum. Other tracked
files require an owned immutable graph input; unknown inputs refuse reuse.
Lexical path lookup resolves ordinary `src/../README.md` against the policy;
raw compiler paths, diagnostics, arguments and solver streams are unchanged.
Feature admission accepts exact Cargo spellings and refuses alternate Rust
spellings rather than overlooking them. All unsupported shapes compile normally. Native lookup graphs require canonical
owned target paths; cache paths through system aliases decline this research
path. Ordinary macOS split-debug enum values remain literal keyed options.

Fork `067d0d7c` implements this native protocol. Pinned compiler API controls
prove retained source checksums survive restoration and no-request output bytes
match, but they do not qualify the full fork or this Sail integration. A genuine
runtime build, native source/transitive mutation controls, measured reuse and
full acceptance remain required. No default activation follows from these
source checks.

## Consequences

A successful replacement would retain fresh native compilation failures while
avoiding vstd's repeated verification. Native prerequisite compilation remains
part of the first-project cost. If fresh dependency bytes differ, the lookup
must miss. The direct-observation native prototype measures three fresh-project pairs at
39.840 s versus 9.336 s (4.27×), with separate compiler-read records for both
phases and complete paired literal cold controls. Its enclosing final retention
step remains failed; the compiler-input boundary and full parity remain unqualified. The native changed-compiler reproduction,
per-function verdict and diagnostic parity, corruption and mutation refusals,
both platforms and measured first-project cost gate any default enablement.

See [the implementation record](../validation/vstd-compile-boundary.md).
