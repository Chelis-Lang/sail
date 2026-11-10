# Spike: hand-applied prover isolation for changed-proof cost (#158, #244)

**Status: spike complete; `sail tune` is dropped for v0.3.0 and
`--isolation auto` has no case.** On Anvil one three-function isolation met
the timing and conformance bars but moved one proof from 10% to 89% of its
rlimit. IronKV has almost no changed-proof gap.

**Question.** Sail keys default-mode replies by the whole solver-process
prefix (ADR 0008), so after a miss every later query in a module's shared
process is re-solved, and an edit to one function costs about the whole
bucket. Does adding `#[verifier::spinoff_prover]` by hand to a module's
dominating functions make a one-function sibling edit cheap, with stock and
Sail agreeing on the patched source?

**Pass rule.** Set in the owner-approved v0.3.0 plan of 2026-11-09 (kept
outside the repository; ADR 0109 records its pillars and the spikes-first
rule) and written before the spike ran: 0 verdict or diagnostic differences
between stock and Sail on the patched source, sibling edit at most 60 s, cold
at most 3% slower than unpatched. The plan's `sail tune` design also required
each proposal to state its rlimit headroom.

| Field | Value |
|---|---|
| Environment | Native EC2 c7i.8xlarge (32 vCPU), us-west-2, Ubuntu 24.04, one idle lane; pinned Z3 4.16.0 |
| Sail | release `sail`, `sail-z3`, `sail-conformance` built from main `d45e75eb` (the spike adds no Rust code) |
| Verus | stock `0.2026.10.18.7d82994` (plain arm and oracle); Sail uses the pinned fork runtime |
| Tools | `scripts/perf-real.sh` with `SAIL_PERF_SOURCE_PATCH` (timed rows). The ranking and warm-cache screen prototypes are not kept on `main`; they are in PR #709's history (`git fetch origin pull/709/head`, commit `ebd72643`): `scripts/spike-tune-rank.py` blob `35cbad32`, `scripts/spike-tune-screen.sh` blob `7fd95afc` |
| Raw evidence (outside the repository) | `screen-anvil.tgz` sha256 `944bd34d668bac2d4857607b6172a9a8b062e8832027c48bc39b1b42817645db`; `final-anvil-q3.tgz` sha256 `c262b4f10b7417232cbeabfc75b489dfa1bdc5eb54476355ec12d155c4bf27bb`; IronKV, conformance and 7-thread rows `rest.tgz` sha256 `d29f318a1970cfc60f3dcaeb256ad17b53c268ba13b89f134bc13d9162d55577` |

## Anvil (`--lib`, `--verify-module vdeployment_controller`)

The perf-real `body-early` (L23) and `body-late` (L2025) edits live in
`proof::liveness::resource_match`, whose shared bucket costs 63.9 s over 9
functions. Its largest shared functions are L818 (20.9 s), L711 (16.4 s),
L1106 (9.0 s) and L538 (8.8 s). L711 was excluded: isolated, it exceeds its
rlimit on stock (#244).

Warm-cache screen, one-line sibling edits; every function kept its verdict:

| Spun-off lines | Edit L23 | Edit L2025 | Note |
|---|---:|---:|---|
| none | 86.7 s | 88.0 s | |
| `p1` 818 | 124.3 s | 113.4 s | L1106, still shared, rose from 15.5M to 72.5M rlimit |
| `r1` 818, 538 | 79.8 s | 80.1 s | L711 at 92% of its limit |
| `q3` 818, 1106, 538 | 54.5 s | 53.8 s | L1106 alone: 133.7M of 150M (89%), 96–107 s |
| `q4` `q3` plus 649 | 52.3 s | 52.4 s | same L1106 cost |

Isolation is not local: removing a function from a shared process changes
what the rest cost, so each set must be measured.

Timed rows, `q3` against unpatched: six rounds of four arms in rotated order,
fresh copy and empty cache per arm; median [range], n = 6:

| Row | Unpatched | `q3` | Change |
|---|---|---|---:|
| Stock cold | 190.4 s [184.7–193.8] | 189.0 s [187.2–192.2] | −0.7% |
| Sail cold | 183.7 s [180.7–186.0] | 184.5 s [181.2–190.2] | +0.4% |
| Sail comment-only edit (floor) | 34.7 s [34.4–35.4] | 34.8 s [34.6–35.5] | +0.3% |
| Sail `body-early` | 88.5 s [86.6–89.3] | 54.4 s [54.1–54.8] | −38.5% |
| Sail `body-late` | 89.2 s [87.1–90.5] | 54.3 s [53.6–54.5] | −39.2% |

Every run exited 0; all 12 stock runs reported 144 verified, 0 errors. Stock
cold at `--num-threads 7` (n = 3): 174.9 s unpatched against 178.6 s for
`q3` (+2.1%, ranges overlap).

Conformance: `sail-conformance run --tier standard --project anvil` on the
`q3` source, pass-through, cache fill and cache warm: each arm **Same**, 143
functions, 0 different, 0 unstable. Each arm reports rlimit drift for the same
10 functions in `proof::liveness::spec` (outside the patched module) as an
unpatched pass-through control, with identical values.

## IronKV (`ironsht`, whole crate)

The edits live in `host_impl_v`, whose shared bucket costs 2.4 s over 23
functions; the largest bucket in the crate is 4.9 s. Isolating L1173, L1439,
L1070 and L656 (`i4`), six rotated rounds, median, n = 6:

| Row | Unpatched | `i4` | Change |
|---|---|---|---:|
| Stock cold | 44.6 s | 44.4 s | −0.5% |
| Sail cold | 42.1 s | 42.1 s | +0.2% |
| Sail floor | 4.8 s | 4.9 s | +0.8% |
| Sail `body-early` | 7.2 s | 5.6 s | −22.5% |
| Sail `body-late` | 7.2 s | 5.6 s | −22.7% |

Every run exited 0 and every cold run reported 313 verified, 0 errors; no
check exceeded 9% of its limit. Conformance (`--project verified-ironkv`,
three arms): **Same**, 328 functions, 0 different; the 23 rlimit-drift
entries match an unpatched control exactly.

## Decision

- **Ranking cannot pick the set.** Isolating the top-ranked function (L818)
  alone made the edit 43% and 29% slower (124 s and 113 s against 87 s and
  88 s); the second (L711) cannot be isolated at all. The passing set came
  from screening sets, each costing a module re-verification, so a tool would
  be a validated search over sets, not a ranking.
- **The only passing set fails any headroom gate.** `q3` moves L1106 from 10%
  to 89% of its rlimit, which a no-new-near-limit or 50–80% ceiling rejects.
  Trading proof robustness for edit latency is the project author's call.
- **The gain is concentrated.** Only Anvil's `resource_match` shows a large
  gap; IronKV leaves under 2 s per edit.
- **`--isolation auto` has no case.** Isolation changes the solver input and
  its verdict and speed effects run both ways (L711 fails, L818 alone is
  slower), so it would need separate keys and a divergences row for one
  project's benefit.

The [performance guide](../performance.md) documents the manual technique and
its headroom risk. Making a shared-process miss cheaper (#158) still needs
solver-state reuse without prefix replay.

## Attempts

- 2026-11-09, c7i.4xlarge us-east-1: interrupted; agent stopped, host
  self-terminated, no data.
- 2026-11-10, first screen: discarded; probes inserted a line, moving later
  spun-off spans so they missed. Probes now edit one line in place.
- 2026-11-10, second screen: discarded; a leftover process from the first
  screen wrote to the same directory.
