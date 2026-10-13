# L5 readers: native reader latency, regression check and fixes

Lane L5 of the M7 rescue (ADR 0038). Host: native `c7i.4xlarge` (16 vCPU,
32 GiB), `us-east-1`, Ubuntu 24.04, CI image `sail-ci:8e24fb5f7f490c01` built
on the host. Workload: the reader acceptance in
[reader acceptance](../../reader-acceptance.md). It uses HumanEval
`compute_gcd` on a fresh repair job, with four concurrent clients, 200 samples
per collector, a 5 s RPC deadline and a 200 ms p95 limit, during active
verification, followed by retained CLI/MCP. All runs below pass every
admission check. Every sample is Current, with the same live verifier.

## Native baseline

The current `rescue/m7` product (`73a3b46`) already passes natively, with about
2x margin. Pooled over three runs (ms, p50/p95/p99):

| Collector | Status | Result |
|---|---|---|
| Python | 39.9 / 80.0 / 94.6 | 70.5 / 100.5 / 110.1 |
| Rust | 32.6 / 39.7 / 41.5 | 42.9 / 69.9 / 82.7 |

Retained CLI/MCP passed in all three runs.

## Regression check

| Product | Python Status p95 | Python Result p95 | Rust Status p95 | Rust Result p95 |
|---|---:|---:|---:|---:|
| `b89546c` | 61.5 | 96.6 | 44.7 | 67.2 |
| `387ba80` | 63.3 | 93.3 | 45.2 | 67.4 |
| `73a3b46` (3 runs) | 80.0 | 100.5 | 39.7 | 69.9 |

There is no native regression to bisect. `b89546c` and `387ba80` are within
run-to-run noise of each other, and both are far below 200 ms. Source confirms
this: between `b89546c` and `387ba80`, the Status/Result path is unchanged
(`jobs/store.rs`, `jobs/output_service.rs`, `jobs/result_cache.rs`,
`managed/freshness.rs`, `lease.rs`, `kernel_leases.rs`, `precise.rs`). The range
adds a `GetBaseline` request with baseline product code, MCP baseline tools, and
managed source-inventory changes. The inventory changes could change the leased
input set, but the native measurements show no effect.
The earlier emulated history is not monotone: `4472279` and `0f31c1d` failed,
`730a321` and `b89546c` passed, and `387ba80` failed. That pattern fits
environmental variance in the emulated runs. The emulated `387ba80` failure
(436 ms) is not reproducible natively. Its cause remains unknown.

## Where a read spends its time

From a diagnostic run (`SAIL_JOB_RPC_TIMINGS=1`, `SAIL_FRESHNESS_TRACE=1`, not
an acceptance sample), with server-side p50 values:

- **Freshness: 18-24 ms.** This dominates both Status and Result.
  - `ReadLease::clean` re-resolves bindings for ~1,180 leased files (~5 ms).
  - It then sweeps a ~225 KB `/proc/locks` twice, at 3-7 ms per sweep (p95
    ~16-19 ms under concurrency). The table holds ~2,370 lease rows from two
    contexts.
  - The namespace observer barriers add ~2 ms before and ~2-3 ms after.
- **Output seal: ~1.6 ms** for the full stdout read and SHA-256.
- **Socket output of the 4 MB frame: ~1.7 ms.** Frame key 0.02 ms; dispatch
  0.1-0.3 ms.

Review suspects, checked against these measurements:

| Suspect | Finding |
|---|---|
| Whole-record JSON + SHA-256 for the frame key | 0.02-0.15 ms natively. Replaced anyway (fix 3). |
| Status rereads and hashes 4 MB stdout | 1.6 ms natively. Status now reuses a guarded owner when one exists (fix 3). |
| Readers don't share in-flight freshness and wait up to 100 ms | Refuted. 192-203 of 416 reads joined another reader's validation, and no reader hit the Busy bound. A reader can join only a validation whose checkpoint follows its own start; that is the linearizability rule. |
| `try_lock` falls back to a full rehash | Real, including a lost-offer race (fix 2). Natively it rarely matters, because the guarded owner seldom exists (below). |
| Accept loop sleeps 10 ms | Real. It was the largest measurable fix (fix 1). |
| Clients must pull the whole 4 MB Result | Python parse dominates its Result time. No summary variant added (see below). |

The guarded output owner (ADR 0035) almost never engages in this workload. It
needs an 80 MiB construction reservation and 16 MiB per inspection from the
shared 128 MiB budget. By census time, earlier setup jobs' frame families and
owners hold most of that budget. 86 of 88 sampled guarded checks fell back to
the 1.6 ms full hash. This is documented in
[result-client](../../result-client.md) and left unchanged, because the native
gain would be ~1.6 ms.

## Fixes

| Commit | Change |
|---|---|
| `366f166` | **Fix 1.** The job endpoint blocks in `accept` (Linux, 100 ms `SO_RCVTIMEO`) instead of sleeping 10 ms between polls; shutdown wakes it. |
| `9c14cd0` | **Fix 2.** Output-owner `offer` and `validate` wait at most 50 ms for their short critical sections instead of `try_lock`. Expiry or poison still takes the complete read-and-hash path. This fixes a race where a constructor briefly holding its slot silently lost a frame family's one-shot offer. The same race made the output overlap tests fail intermittently on this host at `73a3b46`: 1-5 of 6 failed per run in 6 of 6 runs. After the fix, 10 of 10 runs were clean. |
| `be2246c` | **Fix 3.** Status reuses a Result's guarded output owner, bracketing freshness with its barriers exactly as Result does, instead of rereading stdout. The frame key binds status, mode and stdout seal instead of serializing the whole record. |

No change touches freshness classification or Current admission. The guarded
paths keep both barriers around the independent freshness check, and every
doubt falls back to the original complete read and hash. New tests:
- Status uses no reread when guarded, falls back under contention, detects a
  write bracketed by its own freshness check, stays sticky after invalidation,
  and errors on a wrong seal.
- A bounded lock acquires within its limit and declines after it.
- An offer survives a briefly held constructor slot.
- The blocking accept serves requests and shuts down promptly.

### Before/after (ms, p50 / p95 / p99)

| Collector, operation | Before (`73a3b46`, 3 runs) | Fix 1 only (`366f166`, 1 run) | All fixes (`be2246c`, 3 runs) |
|---|---|---|---|
| Rust Status | 32.6 / 39.7 / 41.5 | 25.4 / 32.8 / 35.0 | 25.5 / 34.0 / 38.8 |
| Rust Result | 42.9 / 69.9 / 82.7 | 27.0 / 34.9 / 56.2 | 27.3 / 43.6 / 71.4 |
| Python Status | 39.9 / 80.0 / 94.6 | 48.3 / 61.3 / 83.6 | 46.4 / 68.6 / 84.5 |
| Python Result | 70.5 / 100.5 / 110.1 | 77.4 / 102.9 / 126.3 | 72.1 / 101.9 / 115.7 |

Fix 1 alone reproduces the whole Rust gain: Status p50 falls by ~7 ms and
Result p50 by ~16 ms. Fixes 2 and 3 add no measurable latency change here,
because the guarded owner rarely exists. They remove a lost-offer race and a
redundant reread when it does. The Python collector does not improve. Its timed window shares the
interpreter lock with three reader threads that scan `/proc` and parse 4 MB
Results, and its p50 moves with client scheduling rather than server cost. The
Python Status p50 rose by ~8 ms, but the server-side Status total moved only
from 23.7 to 25.5 ms p50.

## Not done, and next steps

- No summary or paged Result. Gate reads use the full Result unchanged, and the
  server cost is not transfer: socket output of 4 MB takes ~1.7 ms. A
  summary variant would mainly spare Python clients a ~30 ms parse. That is a
  protocol and SDK change better scoped with the client work.
- The remaining server floor is the kernel-lease freshness barrier, ~20 ms per
  validation. Lowering it means revisiting ADR 0029. Options include fewer
  `/proc/locks` sweeps, helper-reported lease breaks, a smaller leased
  inventory, or selecting the test-only parent-descriptor binding batch in
  `lease_binding_batch.rs`. Each needs its own ADR and semantics review.
- The overlap-test race (fix 2) is a defect that can make `ci/local.sh gate`
  fail intermittently at `73a3b46` on a native multi-core host. It should be
  listed in `docs/known-issues.md` until this lands; this lane does not edit
  that file.

## Gates

On `l5`, with the tree at `be2246c` plus `scripts/reader-acceptance-run.py`
(later commits change Markdown only) and CI image `sail-ci:8e24fb5f7f490c01`:

- `ci/local.sh gate` passed, with nextest 681 passed and 41 skipped, along with
  fmt, clippy, doctests, rustdoc, layering, deny and the benchmarks page.
- `ci/local.sh solver` passed, with 41 passed.
- The full reader acceptance through `scripts/reader-acceptance-run.py` passed
  (run `final`). Python Status/Result p95 was 62.2/98.9 ms, Rust 32.3/34.9 ms,
  and retained CLI/MCP passed.

The host was torn down after use.

## Raw evidence

Raw evidence lives in `~/sail-raw-evidence/l5/` on the lead's workstation. Each
archive holds the controller summary, both census JSONs (every sample), the
retained CLI/MCP output, daemon stderr and, for diagnostic runs, the RPC stage
export. `l5-host-logs` holds the image, build, gate and solver logs and the host
scripts.

| Archive | sha256 |
|---|---|
| `l5-head-1.tar.gz` | `0fac579b1791f722b59e8484c46c877f33da02b72da399c8c9bb3fd88e5ae04c` |
| `l5-head-2.tar.gz` | `2b0ffa5133e42e51e62260f36013ede35c4642e51c6eb76e0023c7425c52d386` |
| `l5-head-3.tar.gz` | `4094ccd73f95e725f03d23f961e17dafd39d44946f30b531d09c2c47072afaa0` |
| `l5-b895-1.tar.gz` | `212a22b89399b192853dd3dfcb664cd5fd4d02f4cd7987ca8924c68a720e0009` |
| `l5-387-1.tar.gz` | `6bc340130979c88a1e475b83042512c594e57ea781d82a3a673e764cbeee80e3` |
| `l5-fix-1.tar.gz` | `58cce7d21d406e96b5966dd8796d58cc702df0fa6a92987033d913b5317d959a` |
| `l5-fix-2.tar.gz` | `357a599ca5fd1d8ef56ef09e6c99b26bfc88b3d38985b61443fcd49d67468a17` |
| `l5-final.tar.gz` | `210a4c115f3c1193bea6cca2b877cc5c19d90e8ec9642b7bad8f66f90c24ed16` |
| `l5-acc-1.tar.gz` | `588558218f7fad66a9708ef94883382a8e8f0689a4316aaac2e5f41538ab70b3` |
| `l5-head-diag.tar.gz` | `5190436bb119c594b305106b9da52f30f5f0acace07ba16af68bb5cf6e51aa3a` |
| `l5-fix-diag.tar.gz` | `cc7565bd78ad506c55a98e6dcd9890be81448ec1c592c22667e8279e5226bbf5` |
| `l5-diag-1.tar.gz` | `e7e345e8956942120320859edc632f3b32cf51c56a2f3adcaa5da70d47dab174` |
| `l5-host-logs.tar.gz` | `950cfe305725a3b254c82ebcd632fd137e2a37305726d2b4828b65d2cea6c192` |

Runs `head-diag` and `fix-diag` used diagnostic timings and are not acceptance
samples. `diag-1` was a throwaway build of `73a3b46` with the test-only lease
timers enabled; its source is not committed.
