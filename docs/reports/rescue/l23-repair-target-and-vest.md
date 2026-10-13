# L23: one repair target, and Vest's M7 gap

- **Lane:** L23, rescue. **Date:** 2026-10-13.
- **Brief:** two agent-loop costs. First, the per-function Cargo targets that
  made each new repair function verify vstd again (the L19 report's "third
  vstd cost"). Second, Vest's M7 gap, through the L18 report's cheaper items.
- **Answer.**
  - *Repair targets: fixed.* Repair functions now share one Cargo target per
    workspace and compiler identity, as plain `cargo verus` shares one target
    directory. A function's first repair dropped from 40.7 s to 8.9 s on
    HumanEval and from 61.4 s to 29.6 s on Vest. A second new function's first
    repair dropped from 38.2 s to 6.5 s and from 61.9 s to 29.8 s. Every
    repair used one driver and was Current. ADR 0036 is amended.
  - *Vest gate: not met, and the cheap items cannot meet it.* Rustc
    incremental is already on, for plain `cargo verus` and Sail alike. Giving
    Verus's verify pass its own incremental cache, the one untried variant,
    measured no gain. The three-round benchmark gives HumanEval 2.11× (pass)
    and Vest 1.80× (fail, `SAIL7109`). Under ADR 0038 the Vest speed claim
    should be re-scoped rather than met with new mechanism.

## Environment

- **Host.** AWS EC2 `c7i.8xlarge` (32 vCPU), us-east-1, Ubuntu 24.04. Native
  x86-64, launched with `scripts/perf-host.sh up l23 c7i.8xlarge` and torn
  down after use.
- **Container.** CI image `sail-ci:679d9641401c264c`, built natively. Engine
  runtime `d2b69aea`, the committed pin, installed with `sail engine install`.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0, Rust 1.98.1.
- **Sail.**
  - Before: `rescue/m7` at `ca26325`, release `sail` sha256 `6bc5353f…`.
  - After: `25fc7b0`, which is this branch's `e34b83f` before its rebase onto
    a docs-only commit. Release `sail` sha256 recorded in the bench log.
- **Projects.** `sail-bench repair prepare` copies of human-eval-verus and
  Vest, unedited.

## 1. One Cargo target per compiler family

### Before and after

Each sequence ran in a fresh cache with its own `sail serve`, without
`SAIL_MANAGED_FORCE`, as an agent runs Sail:

1. `sail verify --engine interactive --incremental --output json --package P --lib`;
2. `sail repair-check --engine interactive --function F --output json --package P --lib`
   for F1, F1, F2, F2, F1;
3. the step 1 command again.

F1 and F2 are `compute_gcd` and `concatenate_impl` on HumanEval, and
`lemma_from_base128_push` and `lemma_pow128_succ` on Vest. Wall time in
seconds; "drivers" counts verifier invocations.

| Step | HumanEval before | HumanEval after | Vest before | Vest after |
|---|---:|---:|---:|---:|
| engine verify, cold | 56.2 (2 drivers) | 56.3 (2) | 75.5 (2) | 75.3 (2) |
| **F1 first repair** | **40.7 (2, vstd run)** | **8.9 (1, vstd reused)** | **61.4 (2, vstd run)** | **29.6 (1, vstd reused)** |
| F1 second repair | 6.4 (1) | 6.5 (1) | 29.4 (1) | 29.6 (1) |
| **F2 first repair** | **38.2 (2, vstd run)** | **6.5 (1, vstd reused)** | **61.9 (2, vstd run)** | **29.8 (1, vstd reused)** |
| F2 second repair | 6.5 (1) | 6.5 (1) | 29.8 (1) | 29.8 (1) |
| F1 third repair | 6.4 (1) | 6.4 (1) | 29.7 (1) | 29.8 (1) |
| engine verify, unchanged | 0.6 (0) | 6.2 (1) | 1.8 (0) | 27.5 (1) |

- Every result was Current and every repair was accepted, before and after.
- After the change, an agent repairing *n* functions no longer pays about
  *n* × 32 s. The benchmark's own warm-ups show the same: the first repair of
  `concatenate_impl` and `strlen` took 6.7 s and 6.5 s, and Vest's second and
  third functions 30.9 s and 30.8 s.
- **One regression.** An unfunctioned engine verify after repairs, with
  unchanged source, now verifies the selected crate again (last row). A
  repair forces the selected crate fresh in the shared target, and the
  earlier context's receipt no longer matches the rebuilt bytes. Why the
  rebuilt bytes differ was not investigated. After an edit, which is the
  usual agent step, the crate is verified in either layout. This is in
  [known issues](../../known-issues.md).

### What changed

- **One target.** `prepare` no longer gives a repair function its own
  `repair-targets-v1/<context>` target or `repair-observations-v1`
  observations. Every proof context of a family builds in
  `compiler-artifacts/<family>/target`.
- **VI-0005 check.** The function no longer reaches vstd's fingerprint: in
  the after sequence, Cargo reported vstd fresh for the second function, and
  vstd's dep-info on the pin declares only `VERUSDOC` (as L19 found).
- **Receipts.** Sharing the target alone does not help. Cargo reports vstd
  fresh, but the new context has no receipt for it, so the existing retry
  cleaned vstd and verified it again. A Cargo-fresh *dependency* may now be
  accepted on a receipt from another proof context of the same family. That
  receipt must pass every existing check (commit marker, context, input
  revision, declared environment, artifact bytes). Selected targets still
  need their own receipt. Families list their contexts under
  `compiler-artifacts/<family>/contexts/`. The reused target names the
  establishing run.
- **Lock.** The family lock serialises the target. A context determines its
  family, so the context lock was redundant and is gone.
- **Simplification (L13 audit #4).** The change is 168 lines shorter net:
  the layout branch, the second lock and the legacy-migration and
  per-selector byte-isolation tests go; sibling lookup and its tests come in. The negative generation stays: L19
  showed that removing it costs 27 s per failure→pass cycle.
- **Tests.** `repair_functions_share_one_family_target_and_keep_private_proof_paths`
  checks the shared layout, membership and serialisation.
  `sibling_dependency_receipt_needs_identical_fresh_bytes_and_never_covers_a_selected_target`
  uses real Cargo: a sibling's receipt is reused for a fresh dependency, and
  refused for a selected target, after an artifact byte change, for a context
  outside the family, and after Cargo rebuilds the dependency because its
  build inputs changed.

### Correctness

- Verdicts, diagnostics and SMT are unchanged (rule 1). Cargo's fingerprints
  decide rebuilds exactly as in plain `cargo verus`.
- Reuse stays content-addressed (rule 6): a sibling receipt applies only to
  the exact artifact bytes and input revision it recorded.
- Provenance is kept (rule 7): a reused dependency reports the run that
  established it.
- Arguments that change results are in the family digest, so contexts with
  different arguments never share a target. The function is excluded because
  it does not affect a dependency's full verification.

## 2. Vest's M7 gap

### Rustc incremental

- **Plain `cargo verus` already uses it.** Cargo passes `-C incremental` for
  path packages in the dev profile. Neither cargo-verus nor rust_verify
  removes it, and Sail's managed targets have the same
  `debug/incremental` directories. Sail does not disable it.
- **Its effect today.** Vest, plain `cargo verus verify`, `base128-body` edit
  and revert, three rounds: the compile pass took 3.24–3.50 s with
  incremental and 3.82–4.02 s with `CARGO_INCREMENTAL=0`. So incremental
  already saves about 0.5 s per Vest cycle.
- **The verify pass gets nothing.** With `-Zincremental-info`, Verus's verify
  pass reports "completely ignoring cache because of differing commandline
  arguments" on every run. Both passes share one incremental directory, and
  the verify pass adds `--cfg verus_keep_ghost_body`. The compile pass does
  reuse its cache.
- **A separate cache does not help.** A source build of the pinned commit,
  patched so the verify pass uses its own incremental directory (experiment
  only, not committed or pinned), against the stock release, four rounds:
  - Vest verify pass `init-and-types` 3.06–3.09 s with the patch, 3.06–3.10 s
    without; cycle wall 36.1–36.8 s in both arms;
  - HumanEval 1.03–1.08 s and 1.02–1.09 s.

  The verify pass's front-end cost is not reused by rustc incremental.
- **Conclusion.** No simple incremental win is left. No fork change is
  proposed.

### Other items

L18 measured these shares of the median Vest cycle:

| Item | Share | Status |
|---|---:|---|
| rustc incremental on the compile pass | ≤ 12.6% | already on; nothing further (above) |
| Cargo bypass | ≤ 7.1% (1.72 s) | new mechanism; not attempted (ADR 0038) |
| Sail overhead | 7.2% (1.74 s) | not attempted; even removing all of it is short of the gap |

Vest needs about 2.5 s per cycle on L19's host, and 2.9 s per cycle on this
host (M7 at most 132.8 s against 147.3 s). Only the Cargo bypass combined
with most of Sail's overhead could reach that, and the bypass is new
mechanism. **Recommendation:** re-scope the M7 speed claim for Vest under
ADR 0038, recording 1.80× with this profile, and leave further speed to M8.

### Repair benchmark (`sail-bench repair`, 3 rounds)

M7 is `25fc7b0`, M6 is `26f124f`. Arm order alternated each round. All 60
samples were valid, every M7 sample used one driver and was Current, and
every pair's outcomes matched.

| Catalogue | M6 whole loop, median (rounds) | M7 whole loop, median (rounds) | Speedup | Gate |
|---|---:|---:|---:|---|
| HumanEval | 68.1 s (70.9, 68.1, 68.1) | 32.3 s (32.7, 32.3, 32.3) | **2.11×** | pass |
| Vest | 265.5 s (267.5, 265.5, 262.4) | 147.3 s (151.1, 147.3, 146.8) | **1.80×** | fail (`SAIL7109`) |

Per-cycle medians, M6 / M7 in seconds:

| HumanEval | M6 | M7 | Vest | M6 | M7 |
|---|---:|---:|---|---:|---:|
| gcd-body | 14.1 | 6.7 | base128-body | 55.1 | 30.4 |
| concatenate-spec | 14.1 | 6.7 | pow128-spec | 56.1 | 31.1 |
| strlen-refactor | 14.0 | 6.7 | base128-refactor | 55.5 | 30.6 |
| strlen-failing-attempt | 11.9 | 5.6 | pow128-failing-attempt | 43.7 | 25.1 |
| strlen-repaired-attempt | 14.0 | 6.6 | pow128-repaired-attempt | 54.8 | 30.4 |

- Both arms ran about 12% slower than in L19 (HumanEval M6 61.0 s, Vest M6
  235.4 s); the ratios match L19 (2.14×, 1.81×). The host-to-host spread was
  not investigated.
- The benchmark warms every function, so the target change does not move
  its measured samples.

Result file: `bench/results/repair-acceptance-l23.json`, sha256
`2f97b35e…5ea4e`. The [benchmarks page](../../benchmarks.md) is regenerated.

## Gates

On the host above in the CI image, plus `ci/run.sh host` on macOS. `solver`,
`e2e` and `conformance` ran at `e34b83f` plus this report's documentation;
`gate` ran on the final tree, whose code is identical.

| Step | Result |
|---|---|
| `ci/local.sh gate` | Pass: 534/534 tests, including both new tests; links, layering, deny, benchmarks page |
| `ci/local.sh solver` | Pass: 35/35 |
| `ci/local.sh e2e` | Pass: legacy, managed and repair phases |
| `ci/local.sh conformance fast pr` | Pass: 33/33 same, 0 different, in each of the three phases, and engine 33/33 same |
| `ci/run.sh host` (macOS) | Pass: clippy and 511/511 tests |

The first `gate` attempt failed only its link check: the report was not yet
in the synced tree.

## Documentation changed

ADR 0036 (amendment and status), `CHANGELOG.md`, `docs/proof-repair.md`,
`docs/managed-cold-start.md`, `docs/workspace-hygiene.md`,
`docs/known-issues.md`, `docs/validation/m7.md` (Gate 7),
`docs/ROADMAP.md`, `docs/benchmarks.md` (generated) and this report.

## Evidence

Raw files are in `~/sail-raw-evidence/l23/` on the lead's machine.
`SHA256SUMS` covers every file.

| File | Contents | sha256 |
|---|---|---|
| `l23-sequences.tar.gz` | Before/after repair sequences: every JSON result and stderr | `eb66d4c2c0723ac07a33cdaa27daa15db298996489bc16876c291de1cc276127` |
| `l23-incremental.tar.gz` | Incremental on/off, `-Zincremental-info` and split-cache runs: Cargo and Verus output | `590ce6cc597c5d3351c2c7b6bad1b15b36537068bfdec36304d1a1d70e5e62d0` |
| `l23-bench-out.tar.gz` | `sail-bench repair` raw output (commands, stdout, stderr, journal pages; caches excluded) | `cd6a96eb645191b6dd96d216b5b739b0685dc10f5c0f8fb4b48f642a8f1d591a` |
| `repair-acceptance-l23.json` | Benchmark result, also committed under `bench/results/` | `2f97b35ebb1a2a48c54110751accf2a9fa011c1abbd487cb19e3ed5cf5f22028` |
| `l23-scripts.tar.gz` | Drivers: container wrapper, sequences, incremental experiments, patched-Verus build, benchmark | `def37e37d9c1d8d8f491d683a16a705dc92fcca487bdb9ca3ff685322d543640` |
| `before-exp.log`, `after-exp.log`, `split.log`, `incr-info.log`, `bench.log` | Run summaries | see `SHA256SUMS` |
| `gate-*.log` | Gate logs | see `SHA256SUMS` |
