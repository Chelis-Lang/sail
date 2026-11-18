# 0172: Native temporal checking in the main workspace on D.I.V.E. 0.1.0

- **Status:** accepted (owner decision, 2026-11-18; merges once D.I.V.E.
  0.1.0 is on crates.io and native conformance passes)
- **Date:** 2026-11-18
- **Supersedes:** [0083](0083-temporal-extension-workspace.md) (the separate
  extension workspace) and [0101](0101-dive-behind-a-default-off-flag.md)
  (the default-off build flag and its lock guard)

## Context

ADR 0083 kept the D.I.V.E. backend in `extensions/temporal/`, a separate
Cargo workspace with its own lock, only because D.I.V.E. (`Chelis-Lang/dive`)
was a private Git dependency: Cargo resolves optional Git dependencies into
the lock, crates.io refuses a package with a Git dependency, and a public
clone could not build. ADR 0101 then hid it behind a build selection and
guarded the root lock against it.

D.I.V.E. is released publicly as 0.1.0 and published to crates.io in
lockstep (`=0.1.0` between its crates). The owner decided to fold the
extension into the main workspace and ship native temporal checking in the
next release, 0.3.0. None of the original constraints remains: a registry
dependency needs no credential, appears in the lock like any other crate and
is accepted by crates.io.

## Decision

- **One workspace.** `sail-dive` moves to `crates/sail-dive` and becomes a
  published package. It depends on `dive-api` and `dive-ir`, declared once in
  the root `[workspace.dependencies]` as `{ version = "=0.1.0" }`. The
  extension workspace, its lock, `scripts/with-private-dependencies.sh`,
  `docs/private-dependencies.md`, the `extension` and `extension-install`
  steps, `SAIL_RELEASE_TEMPORAL` and the `SAIL_DEPENDENCY_READ_TOKEN`
  forwarding in `ci/run.sh` and `ci/local.sh` are removed.
- **Simplest graph.** `sail-temporal` and `sail-runtime-worker` disappear as
  packages. The `sail` binary of `sail-solve` passes `sail_dive::backend::Dive`
  to `sail_cli::run_with` itself, and `sail-solve` gains a third binary,
  `sail-runtime-worker`, so `cargo install sail-solve` and every release
  archive put the worker beside `sail`. The worker's process and shared
  endpoint tests and its protocol fault fixture (compiled by
  `crates/sail-cli/build.rs`, never installed) move to `sail-solve`.
  The `TemporalBackend` trait in `sail-runtime` and `sail_cli::Extensions`
  stay: worker discovery, authentication and admission are unchanged, and an
  embedder can still run the command line without a backend. The layering
  table gains `sail-dive -> {sail-api, sail-runtime}` and
  `sail-solve -> sail-dive`; ADR 0101's lock guard and publish guard are
  removed with the extension table.
- **Version identity.** `sail_api::temporal::DIVE_REVISION` (a Git SHA)
  becomes `DIVE_VERSION = "0.1.0"`. The wire field `backend_revision` keeps
  its name and now carries the release version; the worker manifest, the
  runtime capability and every response check it exactly as before. The
  `sail-dive` `pins` test requires `DIVE_VERSION` to equal the exact
  requirement on both crates and their locked version, and requires the lock
  source to be crates.io unless the root manifest carries a
  `[patch.crates-io]` entry for the crate, which only pre-release
  development against D.I.V.E.'s main branch uses and publication cannot
  carry. Moving the version requires native conformance, as moving the
  revision did.
- **API adaptation to D.I.V.E. 0.1.0.** Temporal success is
  `Outcome::NativeHolds`; conclusive native reports are schema version 3
  (`SAIL4213` names it); the bundled-module origin (`Origin::Bundled`,
  CommunityModules and TLAPS) is refused like any unqualified module
  (`SAIL4202`). D.I.V.E. 0.1.0 checks on its Core engine, which has no
  bounded final (`FinalShort`) route, so every check uses
  `TemporalMode::Ordinary`, which routes safety-only PROPERTYs to safety
  checking and genuine temporal properties to completed-level checking. The
  request is otherwise unchanged: exact store, seed and fingerprint zero, one
  worker, and the caller's state and depth limits.
- **Release.** Archives always contain `bin/sail-runtime-worker` and its
  installation manifest; `scripts/release.sh stage` refuses a tree whose
  `sail` does not report the worker installed. `THIRD-PARTY-NOTICES` lists
  the D.I.V.E. crates and their dependencies.

## Consequences

- A public clone builds, tests and lints native temporal checking, and CI
  covers it in the ordinary `Workspace gate`; the extension-only local tier
  is gone.
- Every published Sail crate that reaches `sail-dive` (`sail-solve`) depends
  on D.I.V.E. from crates.io; Sail cannot publish until D.I.V.E. 0.1.0 is
  there.
- Sail keeps its exact store and state/depth limits for genuine temporal
  properties rather than a fingerprint store or an unbounded search.
  D.I.V.E.'s Core engine checks temporal properties with an exact store and
  reports a state or depth limit reached during graph capture as a stop
  with no claim, which Sail returns as `Stopped`, never Holds.
- `cargo install sail-solve` installs the worker binary but not its
  installation manifest; such an installation reports the worker not
  installed until the manifest is staged. Release archives are complete.
- Sail inherits D.I.V.E.'s dependency versions; some (`sha2` 0.11, `digest`
  0.11, `hashbrown` 0.17) duplicate Sail's own.
