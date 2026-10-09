# 0021: Optional local Singular source validation

- Status: accepted
- Date: 2026-10-09

## Decision

Keep the required CI image and its fast gates unchanged. Add a separate local
Singular image, extending the same pinned image with Ubuntu's exact Singular
package version. `ci/local-singular.sh` refuses CI execution. The default
release does not support Singular; validation requires an explicit source build.

`cargo xtask build-verus-source --features=singular` creates a separate build
location bound to the Verus commit, Rust toolchain and feature set. The default
build retains its existing directory. The manifest records feature and artifact
identities. `SAIL_VERUS_SOURCE_BUILD` explicitly selects a source build; the
runner validates its commit, toolchain, pinned Z3 hash and runtime artifact
hashes before executing it. An invalid or incomplete build is an infrastructure
failure. It cannot silently replace the pinned release.

Add tagged deep-tier integer-ring examples and the pinned memory allocator.
The allocator's `libc-verus` preparation builds test_libc under the pinned Rust
toolchain in a scratch copy and passes its rlib explicitly to Verus; both sides
use that same prepared tree. No arbitrary preparation script is executed.
The Singular step runs ordinary differential conformance in pass-through,
cache-fill and cache-warm modes. Its reports distinguish unsupported builds,
missing tools and real verifier outcomes.

## Consequences

Singular is optional and local-only. Existing Z3/default-mode keys remain bound
to the actual solver binary and command prefix; there is no cross-build shortcut.
Source builds may take a long time and are never required per PR.
