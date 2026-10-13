# L14: deletions and simplifications from the L13 audit

- **Date:** 2026-10-12
- **Base:** `origin/rescue/m7` at `5ec2063`, rebased onto `d4b8651` (docs only)
- **Scope:** the high-confidence findings of the [L13 audit](l13-simplification-audit.md)
  outside the managed-freshness area: #1, #2, #3, #5, #6, #7, #9, #11, #12,
  #14, #16, #18 and #20, plus mechanism-only docs. Skipped as instructed: #4,
  #8, #10, #13, #15, #17, #19, #21, #22.
- **Total:** 11 commits, 19,823 lines removed and 905 added (net -18,918,
  including this report), from `git diff --shortstat` against the base.

## Per finding

| # | Finding | Removed / added | Kept, and why |
|---:|---|---:|---|
| 1 | `bench/validation/` dumps and six summarisers | 7,518 / 13 | Archived outside the repository, sha256 `dcf30c18…7322c`; the citing reports now name that digest |
| 2, 9 | Python M7 reader-acceptance suite (14 scripts, a fixture) and the `job_read_bench` example's three modules | 4,410 / 319 | One reader probe: `job_read_bench` shrunk to a single 214-line file with no `/proc` census and no `test = true`, driven by the new 82-line `scripts/reader-gate.sh` |
| 3 | Guarded output owner: `output_service.rs`, `output_overlap_tests.rs`, store branches and Linux-only tests, `guarded_output_profile` example | ~1,500 of the 2,821 below | The complete read-and-hash fallback, which already ran in 86 of 88 checks. ADR 0035 is now **rejected**. `managed::GuardedOutput` stays in `sail-verus` (`managed/*` is excluded from this lane) and is now unused; L7c should delete it |
| 7 | RPC timing diagnostics (`rpc_diagnostics.rs`, `Trace` plumbing, metadata test, `SAIL_JOB_RPC_TIMINGS` CLI filter, guide) | ~1,000 of the 2,821 below | The Linux blocking `accept` (`configure_accept`) and its test. The `SAIL_JOB_RPC_TIMINGS` entry in `managed/runner.rs`'s environment filter stays (excluded area; harmless) |
| 14 | Exact allocator accounting in the result frame cache | 366 / 66 in `result_cache.rs` | The cache itself, now a 16-entry LRU bounded by frame bytes. A new cross-platform store test keeps the frame equality, `SAIL0816` seal and `SAIL0814` size checks |
| 3, 7, 14 | Commit total | 2,821 / 206 | ADR 0037's status history is trimmed in the same commit |
| 6 | Offline profilers (three `sail-api` examples, guide, release step) | 983 / 2 | — |
| 11 | Process-tree capture on CI timeouts (`ci/managed-evidence.py` and its test) | 519 / 15 | `ci/test-managed.py` keeps its process-group kill and keeps stdout and stderr; no assertion or deadline changed |
| 5, 12, 16 | Decoded-result reuse and heap accounting, the cancellable RPC monitor, the MCP endpoint table and CLI error mimicry | 2,648 / 159 | `Client::call_with` (~30 lines) hands out the connected stream; `sail mcp`'s `Cancel` shuts it down, so cancellation is still immediate and its two unit tests pass unchanged in substance. `response_matches` (job identity) kept. MCP errors are now plain `SAIL0813` text |
| 18 | MCP live-service CI tests and their controller test | 457 / 4 | MCP cancellation and direct reads stay covered by `sail-mcp` unit tests; verdicts by `check_mcp`, `test-managed.py` and `test-repair.py` |
| 20 | Repair bench preparation telemetry, RSS sampling (`/proc` and GNU `time`) and the raw sidecar (`--raw`) | 463 / 70 | Record-then-judge, the outcome comparison and the median gate. Each invocation's command, stdout and stderr stay under `--out` |
| — | Mechanism-only docs | in the rows above | `result-rpc-diagnostics.md` and `result-frame-profile.md` deleted; `result-client.md` cut from 382 to 52 lines; `reader-acceptance.md`, `native-product-setup.md`, `freshness.md`, `managed-cold-start.md`, `mcp.md` and `m7-repair-acceptance.md` lose their script and mechanism sections; ADRs 0035 and 0037 trimmed; `ci/run.sh release` and AGENTS.md updated; one known issue (the removed cancellable test) dropped |

CHANGELOG entries cover the user-visible removals: `SAIL_JOB_RPC_TIMINGS`,
the `sail_api::jobs::Client` caching and cancellable API, MCP direct-read
error text, and `sail-bench repair --raw` with its RSS and preparation columns.

## Reader gate (M7 gate 5)

The gate stays measurable through one probe: `scripts/reader-gate.sh` runs a
private daemon, warms two HumanEval copies (ordinary full verification, then
an accepted and Current `compute_gcd` repair), starts a third repair in the
second copy, waits for its `human_eval_verus` verifier, and runs
`job_read_bench`: four clients, 200 reads (100 Status, 100 Result of about
4 MB), every sample Current with the active job Running before and after, and
the same verifier still live at the end. The command is in
[M7 validation, gate 5](../../validation/m7.md#gate-5-responsive-reads).

Native EC2 c7i.4xlarge, us-east-1, lane `l14`. p95 in ms:

| Product | Probe | Run | Status p95 | Result p95 | Valid samples |
|---|---|---|---:|---:|---|
| `5ec2063` (before) | old Python controller, Python collector | 1 | 78.0 | 144.8 | 200/200 Current |
| `5ec2063` (before) | old controller, Rust collector | 1 | 32.2 | 33.8 | refused: the active job finished after 88 of 200 samples |
| `5ec2063` (before) | new `reader-gate.sh` | 1 | 36.9 | 59.1 | 200/200 |
| `5ec2063` (before) | new `reader-gate.sh` | 2 | 40.9 | 61.3 | 200/200 |
| L14 branch (after) | new `reader-gate.sh` | 1 | 21.6 | 46.8 | 200/200 |
| L14 branch (after) | new `reader-gate.sh` | 2 | 28.4 | 48.5 | 200/200 |

With the same probe, the branch is faster than the base by about 12-15 ms
of p95 on both reads; both are far below 200 ms. The cause of the
difference was not profiled.
The old controller's Rust census failed on this host because the engine
without prepared lowering (L7a) finishes the active repair sooner, so the
Python census consumed most of the active window. The first branch run with a
cold active copy refused because the read job was not Current; the script now
warms both copies, as the old controller did. That cause was not established.

## Environment and limits

- **Image.** The committed engine pin is stale (`SAIL0944`, as the brief
  expected), so the image was built from `ci/Dockerfile` without its engine
  fetch step, with the tag of the committed inputs. The engine was built on
  the host from the committed patch with `cargo xtask build-verus-engine`
  (variant `5d281662a13d2227`, the L7a candidate) and supplied as
  `/opt/sail-engine`. No overlay was committed.
- **Gates on the host, in that image, at `9991402` (before the docs-only rebase):** `ci/local.sh gate`
  pass (655 tests, fmt, clippy, rustdoc, layering, deny, benchmarks page);
  `solver` 41/41; `e2e` all three phases pass; `conformance fast pr` 33/33
  same in pass-through, cache fill and cache warm, and the engine arm 33/33
  same, 0 unstable.
- **macOS:** `ci/run.sh host` passes (clippy, 507 tests).
- **Not run:** `ci/local.sh release` and the standard conformance tier. The
  reader numbers are two runs per product, not a distribution.
- **Coordination.** L15 is changing `sail-daemon/src/jobs` (`store.rs`,
  `worker.rs`) and the repair result shape. This lane rewrote parts of
  `jobs.rs`, `store.rs`, `worker.rs` and `result_cache.rs`; expect conflicts
  there. The host was torn down.

## Raw evidence

In `~/sail-raw-evidence/l14/` on the lead's workstation:

| Archive | sha256 |
|---|---|
| `bench-validation-5ec2063.tar.gz`: the removed `bench/validation/` | `dcf30c1827434a157d17d15825305cdb8eac1265e99642489f8f37b97887322c` |
| `l14-reader-runs.tar.gz`: every reader sample, both controllers, host scripts | `fb0508b94e956867352bff9fe55e0187f318be45bfd508575f23cef53add8ac6` |
| `l14-host-logs.tar.gz`: gate, solver, e2e, conformance and macOS logs | `1b9cccd03df256d7254d91342541f8c73dcc6fc12366a4f10a5881c476691e14` |

## Documentation impact

Changed: the guides and ADRs listed above, `docs/README.md`, `README.md`,
`AGENTS.md`, `CHANGELOG.md`, `docs/ROADMAP.md` (reader row),
`docs/validation/m7.md` (gate 5 command and latest results),
`docs/known-issues.md`, and this report.
