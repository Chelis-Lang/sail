# 0101: D.I.V.E. behind a default-off build flag

- **Status:** accepted
- **Date:** 2026-11-06
- **Amends:** [0083](0083-temporal-extension-workspace.md) (adds guards and a
  named install step; the workspace split is unchanged) and the temporal hold
  in [0100](0100-mutation-first-roadmap.md)

## Context

On 2026-11-06 the Oyster owner resumed Oyster for shelf completion and asked
that Sail start working with D.I.V.E. now, hidden behind a configuration or
feature flag so that no public Sail build ships it, with a queued pull request
that flips it on once D.I.V.E. is ready. ADR 0100 had held temporal packaging
until D.I.V.E.'s distribution decision.

Three facts constrain the shape of that flag:

- crates.io refuses a package with a git dependency, even an optional one.
  Every published Sail crate must therefore stay free of any D.I.V.E.
  dependency until D.I.V.E. itself is on a registry.
- Cargo resolves the whole workspace lock, optional git dependencies
  included, independently of the selected features (the reason for ADR
  0083). A `dive` Cargo feature on any main-workspace crate would put the
  private repository in the root `Cargo.lock`, and a public clone could no
  longer build, test or lint Sail without credentials.
- ADR 0083 already keeps D.I.V.E. in `extensions/temporal/`, a separate,
  unpublished workspace with its own lock, behind the `TemporalBackend`
  trait. Its tests pass at the pinned revision, but nothing in the main gate
  stops the private sources from reaching the root lock or an extension crate
  from becoming publishable.

## Decision

- **The flag is the build selection, not a Cargo feature.** A default build
  (the root workspace, `cargo install sail-solve`, every public archive)
  links no temporal backend. Its `sail runtime capabilities` reports temporal
  `installed: false` with "native temporal support is not included in this
  sail build". The D.I.V.E.-enabled `sail` exists only when a developer
  explicitly selects `--manifest-path extensions/temporal/Cargo.toml`, or the
  named steps below. No main-workspace crate gains a `dive` feature.
- **Named, explicit entry points.**
  - `ci/run.sh extension` (and `ci/local.sh extension`): fmt, clippy and
    tests of the extension, with D.I.V.E. access. It remains the required
    local gate for changes to `extensions/`, `sail-runtime::temporal` or the
    `TemporalBackend` interface.
  - `ci/run.sh extension-install DIR`: builds the extension's release `sail`
    and worker into `DIR/bin`, stages the worker manifest, and requires the
    installed `sail` to report temporal installed. This is the build Oyster
    and other internal consumers use for native temporal work.
  - `SAIL_RELEASE_TEMPORAL=1 scripts/release.sh stage` keeps staging the
    internal archive tree, and `scripts/release.sh VERSION` keeps refusing to
    draft a public release with it set.
- **Guards in the main gate.** `cargo xtask check-layering`, which already
  runs in `Workspace gate`, now also refuses:
  - a root `Cargo.lock` entry fetched from the D.I.V.E. repository, named
    `dive-*`, or naming an extension crate;
  - an extension crate that does not set `publish = false`.
  `sail-solve`'s CLI tests assert that the default `sail` reports the
  temporal backend absent, even with a worker manifest beside it.
- **No CI lane.** The extension stays a local tier. Sail is public, its CI
  holds no private-dependency credential, and that policy (ADR 0083,
  amended 2026-10-26) is unchanged. A pull request that touches the extension
  reports its `ci/local.sh extension` result.
- **The D.I.V.E. pin stays at `1025cd6a` here.** Published `sail-api` 0.2.0
  states that revision as `temporal::DIVE_REVISION`, and consumers check a
  worker's reported revision against it. A pin bump moves the adapter, the
  extension lock and `DIVE_REVISION` together, and needs a `sail-api` release
  that consumers adopt.
- **The flip.** When D.I.V.E. publishes a public v0.1 source, its engine
  cutover has landed, and a `sail-api` release carrying the new revision is
  consumed downstream, a queued pull request moves the extension crates into
  the main workspace as unpublished packages, builds public archives with
  the extension's `sail` and worker, lists D.I.V.E. in the notices, lifts the
  release refusal, and replaces the lock guard with one that refuses any
  published crate reaching D.I.V.E. Published crates still gain no D.I.V.E.
  dependency until D.I.V.E. is on crates.io.

## Consequences

- Temporal development resumes in the extension without touching public
  builds; a regression that would leak D.I.V.E. into the root lock or make an
  extension crate publishable fails the required gate.
- The extension is still not covered by CI. Reviewers must not read a missing
  extension result as passing.
- The flip is a separate decision point gated on D.I.V.E.'s distribution. Until
  it lands, public archives and crates.io installs answer temporal jobs with
  `Unavailable`, as they do today, and there is no TLC fallback.
