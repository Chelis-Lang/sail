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
| Raw evidence | `screen-anvil.tgz` sha256 `944bd34d668bac2d4857607b6172a9a8b062e8832027c48bc39b1b42817645db`; `final-anvil-q3.tgz` sha256 `c262b4f10b7417232cbeabfc75b489dfa1bdc5eb54476355ec12d155c4bf27bb`; IronKV, conformance and 7-thread rows `rest.tgz` sha256 `d29f318a1970cfc60f3dcaeb256ad17b53c268ba13b89f134bc13d9162d55577` (outside the repository) |

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

To see whether spare threads hide the extra solver work, stock cold was
also run with `--num-threads 7`, unpatched and `q3` alternating, 3 rounds:
174.9 s [172.8–182.0] unpatched against 178.6 s [173.8–179.4] for `q3`
(+2.1%, ranges overlap, n = 3). All six runs reported 144 verified.

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
L1106 alone uses about 118M more rlimit than before. With 7 threads, stock
cold rises 2.1%, inside the bar but close to it.

## IronKV (`ironsht`, whole crate)

The perf-real edits live in `host_impl_v`. Its shared bucket costs only
2.4 s over 23 functions, and the largest module bucket in the crate,
`marshal_ironsht_specific_v`, costs 4.9 s. The screen used one-line probes
at the `body-early` (L151) and `body-late` (L1780) functions:

| Variant (spun-off lines in `host_impl_v.rs`) | Edit at L151 | Edit at L1780 |
|---|---:|---:|
| none | 7.7 s | 7.7 s |
| `i4` 1173, 1439, 1070, 656 | 6.0 s | 5.9 s |
| `i8` `i4` plus 1780, 765, 1722, 553 | 5.6 s | 5.2 s |

No verdict changed, and no check came near its limit (largest: 9%).
`i8` isolates the L1780 edit target itself, so only `i4` was timed. Timed
rows, `i4` against unpatched source, use the same six-round rotated protocol
(median [range], n = 6):

| Row | Unpatched | `i4` | Change |
|---|---|---|---:|
| Stock cold | 44.6 s [44.2–45.5] | 44.4 s [44.2–45.2] | −0.5% |
| Sail cold | 42.1 s [42.0–42.2] | 42.1 s [42.1–42.9] | +0.2% |
| Sail comment-only edit (floor) | 4.8 s [4.7–5.1] | 4.9 s [4.8–5.0] | +0.8% |
| Sail `body-early` | 7.2 s [7.2–7.3] | 5.6 s [5.5–5.7] | −22.5% |
| Sail `body-late` | 7.2 s [7.2–7.3] | 5.6 s [5.6–5.6] | −22.7% |

Every run exited 0. Every stock and Sail cold run reported 313 verified
(and 2,059 for vstd), 0 errors. Conformance on the `i4` source
(`--project verified-ironkv`, pass-through, cache fill, cache warm): each arm
1 unit **Same**, 328 functions compared, 0 different. Each arm reports rlimit
drift for 23 functions in `host_protocol_t`, `SingleDelivery` and
`single_delivery_t`. An unpatched pass-through control gives the identical
23 entries. Isolation saves 1.6 s per edit
here. The edit was already within 2.4 s of the 4.8 s floor.

## Decision for v0.3.0

**`sail tune` (rank, propose spinoff, validate in an owned copy, emit a
patch) is not built for v0.3.0.**

- Ranking cannot pick the set. The top-ranked function (L818) made the edit
  40% slower on its own. The second (L711) cannot be isolated at all, because
  its own process exceeds its rlimit. The passing set came from screening
  sets. Each candidate costs one module re-verification plus edit probes,
  and the remaining functions' costs change with every set. A tool would be a
  search over sets with full validation per candidate, not a ranking.
- The plan requires each proposal to state its rlimit headroom. The only
  passing Anvil set moves L1106 from 10% to 89% of its limit. Any sensible
  headroom gate (no new near-limit flag, or a 50% to 80% ceiling) rejects it.
  Accepting it trades proof robustness for edit latency. That is the
  project author's call, made with the numbers in hand, not a patch a tool
  should propose.
- The gain is concentrated. In the evaluated corpus, only Anvil's
  `resource_match` shows a large gap. IronKV's heaviest module leaves at most
  about 2 s per edit.

**`--isolation auto` has no case.** Isolation changes the solver input. Its
effects run in both directions: L711 fails when isolated, and isolating L818
made an edit slower. Neither the verdict nor the speed of an automatic
isolation can be known without running it. As an opt-in mode it would need
separate cache keys and a row in `docs/divergences.md`, all for one project's
benefit.

What remains: the [performance guide](../performance.md) describes the
manual technique and its headroom risk. The spike scripts stay available
for anyone measuring it. Making a shared-process miss cheaper (#158) still
requires reusing a solver state without replaying the prefix. Isolation does
not address that.

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
