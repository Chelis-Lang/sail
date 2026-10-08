# 0013: One container image for CI and local testing

- **Status:** accepted (owner direction, 2026-10-08)
- **Date:** 2026-10-08

## Context

The checks that matter most for sail's correctness had been running
unevenly:
- **Real-solver suites never ran in CI.** The cached-session and
  fault-injection tests need the pinned Z3, so they were `#[ignore]`d and
  run by hand.
- **Corpus requirements varied by machine.** OpenSSL for anvil; cmake and
  ninja for verismo; the bare-metal target for verismo. The development
  machine had no OpenSSL at all, so anvil could not run there, and setup was
  spread across workflow steps and ad hoc installs.

The owner asked for a container used for both testing and CI, so that CI is
sound and complete for all critical functionality.

## Decision

- **`ci/Dockerfile` defines the test environment** for the reference
  platform (Linux x86-64, ADR 0010):
  - a digest-pinned Ubuntu 24.04;
  - sail's Rust toolchain and Verus's toolchain, with the
    `x86_64-unknown-none` target;
  - pinned cargo-nextest and cargo-deny;
  - the pinned Verus release, with the archive and bundled z3 checked by
    SHA-256 (`ci/fetch-verus.py`);
  - OpenSSL, cmake, ninja and GNU time.

  It sets `SAIL_VERUS_HOME` and `SAIL_TEST_Z3`.
- **Tagging.** The tag is a hash of every input (`ci/image-tag.sh`). The
  reusable workflow `image.yml` builds and pushes the image to GHCR only when
  that tag is missing.
- **`ci/run.sh` is the single definition of every CI step:**
  - `gate`;
  - `solver` (every test that needs the pinned Z3);
  - `e2e` (`sail verify` against `cargo verus verify`);
  - `conformance` (pass-through, then cache-on fill and warm);
  - `bench`.

  Workflows run these steps inside the image. `ci/local.sh <step>` runs the
  same step in the same image locally. On Apple silicon it runs under amd64
  emulation, because Verus publishes no Linux arm64 build.
- **CI coverage:**
  - **Every PR and push:** `No AI authorship markers`, `Workspace gate`,
    `Solver tests`, `End to end`, and `Conformance` (the guide subset,
    pass-through and cache-on).
  - **Nightly:** the fast tier.
  - **Weekly:** the standard tier, now including anvil.
  - **Benchmarks:** dispatched.
- **Required checks.** All five per-PR checks are required on `main`
  (`cargo xtask protect-main`).

## Consequences

- **One environment definition.** A check that passes locally under
  `ci/local.sh` passes in CI, and the other way round. Adding a corpus
  requirement means changing `ci/Dockerfile`.
- **Host-native development still works:** `cargo xtask fetch-verus` and the
  host's own toolchain. It is not the reference.
- **Singular is not in the image.** The pinned Verus release is built
  without Singular support, so the Singular entries stay in tier 2.
- **macOS is not gated in CI.** It is the development platform and the
  secondary benchmark platform (ADR 0010). Platform-specific code paths, such
  as BSD `time` and Unix-socket paths, are exercised by local runs. Revisit
  if macOS-only regressions appear.
