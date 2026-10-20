# 0039: The engine is built from a private Verus fork

- **Status:** accepted
- **Date:** 2026-10-12

## Context

The interactive engine (ADR 0027) was a single patch, `engine/verus/interactive.patch`, on the pinned Verus commit. Several tracked Verus issues hurt agents and Sail directly:
- run-to-run output order (VI-0001, VI-0007, VI-0008, VI-0012, VI-0015) lowers cache hit rates and makes cached-versus-fresh comparisons unusable;
- wrong or ambiguous structured output (VI-0003, VI-0009, VI-0010) misleads agents;
- Cargo integration gaps (VI-0004, VI-0005) lose results.

The owner decided on 2026-10-12 that Sail fixes these itself and stays forked. Upstream contribution is not planned: "verus makes ai code contribs high overhead. we'll track these and stay forked."

## Decision

- **Fork repository.** The fork is the private repository `Chelis-Lang/verus`.
  - Branch `sail` is the pinned upstream commit, then one small commit per fix, each with a regression test, then the interactive engine.
  - Branches `upstream/<sha>` mark each upstream base.
  - Lanes push their own branches. The lead fast-forwards `sail` after review.
- **Pinning.** Sail pins one fork commit in `engine/verus/fork.toml`. The engine artifact's provenance records that commit. The patch file is retired.
- **Fix rules.** A fork fix may only:
  - make stock's output deterministic by choosing one of the outputs stock can already produce;
  - correct structured output (JSON, timing, logs) without changing verdicts or diagnostics.

  Anything that would change a verdict needs its own ADR.
- **Scope.** The fork serves the opt-in engine only. Default `sail verify` keeps running the stock pinned release, so Sail's stock-mode workarounds stay. *Superseded by
  [ADR 0043](0043-default-mode-runs-the-fork.md): default mode runs the fork
  too, and stock Verus is only the conformance oracle.*
- **Upgrades.** Push the new upstream commit as `upstream/<sha>`, rebase `sail` onto it, rerun every fix's regression test and Verus's `rust_verify_test` suite, then rebuild and re-pin.
- **Tracking.** `docs/verus-issues/` records each issue and the fork commit that fixes it. Nothing is filed upstream.

## Consequences

- Determinism in the engine makes cached-versus-fresh comparisons exact, which the M8 bucket memo relies on.
- Each Verus upgrade costs a rebase. Small, isolated commits keep that cost low.
- Building the engine needs access to the private repository, through `gh auth token` or `SAIL_ENGINE_ARTIFACT_TOKEN`. CI uses only the prebuilt artifact and never builds Verus.

## Note (2026-10-14): runtimes are published on the public fork

`Chelis-Lang/verus` is now public. Prebuilt engine runtimes are GitHub release assets on it, one prerelease per pinned fork commit (`engine-runtime-<short sha>-linux`), instead of private assets on the Sail repository. `engine/verus/artifact-linux.json` names the release (`repo`, `release_tag`, `asset`) and keeps every digest check. Fetching the runtime needs no token, so the CI image build has no BuildKit secret and `ci/local.sh` and `scripts/perf-host.sh run` pass none. Publishing a release needs write access to the fork. See `engine/verus/README.md`.

Building the engine from source no longer needs access either:
`cargo xtask build-verus-engine` clones the pinned fork commit anonymously.
The owner also decided on 2026-10-14 that the repair engine ships with Sail:
release binaries are to include prebuilt engine runtimes.

## Note (2026-10-16): renaming solver symbols

The fix rules above cover orders and structured output. A fix that renames
solver symbols one-to-one is neither: for a given source it prints names
stock never prints. [ADR 0049](0049-stable-closure-names.md) (proposed)
admits one such fix, stable closure-type names (VI-0019), on fork-versus-stock
conformance and the fork's full `rust_verify_test` suite instead of a
construction argument.

## Note (2026-10-20, proposed): output-preserving performance changes

Two fork changes on branch `perf/7d82994` change no output at all: the order
in which parallel workers take buckets (issue #244, `1a892f97`) and the
hasher of VIR's hot maps (issue #248, `51afcba6`). Neither fix rule above
names this kind. The proposal: a fork change may also alter scheduling or
internal data structures when its solver input (multiset of session
digests), verdicts and diagnostics equal the preceding commit's on the fast
tier, and its determinism check passes. This awaits the owner.
