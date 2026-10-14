# Releasing Sail

Sail ships prebuilt binaries for Linux and macOS as GitHub release assets on
`Chelis-Lang/sail`. Each archive includes the repair engine runtime. Until
the owner's open-source release, every release stays a **draft**: the release
script never publishes one, and only the owner publishes.

Releases are built locally. CI stays Linux-only and never builds, packages or
publishes a release (ADR 0013); there are no macOS runners.

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
build (for example `426d8b01e7ffb910a36c15e1f870ff985bb61eef-9a788f8bd626312c`
on Linux). Users should not rename or move anything inside the directory.

What users still install themselves: the stock pinned Verus release, with
`cargo-verus` on `PATH` (default `sail verify` runs it, and the engine falls
back to it), and `rustup` with the Rust toolchain Verus pins
(`corpus/toolchain.toml`), which both stock Verus and the engine use. The
README's [Install](../README.md#install) covers these, along with the archive
download and checksum check. When the Verus pin changes, update the release,
toolchain and archive digests quoted there.

## How sail finds the bundled engine

No setup is needed. `sail` looks for an engine in this order:

1. the cache variant, `<cache>/verus-src-build/<variant>`, installed by
   `cargo xtask build-verus-engine` or `sail engine install --from PATH`;
2. the bundled runtime, `<release>/engine/<variant>`, where `<release>` is
   the parent of the directory holding the running `sail` (symlinks to
   `sail`, for example from `~/.local/bin`, are resolved first).

The first candidate that exists is validated exactly as before: the manifest
must name the pinned Verus commit, Rust toolchain, fork commit, host, source
lock and solver digest, and every runtime file must match its recorded
SHA-256. A bundle that fails validation reports `SAIL3120`, like a corrupt
cache install. Because the variant name binds the host and fork commit, a
bundle from another platform or Sail version is never selected.
`sail engine status` shows the selected build.

A macOS archive built without the engine (see below) has no `engine/`
directory; `sail engine status` then reports `SAIL3120`, and repairs report
it too, while `sail verify` runs stock Verus as usual.

## Procedure

Prerequisites: an Apple-silicon Mac with this repository, `gh` signed in with
access to `Chelis-Lang/sail`, AWS credentials for `scripts/perf-host.sh`,
`rustup`, and the pinned Verus (`cargo xtask fetch-verus`).

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
   `sail --version` reports the exact release version, suffix included (a
   build without `SAIL_VERSION` reports the workspace version). The script:
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
     `sail VERSION`, and `sail engine status`
     with an empty `SAIL_CACHE_DIR` must select and validate the bundled
     engine;
   - packages each tree as `dist/sail-VERSION-TARGET.tar.gz` (entries owned by
     root) with its `.sha256` file, creates the **draft** release `vVERSION`
     targeting the commit, uploads the four assets and prints the release URL
     and asset list.

   `SAIL_RELEASE_MACOS_ENGINE=0` ships the macOS archive without the engine,
   for when the macOS engine is not qualified; the release notes say so.
4. Smoke-test each archive on a machine with only the user prerequisites
   (Linux: a fresh `scripts/perf-host.sh` host; macOS: a fresh directory):

   ```sh
   scripts/release-smoke.sh sail-VERSION-TARGET.tar.gz [--no-engine]
   ```

   It checks the checksum, unpacks into a new directory and, with an empty
   `SAIL_CACHE_DIR`, requires `sail engine status` to select the bundled
   engine, `sail verify` to pass on a new `cargo verus new` project, and one
   `sail repair-check` through the engine to be accepted, with no engine
   fault. `--no-engine` is for an archive built without the engine.
5. The owner reviews the draft and decides when to publish.

`scripts/release.sh stage OUT [ENGINE_BUILD]` is the staging step alone: it
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

If it fails, ship macOS without the engine (`SAIL_RELEASE_MACOS_ENGINE=0`),
record why, and keep the failure in [known issues](known-issues.md).
