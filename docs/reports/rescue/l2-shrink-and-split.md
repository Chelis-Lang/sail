# Rescue lane L2: shrink and split (2026-10-12)

Lane L2 removed bookkeeping from the M7 branch, re-ran the gate on a native
host and analysed how to land the non-engine product separately. The branch is
cut from `rescue/m7` (`73a3b46`). Pre-rescue history is in the
`archive/m7-contracts-f7103bf` tag.

## Size

| | Files changed vs `main` (`0fde777`) | Lines added | Lines removed |
|---|---:|---:|---:|
| `rescue/m7` before L2 | 1,245 | 458,467 | 656 |
| After L2 | 254 | 67,600 | 658 |

What remains is mostly product: crates and xtask are +37.9k lines. The
selected engine (`engine/verus/`: patch, lockfile, tests and qualification
pipeline) is 16.9k lines.

| Area | Files deleted | Files added | Lines removed | Lines added |
|---|---:|---:|---:|---:|
| `engine/verus/candidates/` | 339 | 0 | 211,322 | 0 |
| `engine/verus/qualifications/` | 10 | 0 | 4,885 | 0 |
| `engine/verus/` (other; ten scripts moved to `qualify/`) | 1 | 10 | 378 | 1,083 |
| `docs/validation/` | 483 | 1 | 124,758 | 225 |
| `bench/validation/` (M7 additions only) | 137 | 0 | 39,961 | 0 |
| `scripts/` | 33 | 2 | 9,735 | 110 |
| `docs/ROADMAP.md` | 0 | 0 | 327 | 17 |
| `docs/known-issues.md` | 0 | 0 | 832 | 73 |
| Other docs and code | 0 | 0 | 407 | 228 |
| **Total** | 1,003 | 13 | 392,605 | 1,736 |

## Inventory before deletion

`git grep` found no reference to `docs/validation/`, `bench/validation/`,
`engine/verus/candidates/` or `engine/verus/qualifications/` from crates,
xtask, CI workflows or `include_str!`/`include_bytes!`. The only exceptions
were the following, each handled:

- **`ci/run.sh` gate.** It ran thirteen Python tests of validation tooling
  (`scripts/test-*`, plus three under `docs/validation/`). They were removed
  together with the tools they tested. No product test was removed or changed.
- **`scripts/test-m7-retained-jobs-acceptance.py`.** It read one retained MCP
  projection from `docs/validation/m7-rpc-retained-functional.json`. That
  message was relocated to `scripts/fixtures/m7-rpc-retained-wrong-stream.json`
  and the test is unchanged otherwise. It passes 12/12.
- **`engine/verus/candidates/qualify-same-input.py` and its nine helpers.**
  These are the thirteen-stage pipeline that qualified the selected runtime.
  They moved to `engine/verus/qualify/`. Only their candidate paths and the
  default patch (now `interactive.patch`) changed, so they differ from the
  frozen hashes in `qualification-linux.json`. The engine README says so and
  documents the command.
- **`crates/sail-verus/tests/fixtures/README.md`.** Its prose pointer to two
  records now names the archive tag.

## What was kept, and why

- **`engine/verus/`.** Kept `interactive.patch`, `Cargo.lock` and
  `artifact-linux.json`, which CI and the Docker image use. Kept
  `qualification-linux.json` (the selected patch), `test-engine.py`,
  `test-prepared.py`, `test-cancel.py` and `test-real-he.py` (pipeline
  stages), `qualify/` and the README, whose candidate narration is condensed.
  Deleted `qualification-host.json` and `qualifications/`, which belong to
  older patches.
- **`bench/`.** `bench/results/*.json` is untouched, so the gate's
  `docs/benchmarks.md` equality check still holds. `bench/acceptance/`,
  `bench/portfolio/` and the 29 `bench/validation/` files from before M7 are
  unchanged (`bench/validation/README.md` is restored to its `main` text).
  The 137 M7 records are deleted.
- **`scripts/`.** Kept `check-ai-markers.sh` (CI) and `perf-host.sh`, plus the
  acceptance harnesses that guides document and the gate tests:
  `m7-repair-bench.py`, `m7-job-read-bench.py`, `m7-reader-acceptance.py`,
  `m7-retained-jobs-acceptance.py`, `m7-native-product.py` and
  `m7-result-frame-profile.py`, each with its test. Also kept
  `test-managed-timeout-evidence.py`, which tests `ci/managed-evidence.py`.
  Added `check-links.py`.
- **Deleted from `scripts/`** (no caller in ci, crates or xtask, and no kept
  user guide), each with its `test-` file where one existed:
  - `inventory-old-benchmark-single-deps.py`;
  - `local-combined-gates.py`, `local-product-install.py`,
    `local-product-rebind.py` and `local-standard-conformance.py`;
  - `m7-baseline-product-smoke.py`, `m7-conformance-observation.py`,
    `m7-current-publication-controller.py`, `m7-current-reader-controller.py`,
    `m7-faults-live-observer.py`, `m7-filtered-progress-control.py`,
    `m7-ordinary-wait-diagnostic.py`, `m7-publication-timing-diagnostic.py`,
    `m7-stock-smt-process-diagnostic.py`, `m7-tspec-transcript-freeze.py` and
    `m7-tspec-transcript-run.py`;
  - `retire-completed-run.py` and `retire-old-functional-two-incremental.py`;
  - `test-local-*`, `test-retire-*` and the matching `test-m7-*` files.

  `docs/workspace-hygiene.md` documented `retire-completed-run.py`. The guide
  is kept, because AGENTS.md links it, but its retirement-tool sections are
  removed under the ADR 0038 evidence policy.
- **Docs.**
  - [`docs/validation/m7.md`](../../validation/m7.md) replaces the 483 records
    with one table per gate and an indexed one-line list of failed and
    interrupted attempts. It states once that local measurements ran under x86
    emulation and are non-reference.
  - The roadmap's two M7 log sections became a seven-row gate table and one
    paragraph. "Native-binding reader follow-up" is folded into M7 validation.
  - Known issues now lists open defects only.
  - The M7 sections of the README, `docs/README.md` and the acceptance guide
    are condensed.
  - In AGENTS.md, CHANGELOG.md and the ADRs, only link targets changed.

## Link check

`scripts/check-links.py` checks every inline repo-relative Markdown link in
tracked files, including multi-line link text, and verifies heading anchors on
`.md` targets. It skips code blocks and code spans. It falls back to a
filesystem walk when git is unavailable, as it is in a synced checkout inside
the CI container. A negative control caught missing files, a missing anchor
and a multi-line link. The check is added to the `gate` step.

- Before the fixes: 180 broken links across 34 files, including two survey
  anchors already broken on `main`.
- 178 pointed at removed records and now point to `docs/validation/m7.md`. The
  two survey anchors now point to the existing sections.
- After the fixes: 0 broken links.

## Native gate

The host was `scripts/perf-host.sh up l2`: c7i.4xlarge, us-east-1, Ubuntu
24.04 x86-64. It ran `ci/local.sh gate` in the CI image (`sail-ci:8e24fb5f7f490c01`).

| Tree | Result |
|---|---|
| `rescue/m7` `541d3e7`, unmodified | **Fail.** 676/677; `output_service::overlap_tests::restored_output_write_invalidates_both_overlapping_input_observations` |
| L2 before the fixture fix | **Fail.** 673/677; four `sail-daemon` output-service tests. All Python gate tests and the link check pass |
| Output-service tests ×10, before the fix | Failures in 8 of 10 runs (1–4 tests each) |
| Output-service tests ×10, after the fix | 10/10 runs pass |
| L2 at `68a22c7` | **Pass.** 677/677 workspace tests, all ten Python gate tests, link check, fmt, strict Clippy, doctests, rustdoc, layering, cargo-deny and generated benchmarks-page equality. The final commit differs from `68a22c7` only in this table row |

**Cause.** `Queue::offer` takes the task slot with `try_lock` and silently
drops the offer when the lock is busy. The fixtures spawned the worker
immediately before offering. On a native host the new thread often held the
slot mutex at that moment, so no output owner was built and the fixture's own
precondition failed. Under emulation, slower thread start hid the race.

**Fix.** Commit `test: queue the output-owner fixture task before spawning
its worker` offers first, then spawns. No assertion changed. It is a
test-only change in `crates/sail-daemon`, outside the shrink scope, kept as a
separate commit so the lead can take or drop it. The product behaviour is
fail-safe (original hashing) and is listed in known issues.

## Split proposal (analysis only)

The non-engine product can land on `main` before the engine, with about four
small decoupling edits.

**Dependencies found:**

- `sail-api` does not depend on `sail-verus`. Baselines, comparison and
  explanation (`explain()`, `compare()`, `Baseline`, `Comparison`,
  `Explanation`) do not use the repair batch types.
- `sail explain` does not depend on the engine.
- `xtask/src/layering.rs` is unchanged. The only new crates are `socket2` and
  `rustix`, both infrastructure.
- The proof-event journal has no stock producer. The only writer is
  `engine_events::Relay`, started from `adapter.rs` when an engine runs. So
  `jobs events` and `verification_events` return empty pages until the engine
  lands.
- The managed runner signatures take `Option<&engine::Request>`, and
  `engine.rs` pulls in the patch and lockfile with `include_str!`.

| PR | Contents | Approx. Rust added (product / tests) | ADRs and docs | Notes |
|---|---|---:|---|---|
| A. Evidence product | `sail_api::proof`; the baseline/compare/explain half of `sail_api::repair`, moved to a new `sail_api::evidence`; proof schemas; daemon `jobs/product.rs`, `proof_journal.rs`, dispatch; CLI `product_cli/evidence.rs`, `events`; MCP `proof_tools.rs` minus `start_repair_validation`; `proof_contract`, `proof_product`, `baseline_tools_tests` | 1.9k / 1.35k | ADR 0027 product sections (or a short new ADR), ADR 0016 amendment; `mcp.md` baseline/explain/events, `output-schema.md` | Carries the socket-name protocol bump. MCP tool-list goldens gain six tools. Either document the empty event stream or defer the journal to C |
| B1. HTTP shared cache | daemon `shared*`, `presence`, `initialization`, `remote`, `server`; `tests/http_shared.rs` | 2.2k / 0.1k | ADR 0025 and 0017 amendments, ADR 0037 (rejected); `http-shared-cache.md` | Independent of A; can run in parallel |
| B2. Managed freshness, sysroot, artifact families | `sail-verus/managed/*`, `attribution.rs`, `run.rs`, non-engine adapter changes; `sail-api` and daemon `sysroot.rs`; `socket2`/`rustix`; shim test support | 14k / 3k | ADRs 0029, 0031, 0032, 0035, 0036; `freshness.md`, `guarded-sysroots.md`, `managed-cold-start.md`, `native-product-setup.md`, `trust.md`, `fault-injection.md` | Needs an engine placeholder: a dataless `engine::Request` stub with `run_engine` returning `None`, without the `include_str!`. Run the standard conformance tier, since it touches keys and the daemon |
| B3. Job read path | api `cancellable`, `decoded`, `ownership`; daemon `result_cache`, `output_service`, `rpc_diagnostics`, `store`/`worker`/`jobs`; MCP job clients/tools/server; CLI `watch`, `job_read_bench`; `ci/test-mcp-live-service*.py`, `managed-evidence.py` | 3k / 2.9k | ADR 0034; `result-client.md`, `result-frame-profile.md`, `result-rpc-diagnostics.md`, `reader-acceptance.md` | After B2. Should include the fixture fix above and the reader-regression bisect |
| C. Engine and repair | `engine.rs`, `engine_events.rs`, engine parts of the adapter and runner, `Mode::Interactive`/`Mode::Repair`, repair batch types and schema, `engine_cli`, `repair-check`, `start_repair_validation`; `engine/verus/`; `ci/engine-artifact.py`, `test-engine-artifact.py`, `test-repair*.py`, the Dockerfile engine layer and secret, the e2e repair phase; xtask `build-verus-engine` | 3k / 1.2k (plus the 11k-line patch) | ADRs 0027, 0028, 0030, 0033, 0038 engine conditions; `proof-repair.md`, `proof-repair-walkthrough.md`, `mcp.md` repair, `divergences.md`; `docs/validation/m7.md` | Carries the third e2e phase, the CI completion guard's three-phase expectation and the private-token image build |

**Order:** A, then B1 in parallel, then B2, B3 and C.

**Decoupling edits:**

1. Move the evidence types out of `sail_api::repair` into `sail_api::evidence`.
2. In A, leave out the `Submission.mode` repair variants and
   `Capabilities.repair`, keeping `#[serde(default)]` so the wire stays
   compatible.
3. Add the engine stub described in B2.
4. Split `crates/sail-cli/tests/proof_workflow.rs` into a stock-submission half
   (A) and a `repair-check` half (C).

**Risks:**

- Rebase conflicts in `jobs.rs`, `store.rs`, `worker.rs`, MCP `tools.rs` and
  `server.rs`, which A, B3 and C all touch.
- Each PR must reconcile its own `ci/run.sh` gate list.
- Only C may contain the Docker secret mount, or every image build will need
  the private token.

## Documentation changed

- Rewritten: `docs/ROADMAP.md` (M7), `docs/known-issues.md`,
  `docs/validation/m7.md` (new), the README's M7 section, `docs/README.md`,
  `docs/bench/m7-repair-acceptance.md` (status section),
  `docs/workspace-hygiene.md` (retirement tooling) and `engine/verus/README.md`.
- Link targets only: guides, ADRs 0017, 0025, 0027 and 0030–0037,
  VI-0001/0012/0013/0014, CHANGELOG.md and the two survey anchors.
- AGENTS.md is not edited.
