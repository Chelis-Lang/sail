# 0080: Optional native temporal runtime

- **Status:** superseded by [0083](0083-temporal-extension-workspace.md)
- **Date:** 2026-10-26

## Context

`sail-runtime` linked `sail-dive`, and through it the private D.I.V.E.
crates, unconditionally, so every `sail` binary carried the native temporal
client even though `sail verify` never uses it. A malformed or unreadable
`sail-runtime-worker.json` also stopped the runtime endpoint, and so
`sail serve`, which `sail verify` starts on demand.

## Decision

- `sail-runtime` has a `temporal` feature, on by default, that alone enables
  its `sail-dive` dependency. `sail-daemon` and `sail-cli` forward it as a
  default feature; `sail-runtime-worker` requires it. The workspace
  dependencies on `sail-runtime` and `sail-daemon` disable default features so
  that a member's choice is respected.
- Without the feature the temporal capability reports `installed: false`
  with an `unavailable` reason, exactly as for an uninstalled worker, and
  temporal jobs return `Unavailable`. SMT execution and `sail verify` are
  unchanged.
- Worker discovery for the service never fails: a missing, unreadable,
  oversized, malformed or incompatible manifest leaves the worker
  unconfigured and its reason becomes the capability's unavailable error.
- The workspace gate lints `sail-cli`, `sail-daemon` and `sail-runtime` with
  `--no-default-features`.

The layering table is unchanged: the `sail-runtime` to `sail-dive` edge is
still permitted, now optional.

## Consequences

`cargo build -p sail-cli --no-default-features` compiles and links no
D.I.V.E. code. Cargo still resolves the whole workspace lockfile, including
optional git dependencies, so that build still needs to fetch (not compile)
the pinned D.I.V.E. repository. Building without access to it needs
`sail-dive` and everything that enables it outside this workspace's
resolution; revisit when D.I.V.E. is published or a credential-free build
becomes a requirement.
