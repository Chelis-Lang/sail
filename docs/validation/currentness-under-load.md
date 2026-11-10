# Currentness under load (#696, #683)

**Status:** met for this candidate. All 200 background jobs on three loaded
services were Current. None was Unknown, and no check lacked a source. The
first runs that followed an interrupted first run (a preempted new-project
preparation or a cancelled job) took the ADR 0110 discard path and were
Current.

| Field | Evidence |
|---|---|
| Source | Branch `fix/v030-currentness` at `e69467fe` (PR #704), measured from the synced tree. The branch was then rebased onto `765a3866`; `git range-diff` shows its code and test commits (`c795bbd6`, `66399247`, `b482047b`) identical to the measured ones, and only the documentation commit changed, in context lines |
| Runtime | The CI image's validated fork runtime (`SAIL_ENGINE_BUILD`) and pinned Z3, with release `sail` and `sail-z3` built in that image |
| Environment | Native EC2 c7i.8xlarge, us-west-2, Ubuntu 24.04, inside the `core` CI image. `stress-ng --cpu 14 --vm 2 --vm-bytes 1G` ran on the host throughout, with a load average of 20 to 44. 1,000 create/delete pairs per second ran in the container's `/tmp`, which the absent-configuration watcher watches as a workspace ancestor |
| Scope | Three `sail serve` services, each fed one job at a time. Service A: 70 `sail new --lib` Cargo projects, then 30 standalone files. Service B: 50 projects submitted 1, 2 or 3 s after `sail new`, preempting the service's preparation. Service C: 25 projects whose first job was cancelled after 5 or 8 s and then resubmitted, then 25 Cargo projects. In services B and C each counted job was followed by an unchanged resubmission; all 75 were Current. All 25 cancelled first jobs ended `cancelled` |
| Limits | One host, one run. Neutral source jobs were not part of the loop. The discard events were counted only partly, because the services' caches lived in a removed container: 6 distinct discarding runs were seen in service A before collection was cut short, and an interim check found them in all three services. Organic shim handshake failures (#683) were not reproduced, so the unsourced-check rule is covered by its unit test, not by this loop |

## Results

| Service and mode | Jobs | Current | Unknown | Checks without a source |
|---|---:|---:|---:|---:|
| A: Cargo | 70 | 70 | 0 | 0 |
| A: standalone | 30 | 30 | 0 | 0 |
| B: preempted preparation | 50 | 50 | 0 | not counted |
| C: cancelled first job | 25 | 25 | 0 | not counted |
| C: Cargo | 25 | 25 | 0 | 0 |
| **All** | **200** | **200** | **0** | **0 in the 125 counted jobs** |

Every `currentness-decision` event that was read had empty `reasons`.

For comparison, the spike's control arm (prototype toggles off, #696 comment of
2026-11-09) recorded 9 of 11 preempted-preparation jobs, 2 of 2
cancelled-first jobs and 53 of 83 loaded Cargo jobs as Unknown, and 5 of 5
jobs as Unknown at the same 1,000 pairs per second of `/tmp` churn.

## Evidence

The raw JSONL, load samples and suite logs are kept outside the repository:
`lane-evidence.tgz`, sha256
`00c90727ec64fa0f25c4285091af68f21dcc9b5a8aca9cb425ffcaf8c16a76df`.

## Attempts

- `e69467fe`, 2026-11-10: 200 of 200 Current, as above.
