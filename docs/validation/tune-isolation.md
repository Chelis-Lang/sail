# Spike: hand-applied prover isolation for changed-proof cost (#158, #244)

**Status: spike complete. On Anvil, one three-function isolation met the
timing and conformance bars but moved one proof from 10% to 89% of its
rlimit. IronKV's heaviest module has almost no changed-proof gap. Decision:
`sail tune` is not built for v0.3.0, and `--isolation auto` has no case.**

The question was whether adding `#[verifier::spinoff_prover]` by hand to the
functions that dominate a module's shared solver process makes a one-function
sibling edit cheap under Sail. Stock Verus and Sail must still agree on the
patched source. Sail keys default-mode replies by the whole process prefix
(ADR 0008). After a miss, the solver therefore replays every earlier query in
that module's process. An edit to any function in a shared bucket costs
about the whole bucket. A function with `spinoff_prover` gets its own
process, keyed only by its own bytes, so it hits the cache when a sibling
changes.

Pass rule (v0.3.0 plan, Track C): no verdict or diagnostic differences
between stock and Sail on the patched source, sibling edit at most 60 s, and
cold at most 3% slower than unpatched.

| Field | Value |
|---|---|
| Environment | Native EC2 c7i.8xlarge (32 vCPU), us-west-2, Ubuntu 24.04, one idle lane (`v3-piso2`); pinned Z3 4.16.0 |
| Sail source | branch `spike/v030-piso` on main `765a3866`; release `sail`, `sail-z3`, `sail-conformance` built there |
| Verus | stock `0.2026.10.18.7d82994` (plain arm, conformance oracle); Sail's Verus is the pinned fork runtime |
| Tools | `scripts/spike-tune-rank.py` (rank, compare), `scripts/spike-tune-screen.sh` (warm-cache screen), `scripts/perf-real.sh` with `SAIL_PERF_SOURCE_PATCH` (timed rows) |
| Raw evidence | `screen-anvil.tgz` sha256 `944bd34d668bac2d4857607b6172a9a8b062e8832027c48bc39b1b42817645db`; `final-anvil-q3.tgz` sha256 `c262b4f10b7417232cbeabfc75b489dfa1bdc5eb54476355ec12d155c4bf27bb`; IRONKV_EVIDENCE (outside the repository) |

## Anvil (`--lib`, `--verify-module vdeployment_controller`)

**Ranking.** A Sail cold result ranks
`proof::liveness::resource_match`, where the perf-real `body-early` (line 23)
and `body-late` (line 2025) edits live. Its shared bucket costs 63.9 s over
9 functions. Ten other functions in the module already carry
`spinoff_prover` upstream. The biggest shared functions are L818 (20.9 s),
L711 (16.4 s), L1106 (9.0 s) and L538 (8.8 s). L711 was ruled out: spinning
it off gave "rlimit exceeded" on stock in the #244 experiment.

**Screen** (one owned copy, one warm cache, each variant then two one-line
sibling edits; `spike-tune-screen.sh`):

| Variant (spun-off lines) | Edit at L23 | Edit at L2025 | Note |
|---|---:|---:|---|
| none | 86.7 s | 88.0 s | |
| `p1` 818 | 124.3 s | 113.4 s | L1106, still shared, rose from 15.5M to 72.5M rlimit (9 s to 50 s) |
| `r1` 818, 538 | 79.8 s | 80.1 s | L711 rose to 92% of its limit |
| `q3` 818, 1106, 538 | 54.5 s | 53.8 s | L1106 alone: 133.7M of 150M (89%), 96–107 s |
| `q4` 818, 1106, 538, 649 | 52.3 s | 52.4 s | the same L1106 cost |

In every variant, every function kept its verdict. The effect of isolation
is not local, though. Removing one function from a shared process changes
what the remaining functions cost, because Z3's incremental state from
earlier queries helps or hurts later ones. Isolating L818 alone made the edit
slower. Each set must therefore be measured, not predicted from recorded
costs.

**Timed rows**, `q3` against unpatched source. Six rounds; each round runs
four arms (stock cold unpatched and patched, Sail unpatched and patched), and
the arm order rotates each round. Sail arms: cold, one comment-only edit,
`body-early`, `body-late`, then a comment-only edit at the end. Each arm uses
a fresh copy and an empty cache. Median [range], n = 6 each:

| Row | Unpatched | `q3` | Change |
|---|---|---|---:|
| Stock cold | 190.4 s [184.7–193.8] | 189.0 s [187.2–192.2] | −0.7% |
| Sail cold | 183.7 s [180.7–186.0] | 184.5 s [181.2–190.2] | +0.4% |
| Sail comment-only edit (floor) | 34.7 s [34.4–35.4] | 34.8 s [34.6–35.5] | +0.3% |
| Sail `body-early` | 88.5 s [86.6–89.3] | 54.4 s [54.1–54.8] | −38.5% |
| Sail `body-late` | 89.2 s [87.1–90.5] | 54.3 s [53.6–54.5] | −39.2% |

Every run exited 0. All 12 stock runs reported 144 verified, 0 errors.
Comparing Sail's cold results, unpatched against patched, shows 0 verdict
changes in every round.

STOCK7

**Conformance on the patched source.** `sail-conformance run --tier standard
--project anvil` ran on the corpus source with the `q3` patch, in three arms:
pass-through, cache fill and cache warm. Each arm: 1 unit **Same**,
143 functions compared, 0 different, 0 unstable. Each arm reports rlimit
drift for 10 functions in `proof::liveness::spec`, none of them in the
patched module. A pass-through control on the unpatched source gives
the same verdict and the same 10 drift entries, with identical values. The
drift therefore comes from the fork-against-stock pairing, not from the
patch.

**Result.** `q3` passes all three bars on this host: 0 differences, sibling
edit 54 s (bar 60 s), and cold within 1% for both stock and Sail. Its cost
is proof headroom. L1106 goes from 10% to 89% of its own rlimit, and Sail now
flags it as near its limit. Its process alone takes 96–107 s. That is close
to the 101–108 s composition query that already bounds cold on 32 threads.
The extra solver work (about 118M more rlimit for L1106) is hidden only
because this host has spare threads.

## IronKV (`ironsht`, whole crate)

IRONKV

## Decision for v0.3.0

DECISION

## Reproduction

```sh
# Unpatched Sail round (JSON cold result for ranking):
SAIL_PERF_OUT=~/sp/b-anvil SAIL_PERF_MODES=sail SAIL_PERF_EDITS="body-early body-late" \
  SAIL_PERF_NOOPS=1 scripts/perf-real.sh run anvil 1
python3 scripts/spike-tune-rank.py rank ~/sp/b-anvil/r1/sail.cold.log \
  --source-root ~/sp/b-anvil/pristine --min-seconds 20
# Warm-cache screen with one-line sibling-edit probes:
R=src/controllers/vdeployment_controller/proof/liveness/resource_match.rs
TUNE_PROBES="e23=46 e2025=2050" scripts/spike-tune-screen.sh ~/sp/w-anvil ~/sp/b-anvil/pristine . \
  $R '--lib' '--verify-module vdeployment_controller' p1:818 r1:818,538 q3:818,1106,538
# Timed rows: perf-real.sh per arm with SAIL_PERF_SOURCE_PATCH set to the q3 patch.
```

## Attempts

- 2026-11-09, lane `v3-piso` (c7i.4xlarge, us-east-1): interrupted. The agent
  stopped and the host self-terminated; no data survived.
- 2026-11-10, first screen on `v3-piso2`: discarded. The probes inserted a
  new line, which moved the spans of every later spun-off function, so
  those functions missed. The probes now edit one line in place.
- 2026-11-10, second screen: discarded. A process left over from the first
  screen wrote to the same directory at the same time.
