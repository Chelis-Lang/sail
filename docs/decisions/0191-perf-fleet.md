# 0191: Perf fleets with detached, resumable jobs

- **Status:** accepted
- **Date:** 2026-11-28

## Context

Kani work is memory-bound throughput with a small amount of timed A/B
(`docs/design/kani-phase.md` §7, ADR 0180). Harnesses reach about 10 GB, history
replays and corpus runs need several hosts at once, and a release claim needs
quiet timing hosts. `scripts/perf-host.sh` (ADR 0038) gives one tagged lane with
TTL self-termination and verified teardown, but its `run` is a blocking ssh, it
has no per-job isolation and it runs one host at a time.

The Kani spike wave showed the cost. Spikes ran long jobs with ad hoc `nohup`
markers, and the S2 and S3 agents collided on one shared scratch ssh wrapper.
D.I.V.E.'s AWS runs (its `scripts/aws/`) already proved the pieces that work:
`systemd-run` units with a completion marker, cached per-item `.rc` results on a
gate node, sharding over up to ten nodes, and a bare-metal bench node with SMT
and turbo off at a fixed frequency.

The `sail-automation` IAM user cannot read service quotas
(`servicequotas:GetServiceQuota` is denied), cannot view or create AWS Budgets
(`budgets:ViewBudget` is denied) and cannot read CloudWatch alarms. Its policy
covers EC2, EventBridge Scheduler, STS, SSM, pricing and `sail-*` IAM roles.

## Decision

Add `scripts/perf-fleet.py` beside `perf-host.sh`. Fleet lanes are ordinary
`perf-host.sh` lanes named `FLEET-1..FLEET-N`, so every lane keeps the same
tags, recorded region, TTL watchdog and verified `down`. `perf-host.sh` keeps
its interface and gains three opt-ins: `SAIL_PERF_MARKET=spot` (a one-time
spot request that terminates on interruption), `SAIL_PERF_SSH_CONFIG` (for
example `/dev/null`, so no PATH wrapper is needed) and `time` and `numactl`
in the host image. `down` also removes the lane's helper directory.

- **Detached jobs.** `submit` starts a command as a root `systemd-run` unit
  running `job.sh`, which runs the command in its own transient unit as the
  host user with `MemoryMax` (default 12G), `MemorySwapMax=0`,
  `TimeoutStopSec` equal to the grace period and `BindsTo` its parent unit,
  under `timeout --kill-after` and `/usr/bin/time -v`. Because the memory limit
  is on the inner unit, an out-of-memory kill or timeout ends only the command,
  and `job.sh` always writes `rc`, `result` (systemd's unit result) and `DONE`.
  `status` reports `ok` only for exit 0 with a successful unit; it reports
  `oom`, `timeout`, `killed`, `failed`, `refused`, `running`, `died` (no
  marker and no active unit) or `absent`.
- **Fleets.** `up FLEET COUNT` launches lanes in parallel. The default market
  is spot with on-demand fallback when the spot request is refused for
  capacity, price or limit before an instance exists. `list`, `extend` and
  `down` act on the whole fleet, and `down` fails unless `list` then shows
  none of its lanes. `up` requires an explicit `AWS_REGION`.
- **vCPU precheck.** Before launch, the standard-family vCPU quota for the
  market is compared with vCPUs in use in the region. Without the
  servicequotas permission the quota is unknown: the check uses
  `--vcpu-limit` or `SAIL_FLEET_VCPU_LIMIT` when given, otherwise it warns
  and proceeds, and a launch refused for `VcpuLimitExceeded` still cleans up
  through `perf-host.sh`.
- **Resumable runs.** `run FLEET JOBS.jsonl --results DIR` takes items
  `{"id", "cmd"}`. An item's result key is its id plus a digest of its
  command, so an edited command never reuses a result. Items are sharded
  longest prior time first (`--times`, or earlier results in DIR) across idle
  lanes, each round running as one detached runner unit with an optional
  per-lane setup. The coordinator copies finished items off every live lane
  each poll. All durable state is DIR plus what live hosts report: a
  restarted coordinator skips collected items, leaves items in a live lane's
  active round alone, and re-shards only items whose lane is lost (instance
  terminated, stopped or absent, or unreachable past `--dead-after`). A
  collected key is never replaced; a second result for it is recorded as a
  duplicate and fails the run. An item that died without output is retried
  `--retries` times. The run writes `summary.json` and `times.json`.
- **Heartbeat TTL.** While a run polls, it resets each live lane's shutdown to
  `--heartbeat` minutes from now (default 120). A lane outlives a lost
  coordinator by at most that long; `extend FLEET MINUTES` does the same by
  hand.
- **One build everywhere.** `ship FLEET PATH...` tars the paths with a per-file
  sha256 manifest, and each lane accepts the archive only if its sha256
  matches and every file checks against the manifest.
- **Lane-private helpers.** `helpers LANE` writes `ssh`, `scp` and `host` into
  `~/.sail-perf/LANE.bin`, with `-F /dev/null` and that lane's key and known
  hosts. No lane shares a wrapper path with another.
- **Quiet pool.** `quiet LANE` configures a bare-metal lane (the type must
  contain `.metal`, such as `c7i.metal-24xl`): SMT off, turbo off, governor
  `performance`, min and max frequency fixed (default 2.4 GHz), background
  timers stopped, then verified and marked in `/run/sail-fleet-quiet`.
  `--quiet-pool` jobs refuse a host without the mark and run one at a time
  behind a lock. Before each one they require zero stolen CPU time over one
  second and at most two runnable tasks, and they bind to `--cpus` and NUMA
  node 0. Alternating arm order belongs to the benchmark harness that submits
  the jobs. `quiet --dry-run` prints the script; no metal host was launched.
- **Budget alarm.** Not created: the IAM user cannot view or create budgets or
  CloudWatch alarms, and this decision does not change IAM. Cost stays bounded
  by TTL self-termination, the heartbeat and `down`'s listing check.
- **Results.** Nodes have no S3 permission or instance profile, so results
  come back over ssh into DIR. Evidence tarballs stay under
  `~/.sail-perf/evidence/`, cited by sha256 (ADR 0038).

## Consequences

Kani history replays, corpus runs and the Verus history and mutation lanes can
run on several hosts, survive a lost spot node or coordinator, and rerun only
unfinished items. `docs/validation/perf-fleet.md` records the gate.

An item that finished on a lost host after the last poll is rerun, because its
result went with the host; polls bound that loss. Writing results to S3 from
the node would remove it but needs an instance profile and bucket policy the
IAM user does not have. Revisit when the evidence bucket exists. A budget alarm
needs `budgets:*` or `cloudwatch:PutMetricAlarm` on a separate administrative
principal; that is an owner decision. The quiet profile is dry-run tested
only. Its first real use on `c7i.metal-24xl` must record the verified `quiet`
output with the timed rows.
