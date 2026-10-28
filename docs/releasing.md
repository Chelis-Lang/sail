# Releasing Sail

Development and performance work follow current main, including the Oyster
integration. Release composition remains the owner's decision. Do not create
a release branch, tag or draft merely because a sprint validation completes.

The earlier private 0.1.0-rc.6 artifacts were frozen on `release/0.1.0`, starting at
`fc29023a`. Their Rust/Cargo/runtime inputs equal the qualified `cda42861`
consumer. Later Oyster integration on main is outside those historical artifacts;
they are a baseline, not the selected scope of a future release.
Both frozen archives pass first-use proof reuse, an edit, actual repair and
real Claude Code MCP checks. All mutation comparisons pass; remaining cold
and unchanged costs keep performance acceptance open. The
[release record](validation/release-candidate.md) binds the exact artifacts.

For private draft review, collaborators can download the archives and checksums
with GitHub CLI, then follow the README's checksum and extraction commands:

```sh
gh release download v0.1.0-rc.6 --repo Chelis-Lang/sail \
  --pattern 'sail-0.1.0-rc.6-*.tar.gz*'
```

The ordinary release download URLs become available when the owner publishes.

The portable native path requires the pinned verifier toolchain's canonical
`lib/rustlib/src/rust` tree; a missing, aliased or changed tree declines proof
reuse and runs genuine verification. The bundle binds every source byte and
the exact compiler remap option. It does not permit user-supplied remap flags
or change machine-local research mode. See the
[compiler-boundary record](validation/vstd-compile-boundary.md).

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

### Attestations

The checksum file comes from the same release as its archive, so it detects
corruption but not a replaced asset. Each archive therefore also gets a GitHub
build-provenance attestation, signed by the manual
[`attest-release`](../.github/workflows/attest-release.yml) workflow. Given a
release tag, the workflow downloads that release's archives, checks them
against their `.sha256` files and attests each archive's SHA-256 for
`Chelis-Lang/sail` with `actions/attest-build-provenance`. It builds,
uploads and publishes nothing. The archives are still built locally, so the
attestation binds each archive's digest to this repository's workflow run; it
does not claim that GitHub built the archive. `scripts/install.py` and the
[install guide](install.md#verify-provenance) verify it with
`gh attestation verify ARCHIVE --repo Chelis-Lang/sail`.

Attestations on a private repository need GitHub Enterprise Cloud, so the
workflow first works once the repository is public; the first published
release is the first end-to-end check.

Each archive unpacks to one directory:

```
sail-VERSION-TARGET/
  LICENSE                       Sail's licence (MIT)
  THIRD-PARTY-NOTICES           licences of everything else the archive holds
  bin/sail                      the CLI, daemon and MCP server
  bin/sail-z3                   the solver shim; must stay beside sail
  engine/<variant>/             the repair engine runtime
    sail-build.json             its build manifest (provenance and digests)
    verus/source/target-verus/release/
                                cargo-verus, verus, rust_verify, z3, vstd.vir, ...
  deps/<variant>/               the shipped vstd proof (see below)
  replies/vstd.pack             signed solver replies for the pinned vstd (ADR 0062)
  replies/release.pub           the public key they are signed with
  release-files.json            the index binding every member's SHA-256, size and mode
```

`<variant>` is the engine's source-variant name: the pinned Verus commit
followed by a hash of the toolchain, fork commit, host, source lock and vstd
build (for example `7d829946231d6413a398f6e4160cdfe951261dad-81186034d2e137a2`
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
proof bundle. Actual Mac staged first-use components pass at different physical
Rustup/sysroot roots. The candidate is not yet a qualified release: Linux
producer/clean-consumer acceptance and measured fresh-machine installation cost
remain pending. A changed-root test does not establish that fresh-machine
boundary.

The record recipe is `scripts/shipped-deps`: exact vstd version, canonical
Cargo-produced lock, one requested proof. `scripts/release-shipped.py record`
uses a fresh private project/Cargo home and an owned daemon. Native prerequisites
compile normally. A release carries the default-limit proof and, when a
resource-limit subsumption witness is unavailable, a separately verified
`--rlimit 100` proof. Both retain exact keys and their own establishing runs.
The current macOS solver has reproduced resource-count variation, so its
recorder uses these exact profiles. Other limits verify normally on Mac.
Every member of both profiles is bound by the release index; an incomplete
second recording stops packaging. It requires Current, complete verified targets and the original
run before a complete dependency directory becomes discoverable. Refusals and
failures stop release packaging. Evidence is retained beside the stage directory
with `-proof-evidence` appended; it is not shipped or treated as proof authority.
On termination, the recorder unwinds its existing cleanup before exiting: it
stops its owned command/server groups, then reaps each held direct leader before
removing its private scratch. Cleanup checks the owned process group even
when its leader has already exited; a descendant ignoring termination still
receives the bounded kill escalation. Zombie-only groups have no live work.
The result records group cleanup outcomes and errors; an exited leader alone
is not successful server cleanup. This accounting is confined to the private
release recorder, outside public first-use timing.
Before launching its owned groups, the recorder requires a working bounded `ps`
process census (`procps` on Linux). The CI image provides it and the pinned
`rust-src` component needed by the proof-bundle factory. These producer
prerequisites do not establish a fresh consumer installation measurement.
A cancelled recording never installs a dependency bundle or release index.
The ignored actual-compiler witness entrypoints are separate from the pinned-Z3
solver suite. Their owning bounded acceptance runner invokes
`ci/run.sh boundary-witness admission` with `SAIL_ACTUAL_ADMISSION_CONFIG`, or
`ci/run.sh boundary-witness shapes` with `SAIL_ACTUAL_NATIVE_TUPLES`. These commands
remain local-only and require the frozen actual inputs; missing inputs fail.
The ordinary solver step runs the solver, cache and fault-injection tests and
does not manufacture compiler-witness inputs. Passing that step does not qualify
the separate compiler witnesses.
The adjacent evidence also names the actual recorder PID, owned scratch root and
UID for external resource accounting; this metadata is not proof authority.
Before deleting scratch, it also retains a bounded, content-once index of native
and verification/compilation input certificates and skipped records. The factory
also writes the existing compiler-boundary trace under its owned evidence
directory so native-provenance refusals remain diagnosable. This
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
is later removed; naming alone never authorizes proof reuse. Compiler children
run under cache hardlink/copy names, so their bundle location is derived from
the bound installed runtime, not the alias directory. The same admitted index
and fresh running-alias bytes, mode and size must match its indexed `bin/sail`.
The actual source/feature closure, successful current-run native records, native
bytes, compiler SourceMap phases, solver and exact options remain bound. There
is no self-declared qualification bit. Explicit Record, MachineLocal and alternate
bundle directories remain research-only. A family using a shipped plan does not
also acquire the retained-content authority. Without a shipped plan, ordinary
retained eligibility is unchanged. `sail new` uses the accepted bundle's lock for
its new private staged project only, before the project is moved into place.
The write uses create-new semantics: a generator's existing lock is preserved;
I/O failure reports `SAIL0132` and discards the staged project. Ordinary installs
without an admitted bundle add no lock. Existing projects' lockfiles are never
rewritten.

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

## Shipped vstd solver replies

The release factory also ships the pinned solver's replies from its own vstd
recording ([ADR 0062](decisions/0062-shipped-vstd-solver-replies.md)). After
`record` verifies vstd, it drains and stops its daemon, then runs
`sail cache bundle-replies --cache <recording cache> --out <dir>`. That
command signs every exportable entry with a key it generates in memory,
writes `vstd.pack` and `release.pub`, and never writes the secret. The
factory moves both into `replies/` before writing `release-files.json`.
`release-shipped.py verify`, `package` and the smoke test require exactly
these two files, a well-formed, non-empty pack and a public key.

When the shipped vstd proof misses (another toolchain component set, lock
closure, loader variable or Cargo), the daemon serves byte-identical vstd
solver queries from the pack. It trusts the key only for that pack, and only
if both files match the index. Every entry is still verified. The solver
answers anything else. `sail status` shows the pack under `bundled`, and
`SAIL_BUNDLED_REPLIES=0` turns it off. See the
[trust model](trust.md#bundled-tier-the-releases-own-replies).

## Proof-bundle prerequisites and first use

Ordinary releases without a qualified dependency proof bundle keep the README's
minimal Rust installation. A proof-bundle factory and its strict release smoke
additionally require the complete `rust-src` component for the Rust version
bound by the admitted runtime. The current pin is `1.99.1`:

```sh
rustup component add rust-src --toolchain 1.99.1
```

The factory derives that version from the validated runtime and checks it against
Sail's pin. Its preflight resolves the actual pinned sysroot, requires the
installed component's manifest, verifies the exact complete regular-file set
and hashes every source body within the separate 128 MiB source bound. Missing,
extra or aliased source members fail packaging; a directory alone is insufficient.
The resulting prerequisite inventory is diagnostic evidence, not a proof key or
certificate. Serving still requires the compiler-source tree/key and fresh
compiler-read/native-byte admission implemented by the boundary guard. Changing
a source body changes the prerequisite inventory and must still pass those
independent guards. No component is downloaded automatically by the factory or
installed Sail.

Record release proofs with the verifier's minimal toolchain plus `rust-src`.
The factory also pins Cargo itself to that toolchain. Cargo versions can add
different compiler flags even when they invoke the same Rust compiler; using the
Sail build toolchain's newer Cargo therefore produces a different exact proof key.
Managed verification selects the same Cargo for consumers
([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md)): when the
caller made no toolchain choice (no `RUSTUP_TOOLCHAIN`, `CARGO` or `RUSTC`, no
`rust-toolchain` file or rustup override for the project) and `cargo` is a
rustup proxy, `sail verify` runs Cargo with the pinned toolchain if it is
installed with `rust-src`, so a newer default Cargo still matches the bundle.
Otherwise the caller's Cargo runs; another Cargo version safely misses and
verifies vstd afresh, and a missing pinned toolchain is noted as `SAIL3137`.
Sail never installs it. The current factory does not produce a multi-Cargo bundle.
A first-use result is qualified for the Cargo the run selected: a fresh home
whose only toolchain is `1.99.1` does not by itself qualify the selection from
a newer default, which needs its own first-use measurement.
The compiler identity conservatively includes all installed target libraries;
an extra target therefore produces a different proof key even for a host build.
The bundle's `manifest.json` also records `sysroot_shape`: the name and size of
every library that identity hashes, without their contents. A consumer whose
sysroot has another shape (an extra component such as `rustc-dev`, or a
resized library) cannot have the proof's identity, so it refuses the proof
before hashing anything (#286). A matching shape grants nothing; the complete
identity still decides. A bundle without the field is decided by the identity
alone.
The Linux `ci/local.sh release` recipe removes the CI image's bare-metal target
inside its disposable container before recording. The shared image and its
conformance prerequisites remain intact. The factory refuses to record unless
the verifier toolchain's installed components are exactly the minimal profile
(`rustc`, `rust-std` and `cargo` for the host) plus `rust-src`. The notices
check that staging runs first reads the fork's lock with Sail's own toolchain,
so the fork's `rust-toolchain.toml` cannot make rustup add `rustc-dev`,
`llvm-tools` or `rustfmt` to the verifier toolchain. For macOS packaging, prepare a separate
Rustup home with the pinned Sail toolchain and the verifier's minimal toolchain
plus `rust-src`, and use it for the release command. Existing full-toolchain
changed-root controls do not qualify a minimal installation.

The factory sets its owned proof bundle's `Cargo.lock` to mode `0644` before
binding the complete member index. This keeps archives portable across producer
umasks and strict extraction without relaxing file-mode or content checks.

`scripts/release-smoke.sh` verifies prerequisites after strict archive unpacking,
then uses public `sail new` and a locked first verification. Its owned work
directory is canonicalized before exporting the cache path, including macOS
`/tmp` aliases, so native dependency snapshots receive supported physical paths.
Success now requires Current complete reports, exactly one fresh requested-target
invocation and the vstd target's exact `release` origin from this indexed bundle. An ordinary
successful full verification (SAFE_MISS), a machine-local/store proof or warm
local receipt reuse does not pass this first-Serve smoke. The later edited
verification and repair smoke remain. This stricter smoke does not enable the
currently unqualified release eligibility.

Before first-use acceptance on each platform:

1. Start with physically separate, empty Rustup/Cargo homes and project/cache/target
   roots. Install the pinned minimal toolchain and, for the proof-bundle actor,
   `rust-src`. Retain complete installer output, actual downloaded archive byte
   counts/digests and checksum-verification results, installed source inventory
   and elapsed setup time. Existing Mac sources or a cloned preinstalled toolchain
   establish only a portability control, not fresh installation performance.
2. Measure public project creation and verification separately and report their
   sum. Also report setup plus creation plus verification, with equivalent stock
   setup/project layout. Component download/verification time and bytes cannot
   be hidden outside the first-use totals. Use three paired rounds and retain
   every refusal/failure and all ordinary command output.
3. Require actual release origin, all complete stock function/verdict/diagnostic
   comparisons, missing/corrupt/index warm cases, failed requested target and
   native-compiler replacement controls. Complete native Linux and Mac gates,
   changed physical roots and a useful first-use gain remain prerequisites to
   selection. Existing same-host factory or research-mode results are insufficient.

The prerequisite/origin source controls use owned compiler/runtime doubles.
They test complete, missing/extra/aliased component membership and content
inventory changes, exact release origin, and SAFE_MISS/warm/store/foreign-origin
refusals. They do not qualify an actual release or first-use performance.
The helper refuses Python optimization (`-O`, `-OO` or `PYTHONOPTIMIZE`) because
its required validation assertions must execute.

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
   moves `CHANGELOG.md`'s Unreleased entries under that version. In the same
   PR, regenerate `THIRD-PARTY-NOTICES` if the locks, the pinned fork or the
   pinned Z3 changed: `scripts/third-party-notices.py --work NEW_DIR`
   (`--verus-repo FORK_CLONE` uses a local clone instead of fetching the
   pinned fork commit). A new Z3 version needs its licence vendored as
   `scripts/notices/z3-VERSION-LICENSE.txt` first. Staging runs the same
   script with `--check` and fails the release on stale notices, then copies
   them and `LICENSE` into each archive.
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
     and asset list;
   - starts the [`attest-release`](#attestations) workflow for `vVERSION`
     and prints the `gh attestation verify` command for each archive.

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
5. When the `attest-release` run succeeds, check each archive's attestation
   (`gh attestation verify dist/sail-VERSION-TARGET.tar.gz --repo
   Chelis-Lang/sail`). Publish only after both verify.
6. The owner reviews the draft and decides when to publish.

When the installed stages have already passed the native matrix, full mutation
cohorts and walkthroughs, package those same bytes on an Apple-silicon Mac:

```sh
scripts/release.sh package VERSION dist/stage/linux dist/stage/macos
```

This checks both complete release indexes and their platform runtime pins,
then writes the same four assets under `dist/`. It refuses existing output
names. It does not rebuild, record new proofs, upload, create a tag or publish.
Keep the stages' native `sail --version` results and CLI hashes with their
qualification results; `VERSION` must match the version they report. Review
and create the draft from these assets only after the release gates pass,
then attest them with `gh workflow run attest-release.yml --repo
Chelis-Lang/sail -f tag=vVERSION`.
The one-command build-and-draft route above remains available for a new build;
its output needs qualification before publication.

`scripts/release.sh stage OUT ENGINE_BUILD` is the staging step alone: it
copies `target/release/sail` and `sail-z3` (stripped of the release
profile's line tables with `strip -S`; function symbols stay), and the engine build's manifest
and runtime files, into `OUT`, then runs the self-check.

## Qualifying the macOS engine

The Linux runtime is qualified when it is pinned (engine README, Validation).
The macOS runtime is built on an Apple-silicon Mac from the same fork commit,
so it needs its own run, recorded in
[the macOS engine record](validation/macos-engine.md). As on Linux, the
Verus toolchain must not have `rust-src`
([VI-0031](verus-issues/VI-0031-rust-src-std-paths.md)). The binaries record
the absolute paths of their dependency sources (panic locations) and vstd.vir
those of vstd's, so set `RUSTUP_HOME`, `CARGO_HOME` and `SAIL_CACHE_DIR` to
directories outside the home directory (for example under `/tmp`) and check
the packed archive's strings for `/Users/` before publishing it.
The unpacked-tree check uses `ci/engine-artifact.py scan --root TREE`, which
checks file bytes, including tokens spanning read chunks. Enumeration and
read errors, symbolic links and other nonregular entries fail the scan.
`SAIL_SCAN_TOKENS` adds whitespace-separated tokens such as the build hostname.
`scripts/build-macos-runtime.sh build` does all of this, plus the fork's
library and regression tests and the determinism check, and packs the
archive; after publishing, `scripts/build-macos-runtime.sh published` fetches
the pinned runtime anonymously and runs the conformance and `cargo verus new`
checks. The steps it runs:

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

`test-engine.py` requires the engine to match the plain fork run exactly,
raw SMT included, and stock on everything but raw SMT, which it records in
`summary.json` (ADRs 0054 and 0056; engine README, Validation).

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

The ordinary shipped profile is vstd's exact public `alloc/default/std` closure.
Ordinary retained restoration remains a separate qualified Linux `alloc/std`
profile without `default`. A shipped plan suppresses retained authority for the
whole run, including the requested target. No combined-provider authority or
returned-A speedup is inferred for a newly distributed proof bundle; Linux
release acceptance must check A/B/A and the existing qualified Vest profile
under the installed bundle's source mapping.


The unqualified compiler-source portability candidate also requires ordinary
pinned Cargo native compiler shapes. Forced warnings, custom non-feature cfgs,
unsupported compiler/codegen/linker inputs and global Rustflag overrides
(including Cargo configuration and inline `--config`) decline portable
recording/serving and use genuine verification. The real admitted
forced-warning diagnostic mismatch remains indexed in the
[boundary validation record](validation/vstd-compile-boundary.md); the
eligibility repair passes its source-qualified Mac native admission/refusal
controls, but has not yet qualified a release.


The guarded Mac compiler-source candidate applies compile-time native
proc-macro debug-map removal for portable Record/Serve only. The composed Mac
components pass actual recording, first Serve at changed physical Rustup roots
and unmodified consumer diagnostic comparisons. This is not a selected release
option: native Linux qualification, a clean consumer and useful first-use
performance remain pending. It changes neither consumer flags nor the Linux or
machine-local modes, and never rewrites a compiled library.
