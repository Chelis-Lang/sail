# L15: concurrent daemon jobs and one full report per repair result

Lane L15 of the M7 rescue (ADR 0038). Two user-facing fixes from the
[L13 audit](l13-simplification-audit.md#behaviour-fixes-small-diff-large-effect),
B1 (the daemon half) and B2.

Host: native EC2 `c7i.4xlarge` (16 vCPU, 30 GiB), `us-east-1`, Ubuntu 24.04,
lane `l15`. Measurements ran in a CI image built on the host (see
[Environment](#environment-and-limits)).

## Many agents on one daemon

**Change.** The job service ran every job on one FIFO worker, so N agents on
one daemon took N× the wall time. It now runs up to `SAIL_JOB_WORKERS` jobs at
once (default: a quarter of the available cores, from 1 to 4; 4 on this host).
Jobs whose project resolves to the same Cargo workspace still run one at a
time, in submission order: a later one stays `queued`, it never fails. The
workspace key is the outermost ancestor of the project (or its
`--manifest-path`) holding a `Cargo.toml`, which can only serialise more than
Cargo's own resolution, never less. The scheduler is
`crates/sail-daemon/src/jobs/schedule.rs` (about 80 lines plus tests); the
worker loop is unchanged apart from taking a job and releasing its workspace.

**Workload.** HumanEval (`human-eval-verus` from `sail-bench repair prepare`),
four independent copies. Each copy was first verified once, unmeasured as a
warm-up (cold: 75 s for the first copy, then about 50 s each). In each measured
round, N agents simultaneously append a comment to a task file and submit
`sail verify --background` to one daemon, then poll until their job ends.
"Agent wall" is submit to finished; "total" is the round. Three rounds per
cell; all 117 measured jobs completed with exit code 0.

Separate workspaces (one agent per copy), median total and mean agent wall, s:

| Agents | Before (`5ec2063`) total | Before agent mean | After, 1 worker total | After, default total | After agent mean |
|---:|---:|---:|---:|---:|---:|
| 1 | 8.7 | 9.1 | 8.7 | 8.6 | 9.1 |
| 2 | 17.6 | 13.3 | 17.4 | 9.7 | 9.9 |
| 4 | 37.7 | 23.4 | 37.4 | 13.8 | 13.0 |

Four agents now finish in 13.8 s instead of 37.7 s (2.7×), and the slowest
agent waits 14.4 s instead of 38.1 s. Each job runs slower when four share the
host (12.9 s against 9.0 s alone), because Verus and Z3 already use several
cores. `SAIL_JOB_WORKERS=1` reproduces the old serial behaviour exactly.

One shared workspace (all agents edit different files of copy 1):

| Agents | Before total | After total | After agent mean | Mean queue wait |
|---:|---:|---:|---:|---:|
| 2 | 10.8 | 10.5 | 9.5 | 4.2 |
| 4 | 14.9 | 14.5 | 11.6 | 7.9 |

Jobs in one workspace queue as before and none fails. Queued jobs are short
(about 2 s) because the first job's managed run already verified all the
edits; each later job finds its inputs current.

After the rebase onto L14 (`5e99481`), two further rounds with the rebased
product gave the same picture: 4 separate agents 14.4 s total, 2 agents
10.6 s, shared 4 agents 14.8 s.

**SAIL3104 (for L7c).** Outside the job service, concurrent foreground
`sail verify --incremental` runs on one workspace still collide in the managed
runner's non-blocking ownership lock. With 2 runs, one failed after 1.4 s with
`error[SAIL3100]: managed verification: error[SAIL3104]: managed target is in
use: lock acquisition failed because the operation would block`; with 4, three
failed that way after 0.5 s. The daemon never hits this because it never runs
two jobs of one workspace together. Making the lock wait is L7c's
(`sail-verus` `managed/runner.rs`, untouched here).

## One full report per repair result

**Change.** A repair job result carried the complete full-phase project result
twice: as the job's `result` and as `repair.batch.full`. Now `result` holds it
once, as for every other job, and `repair.batch` omits `full`; `repair.accepted`
is still recomputed from `result`. A standalone repair document (the worker's
own output and foreground `sail repair-check --output json`) still carries
`batch.full`, so that document stays self-contained. `BatchResult::full` is
`Option<ProjectResult>` with `accepted_with(&full)` for the job case;
`repair-result-v1.json` no longer requires `full`. The contract had not
shipped, so version 1 changed in place (note in
[proof-repair](../../proof-repair.md)). Agents read verdicts from `result` for
every job kind; generic readers (MCP lookups, `sail jobs result`, `sail watch`,
baselines) needed no change.

**Measured** on one completed HumanEval `compute_gcd` repair job per product,
direct socket reads of the Result (100 reads, one client) and the CLI
(`sail jobs result --output json`, 20 reads):

| | Before | After |
|---|---:|---:|
| Frame bytes | 4,940,061 | 2,471,892 |
| `result` bytes | 2,468,092 | 2,468,024 |
| `repair` bytes | 2,471,516 | 3,413 |
| Socket read p50 / p95, ms | 26.1 / 29.8 | 22.3 / 26.9 |
| Client JSON decode p50, ms (Python) | 31.5 | 15.1 |
| CLI read p50 / p95, ms | 92.5 / 97.4 | 62.6 / 69.4 |
| CLI output bytes (pretty JSON) | 11,946,455 | 5,717,153 |

The frame halves. The server-side read barely moves, as L5 found (it is
dominated by input freshness); every reader's decode and the CLI halve or
nearly so. These jobs were Historical rather than Current (see below), which
does not change the frame's shape.

**Reader gate.** `scripts/reader-gate.sh` (L14's probe) could not run to a
sample on this host, for the base (`5e99481`) and the branch alike. With the
committed-patch engine built as L14 did (variant `5d281662a13d2227`), both
copies warmed to an accepted Current repair, but the probe then refused:
"the read job is not completed and Current: Completed, Unknown". This repeated
in 3 runs per product, with the workspaces on a bind mount and under the
container's `/tmp`, and with the solver job's namespace capabilities. With the
fork engine it refused earlier (see below). The cause was not established; it
is the same before and after this change, and L14's gate numbers stay the
reference. The frame halving applies to its Result
reads unchanged.

## Verification

- Mac: `ci/run.sh host` (clippy, 510 tests) passes on the rebased branch. One
  earlier run had one failure in
  `managed::observe::tests::explicitly_declared_inputs_in_reserved_directories_are_observed`,
  a macOS file-event test this lane does not touch; it passed three times
  alone and in the next full run.
- Host, rebased branch tree (`2417b1f` code) in the image: `gate` passes
  (658 tests, fmt, clippy, rustdoc, layering, deny, benchmarks page);
  `solver` 41/41; `e2e legacy` and `e2e managed` pass.
- Host, rebased branch merged with L12's `7772d9f` (the engine pin the image
  carries): `e2e repair` passes ("success, missing selector, full regression,
  late EXEC, exact evidence/cursors passed", with the updated
  `ci/test-repair.py`); `conformance fast pr` 33/33 same in pass-through,
  cache fill and cache warm, and the engine arm 33/33 same, 0 unstable. On the
  branch alone the engine arm refuses with `SAIL3120` (engine identity),
  because the image's engine is L12's; that is the stale-pin condition, not
  this change.
- New tests: scheduler order and workspace keys (`jobs/schedule.rs`), two
  workspaces running while a third job queues (`tests/jobs.rs`), the job result
  without `batch.full` (`tests/jobs.rs`, `tests/proof_product.rs`), acceptance
  with a separate full phase (`sail-api/tests/repair_contract.rs`), and
  `sail-bench` facts from a job result.

## Environment and limits

- **Image.** The committed engine pin is stale (`SAIL0944`, expected until L12
  re-pins). The image was built from a local, uncommitted merge with L12's
  unpushed `l12-fork-fixes` (`7772d9f`), whose pin fetches; the tag was
  `sail-ci:cca895edd85f0a5a`. Gates ran on the L15 branch tree alone in that
  image; the repair measurements ran on merges of base and branch with
  `7772d9f`. No overlay is committed.
- **Fork engine Currentness.** With L12's fork engine (`7772d9f`), repair jobs
  here completed Historical with `SAIL3114 native-observation-invalidated;
  detail=namespace-tracked-input-event`, before and after this change alike,
  and `reader-gate.sh` refused: "no accepted Current repair job". For L12.
- **Not run:** standard conformance, `release`, Vest. Concurrency is HumanEval
  only, with warm SMT cache (edits are comments, so queries hit the cache):
  the numbers measure the agent loop's frontend and scheduling, not solving.
- **Coordination.** Rebased on L14 (`5e99481`); conflicts in `jobs.rs` and
  `sail-bench` resolved, L14's deleted `job_read_bench/measure.rs` stays
  deleted. `sail-verus` `managed/*` untouched.

## Raw evidence

Outside the repository (lead's workstation, `~/sail-raw-evidence/l15/`):

| File | sha256 |
|---|---|
| `results.jsonl` (concurrency, before / after 1 worker / after default) | `1bdad71c4109f88ceb31e8793d2c86a307d95f018881712f0487cb347e06598c` |
| `results-rebased.jsonl` (concurrency, rebased product) | `63d3a96f06533f942446cf631ebdc010e706be3406a52a012ff47a2b010c3050` |
| `repair.jsonl` (frame sizes and read times) | `dc13b31f3421b8e8f46e43cfd55d30748176afd2641430c811fb93f0d71420ef` |
