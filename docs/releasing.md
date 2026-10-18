# Releasing Sail

Sail ships prebuilt binaries for Linux and macOS as GitHub release assets on
`Chelis-Lang/sail`. Each archive includes the repair engine runtime. Until
the owner's open-source release, every release stays a **draft**: the release
script never publishes one, and only the owner publishes.

Releases are built locally. CI stays Linux-only and never builds, packages or
publishes a release (ADR 0013); there are no macOS runners.

Source-suite validation checks both runtime-artifact and test-executable
SHA-256 digests before reusing a cached build. A suite manifest without test
digests predates this check: rebuild it using `cargo xtask build-verus-source
--force` with the intended variant options. Do not fill in digests from an
existing cache. This development-suite requirement does not change bundled
runtime identity or qualify any new release; see
[ADR 0019](decisions/0019-rust-verify-test-conformance.md).

For the source-suite integrity control, use an owned native Linux stock build;
the script deliberately changes a named test executable and then rebuilds it.
Keep its evidence outside the source/cache tree. Build the matching xtask and
conformance binaries first, with no caller build-directory override:

```sh
cargo build --release --locked -p xtask -p sail-conformance
target/release/xtask build-verus-source --keep-intermediates
sail_stock_suite="$(target/release/xtask build-verus-source --print-path)"
python3 scripts/test-source-suite-integrity.py --build "$sail_stock_suite" \
  --binary target/release/sail-conformance --out "$SAIL_INTEGRITY_OUT"
```

`--build` must identify the single active-pin stock build; do not use a fork,
engine, shared build or unrelated cache. Set `SAIL_TEST_Z3` to the pinned binary
release's Z3, and use Python 3.11 or newer. The control refuses mismatched
manifests and retains failed/interrupted raw results. If a rebuild is
interrupted, the source cache may be incomplete and needs rebuilding before
validation. The [qualification record](validation/source-suite-integrity.md)
states the remaining gates; a list-only check does not qualify verifier results.

## Assets

For version `VERSION`, the draft release `vVERSION` holds four assets:

| Asset | Platform |
|---|---|
| `sail-VERSION-x86_64-unknown-linux-gnu.tar.gz` | Linux x86-64, glibc 2.39 or later (built on Ubuntu 24.04) |
| `sail-VERSION-x86_64-unknown-linux-gnu.tar.gz.sha256` | its checksum |
| `sail-VERSION-aarch64-apple-darwin.tar.gz` | macOS on Apple silicon |
| `sail-VERSION-aarch64-apple-darwin.tar.gz.sha256` | its checksum |

A checksum file has one line in `shasum -a 256` format, so
`shasum -a 256 -c sail-VERSION-TARGET.tar.gz.sha256` (or `sha256sum -c`)
checks the archive.

Each archive unpacks to one directory:

```
sail-VERSION-TARGET/
  bin/sail                      the CLI, daemon and MCP server
  bin/sail-z3                   the solver shim; must stay beside sail
  engine/<variant>/             the repair engine runtime
    sail-build.json             its build manifest (provenance and digests)
    verus/source/target-verus/release/
                                cargo-verus, verus, rust_verify, z3, vstd.vir, ...
```

`<variant>` is the engine's source-variant name: the pinned Verus commit
followed by a hash of the toolchain, fork commit, host, source lock and vstd
build (for example `426d8b01e7ffb910a36c15e1f870ff985bb61eef-6c1904a2c6736118`
on Linux). Users should not rename or move anything inside the directory.

The bundled runtime is Sail's Verus: every verification Sail runs uses it,
default mode included ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)),
so every archive must carry it. What users still install themselves is
`rustup` with the Rust toolchain Verus pins (`corpus/toolchain.toml`). The
README's [Install](../README.md#install) covers that, along with the archive
download and checksum check. When the Verus pin changes, update the toolchain
and the `vstd` version quoted there.

The release factory and smoke scripts require Python 3.12 or newer. Set
`SAIL_RELEASE_PYTHON=python3.12` if the default `python3` is older. This is a
packaging/test prerequisite, not a dependency of the installed Sail binaries.

The candidate release stage additionally records one complete vstd proof with
its final stripped Sail/shim binaries and installs it under `deps/<variant>`.
`release-files.json` binds every regular file and directory mode, including the
proof bundle. This source implementation is not yet a qualified release: actual
staged first-use on both platforms and at different Rustup roots remains pending.
The existing machine-local and changed-Cargo-home results do not establish that
fresh-machine boundary.

The record recipe is `scripts/shipped-deps`: exact vstd version, canonical
Cargo-produced lock, one requested proof. `scripts/release-shipped.py record`
uses a fresh private project/Cargo home and an owned daemon. Native prerequisites
compile normally. It requires Current, complete verified targets and the original
run before a complete dependency directory becomes discoverable. Refusals and
failures stop release packaging. Evidence is retained beside the stage directory
with `-proof-evidence` appended; it is not shipped or treated as proof authority.
On termination, the recorder unwinds its existing cleanup before exiting: it
stops and reaps its owned command/server groups, then removes its private scratch.
A cancelled recording never installs a dependency bundle or release index.
The adjacent evidence also names the actual recorder PID, owned scratch root and
UID for external resource accounting; this metadata is not proof authority.
Before deleting scratch, it also retains a bounded, content-once index of native
and verification/compilation input certificates and skipped records. This
evidence excludes full native compiler bodies; the staged unit keeps its vstd
artifacts. Missing certificates in a successful record or failed capture stops
packaging and still runs owned cleanup. Already admitted certificate bodies remain
indexed on a later capture refusal. A failure while removing scratch also marks
the record failed and removes its staged proof and index.

Normal verification may serve only a matching adjacent `deps/<variant>` bundle
whose complete proof members match its adjacent `release-files.json` index;
missing, old-schema, unindexed, changed or unsupported entries compile and verify
normally. Full member admission runs for a new target; the per-run admission
binds the exact unit bytes served and the unchanged index. Benign diagnostic
source naming remains available for already-restored artifacts when the index
is later removed; naming alone never authorizes proof reuse.
The actual source/feature closure, successful current-run native records, native
bytes, compiler SourceMap phases, solver and exact options remain bound. There
is no self-declared qualification bit. Explicit Record, MachineLocal and alternate
bundle directories remain research-only. A family using a shipped plan does not
also acquire the retained-content authority. Without a shipped plan, ordinary
retained eligibility is unchanged. `sail new` uses the accepted bundle's lock for
its new project only; existing projects' lockfiles are never rewritten.

`package` verifies all indexed release members before archiving. The smoke
script first checks the entire archive checksum, rejects unsafe/duplicate/link
members before extracting, then checks its complete file/mode/directory index and
proof bundle. The archive checksum and qualified release producer are the trust
boundary; an arbitrary self-authored manifest is not evidence of qualification.
After source review, release acceptance must execute the staged archive with no
engine/compiler/research override, compare its fresh Current results with plain
Verus, and prove the second unchanged call invokes no verifier. A native-byte or
key mismatch is a safe miss, not a first-use gain. See the
[boundary record](validation/vstd-compile-boundary.md).

## How sail finds the bundled engine

No setup is needed. `sail` looks for its Verus (the runtime, which is also
the engine) in this order:

1. `SAIL_VERUS_BUILD`, a build directory named explicitly (CI images and
   development);
2. the cache variant, `<cache>/verus-src-build/<variant>`, installed by
   `cargo xtask build-verus-engine` or `sail engine install --from PATH`;
3. the bundled runtime, `<release>/engine/<variant>`, where `<release>` is
   the parent of the directory holding the running `sail` (symlinks to
   `sail`, for example from `~/.local/bin`, are resolved first).

The first candidate that exists is validated exactly as before: the manifest
must name the pinned Verus commit, Rust toolchain, fork commit, host, source
lock and solver digest, and every runtime file must match its recorded
SHA-256. A bundle that fails validation reports `SAIL3120` from
`sail engine status` and repairs, and `SAIL0101` from verification, like a
corrupt cache install; Sail never falls back to another Verus. Because the
variant name binds the host and fork commit, a bundle from another platform
or Sail version is never selected. `sail engine status` shows the selected
build. Ordinary verification reads every runtime file in full once per
install and then compares file stats (ADR 0043).

## Procedure

Prerequisites: an Apple-silicon Mac with this repository, `gh` signed in with
access to `Chelis-Lang/sail`, AWS credentials for `scripts/perf-host.sh`,
`rustup`, and the pinned stock Verus (`cargo xtask fetch-verus`, for
qualifying the macOS runtime against it).

1. Land a PR that sets `[workspace.package] version` in `Cargo.toml` and
   moves `CHANGELOG.md`'s Unreleased entries under that version.
2. Check that `engine/verus/artifact-macos.json` names the fork commit in
   `engine/verus/fork.toml` and that the
   [macOS engine record](validation/macos-engine.md) qualifies that runtime.
   If the fork commit changed, build, qualify, publish and re-pin it first
   (below).
3. On a clean checkout of that commit of `main`:

   ```sh
   scripts/release.sh VERSION
   ```

   `VERSION` must start with the workspace version (for example `0.1.0` or
   `0.1.0-rc.1`). Both platforms build with `SAIL_VERSION=VERSION`, so
   `sail --version` and every report's `sail_version` (and the MCP server's
   version) state the exact release version, suffix included (a build without
   `SAIL_VERSION` reports the workspace version). The script:
   - refuses a dirty checkout, a commit that is not on `origin`, or an
     existing release `vVERSION`;
   - **Linux:** brings up a native host (lane `$SAIL_RELEASE_LANE`, default
     `release`, reusing it if already up and otherwise tearing it down at the
     end), syncs the checkout and runs `ci/local.sh release` there. That
     builds the CI image, whose engine is the pinned artifact
     (`engine/verus/artifact-linux.json`, checked by `ci/engine-artifact.py`),
     builds `sail` and `sail-z3` in release mode and stages the tree;
   - **macOS:** builds `sail` and `sail-z3` in release mode, downloads the
     pinned runtime (`engine/verus/artifact-macos.json`) from the public
     fork's release with `ci/engine-artifact.py fetch` (no token; archive
     digest and every runtime file checked) and stages the tree with it;
   - self-checks each staged tree: `sail --version` must print
     `sail VERSION`, `sail mcp` must state the same version, and
     `sail engine status`
     with an empty `SAIL_CACHE_DIR` must select and validate the bundled
     engine;
   - packages each tree as `dist/sail-VERSION-TARGET.tar.gz` (entries owned by
     root) with its `.sha256` file, creates the **draft** release `vVERSION`
     targeting the commit, uploads the four assets and prints the release URL
     and asset list.

   Every archive carries the runtime; there is no engine-less archive, since
   Sail runs no other Verus.
4. Smoke-test each archive on a machine with only the user prerequisites
   (Linux: a fresh `scripts/perf-host.sh` host; macOS: a fresh directory),
   with no `cargo-verus` on `PATH`:

   ```sh
   scripts/release-smoke.sh sail-VERSION-TARGET.tar.gz
   ```

   It checks the checksum, unpacks into a new directory and, with an empty
   `SAIL_CACHE_DIR`, requires `sail engine status` to select the bundled
   runtime, `sail verify` to pass on a new project made as the README's First
   verification says, and one `sail repair-check` through the engine to be
   accepted, with no engine fault.
5. The owner reviews the draft and decides when to publish.

`scripts/release.sh stage OUT ENGINE_BUILD` is the staging step alone: it
copies `target/release/sail` and `sail-z3` (stripped of the release
profile's line tables with `strip -S`; function symbols stay), and the engine build's manifest
and runtime files, into `OUT`, then runs the self-check.

## Qualifying the macOS engine

The Linux runtime is qualified when it is pinned (engine README, Validation).
The macOS runtime is built on an Apple-silicon Mac from the same fork commit,
so it needs its own run, recorded in
[the macOS engine record](validation/macos-engine.md):

```sh
cargo xtask build-verus-engine       # also builds and verifies vstd
build="$(target/release/sail engine status --output json | python3 -c 'import json,sys; print(json.load(sys.stdin)["build"])')"
stock="$HOME/.cache/sail/verus/<release>/verus-arm64-macos"
python3 engine/verus/test-engine.py --engine "$build/verus/source/target-verus/release/verus" \
  --stock "$stock/verus" --z3 "$stock/z3" --out OUT/source-fixtures
python3 engine/verus/test-cancel.py --engine "$build/verus/source/target-verus/release/verus" \
  --stock "$stock/verus" --z3 "$stock/z3" --out OUT/cancel-restart
SAIL_ENGINE_BUILD="$build" ci/run.sh conformance fast pr
```

Then pack and pin it, and publish the archive as a prerelease on the public
fork, like the Linux runtime (engine README, Publishing):

```sh
python3 ci/engine-artifact.py pack --platform macos --build "$build" \
  --archive OUT/engine-runtime.tar.gz --pin engine/verus/artifact-macos.json
gh release create engine-runtime-<short sha>-macos -R Chelis-Lang/verus --prerelease \
  --latest=false --target <fork commit> OUT/engine-runtime.tar.gz
```

If it fails, do not release macOS: Sail on macOS needs the runtime. Record
why and keep the failure in [known issues](known-issues.md).
