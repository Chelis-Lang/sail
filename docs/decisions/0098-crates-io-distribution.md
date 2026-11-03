# 0098: Publish Sail as sail-solve on crates.io

- **Status:** accepted; owner authorized initial and future crate publication on 2026-11-02
- **Date:** 2026-11-02
- **Amends:** ADR 0010's restriction on crate publication

## Context

The owner requested an initial crates.io version named `sail-solve` and crate
publication for future releases. GitHub archives remain available. Bare `sail`
already identifies another project's registry package. Existing path-only
dependencies and repository-relative embedded files prevent standalone Cargo
packages from building, and the CLI needs its shim and pinned verifier runtime.

## Decision

Publish the CLI package as `sail-solve`, retaining its `sail` executable and
the `sail-cli` source directory/dependency alias. Publish its eighteen-package
closure, including the contract crates used by its development dependencies,
with explicit `publish = ["crates-io"]` and exact matching internal versions.
The workspace default stays `publish = false`; benchmark/conformance tools,
xtask and the private temporal extension remain unpublished. The initial
registry version is `0.2.0-rc.1`; this prerelease does not establish completion
of the broader release performance program.

Add the `sail-solve -> sail-shim` dependency solely for a second thin binary,
`sail-solve-z3`, calling the existing shim library. This avoids workspace
artifact collisions with the archive's existing `sail-z3`. The CLI locates its
adjacent Cargo-installed companion, falling back to the archive name. The
shim library retains its existing small dependency graph and pinned-Z3 fallback.
The temporal extension retains its `sail-cli` import through an explicit
package alias; no private source enters the public dependency closure.

Embedded resource copies live inside their consuming crates. Their existing
repository files remain authoritative; `scripts/crate-resources.py` synchronizes
and checks exact bytes. CI rejects stale copies. No solver bytes, runtime pins,
proof keys or proof-admission policy are changed by copying resources.
The refinement runner retains its exact source and assembly guards: reviewed
Cargo metadata/lock hashes and the API test-fixture source inventory are rebound
to the new package layout. Executable/proof/import/module digests and declaration
stencils remain unchanged. Full controls and Stock refinements qualify this
metadata update before adoption.

`sail engine install` explicitly invokes an embedded exact copy of the existing
Python runtime installer. It downloads the committed platform artifact and
preserves its archive digest, provenance, namespace and per-file checks.
Existing import and runtime-selection behavior remain. Installation requires
Python 3; verification does not invoke the downloader. The verifier's Rust
toolchain is installed explicitly through Rustup without changing its default.
Cargo installations establish their own proofs; they do not claim the release
archive's shipped vstd proof or signed-reply acceleration.

Verify extracted packages together with Cargo before uploading. Publish only
from a clean committed source, retain registry checksums, and resume only
identical non-yanked versions. Keep the token out of preparation commands,
arguments, source and logs. Release publication uses a repository Actions
secret, isolated from ordinary CI. Future published GitHub releases trigger
the publisher for their immutable event commit SHA. Before checkout, resolve
the published tag through any annotated tag objects and require its commit to
match the event SHA. Manual retries require an explicit full qualified commit
SHA and the same tag-resolution check. Check out that commit directly and
recheck HEAD and the tag/workspace version before running source commands or
exposing the upload token. A moved tag therefore fails instead of selecting
different source; checksum-identical resume still requires the original commit.

## Consequences

Users can install the real CLI and shim with one Cargo command and install
the pinned runtime without cloning Sail. Crate versions and package-local
resource copies become release inputs. Future releases update and verify the
full published dependency set together. Public library APIs in the initial
prerelease retain their documented scope; publication grants no additional
proof authority and does not promote experimental verification modes.
