# Fork branch perf/7d82994: macro-profile names, dispatch order, Fx hashing

**Status: Linux and macOS runtimes `f8589ed4` published and pinned (`engine-runtime-f8589ed4-linux`, `-macos`); Linux and macOS bundles re-recorded and instant revert requalified on both; conformance gates pending.** Fork branch
`perf/7d82994` adds three commits to `9c0ca7e7` (Verus `0.2026.10.18.7d82994`,
Rust 1.99.1): `912052eb` (VI-0033, issue #247), `1a892f97` (issue #244) and
`51afcba6` (issue #248). Selecting any of them needs a fresh matching runtime,
vstd and exports, and the owner's decisions on the proposed notes in ADRs
0039, 0049 and 0056.

| Field | Value |
|---|---|
| Environment | Native EC2 c7i.8xlarge (32 vCPU), us-east-1, Ubuntu 24.04, one idle lane; each commit built there with `vargo build --release`; pinned Z3 4.16.0 |
| Sail source | `c4cb6076` (main) scripts: `perf-real.sh`, `verus-determinism.py` helpers |
| Raw evidence | `fperf-evidence2.tgz`, sha256 `04d8f6800741dcb9212100884502e4de0a491345ad84962d6e932a10e3099c46` (outside the repository) |

## Fork tests (local Linux x86-64, a fresh clone per commit)

Every commit: build; `rust_verify`, `vir` and `air` library tests (17, 13 and
144 passed); every `sail_vi*` test plus `basic` (90 passed on `912052eb`,
93 from `1a892f97`; 1 ignored); then the release-style vstd build (2,059
verified, 0 errors). The full `rust_verify_test` suite on `51afcba6`: 4,833
passed, 63 ignored, 2 failed: `cargo_new_builds` and `cargo_new_verifies`,
which fail identically on `9c0ca7e7` (VI-0018, source builds).

## Solver input, verdicts and determinism (fast tier, 147 direct units)

Each unit ran with the conformance flags and a tee solver wrapper; stdout is
compared with times and build identity blanked.

| Comparison | Result |
|---|---|
| `912052eb` vs `9c0ca7e7` | 126 of 147 units identical; 21 differ in solver input only (closure/alias renaming); stdout, stderr and exit codes identical in all 147 |
| `1a892f97` vs `912052eb` | 147 of 147 identical (763 sessions) |
| `51afcba6` vs `1a892f97` | 147 of 147 identical (763 sessions) |
| `51afcba6`, 3 runs | 147 of 147 deterministic |

The renaming check over every example's `--log smt` output (`9c0ca7e7` vs
`912052eb`) found 335 files identical and 20 equal up to the renaming. In
3 files the two closures' `declare-sort` lines also swap places, because
they are emitted in name order. Anvil (`vdeployment_controller`,
31 threads): the 38 session digests are identical across `912052eb`,
`1a892f97` and `51afcba6`, all with 144 verified and the same `func-details`.

## VI-0033: proc-macro opt-level (Vest, plain `cargo verus verify`)

| Build | opt 0 vs opt 3 | opt 0 rerun |
|---|---|---|
| `9c0ca7e7` | 103 of 631 sessions differ | identical |
| `912052eb` | identical | — |
| `51afcba6` | identical | identical |

Every arm reports 3,236 verified and the known `parse_cbor_with_child`
rlimit error ([compatibility](../compatibility.md)).

## #244: dispatch order (wall time of one re-verification after a comment edit)

| Project | Threads | Stock order median [range] | Cost order median [range] | n |
|---|---:|---|---|---:|
| Anvil | 7 | 142.6 s [141.7–144.9] | 140.0 s [139.8–140.4] | 3 |
| Anvil | 15 | 141.9 s [141.1–146.4] | 145.9 s [140.2–150.3] | 6 |
| Anvil | 31 | 144.9 s [142.9–151.9] | 147.9 s [143.6–153.1] | 6 |
| Vest | 7 | 41.1 s [40.9–41.5] | 34.9 s [34.8–35.4] | 3 |
| Vest | 15 | 36.5 s [36.3–36.8] | 33.0 s [32.9–33.0] | 3 |

Paired rounds alternate arm order. On this runtime Anvil's makespan is one
bucket. Its single query `lemma_new_vrs_replicas_diff_never_increases`
takes about 101 s, and both orders start that bucket within 2 s. The 426d8b0-era model's
gain (#244) therefore does not apply. At 15 and 31 threads the cost order
lengthens that bucket by up to 8 s in some rounds (contention with the other
large buckets, now started together); the ranges overlap.

## #248: all-hits floor (fork-shim, 31 threads, three noop reruns per round)

| Project | `1a892f97` median [range] | `51afcba6` median [range] | n |
|---|---|---|---:|
| Vest | 32.42 s [32.19–32.73] | 31.36 s [31.22–31.74] | 9 / 9 |
| ironkv | 4.39 s [4.33–4.68] | 4.34 s [4.27–4.41] | 9 / 9 |

`perf` self time on one Vest all-hits run: SipHash 22.9% of samples on
`1a892f97`, 2.9% on `51afcba6` (Fx 8.8%; the rest is the path and node
hashing itself).

## R3 runtime candidate (fork `f8589ed4`)

The three commits rebased onto the span-remap fork `64baccbb`
(`bd79859c`, `ee8dd97d`, `793468ea`, plus `f8589ed4` updating SAIL.md),
fork branch `candidate/7d82994-r3`. Every commit: build, the library tests,
`sail_vi*` plus `basic` (268, then 271 passed; 1 ignored), the `dbg!`
location guard (37 of 37) and the release-style vstd build (2,059 verified).

| Field | Value |
|---|---|
| Build | `cargo xtask build-verus-engine` on a fresh native c7i.8xlarge (us-east-1), Rust 1.99.1 without `rust-src`, `RUSTUP_HOME`, `CARGO_HOME` and `SAIL_CACHE_DIR` under `/tmp/sail-engine-f8589ed4` |
| Variant | `7d829946231d6413a398f6e4160cdfe951261dad-81186034d2e137a2`; features `sail-fork:f8589ed4…`, `source-lock:04efc53f…`, `vstd-build:cargo-verus-release` |
| Manifest / archive | `576558fc76eecfdd91908fb85e30916e9d78b0153a144041f5b03937e4bcde26` / `93ea60172527a25dc9b433387c82924740c6185fb57aa60554ce501cf4e8f71c` |
| `test-engine.py` | 11 of 11 fixtures match (raw SMT of `recursive` differs from stock, as ADR 0054 renames it) |
| `test-cancel.py` | no seal; the restart matches stock |
| Determinism | fast tier, 3 runs: 147 of 147 units deterministic (763 sessions); session digests identical to `51afcba6`'s in all 147; one stderr differs (`basic_failure.rs` now prints `seq.rs:980:12`, stock's form, from the release-style vstd) |
| vstd.vir | 0 `rustlib/src/rust` paths, 10,199 `/rustc/58cf8f9c…` spans, 0 `/home/`, 511 build-root spans |
| Strings scan | every archive file: no `/home/`, `/Users/`, host name or account id; only false positives (version strings, `Z3 4.16.0.0`) |
| Raw evidence | `r3-evidence.tgz`, sha256 `045a6e7f88396e8a6719f116ca01eb791c94802e1fbd714fba105b708da41d93` |

Published as prerelease `engine-runtime-f8589ed4-linux`; the anonymous download matches the archive digest. The Linux shipped vstd bundle is re-recorded
([record](vstd-compile-boundary.md#verus-7d82994-bundle-on-runtime-f8589ed4))
and Linux instant revert requalified
([record](instant-revert.md#runtime-f8589ed4-requalification)) on this
runtime, and so are the macOS bundle and instant revert
([record](instant-revert.md#macos-qualification-on-runtime-f8589ed4)).
Pending: the conformance tiers.

## Attempts

- 2026-10-20, R3 build: `build-verus-engine` stopped twice before building (pinned Verus not fetched; `unzip` missing; cache directory mismatch), then passed.
- 2026-10-20, R3 determinism pass 1: wrong corpus path (exit 1, no units run); rerun above.
- 2026-10-20, profiling pass 1: `perf record` refused (`perf_event_paranoid`); rerun after setting it, results above.
