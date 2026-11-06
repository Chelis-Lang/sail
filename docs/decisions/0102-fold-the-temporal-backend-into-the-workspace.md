# 0102: Fold the temporal backend into the workspace

- **Status:** proposed; queued until its preconditions hold
- **Date:** 2026-11-06
- **Supersedes:** [0083](0083-temporal-extension-workspace.md) and the flag
  in [0101](0101-dive-behind-a-default-off-flag.md), once accepted

## Context

ADR 0101 keeps D.I.V.E. behind a default-off build flag: the separate
`extensions/temporal` workspace, never built by CI or shipped in public
archives. It queues this flip for when D.I.V.E. is ready. The flip is
acceptable only when all of these hold:

1. D.I.V.E. publishes a v0.1 release with a public source, so that a clone
   with no credentials can fetch the pinned revision.
2. D.I.V.E.'s engine cutover has landed. The adapter selects D.I.V.E.'s
   new engine with no fallback, and every temporal test must pass on it.
3. A `sail-api` release carrying the new `temporal::DIVE_REVISION` and
   native schema is published and adopted by its consumers, which check a
   worker's revision against that constant.

Folding also changes the root `Cargo.lock`, which
`scripts/check-acceptance-refinements.py` binds by hash as a reviewed parser
dependency edge. The new lock needs that explicit source review before the
binding moves; the checker refuses it until then.

## Decision

- Move `sail-dive`, `sail-runtime-worker` and `sail-temporal` into
  `crates/` as unpublished members of the root workspace, and remove
  `extensions/temporal`. The root manifest pins `dive-api` and `dive-ir` by
  revision; `deny.toml` allows only that git source.
- `sail-temporal`'s binary is `sail-temporal`, because `sail` belongs to
  `sail-solve` in the same workspace. Release archives install it as
  `bin/sail`, beside `bin/sail-runtime-worker` and its manifest.
  `THIRD-PARTY-NOTICES` is generated from those roots too.
- The release refusal and `SAIL_RELEASE_TEMPORAL` are removed: every archive
  is temporal-enabled.
- `cargo xtask check-layering` replaces ADR 0101's lock guard with a
  published-package guard. A publishable package may not depend on a git
  source, a `dive-*` crate or a temporal package, and the temporal packages
  must set `publish = false`. crates.io `sail-solve` therefore still reports
  temporal unavailable until D.I.V.E. is on a registry.
- The adapter selects D.I.V.E.'s new engine for checking and simulation,
  refuses module sources that D.I.V.E. bundles as unsupported, and `sail_api::temporal::NATIVE_SCHEMA_VERSION` states the
  native report schema that moves with `DIVE_REVISION`.
- CI covers the temporal packages through `Workspace gate`. No CI job needs a
  credential.

## Consequences

- Temporal regressions fail the required gate instead of a local tier.
- Archives grow by the worker and the D.I.V.E. closure, and their notices
  list them.
- A `DIVE_REVISION` change is a `sail-api` release event. Consumers that bind
  evidence to the old revision keep reading it under that revision and
  require the new one for new evidence.
