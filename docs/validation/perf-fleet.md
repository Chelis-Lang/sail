# Perf fleet gate (ADR 0191)

Status: **passes.** On three spot `c7i.2xlarge` lanes, a 60-item job list ran
twice cleanly. A third run lost one node mid-run (instance terminated) and its
coordinator was killed and restarted. That run completed all 60 items once.
Only the 14 items the lost node had not returned were run again, on the two
live lanes, with 0 duplicates. On the same hosts, detached jobs reported an
out-of-memory kill as `oom` and timeouts as `timeout`, never `ok`.

## Environment

| | |
|---|---|
| Hosts | `kani-fleet-1..3`, `c7i.2xlarge` (8 vCPU, no swap), spot, us-east-2a, Ubuntu 24.04 from `perf-host.sh`, systemd cgroup v2, 2026-11-28 10:52–11:22 UTC, 3 × 0.5 host-hours |
| Tooling | `scripts/perf-fleet.py` sha256 `e1d06e81…5461c`, `scripts/perf-host.sh` sha256 `40109f1d…71d6e`, unchanged from the first gate run to the last |
| Workload | serde-rs/json, the 60 first-parent commits of `master` from `2a5b8531` (2025-08-18) to `f51fa15a` (2026-11-15). The bare clone was shipped once to all three lanes and verified on each by sha256 (`1e039e99…b9efed`, 22 files) |
| Toolchain | Rust 1.99.0 via rustup, installed by each lane's `--setup` |

Each item clones the shipped repository into a temporary directory, checks out
its commit and builds with a private target directory. All items used
`--slots 1`, `--mem 12G`, `--timeout 900` and `--poll 15`.

## Runs

| Run | Item command | Items | Item wall (s) | Result |
|---|---|---|---|---|
| `replay60` | `cargo build -q --lib` | 60, 20 per lane | about 2 | 60 `ok`, 0 duplicates |
| `replay60-kill` | `cargo test -q --no-run` | 60, 20 per lane | about 7 | 60 `ok`, 0 duplicates |
| `replay60-resume` | `cargo test -q --release` twice (default and all features) | 60 | 24.2–25.5 | 60 `ok`, 0 duplicates, see below |

The first two runs finished too quickly to kill a node mid-run. They are clean
passes, and their `times.json` gave the next run its prior times for sharding.

### Lost node and restarted coordinator (`replay60-resume`)

1. Invocation 1 sharded 20 items to each lane (round `r1`) and collected 18
   items, 6 of them from `kani-fleet-2`. The last of those was collected at
   11:11:41.
2. At 11:11:56, `aws ec2 terminate-instances` ended `kani-fleet-2`. Five
   seconds later the coordinator was killed (exit 143).
3. Invocation 2 started at 11:12:14, found 18 items done and marked
   `kani-fleet-2` lost at 11:12:30 (instance `terminated`). It left lanes 1 and
   3 alone while their `r1` rounds ran. At 11:17:40 it sharded the 14 items
   `kani-fleet-2` had not returned into two `r2` rounds of 7, one on each live
   lane.
4. Invocation 2 exited 0 with 60 `ok`: 27 from lane 1, 27 from lane 3 and 6
   from lane 2. Invocation 3 found 60 done and submitted nothing.

Checks over `events.jsonl` (the analysis script is in the evidence archive):

| Check | Result |
|---|---|
| Items collected / unique / result directories | 60 / 60 / 60 |
| Resubmitted items = lost lane's `r1` minus its collected items | 14 = 20 − 6, equal sets |
| Resubmitted items already collected before the kill | 0 |
| Resubmitted items that belonged to a live lane | 0 |
| `duplicate` events, `summary.json` duplicates | 0, 0 |
| `DONE` items on lanes 1 and 3 at the end | 27 and 27 (54 + 6 from lane 2 = 60) |

The item running on `kani-fleet-2` when it was terminated, and any item it
finished in the 15 s after the last collection, were lost with the host and ran
again elsewhere. Lane 2's host-side count at the kill was not captured. The
design accepts this cost: the poll interval bounds it (ADR 0191).

## Detached jobs on a real host

These ran on `kani-fleet-1` through `submit`, `status` and `fetch`:

| Job | Command and limits | Status |
|---|---|---|
| `fine` | `echo`, `nproc`, `swapon --show` | `ok` (rc 0, `success`), 8 CPUs, no swap |
| `oom` | allocate 1 GiB under `--mem 256M` | `oom` (rc 1, unit `oom-kill`, main process status 137) |
| `slow` | `sleep 100` under `--timeout 3 --grace 2` | `timeout` (rc 124) |
| `stubborn3` | child ignores SIGTERM, `--timeout 2 --grace 2` | `timeout` (rc 124), unit ended after 4.24 s |
| `gone3` | `sleep 300`, its job unit killed with SIGKILL | `died`, no stray process left |

Two defects showed up here and were fixed before the gate runs. A child that
survived `timeout` held its unit in `stop-sigterm` for systemd's default 90 s;
the item unit now sets `TimeoutStopSec` to the grace period (`stubborn` and
`stubborn2` ran before the fix). Killing a job's outer unit left its item
running; the item unit now sets `BindsTo` its parent (`gone` and `gone2` ran
before the fix). The fake-host tests found a third: the poll script exited
nonzero when its last item was unfinished, so a busy lane looked unreachable.

The heartbeat reset `kani-fleet-1`'s scheduled poweroff to 119 minutes ahead
(`--heartbeat 120`). The vCPU precheck ran its degraded path: `servicequotas`
was denied, so it reported the quota as unknown, 0 spot vCPUs in use and 24
needed. All three spot requests were filled, so the on-demand fallback ran only
in `scripts/test-perf-fleet.py`. The quiet bare-metal profile was tested only
with `--dry-run` and in the unit tests; no metal host was launched.

## Teardown

`perf-fleet.py down kani-fleet` reported `kani-fleet-1`, `-2` and `-3` down
and `fleet kani-fleet down: 3 lane(s), none listed`. `perf-host.sh list`
afterwards showed no `kani-fleet` lane, and no `kani-fleet` state remained
under `~/.sail-perf`.

## Evidence

`~/.sail-perf/evidence/2026-11-28-perf-fleet-gate.tar.gz`, 201322 bytes,
sha256 `54df35d35c5254a76d41e86e54de39f53a2bd3e1cb0239977fce124dc9285c68`. It
holds the three job lists, the commit list, every coordinator log, the results
directories (events, summaries, times and per-item `rc`, `result`, `time`,
`stdout` and `stderr`), the fetched `oom` and `stubborn3` jobs, host checks,
teardown output and the analysis script and output.
