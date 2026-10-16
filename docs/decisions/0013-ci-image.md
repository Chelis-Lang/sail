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

## Update 2026-10-08: CI runs only fast checks

**Owner direction:** long-running tests run locally only, never in CI. A
standard-tier conformance dispatch (with page-table at about 46 minutes on
CI) was cancelled, and CI was reshaped:
- **Five per-PR jobs only.** CI runs the five required checks and nothing
  else. Conformance (the PR subset) is a job in `ci.yml`. The scheduled and
  dispatched workflows (`conformance.yml`, `bench.yml`) are removed.
- **Time limits.** Every job has at most a 10-minute limit; measured testing
  times are 1.2–3.8 minutes.
- **Guard.** `ci/run.sh` refuses the long steps (other conformance tiers,
  benchmarks) when `CI=true`. They run through `ci/local.sh`, in the same
  image.
- **Faster image rebuilds.** The image installs prebuilt, digest-checked
  nextest and cargo-deny instead of compiling them, so a pin change rebuilds
  it in minutes.

## Update 2026-10-09: skipped gates cannot authorize a merge

An image failure skipped the dependent required jobs after PRs #41 and #43.
`CI completion` now runs with `always()` on a normal runner and requires
authorship, image creation and all four testing jobs to report `success`.
Failure, cancellation, skipping and missing results fail the guard. It is
the sixth required check. Branch protection also requires branches to be
updated with `main`, so independently passing changes are tested together.
The guard's negative cases run in the local and CI workspace gate.

## Update 2026-10-10: complete end-to-end phases run in parallel

Managed workspace acceptance adds cold proof establishment, unchanged complete
report reuse, durable CLI/MCP jobs, missing-receipt recovery and restored-mtime
failure checks. CI runs the existing legacy suite and the new managed suite in
parallel within their original ten-minute job limits. The required `End to end`
aggregate runs even after a failure and requires both phases to succeed.
Failure, cancellation, skipping or a missing phase fails the aggregate; its
negative cases run in the workspace gate. The six protected check names and
completion guard stay unchanged.

Local `ci/run.sh e2e` continues to execute both complete phases. Explicit
`legacy` and `managed` selection supports diagnosis. This split changes
scheduling, not proof deadlines, acceptance assertions or coverage. At revision
`a518a104`, native Linux legacy and managed jobs completed in 316 and 348 seconds;
all required checks passed. Long corpus runs and performance measurements remain
local-only.

## Candidate 2026-10-16: share the core image without the optional MCP client

Issue [#195](https://github.com/Chelis-Lang/sail/issues/195) records an actual
five-minute image pull inside a ten-minute job, and a later same-digest control
with a much shorter startup. The delay is variable. The published image's
pinned external MCP client occupies a separate 119,609,629-byte compressed
layer, although only the managed e2e phase invokes it.

The candidate keeps one Dockerfile and every existing installation and digest
check. Its `core` target retains both Rust toolchains, stock Verus/Z3, the
validated fork engine and all corpus/build tools. Its final, default `mcp`
target adds exactly the existing pinned client. No validated runtime file is
removed, deduplicated, rewritten or shared through new hard links.

The image workflow publishes both content-tagged targets; managed e2e uses
`mcp`, while all other required jobs use `core`. The existing default image
tag remains the complete target; `ci/image-tag.sh core` adds `-core` to that
tag. Local `ci/local.sh e2e managed` and full `e2e` select `mcp`, with every
other phase selecting `core`. The real client smoke check stays mandatory,
and missing its executable still fails. Protected checks, seven e2e phases,
test commands, runtime validation and all timeouts stay unchanged.

This is a packaging prototype, not an accepted speed claim. The
[startup gate](../validation/ci-image-startup.md) requires native image/gate
parity and actual cold GitHub job startup/check timings, with failed attempts
retained. Compressed-byte reduction alone cannot establish a startup gain.
