# Rescue lane L10: CI diet and macOS parity (2026-10-12)

Owner direction: CI serves rapid development, so heavyweight, acceptance-,
evidence- and release-style checks move to local tiers. Code must also pass on
both macOS and Linux. macOS runners are not permitted in CI, so the macOS check
runs locally.

## Measurement

- **Host.** EC2 c7i.4xlarge in us-east-1, with the CI image from
  `ci/local.sh`.
- **CPU limit.** Each step ran pinned to four CPUs (`--cpuset-cpus=0-3`, which
  matches GitHub's `ubuntu-latest` runner).
- **Cache.** Each step started with the dependencies built and every workspace
  crate cleaned, as `Swatinem/rust-cache` leaves the target directory.
- **Timing.** Wall times come from `bash -x` timestamps.
- **Engine pin.** On the measured source (`994d452` and this branch), the
  committed engine pin is stale (`SAIL0944`) until L7a re-pins it. For timing
  only, the host checkout used the patch and feature list of the published
  runtime (`aaa3b2e4`). That overlay was never committed.
- **GitHub comparison.** GitHub runners are about 2.2 times slower than this
  proxy. For example, `main` run 38262935049 took 5 min 22 s for the e2e
  legacy step, against 146 s here. Each GitHub job also spends 40–60 s
  initializing its container.

## Per step

| CI step | Before (s) | After (s) | What changed |
|---|---:|---:|---|
| Workspace gate | 115 | 109 | M7 tooling tests, harness-helper tests and the profile/bench example tests moved to `release` |
| Solver tests | 40 | 36 | unchanged (noise) |
| e2e legacy | 146 | 137 | unchanged (noise) |
| e2e managed | 150 | 142 | MCP/live-service transport controls moved to `release` |
| e2e repair | 106 | 76 | engine provenance and persistent-preparation controls moved to `release` |
| Conformance (`fast pr`) | 94 (oracle hard failure, `traits.rs`) | 93 (pass) | engine arm runs `--no-oracle` |
| `release` (local only, new) | n/a | 87 | where the moved checks run |

In the before run, the conformance step failed on the old runtime's oracle
(VI-0015 without confirming arms). That failure made no difference to its
timing.

Evidence uploads (managed, repair and conformance reports) now run only when a
job fails.

The time saved is modest: about 30 s on the repair job and about 6–8 s on the
others. The old agent's additions were mostly cheap. The remaining time is
spent as follows:

| Cost | Seconds (4 CPUs) | Jobs that pay it |
|---|---:|---|
| Release build of `sail`/`sail-shim` | 37–39 | legacy, managed, repair, conformance |
| Building and running the workspace tests | 80 | gate |
| Fixture `cargo verus verify` warmup, which verifies vstd | 43 | legacy, and inside `test-managed.py` |
| First `sail mcp` verify of a project | 49 | legacy (`check_mcp`), and `managed-01` in `test-managed.py` (50 s) |
| `test-repair.py` | 36 | repair |
| Conformance: three rounds plus the engine arm | 13 + 13 + 13 + 15 | conformance |

**Product finding.** The first `sail mcp` verify of a project takes about
49 s even though every check is served from the cache. A CLI `sail verify` of
the same project takes 0.5 s. The cause is that a managed job builds in its
own Cargo target (`managed/runner.rs`, `--target-dir`), so its first run
re-verifies vstd. This is agent-loop latency, not CI weight, and belongs to
the managed-overhead lane (L7c). Fixing it would remove about 50 s from both
the legacy and managed jobs.

**Projected CI time.** The longest jobs (e2e legacy and e2e managed) take
about 140 s here, which projects to about 5–5.5 min on GitHub including
container start. The whole PR wall projects to about 6 min, inside the
5–6 min target. The per-job target of about 4 min is not yet met for those two
jobs, and the managed first-run cost above is what stands in the way. The gate
(109 s), solver (36 s), repair (76 s) and conformance (93 s) jobs project to
4 min or less.

## Where each moved check runs, and why moving it is safe

Every moved check stays runnable through `ci/local.sh release`, and AGENTS.md
lists it under Gates. `ci/run.sh` refuses `release` and `host` when
`CI=true`.

- **M7 tooling tests** (`scripts/test-m7-*.py`,
  `scripts/test-managed-timeout-evidence.py`). These test acceptance and
  benchmark harnesses that the guides document (reader acceptance,
  native-product setup, result-frame profile). They are not product code, and
  they change only when those harnesses change. They are kept, not deleted,
  because the reader-acceptance and repair gates still use the tooling.
- **Harness-helper tests** (`ci/test-repair-lifecycle.py`,
  `ci/test-mcp-live-service-controller.py`). They test the helpers of test
  harnesses, not product code.
- **`cargo test --example`** for `result_frame_profile`,
  `result_client_profile` and `result_stream_profile`. These are
  benchmark/profile tools. Clippy still compiles every example
  (`--all-targets`). The `job_read_bench` line was a duplicate: that example
  sets `test = true`, so nextest already runs its tests in the gate. The
  `check_mcp` example tests stay in the gate, because e2e legacy drives that
  example.
- **MCP/live-service transport controls** (`ci/test-mcp-live-service.py`). By
  their own statement they establish no proof. Product MCP behaviour stays
  covered in CI by `check_mcp` (e2e legacy), the MCP calls in `test-managed.py`
  and `test-repair.py`, and the `sail-mcp`/`sail-cli` nextest suites.
- **Repair engine provenance and persistent-preparation controls**
  (`ci/test-repair-engine-provenance.py`). Persistent preparation is off by
  default (ADR 0038), and L7a is removing it. The default engine provenance
  contract stays covered in CI:
  - by `sail-api` `repair_contract` (engine provenance and stock fallback);
  - by `test-repair.py`, the real repair CLI/MCP run against the engine.
- **Same-input byte oracle in the PR subset.** It guards prepared-lowering
  reuse, which users do not get by default, and L7a is deleting it. The CI
  engine arm keeps the user-level guard: the engine's full phase against plain
  Verus, with the same verdict and diagnostic rules. The other `conformance`
  tiers still run the oracle with `--oracle-fresh-arms 4` until L7a removes
  it.

**Kept in CI, on evidence:**

- **`ci/test-engine-artifact.py`** (0.08 s). It is the only test proving that
  the image's engine installer refuses corrupt, escaping or stale runtimes.
- **`check-links.py`** (0.3 s) and **`test-completion.sh`** (0.05 s).
- **All three default-mode conformance rounds** (pass-through, cache fill and
  cache warm), at about 13 s each. Each round is the only per-PR guard of its
  path, which the charter requires.
- **The engine arm.** It is the only per-PR engine-versus-stock guard.

## macOS

- **Clippy.** `cargo clippy --workspace --all-targets --locked -- -D warnings`
  passes on this Mac (load average 45–90 during the run).
- **Tests.** `cargo nextest run --workspace --locked --profile ci` passed
  565/565, with 37 skipped. After the rebase onto `1e64cd8`, `ci/run.sh host`
  passed with 570/570 tests (load average 20).
- **Command.** `ci/run.sh host` runs both checks.
- **Flake.**
  `jobs::cancellable::blocked_read_and_write_cancel_and_leave_no_polling_monitor`
  was a test-fixture timing race, not a product race. It serialized an 8 MiB
  request in a debug build before sending any byte, which took 0.9 s of its
  2 s readiness wait. The body is now four times the platform's default Unix
  stream send plus receive buffer, with a floor of 256 KiB. That body still
  cannot be buffered, so the write still blocks.
- **Second race.** Stress loops found a second race:
  `only_initial_connect_failures_admit_startup_fallback` failed 1 of 156
  iterations. The fixture closed the peer straight after accept. On macOS,
  `getpeername` then fails with `EINVAL` (reproduced directly), so the client
  correctly reports a pre-RPC connect failure, because no RPC bytes were sent.
  The fixture now closes after receiving the RPC, which is the case the
  assertion tests.
- **After the fixes.** 40/40 full `sail-api` stress iterations passed. Then
  300/300 stress iterations of all ten cancellable tests passed (3,000 runs)
  at load averages between 6 and 40. Before the fixes, 1 of 156 iterations
  failed under similar load.

## Documentation

- AGENTS.md: a ground rule for macOS and Linux, the CI purpose, the new
  `release` and macOS tiers, and failure-only evidence uploads.
- `docs/mcp.md`: the transport controls run in `release`.
- `docs/reader-acceptance.md`: its developer checks run in `release`.
- `docs/proof-repair.md`: the PR subset runs without the oracle.
- No CHANGELOG entry: the CI changes are not user-visible.
