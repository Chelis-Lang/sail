# 0083: The temporal backend is an extension workspace

- **Status:** accepted; amended 2026-10-26 (the extension check is local-only)
- **Date:** 2026-10-26
- **Supersedes:** [0080](0080-optional-temporal-runtime.md)

## Context

Sail is to ship publicly while D.I.V.E. (`Chelis-Lang/dive`, linked as the
`dive-api` and `dive-ir` git dependencies) stays private, probably for weeks
after Sail's release. ADR 0080 made `sail-dive` an optional, default-on
`temporal` feature. That keeps D.I.V.E. code out of a `--no-default-features`
binary, but Cargo resolves the whole workspace lock, optional git
dependencies included, so every build, test and lint of the workspace still
needed credentials to fetch the private repository. Every CI job therefore
received the read token, and a public clone could not build at all.

The parent side also needs D.I.V.E.: `sail_dive::client::run` parses the
request's configuration with `dive_api::parse_config_source` to bind the
worker's requested-property inventory independently of the worker. A
temporal-enabled `sail` must link it, not only its worker.

## Decision

- The main workspace (the root `Cargo.toml` and `Cargo.lock`) names no
  D.I.V.E. crate, `sail-dive` or `sail-runtime-worker`. It builds, tests and
  lints from a clone with no credentials. The temporal feature and ADR 0080's
  default-feature mechanism are removed; the workspace dependencies on
  `sail-runtime` and `sail-daemon` are plain paths again.
- Dependency inversion. `sail-runtime::temporal` defines `TemporalBackend`,
  with one method, `run(worker, request, deadline, cancel) -> Dispatch`, and
  `Config::temporal_backend: Option<Arc<dyn TemporalBackend>>`. Worker
  discovery, the installation manifest, worker authentication, job admission
  and the mapping to runtime outcomes stay in `sail-runtime`, unchanged; only
  the process client and its evidence validation sit behind the trait. The
  backend owns the worker process and must validate the worker's evidence
  against the request before returning `Dispatch::Completed`. With no
  backend the capability reports `installed: false` with the `unavailable`
  reason "native temporal support is not included in this sail build", no
  worker manifest is read, and temporal jobs return `Unavailable`, as an
  uninstalled worker does.
- `sail-cli` is also a library: `sail_cli::run_with(&Extensions)` runs the
  command line, and `Extensions::temporal` reaches the runtime endpoint of
  `sail serve` and `sail runtime`. Its own `sail` binary passes no extension.
- `extensions/temporal/` is a separate Cargo workspace with its own
  `Cargo.lock` (excluded from the root workspace). It holds `sail-dive`
  (moved, now also providing `sail_dive::backend::Dive`), `sail-runtime-worker`
  (moved) and `sail-temporal`, whose `sail` binary calls
  `run_with` with the D.I.V.E. backend. It uses Sail's crates by path, so the
  temporal-enabled `sail` is the same command line. Its lints, package
  metadata and profiles mirror the root manifest, its lock was seeded from the
  root lock, and the root `rust-toolchain.toml` and `clippy.toml` apply to it.
  The parent-side independent inventory check is unchanged, now inside
  `Dive::run`.
- Layering: `xtask/src/layering.rs` drops `sail-dive`, `sail-runtime-worker`
  and the `sail-runtime` to `sail-dive` edge from the main table and adds an
  extension table (`sail-dive` to `sail-api` and `sail-runtime`;
  `sail-runtime-worker` to `sail-dive`; `sail-temporal` to `sail-cli` and
  `sail-dive`). `cargo xtask check-layering` checks both workspaces from
  unresolved metadata, which needs no private source. No main crate may
  depend on an extension crate.
- CI: only the new `Temporal extension` job (`ci/run.sh extension`) receives
  `SAIL_DEPENDENCY_READ_TOKEN`. Without the token, as on fork pull requests,
  it skips its steps, reports a notice and succeeds. `CI completion` requires
  its result. `ci/run.sh` applies the credential helper only to the
  `extension` and `release` steps. (Amended 2026-10-26: temporal jobs do not
  ship in 0.1.0, so the job was removed from CI and `CI completion` no longer
  requires it. `ci/run.sh extension` and `ci/local.sh extension` remain the
  local tier, run before changes to `extensions/`, `sail-runtime::temporal`
  or the `TemporalBackend` contract; no CI step receives the token.)
- Release: public archives are built from the main workspace and contain no
  worker; `THIRD-PARTY-NOTICES`, generated from the main lock, lists no
  D.I.V.E. crate. `SAIL_RELEASE_TEMPORAL=1 scripts/release.sh stage` stages
  an internal temporal-enabled tree from the extension workspace's binaries;
  drafting a public release with it set is refused.

## Consequences

- A public clone of Sail builds and tests without credentials, and the
  required main-workspace jobs no longer see the token.
- Temporal coverage (the D.I.V.E. client, worker process, shared endpoint
  with the backend linked and the temporal-enabled `sail`) runs only in the
  `Temporal extension` job, and so only where the token exists. (Since the
  2026-10-26 amendment it runs only locally, with D.I.V.E. access.) A fork pull
  request cannot change the extension's result; reviewers must not read a
  skipped extension as passing.
- The extension lock can drift from the root lock. Shared dependency
  requirements are copied, and a dependency update to one should be repeated
  in the other.
- Internal temporal archives ship the public `THIRD-PARTY-NOTICES`, which does
  not list the first-party D.I.V.E. crates or their dependencies; they are not
  for redistribution.
- When D.I.V.E. is published, the extension can fold back into the main
  workspace behind the same trait, or keep the trait and stay separate.
