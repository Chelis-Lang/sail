# Rescue lane L2b: landing the first split PRs (2026-10-12)

Lane L2b landed the first independent pieces of M7 on `main` as small PRs. It
follows the split proposal in [the L2 report](l2-shrink-and-split.md#split-proposal-analysis-only).
Each branch was cut from `origin/main` at `0fde777`. Each one was validated on
its own on a native c7i.4xlarge host in us-east-1 (Ubuntu 24.04 x86-64,
`scripts/perf-host.sh up l2b`) before it was pushed. None of the PRs is
merged; the lead reviews and merges them.

## PRs

| PR | Branch | Paths | Native validation | CI |
|---|---|---|---|---|
| [#79](https://github.com/Chelis-Lang/sail/pull/79) process | `rescue/land-process` | see below | gate pass (327/327) at `91a763e`; later commits are docs only | all required checks green |
| [#80](https://github.com/Chelis-Lang/sail/pull/80) B1 HTTP cache | `rescue/land-http-cache` | see below | gate 359/359, solver 36/36, e2e pass, conformance fast pr 33/33 Same ×3 at `93b6527` | all required checks green |
| [#81](https://github.com/Chelis-Lang/sail/pull/81) A evidence product | `rescue/land-evidence` | see below | gate 352/352, solver 36/36, e2e pass, conformance fast pr 33/33 Same ×3 at `a6595d8`. The first e2e attempt at `7e610a8` failed; see below | all required checks green |

### #79, the process PR

It contains:

- `scripts/perf-host.sh`;
- `scripts/check-links.py`, run in the `ci/run.sh gate` step;
- the AGENTS.md sections on native hosts, workspace hygiene, benchmarks and documentation/evidence policy, without the three-phase e2e and engine text;
- ADR 0038, with a status line saying item 3 applies when the engine lands, and its index entry;
- `docs/reports/2026-10-12-m7-rescue-review.md`, with a note that file paths refer to `rescue/m7`;
- a short M7 section in `docs/ROADMAP.md`, giving the landing order and exit gates;
- the two survey anchor fixes.

### #80, B1 HTTP cache

It contains:

- `crates/sail-daemon/src/{shared.rs, shared/initialization.rs, shared/presence.rs, remote.rs, server.rs}` and `crates/sail-daemon/tests/http_shared.rs`;
- the `curve25519-dalek` dev and test profile override in the workspace `Cargo.toml`;
- the ADR 0017 and ADR 0025 amendments, and ADR 0037;
- `docs/http-shared-cache.md` and a CHANGELOG entry.

Links to `docs/validation/m7.md` were replaced by references to the
`archive/m7-contracts-f7103bf` tag. The guide's validation section was
condensed and records the native result.

The owner-only daemon socket permission in `server.rs` was left out. It came
from the sysroot work (`a706e13`), so it belongs to B2.

### #81, A evidence product

API (`sail-api`):

- `src/proof.rs`, and the new `src/evidence.rs` (explanations, baselines and comparisons);
- in `src/jobs.rs`:
  - `Mode` with only `Stock`, and `Submission.mode` with `#[serde(default)]`;
  - the requests and responses `Capabilities { proof_version }`, `ProofEvents`, `Explain`, `SaveBaseline`, `Baselines`, `GetBaseline` and `Compare`;
  - the socket name `jobs-v1-proof-v1-*`;
- `schema/proof-{event,page}-v1.json`, `tests/proof_contract.rs` and `tests/evidence_contract.rs`.

Daemon (`sail-daemon`):

- `src/proof_journal.rs` and its tests;
- `src/jobs/product.rs`;
- dispatch and the mode-aware coalescing check in `jobs.rs`;
- in the worker, `proof-source.jsonl` and `SAIL_PROOF_EVENTS`;
- `tests/proof_product.rs`, without the repair test and with a new stock empty-journal test.

CLI (`sail-cli`):

- `src/product_cli.rs`, with events and evidence only;
- `product_cli/{arguments,evidence}.rs`, with events only in `arguments.rs`;
- `main.rs` dispatch for `explain`, `baseline` and `compare`;
- `jobs.rs`: `jobs events`, stock mode and the environment filter;
- `tests/proof_workflow.rs`, the stock half;
- `examples/check_mcp.rs`, which lists 14 tools;
- `tests/jobs.rs` and `tests/watch.rs` moved to `/tmp` roots, because of the macOS socket-length limit.

MCP (`sail-mcp`):

- `src/proof_tools.rs` without `start_repair_validation`;
- `baseline_tools_tests.rs`;
- registration in `tools.rs`, the tools-list goldens and the crate docs.

`sail-verus`: `SAIL_PROOF_EVENTS` is added to the managed runner's observation-only environment exclusions.

Docs:

- ADR 0027, with a status line saying only the evidence half is on `main`;
- the ADR 0016 amendment;
- `docs/mcp.md` "Evidence tools";
- `docs/workspace-verification.md` "Evidence from completed jobs";
- `docs/output-schema.md`;
- CHANGELOG.

The proof-event journal has no stock producer. A finished stock job returns an
empty terminal page with the explicit gap `SAIL0903: this job produced no
interactive proof stream`. The PR body, the guides, the CHANGELOG and a daemon
test all say so.

**Failed attempt.** At `7e610a8`, the native `ci/local.sh e2e` managed phase
failed. A background job re-verified (`verifier_invocations` 2) instead of
reusing its receipt. The cause was that the new per-job `SAIL_PROOF_EVENTS`
path counted as part of the verification context. The fix excluded it, as
`rescue/m7` already does. No assertion changed.

## Decoupling edits applied to `rescue/m7`

This branch adds commit `refactor: move evidence types to sail_api::evidence
and split the stock proof workflow test`, so that `rescue/m7` matches #81:

1. `Explanation`, `Baseline`, `Change`, `Difference`, `ObligationDifference`, `Comparison`, `Reference`, `explain()` and `compare()` move from `sail_api::repair` to `sail_api::evidence`. Their users in the daemon, CLI, MCP and tests are updated. `repair.rs` keeps only the repair batch types.
2. The two comparison tests move from `sail-api/tests/repair_contract.rs` to `tests/evidence_contract.rs`.
3. `sail-cli/tests/proof_workflow.rs` is now the stock workflow, identical to #81. The repair workflow moves to `tests/repair_workflow.rs`.
4. `sail-daemon/tests/proof_product.rs` is #81's file plus the repair test.
5. `Response::Capabilities.repair` gains `#[serde(default)]`. A service without the engine (main after #81) answers without the field, and the client then refuses non-stock submissions.

On the same native host, `ci/local.sh gate` passed on this branch with those edits: 679/679 workspace tests and every other gate step, including the link check.

## What remains on `rescue/m7` for later PRs

- **B2, managed freshness, sysroots and artifact families.** `sail-verus/managed/*`, `attribution.rs`, `run.rs`, non-engine adapter changes, `sail-api/sysroot.rs`, daemon `sysroot.rs`, `socket2`/`rustix`, shim test support, and the owner-only daemon socket in `server.rs`. Also the cache socket rename to `daemon-v<N>-workspace-v2-proof-v1.sock`; #81 left the cache socket unchanged and renamed only the job socket. It needs ADRs 0029, 0031, 0032, 0035 and 0036, and the engine stub from the L2 proposal. Run the standard conformance tier.
- **B3, job read path.** API `cancellable`, `decoded` and `ownership`, and the `Client` decoding cache; daemon `result_cache`, `output_service`, `rpc_diagnostics` and the `store`/`worker`/`jobs` read-path changes; MCP `job_clients`, `serve_with_job_clients`, `test_io` and output ownership; CLI `watch` and `job_read_bench`; the `ci/test-mcp-live-service*` and `managed-evidence.py` scripts; ADR 0034 and the ADR 0016 read-only job clients amendment. It should include the output-owner fixture fix.
- **C, engine and repair.**
  - API: `Mode::Interactive`/`Mode::Repair`, `Capabilities.repair`, `ResultDocument.repair`, `sail_api::repair` and `repair-result-v1.json`.
  - Engine crates and tooling: `engine.rs`, `engine_events.rs`, `engine_cli`, `repair-check` and the `_repair-worker`, `start_repair_validation`, `tests/repair_workflow.rs`, the repair test in `proof_product.rs`, `engine/verus/`, the Dockerfile secret, the e2e repair phase and AGENTS.md's three-phase e2e text.
  - Docs: ADRs 0028, 0030 and 0033; `proof-repair*.md`; `docs/validation/m7.md`; and the repair parts of `mcp.md`. It is the first proof-event producer.
- When the later PRs land, each must drop or merge the docs that #79, #80 and #81 put on `main` in condensed form, so that rescue docs do not duplicate them. These are the ROADMAP M7 section, `http-shared-cache.md`, ADRs 0017, 0025, 0027 and 0037, `mcp.md` and `workspace-verification.md`.

## Host and branch state

The host was `l2b`, a c7i.4xlarge in us-east-1. It was torn down with
`scripts/perf-host.sh down l2b` at the end of the lane. At handoff, all three PRs were current with `main` (`0fde777`).
