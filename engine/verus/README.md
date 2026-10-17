# Sail's Verus and its interactive engine

Sail's Verus is a Sail-managed source variant built from Sail's Verus fork
([ADR 0039](../../docs/decisions/0039-private-verus-fork.md)). It is the only
Verus Sail runs: ordinary `sail verify` and every other verification use it
([ADR 0043](../../docs/decisions/0043-default-mode-runs-the-fork.md)), and
it contains the optional interactive repair engine described below. Stock
releases are the conformance oracle. One pinned runtime per platform
(`artifact-linux.json`, `artifact-macos.json`) serves both uses: it holds
`verus`, `cargo-verus`, `rust_verify`, the builtin and macro libraries, vstd
and the pinned Z3, everything `cargo verus` needs.

A repair runs fresh Rust frontend, type, mode, trait and ghost lifetime checks,
then focused verification and an independent full verification in one active
compiler callback. Each phase lowers every bucket fresh, exactly as stock Verus
does, and creates fresh solver contexts. Full stdout and its diagnostics remain
authoritative only after deferred errors and the late EXEC compilation pass;
focused results and progress remain provisional.

The interactive driver catches only the pinned Rust compiler's expected
`FatalErrorMarker` unwind. Unexpected internal panics resume unwinding and cannot
reach a compiler seal; ordinary default verification retains its existing
catch-all behavior. A terminal `compiler-sealed` frame includes
`payload.normal_termination: true` only on a normal driver path, including
reported proof, VIR and late EXEC errors. This flag describes completion, not
success: the full JSON, exit status, selected scope and coherent input checks
still determine authority. Missing or malformed completion, internal panic,
signal, or malformed full output requires the stock fallback.

## Source: Sail's Verus fork

The engine is built from one commit of Sail's public Verus fork,
[Chelis-Lang/verus](https://github.com/Chelis-Lang/verus), pinned in
[fork.toml](fork.toml) (repository, branch, full commit and upstream base).
The owner decided on 2026-10-12 that Sail stays forked and fixes known Verus
issues there; nothing is contributed upstream
([Verus issues](../../docs/verus-issues/README.md)).

The fork's branches:

- `upstream/<short sha>` is an unmodified upstream Verus commit, for example
  `upstream/426d8b0` (the pin in `corpus/toolchain.toml`) and `upstream/8dea4a2`
  (the secondary pin).
- `sail` is that upstream commit plus Sail's commits, in this order:
  1. one commit per Verus issue fix, each with its regression test under
     `source/rust_verify_test/tests/sail_vi<NNNN>_*.rs`, plus one harness
     commit (`--sail-raw=<arg>` in `tests/common/mod.rs`) that the tests use;
  2. last, `feat: interactive repair engine`, the engine itself (formerly
     `engine/verus/interactive.patch`).
- `pinned/<sha>` tags every fork commit a Sail runtime pin has ever named
  (for example `pinned/4881cd0f`), so a pinned commit stays reachable after
  `sail` is rebased or a fix is dropped. Tag the commit when its pin lands.
- Lanes push their own branches (for example `l12/engine-2`); the lead
  updates `sail` after review.

Each fix is small and self-contained so it can be rebased, dropped (when a
Verus release fixes the issue) or reviewed alone. Fix commits change only
orders, the `--output-json` fields named in their issue, Cargo log layout,
or (VI-0019, [ADR 0049](../../docs/decisions/0049-stable-closure-names.md))
closure-type names. An ordering fix must choose an order stock Verus is shown
to produce (ADR 0039); the engine conformance arm, with its stock
confirmation runs, checks this. The closure-name fix produces stock's output
up to a one-to-one renaming of closure numbers, and must qualify on
fork-versus-stock conformance and the full `rust_verify_test` suite. VI-0005's dep-info carry-over was dropped: it made every Sail repair
rebuild (the selected function is a forwarded argument). VI-0007's
bucket-order diagnostics were dropped once, when the check found
`debug_expand.rs` different and no stock run had shown the fork's order, and
reinstated (`bc9cda09`, pinned from `73dea021`) after stock was shown to
print it ([VI-0007](../../docs/verus-issues/VI-0007-nondeterministic-error-order.md)).
Run the repair benchmark on a candidate before pinning it.

`cargo xtask build-verus-engine` fetches exactly the pinned commit (the
fork is public, so no credentials are needed), checks that its upstream base
equals `corpus/toolchain.toml`'s commit, writes the committed
[Cargo.lock](Cargo.lock), writes Verus's toolchain manifest for the build's
version as the release workflow does (so `cargo verus new` works,
VI-0018), and builds. The variant's identity
(`sail-fork:<commit>` feature) and the runtime pin
([artifact-linux.json](artifact-linux.json), `fork_commit`) bind the commit.
CI never builds Verus; it installs the prebuilt runtime.

### Upgrading Verus

1. In every fresh clone of the fork, set the human identity first
   (`git config user.name` and `user.email`, as in this repository). Rebasing
   and cherry-picking rewrite the committer, and without a configured email
   git falls back to `user@hostname`. Check new commits with
   `scripts/check-ai-markers.sh range <base> <head>`, which rejects AI markers
   and hostname-fallback identities.
2. Push the new upstream commit to the fork as `upstream/<short sha>`.
3. Rebase `sail` onto it (`git rebase --onto upstream/<new> upstream/<old> sail`
   on a review branch). Drop fixes the release already contains; resolve
   conflicts per commit.
4. Run every `sail_vi*` regression test and the `rust_verify_test` suite on
   the rebased branch, and record the results.
5. After review, move `sail` to the branch, update `corpus/toolchain.toml` and
   [fork.toml](fork.toml) together, then do one rebuild, the qualification
   steps below, and publish and re-pin the runtime
   ([Publishing a runtime](#publishing-a-runtime)). Tag the pinned commit
   `pinned/<sha>`.

## What the engine commit contains

The engine commit (`feat: interactive repair engine`, formerly
`interactive.patch`) changes four files:

- `rust_verify/src/driver.rs`: the fatal-error handling above and the
  `compiler-started` and `compiler-sealed` observations.
- `rust_verify/src/verifier.rs`: the focused phase before the full phase in the
  same callback, the reporter it uses, and bounded observation hooks
  (diagnostics, obligation progress, functions observed).
- `rust_verify/src/sail_engine.rs` (new): the focused phase, its bounded report
  (`SAIL_ENGINE_FOCUS_REPORT`, holding only the selected function) and the
  bounded observation journal (`SAIL_ENGINE_EVENTS`).
- `vir/src/ast_util.rs`: a friendly-name helper that admits a name's bytes
  before allocating it.

`Cargo.lock` is the stock source lock (identical to the fork's
`source/Cargo.lock`): the engine adds no dependency. The build still writes it
and binds its digest (`source-lock:` feature).

### Removed: prepared-lowering reuse and the same-input oracle

Earlier patches cached prepared lowering (the SST and polymorphism output of
each bucket) within one invocation and, with an opt-in, across edits, and
checked every restored hit with a same-input cached-versus-fresh byte oracle.
Measured reuse was near zero (no in-batch hits on HumanEval; L1) while every
miss paid for key encoding and admission under one global lock, about 6.7 s
per repair on HumanEval and 40 s on Vest. The oracle also needed confirming
fresh arms and an unstable class to stop flaking on stock trigger
nondeterminism (VI-0015). Both were removed (rescue L7a; ADR 0038). Reuse
across edits is redesigned for M8 with content-addressed keys. The removed
patch, its qualification pipeline and its record (`qualification-linux.json`)
are in this directory's git history and the `archive/m7-contracts-f7103bf`
tag.

## Validation

`test-engine.py` and `test-cancel.py` are local-only differential checks that
use an already built variant; CI never compiles Verus.

- `test-engine.py` compares the engine with stock on exit status, exact
  stderr, verification counters, per-function verdicts and full raw SMT
  sessions. Fixtures cover bitvectors, termination, failed focus, failed other
  functions, macro diagnostic spans, missing and ambiguous exact selectors,
  Rust frontend errors and late EXEC errors.
- `test-cancel.py` kills a worker at an actual blocked solver check, requires
  no terminal seal or unfinished focus report, and verifies the restart
  against stock.

```sh
python3 engine/verus/test-engine.py --engine BUILD/verus/source/target-verus/release/verus \
  --stock STOCK_VERUS --z3 Z3 --out OUT/source-fixtures
python3 engine/verus/test-cancel.py --engine BUILD/verus/source/target-verus/release/verus \
  --stock STOCK_VERUS --z3 Z3 --out OUT/cancel-restart
```

Also run the changed crates' library tests in the build checkout
(`cargo test --release -p rust_verify --lib`, and likewise `vir` and `air`),
and the fork's regression tests (`vargo test --release -p rust_verify_test
--test sail_vi...`).
Final performance acceptance uses the frozen repair catalogue and the complete
focused-plus-full loop ([product acceptance](../../docs/bench/m7-repair-acceptance.md)).
The earlier accepted `73dea021` runtime checks are recorded in
[the fork-default record](../../docs/validation/fork-default.md#runtime-qualification);
those of earlier pins in [the M7 validation record](../../docs/validation/m7.md)
(Gate 1, engine build and qualification). A new fork commit needs its own
run, including the determinism check
([determinism record](../../docs/validation/determinism.md)).

## Corpus conformance arm

`sail-conformance engine` (run by `ci/run.sh conformance`) compares this
runtime's full phase with the stock release on the corpus, on verdicts and
diagnostics, and requires a normal, consistent compiler seal; see the
[repair guide](../../docs/proof-repair.md#engine-conformance).

`cargo xtask build-verus-source --interactive-engine` builds vstd the way the
release does (`cargo-verus build --manifest-path vstd/Cargo.toml` from
`source/vstd`, variant feature `vstd-build:cargo-verus-release`), so
diagnostics that point into vstd render as `seq.rs:980:12`, as stock does,
rather than `vstd/seq.rs:980:12` (L4b).

## Pinned Linux runtime for CI

`artifact-linux.json` (schema 2) binds the release asset (`repo`,
`release_tag`, `asset`), archive digest, upstream commit, fork commit
(`fork_commit`), lockfile, Rust toolchain and solver. `ci/engine-artifact.py`
checks those pins and every runtime file before installation. The Docker image
does not build Verus from source; the optional runtime is exercised by the
real repair CLI/MCP end-to-end phase alongside the stock checks. CI downloads
these exact prebuilt bytes; it never compiles the engine.

### Where runtimes are published

Runtimes are GitHub release assets on the public fork,
[Chelis-Lang/verus](https://github.com/Chelis-Lang/verus/releases): one
prerelease per pinned fork commit, tagged `engine-runtime-<short sha>-linux`
and targeting that commit. The asset is the packed archive,
`engine-runtime.tar.gz`. The exact selected commit, release tag, asset name and
archive checksum are recorded in [artifact-linux.json](artifact-linux.json).
The [full closure-identity record](../../docs/validation/closure-full-identity.md)
tracks the selected full-identity runtime's platform bindings, measured scope and
qualification limits.

Fetching needs no token: the CI image build, `ci/local.sh`, native hosts and
`ci/engine-artifact.py fetch` download anonymously. `fetch` builds the URL
from the committed `fork.toml` repository, never from the pin alone, requires
the release tag to name the pinned fork commit, sends no credential and
accepts only HTTPS redirects. Trust comes from the committed archive digest
and the runtime manifest, never from where the bytes were served.

### Publishing a runtime

A new fork commit needs a new runtime. Until it is published and pinned the
committed pin is stale and the CI image build refuses it. Because default
mode runs the runtime too, a re-pin also needs the conformance tiers to pass
with Sail on the new runtime against plain stock Verus
(`ci/local.sh conformance fast`, then `standard`; ADR 0043).

1. On a native Linux x86-64 host, build it with
   `cargo xtask build-verus-engine` and run the qualification steps
   ([Validation](#validation)).
2. Pack it:
   `python3 ci/engine-artifact.py pack --build BUILD --archive OUT/engine-runtime.tar.gz --pin engine/verus/artifact-linux.json`.
   The pin records the archive digest and the release it belongs to
   (`repo`, `release_tag` and `asset`).
3. Publish the archive on the fork, with publish rights to
   `Chelis-Lang/verus` (a maintainer step; fetching needs none):
   `gh release create engine-runtime-<short sha>-linux -R Chelis-Lang/verus --target <fork commit> --prerelease --latest=false --title "Engine runtime <short sha> (Linux x86-64)" OUT/engine-runtime.tar.gz`.
   State the fork and upstream commits, source lock, toolchain, Z3 and
   archive digests and the build host in the notes.
4. Check the published bytes anonymously:
   `curl -fsSL https://github.com/Chelis-Lang/verus/releases/download/<tag>/engine-runtime.tar.gz | sha256sum`
   must print the pin's `archive_sha256`.
5. Commit the pin with the `fork.toml` change and tag the commit
   `pinned/<sha>`. Never replace or delete the asset of a release a pin has
   named: older Sail commits still fetch it.

A runtime prerelease is a prerequisite, not acceptance of the broader repair
benchmark or three-project edit catalogue.

## Release archives

Sail's release archives bundle the runtime under `engine/<variant>/` beside
`bin/sail`, and `sail` uses it when neither `SAIL_VERUS_BUILD` nor the cache
names one, after the same validation
([releasing](../../docs/releasing.md)). Every archive must carry it. The Linux archive carries the pinned
artifact above. The macOS archive carries the runtime pinned in
[artifact-macos.json](artifact-macos.json) (same schema; host
`aarch64-apple-darwin`, from the release tag's `-macos` suffix), built on
Apple silicon from the same fork commit, published as the fork's prerelease
`engine-runtime-<short sha>-macos` (pack with `--platform macos`; otherwise
the steps above apply), and qualified separately in the
[macOS engine record](../../docs/validation/macos-engine.md). CI never builds,
installs or runs the macOS runtime.

The [combined binder/runtime candidate](../../docs/validation/spec-parameter-names.md)
has both platform archives built and remains under qualification. Its native
compiler observations run only when explicitly requested; the quarantined v1
dependency store remains disabled.
