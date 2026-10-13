# L18: M8.0 stage profile of the edit cycle

- **Lane:** L18, rescue, ADR 0038 item 6.1, first step of M8. **Date:** 2026-10-13.
- **Question:** the L9 measurement gate. Is unchanged-bucket work at least 25% of the median edit cycle? If so, the hermetic per-bucket memo (M8.1) proceeds.
- **Answer: no.** On the reference host it is 9–12% of the median cycle on every catalogue. The critical-path bound is 12–15%. M8.1 does not proceed, so no ADR is written. The cheaper items and two Sail defects are worth more; see the recommendation below.
- **Sensitivity.** With Verus limited to 7 worker threads (its default on an 8-CPU machine), the share rises to 18–19% on HumanEval and 27% on Vest. That is one round, so it is indicative only.

## Environment and identities

- **Host.** AWS EC2 `c7i.8xlarge` (32 vCPU), us-east-1, Ubuntu 24.04.5, kernel 7.0.0-1014-aws. Native x86-64, launched with `scripts/perf-host.sh up l18 c7i.8xlarge` and torn down after use.
- **Container.** CI image `sail-ci:3f31e6fc827f00bf`, built natively. Workspaces, caches and targets were on Docker volumes.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0, Rust 1.98.1. Verus used 31 worker threads, its default here.
- **Engine.** Fork `Chelis-Lang/verus` tag `archive/m8-timers-l18` (formerly branch `m8/timers`):
  - `3a91b1d7` adds the timers on top of `sail` (`89709da7`). It is the runtime that was measured.
  - `2ab719dd` fixes only the regression test.
  - The engine was built with `cargo xtask build-verus-source --interactive-engine`, with `engine/verus/fork.toml` pointed at `3a91b1d7` in the host copy only. The committed pin is unchanged.
- **Sail.** `rescue/m7` at `e20d849`, release build.

## The timers (fork tag `archive/m8-timers-l18`)

**Where they appear.** The timers are added under the existing `--time-expanded` flag. With `--output-json`, the timing output gains `times-ms.sail-stages`; in text mode, a stage-totals section is printed. Without `--time-expanded`, the output is unchanged; a regression test checks this.

**What is recorded for each bucket:**
- the worker thread, and the start and end of the bucket;
- `prune`;
- `key`, a proxy for computing the memo key: bincode serialization and SHA-256 of the pruned krate;
- `Ctx::new`, `ast_to_sst` and `poly`;
- AIR context creation and bucket-context lowering;
- op generation (the `OpGenerator` calls);
- AIR commands, minus the solver waits;
- solver spawn and solver wait (the shim under Sail);
- the rest of the bucket's time;
- the bytes and a digest of the SMT stream the bucket sent, across all its solver processes.

**Crate-level timers, for each verification phase:** `GlobalCtx::new`, simplify, bucket setup, the bucket phase, worker merge and the tail.

**Checks, on the fork build:**
- `sail_m8_stage_timers` passes, and so do all ten `sail_vi*` regression tests.
- SMT logs are byte-identical with and without `--time-expanded`.

**Overhead.** Measured on round 0 against a run of the same cycle without the timers:

| Catalogue | Median overhead per cycle |
|---|---:|
| HumanEval | +11 ms (0.2%) |
| Vest | +0.16 to +0.64 s, median 0.56 s (2.3%) |

The overhead is mostly the key proxy. It inflates the bucket phase slightly, so it favours the memo.

## Method

Each catalogue has a private daemon and cache.

**States measured.** Every repair-catalogue cycle (`corpus/repair-edits.json`), and for HumanEval also the six steps of `corpus/agent-loops.toml`. Each state is written from the frozen texts. There is one warm-up per state, then three measured rounds, with arm order alternating each round. Every check was a cache hit: HumanEval 4,523 of 4,523 per cycle, Vest 32,636 of 32,636. Z3 never ran in a measured cycle.

**The two arms:**
- **verus.** Runs the engine runtime the way Sail invokes it:
  - command: `cargo-verus verus verify --target-dir T -p P --message-format=json-render-diagnostics -- --output-json --no-report-long-running -Z write-long-types-to-disk=no --time-expanded`;
  - solver: the `sail-z3` shim on the daemon's cache.

  This arm gives Cargo, rustc, VIR and bucket stages. The full phase uses stock lowering; no engine focus phase runs.
- **sail.** Stock managed `sail verify --incremental --output json --package P` with `SAIL_MANAGED_FORCE=1`. It gives Sail's overhead outside its Cargo-Verus invocation, from the `managed-setup-phase` timers.

**Why the arms are separate.** The edit cycle is Sail overhead plus the verus arm's wall time. `sail verify --engine interactive` cannot be measured as a whole cycle at this commit: it re-verifies vstd on every run (defect 1 below).

**Changed and unchanged buckets.** A bucket is *changed* when the digest of the SMT bytes it sent was not already seen for that bucket in a state the workflow had verified:
- the original project, for every cycle;
- the `from` state, for a repair cycle that starts from one;
- the loop's earlier steps, for an agent-loop step.

Identical SMT bytes are necessary for an unchanged memo key. This count of unchanged buckets is therefore an upper bound on memo hits.

**How time is attributed.**
- At each instant, the running buckets share the wall time equally. Each bucket's share is split over its stages in proportion to their durations.
- *Unchanged-bucket work* is the unchanged buckets' attributed time, minus `prune` and `key`, which a memo still pays.
- *Critical-path saving* is the bucket phase minus two terms: the longest changed bucket, and the prune-and-key cost of all buckets divided over the threads.

## Results (reference host, 31 Verus threads, 3 rounds)

The cells are medians of each component over all cycles and rounds, in seconds. Components are rounded separately, so they need not add up exactly to the cycle.

| Component | HumanEval repair (n=14) | HumanEval agent loops (n=18) | Vest repair (n=15) |
|---|---:|---:|---:|
| **Median cycle** | **5.48** | **5.46** | **24.14** |
| Sail overhead (stock managed, outside Cargo-Verus) | 0.62 | 0.62 | 1.74 |
| Cargo, incl. `rust_verify` start and exit | 0.50 | 0.50 | 1.72 |
| rustc frontend | 1.62 | 1.62 | 6.43 |
|   GHOST pass (`init-and-types`, trait conflicts) | 1.09 | 1.10 | 3.39 |
|   EXEC pass (`compile`; 0 on failing cycles) | 0.53 | 0.53 | 3.04 |
| Verus VIR (HIR, vstd import, `rust_to_vir`, checks, `GlobalCtx`, simplify) | 1.41 | 1.42 | 7.10 |
|   of which vstd import | 0.20 | 0.20 | 0.19 |
| Verus time outside its phase timers | 0.47 | 0.47 | 2.07 |
| Bucket phase (wall) | 0.83 | 0.82 | 5.08 |
|   lowering of unchanged buckets | 0.47 | 0.52 | 3.25 |
|   lowering of changed buckets | 0.05 | 0.00 | 0.16 |
|   solver (shim spawn and wait, all cache hits) | 0.11 | 0.11 | 0.57 |
|   idle and worker merge | 0.19 | 0.19 | 1.08 |
| Changed buckets per cycle (of 105 / 105 / 200) | 1.5 | 1 | 2 |
| **Unchanged-bucket work, share of cycle** | **9.1%** | **10.1%** | **12.4%** |
| Critical-path saving, share of cycle | 12.0% | 14.0% | 15.0% |

**Per cycle.**
- HumanEval: shares of 7–12% on every cycle.
- Vest: shares of 12–15% on every cycle.
- Failing cycles are about 1 s shorter on HumanEval and 4 s shorter on Vest, because they have no EXEC pass. Their share is slightly higher.

**Per-bucket CPU, summed over threads.** HumanEval totals about 10.1 s, Vest about 82 s.

| Stage | HumanEval | Vest |
|---|---:|---:|
| prune | 1.25 s | 19.4 s |
| op generation | 1.81 s | 19.2 s |
| AIR commands | 2.19 s | 11.1 s |
| `ast_to_sst` | 1.76 s | 13.3 s |
| solver wait | 0.92 s | 5.2 s |

On Vest, prune is the largest stage, and a memo must still pay it to compute the key.

**Sensitivity: 7 Verus threads, one round, n = 5 to 6.**

| Catalogue | Unchanged-bucket work | Critical path | Cycle |
|---|---:|---:|---:|
| HumanEval repair | 18.4% | 20.8% | 6.08 s |
| HumanEval loops | 18.8% | 21.4% | 6.00 s |
| Vest | 27.4% | 29.1% | 30.95 s |

The memo's value grows as the cores shrink. The bucket phase parallelises, while rustc and VIR are serial.

## Recommendation: cheaper items first

These are shares of the median reference cycle.

| Item | HumanEval | Vest | Notes |
|---|---:|---:|---|
| (b1) rustc incremental on the EXEC pass | ≤ 0.53 s (9.6%) | ≤ 3.04 s (12.6%) | Passing cycles only. Larger than the memo on Vest. Check first whether `cargo verus` already passes `-C incremental` |
| (d2) Cargo bypass when only target sources changed | ≤ 0.50 s (9.1%) | ≤ 1.72 s (7.1%) | Upper bound: it includes `rust_verify` start and exit, which a bypass keeps |
| (d1) spare process with vstd preloaded | vstd import 0.20 s (3.6%), plus part of the process start | 0.19 s (0.8%) | Process start was not separated from Cargo |
| Sail overhead | 0.62 s (11.3%) | 1.74 s (7.2%) | The L7c target of 0.4 to 0.9 s holds on HumanEval. It is higher on Vest |
| Memo (c), for comparison | 0.45 s (median share 9.1%) | 2.96 s (median share 12.4%) | — |

**Two Sail defects outweigh every item above.** Neither was diagnosed here, and no cause is attributed.

1. **`sail verify --engine interactive` re-verifies vstd on every run.**
   - HumanEval takes about 33 s per run, against 5.7 s for stock managed: 2 verifier invocations and 0 reused targets.
   - `input_revision` changes on every run although no file changed. The context digest is stable.
   - `repair-check` behaves the same: 33.8 s per batch, and the batch is not accepted.
   - Seen on `e20d849`, with the engine built from `3a91b1d7`.
2. **Stock managed: the first passing run after a failing run re-verifies vstd.**
   - Cost: +27.5 s on HumanEval (33 s against 5.7 s) and +27.5 s on Vest (53 s against 25.4 s).
   - It happened in 12 of 12 such samples.
   - It hits the agent's most common cycle, fix after failure: 3 of the 6 agent-loop steps and 1 of the 5 repair cycles per catalogue.
   - L7c's "62 to 71 s" `strlen-repaired-attempt` outlier is likely the same effect. This is not established.

**Further lines of investigation:**
- Add timers inside VIR and the unattributed Verus time before considering option (b2), the GHOST pass. VIR not covered by HIR, import and `rust_to_vir` is 0.42 s on HumanEval and 2.81 s on Vest. Verus time outside its phase timers is 0.47 s and 2.07 s.
- Revisit M8.1 if the target hardware for agents has 8 cores or fewer, or after the items above have shrunk the serial part.

## Determinism on the fork

**Measured buckets whose SMT digest varied between runs of identical input:**
- HumanEval: `human_eval_160` in 16 of 32 samples;
- Vest: `cbor::value` in 10 samples and `asn1::integer` in 4.

**`human_eval_160` reproduced.** Four focused runs logged SMT for this module. Their logs differ in the order of the `arrow_1` and `arrow_2` declarations of `Expr` (`impl&%4`). This is VI-0001's symptom, so the fork fix `9a5b2cc8` does not cover this case.

**Vest buckets.** Their cause was not investigated.

**Effect on these figures.** The changed-bucket rule above counts such a bucket as changed, so these flips lower the unchanged share slightly. The effect is at most one bucket per cycle on HumanEval.

## Limits

- The edit cycle is composed from two arms. Sail overhead comes from stock managed runs, and the Verus part from the engine runtime run directly with the same flags and shim. Neither is a single `sail` command, because of defect 1.
- The memo estimate is an upper bound. It assumes an unchanged bucket costs only prune plus a serialization hash. The real canonical key does more: span and id canonicalisation (L9 §2.4).
- A bucket with identical SMT is counted as a memo hit even when its canonical input could differ.
- The sensitivity run has one round. The process-start share of the Cargo figure was not separated.
- One HumanEval sample, round 2 `gcd-body`, had no Verus run. Cargo found it fresh, because its text equals the agent loop's `split-mod` state verified just before. It is excluded, so HumanEval repair has n=14.

## Evidence

Raw files are in `~/sail-raw-evidence/l18/` on the lead's machine. `SHA256SUMS` covers every file.

| File | sha256 |
|---|---|
| `l18-out.tar.gz`: main campaign, with `runs.jsonl`, every run's stdout, stderr, Verus JSON and setup timers, determinism logs and fork test logs | `3c6487b30e8f97cb07d810ee9fa5aa9d6fa1230da9d1582a1f796050fa1fdaee` |
| `l18-out-t7.tar.gz`: 7-thread sensitivity run | `74a240cf33eba6bf139103d406ae314a7e97a46524aa8e682f9a9a5594c60bb3` |
| `l18-scripts.tar.gz`: driver (`measure.py`), campaign, determinism and fork-test scripts | `b0d5c48bb1303c8c754e95043f279d810eb2514626833dde929c8f3425f05e3e` |
| `analyze-final.py` | `6d9b32950e07a9b02e1f8c7118e5d1bf838af0f463d015fa1592790b705afbf3` |
| `analysis-31-threads.txt` | `ad5679cb79a70a4331d417cc9bdb0528ed016c356c368d0f3e066d013db61851` |
| `analysis-7-threads.txt` | `f2de1823a2f536116fbb4c97ceba5965482e7aaaec760f4d07999a8b055f9025` |
| `image-build.log`, `setup.log` | `3b4273f9…`, `9f7e8864…` |
