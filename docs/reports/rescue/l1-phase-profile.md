# Rescue L1: native phase profile of the M6 and M7 repair loops

Status: diagnostic measurement only. Nothing here is acceptance evidence. No check, catalogue or
harness was changed.

## Environment and identities

- **Host.** AWS EC2 `c7i.4xlarge` (16 vCPU Xeon Platinum 8488C, 30 GiB) in `us-east-1c`, Ubuntu
  24.04 (kernel 7.0.0-1014-aws), Docker 29.1.3.
  - Native x86-64; nothing ran under emulation.
  - Brought up with `scripts/perf-host.sh up l1p`, and down after use.
- **Container.** All runs used the CI image `sail-ci:8e24fb5f7f490c01`, built natively on the host.
  Scratch space, caches and output were on an ext4 Docker volume, with no bind mount.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0 and Rust 1.98.1.
- **Engine.** `/opt/sail-engine` = `426d8b01…-c77e1a1879302e06`; it reports as `426d8b0.dirty`,
  release.
- **M6 binaries** from `0fde777`: `sail` `1977c0f8…ecf392a`, `sail-z3` `d5ee9e4f…15768ad`.
- **M7 binaries** from `541d3e7`, the `rescue/m7` HEAD, which includes e95b8bd: `sail`
  `19db6a47…6187fc`, `sail-z3` `813bd602…756c9c`. Shim sources are identical between `0fde777` and
  `541d3e7`.
- **Workspaces** come from the unchanged `scripts/m7-repair-bench.py prepare`, using the frozen
  catalogue (`6ffc1297…`).

## Method

A throwaway driver (`profile.py`, not committed; hash below) mirrors the harness:
- **Isolation.** Each arm has a private daemon, cache and scratch copy, with `SAIL_MANAGED_FORCE=1`
  set.
- **Warm-up (not timed).** One full run per arm, then each function's focus or repair context.
- **Measured cycles.** Restore the originals, apply the edit, then run each arm, alternating the arm
  order by round.

| Arm | Per cycle |
|---|---|
| M6 | `sail verify --incremental --output json --package P -- --verify-only-module M --verify-function F`, then the same command without the `--` arguments |
| M7 | `sail repair-check --engine interactive --function F --output json --package P --lib` |
| plain | `cargo verus focus -p P -- --verify-only-module M --verify-function F --time-expanded --output-json`, then `cargo verus verify -p P -- --time-expanded --output-json` (stock Z3, no cache) |
| Controls | M7 binary `sail verify --incremental [--engine interactive]` |

Data sources:
- **Phase times** come from Verus `--time` JSON in each run's `sidecars/*/verus.stdout`; the managed
  path already adds `--output-json --time`.
- **Queries served versus solved** are deltas of `sail status --json`.
- **Sail setup phases** come from the M7 `SAIL_JOB_EVENTS` timers.
- **Process structure** (pre- and post-Cargo spans, process counts) comes from separate `strace -f`
  runs (`he1`, `vest1`, `he2`).

`strace` inflated Verus-internal times by 1.5–4×. **All tables below use the untraced runs:**
- `ns-he`: `gcd-body` and `strlen-refactor`, 3 rounds, n=6 per arm.
- `ns-vest`: `base128-body` and `pow128-spec`, 2 rounds, n=4 per arm.
- The unchanged harness: HumanEval, 5 cycles × 3 rounds.

The untraced driver cleans M6's real focus target directory before each focus, outside the timed
region; anomaly 1 explains why.

Cells are median [min–max] in seconds. Thread-s is time summed over Verus's 15 threads.

## Headline (native, warm SMT cache)

| Catalogue | M6 focus | M6 full | M6 loop | M7 batch | M6/M7 | Plain focus / full |
|---|---|---|---|---|---|---|
| HumanEval, driver | 6.22 [5.93–6.30] | 7.74 [7.41–9.74] | ≈14.0 | 14.82 [13.91–16.62] | **0.94×** | 4.12 / 21.44 |
| HumanEval, unchanged harness | – | – | 58.4 per round [55.9–67.6] | 73.8 per round [70.7–74.2] | **0.79×** | – |
| Vest, driver | 23.61 [22.26–26.14] | 33.29 [31.37–33.77] | ≈56.9 | refused, 128 [123–159] | **none** | 19.44 / 42.41 |

- **Harness correctness.** All 15 harness pairs had equal full outcomes and the expected verdicts.
- **One compiler driver on HumanEval.** M7 used 1 driver in all 15 harness samples and all 12 driver
  samples.
- **The harness understates M6.** In 4 of 15 harness samples, the M6 "focus" was a 1.4 s `SAIL3107`
  failure (anomaly 1). With valid focuses, M6 is about 67 s per round, which makes the ratio 0.91×.
- **Native timing removes the emulation gap.** The emulated 387ba80 GCD pair (M7 160.6 s, M6 110.7
  s) takes about 15 s against 14 s natively.

## Phase breakdown (untraced)

Row definitions:
- **Outside Verus** = wall − the selected crate's Verus `total`. It covers Sail freshness, receipts
  and processes, Cargo, and `rust_verify` start-up and exit.
- **Post-VIR** = `total − rust.total − vir.total`. It covers SST/poly lowering, pruning, AIR, SMT
  and shim waits, and fixed per-crate checks. Those checks cost about 0.8 s on HumanEval and 4.4 s
  on Vest even for a single function.
- **SST/poly + other** = `total-verify − air − smt`, including waits on the shim.

| HumanEval | M6 focus | M6 full | M7 batch | Plain focus | Plain full |
|---|---|---|---|---|---|
| Outside Verus | 2.35 [2.28–2.38] | 2.77 [2.70–2.85] | 3.01 [2.90–3.92] | 0.47 | 0.54 |
| rustc frontend | 1.76 | 1.74 | 1.73 | 1.60 | 1.61 |
| VIR construction | 1.31 | 1.30 | 1.28 | 1.28 | 1.27 |
| Post-VIR, wall | 0.79 | 1.84 [1.73–4.12] | **8.51** [8.06–9.86] | 0.78 | 18.03 |
| Thread-s: SST/poly + other | 0.03 | 6.3 | **98** [88–105] | 0.03 | 7.4 |
| Thread-s: AIR | 0.01 | 3.6 | 3.0 | 0.02 | 7.3 |
| Thread-s: SMT, including the shim | 0.00 | 0.13 | 0.37 | 0.00 | 44.1 |
| Cache gets / hits | 11 / 11 | 4523 / 4523 | 4534 / 4534 | – | all solved |

| Vest | M6 focus | M6 full | M7 batch (refused) | Plain focus | Plain full |
|---|---|---|---|---|---|
| Outside Verus | 5.12 | 7.00 | 16.2, plus 41–56 s re-verifying vstd | 1.34 | 1.80 |
| rustc frontend | 7.88 | 8.10 | 7.89 | 7.15 | 6.97 |
| VIR construction | 6.21 | 6.19 | 6.30 | 6.36 | 6.42 |
| Post-VIR, wall | 4.39 | 11.68 | **54.3** | 4.59 | 27.22 |
| Thread-s: SST/poly + other | 0.46 | 69.6 | **650** | 0.48 | 79.6 |
| Thread-s: AIR | 0.17 | 19.1 | 17.0 | 0.16 | 33.8 |
| Thread-s: SMT | 0.00 | 1.34 | 1.86 | 0.00 | 80.1 |
| Cache gets / hits | 7 / 6.5 | 32451 / 32450 | 53469 / 53468 | – | all solved |

Real Z3 never ran in a measured warm cycle.

**Where the outside-Verus time goes** (traced runs, which are mostly single-threaded and so little
inflated):
- **Sail before Cargo** takes 1.1–1.2 s on HumanEval and 2.6–2.7 s on Vest: 3× `cargo metadata`,
  8–10 rustc probes and about 5 rustup proxies.
- **Sail after Cargo** takes 0.85–0.89 s on HumanEval and 1.5 s on Vest.
- **Cargo** takes 0.2–0.5 s on HumanEval and 0.3–1.8 s on Vest.
- **`rust_verify` start-up and exit** take about 0.4 s.

Warm setup timers sum to about 1.5 s:

`runtime-context` 0.32 s, `input-guards` 0.32 s, `owned-paths` 0.22 s, `target-cleanup` 0.14 s,
`runtime-binaries` 0.10 s and `sysroot-guard` 0.10 s. The first run after a daemon start sums to 3.4 s, adding `initial-snapshot` (0.90 s),
`compiler-family-publication` (0.64 s) and `sysroot-hash` (0.44 s).

## Anomalies explained

### 1. M6 "warm" runs with 3 verifier invocations and 0 reused targets, and the SAIL3107 focus

**The 3/0 counts did not reproduce natively.** Every native M6 full had 1 invocation and 1 reused
target (vstd): all 22 driver runs and 15 harness runs.

**The same mechanism appears natively elsewhere.** Without Current evidence, no receipt is
published, so dependencies are verified again on every run. M7 on Vest shows exactly this: 2
invocations, 0 reused, and 41–56 s of vstd re-verification per batch (anomaly 4).

**Likely cause in 387ba80 (hypothesis, not established).** Its Docker Desktop emulation lost
Currentness in the same way. vstd, the target and a SAIL3107 retry would give the 3/0 counts and the
95 s M6 full. The acceptance doc already notes that bind mounts cannot establish kernel coverage.

**The SAIL3107 focus failure does reproduce natively.** The mechanism:
- `cargo verus focus` builds in `<target-dir>/verus-partial`. See cargo-verus `src/subcommands.rs`
  at lines 395–397.
- It also sets `CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true` (line 588).
- Sail's forced or missing-receipt `clean` in `crates/sail-verus/src/managed/runner.rs` runs `cargo
  clean --target-dir <T> -p <pkg>` on `<T>`. The code is unchanged on `rescue/m7`.
- When an edit is byte-identical to the content last compiled for that function's focus context,
  Cargo reports the crate fresh by checksum. No verifier runs and no receipt is produced.
- The retry cleans the wrong directory, so the run fails with `SAIL3107` after 1.4 s on HumanEval or
  3.5 s on Vest.

This affects the GCD and concatenate cycles from round 2 onward. strlen passes because its content
alternates between cycles. `m7-repair-bench.py` does not check the focus exit status, so it counts
the failure as M6's focus time and its first actionable evidence.

### 2. Does M7 use exactly one compiler driver after e95b8bd?

**On HumanEval, yes.**
- `compiler_invocations` was 1 in every sample.
- `strace` shows a single `rust_verify` for the crate.

**e95b8bd adds a cold build per repair selector.** It gives each selector its own
`repair-targets-v1/<hash>` generation, so the first use of each function rebuilds all dependencies,
including vstd. The repair-context warm-up takes 67–72 s, compared with 33–35 s for M6's focused
warm-up.

**On Vest it uses 2 drivers,** because there are no receipts (anomaly 4).

### 3. What does in-batch preparation reuse save?

**Nothing measurable.** Batch hits were always 0. Persistent hits were 0–11 against 95–106 misses on
HumanEval, and 0 against 201 on Vest. Each miss still pays for encoding and admission: post-VIR wall
is 8.5 s against M6 full's 1.8 s on HumanEval (SST/other 98 against 6.3 thread-s), and 54 s against
11.7 s on Vest (650 against 70 thread-s).

**The engine binary is not the cost.** In traced HumanEval controls without a repair function, the
engine's full verify took 11.96 s and M7's stock verify 11.25 s.

**The repair-mode slowdown looks like serialization.** M7 per-module verify times are flat at 2–3.4
s regardless of module size. That is consistent with `PREPARATION_GATE` in
`engine/verus/interactive.patch`.

**The single callback does save something:** one rustc frontend and VIR pass, plus one round of Sail
setup. That is about 5 s on HumanEval and about 19 s on Vest. The preparation overhead currently
cancels it.

### 4. Vest blockers (new)

- **M7 refuses every Vest function.** `repair-check` reports `SAIL3122` ("exact repair function was
  not established"), although the function verifies in its full report.
- **The full result is Unknown.** It records `SAIL3114 native-observer-setup-unavailable;
  barrier=input-barrier`.
- **M6 stays Current** on an identically prepared workspace.
- **The unchanged harness stops before warm-up.** It reports `SAIL0940` because the `vest_lib`
  package also has a `cbor_rfc` test target.

## Is ≥2× over M6 plausible?

**HumanEval: needs two fixes.** The valid M6 loop is about 14.0 s, so 2× needs M7 at 7.0 s or less.
Removing the preparation overhead leaves about 7.9 s (1.8×): outside Verus 3.0 + frontend 1.7 + VIR
1.3 + post-VIR 1.8 + focus about 0.1. Cutting Sail overhead by another 1–1.5 s reaches about 6.5 s
(2.1–2.2×).

**Vest: further off.** The valid M6 loop is about 56.9 s, so 2× needs M7 at 28.5 s or less. Without
preparation overhead the floor is about 33 s (1.7×): outside 7 + frontend 8.1 + VIR 6.2 + post-VIR
11.7. Two things stand in the way:
- **Another 5 s or more must come out** of the warm post-VIR phase or Sail overhead.
- **Anomaly 4 must be fixed first.** There is no Vest M7 sample at all.

Neither catalogue reaches 2× from the single callback alone; the gap must come from removing
overhead.

## Speed levers, ranked

Each saving is an estimate per M7 batch at today's native timings.

| # | Lever | HumanEval saving | Vest saving | Basis and conditions |
|---|---|---|---|---|
| 1 | Stop paying preparation cost on misses: skip encoding and admission when reuse cannot hit, or take it off `PREPARATION_GATE` and off the critical path | about 6.7 s (14.8 → about 8.1 s) | about 40 s | Under 10% hits; post-VIR thread-s are 15× (HumanEval) and 9× (Vest) M6's |
| 2 | Fix Vest Currentness (`SAIL3114`) and selection (`SAIL3122`) | – | 41–56 s of vstd re-verification per batch | Precondition for any Vest sample. The harness `SAIL0940` target check also needs a decision (L3) |
| 3 | Cut Sail's managed overhead | 1–1.5 s | 3–4 s | Today 2.3–3.0 s vs plain 0.5 s (HumanEval) and 5–7 s vs 1.3–1.8 s (Vest). Targets: repeated metadata and probe processes; `runtime-context`, `input-guards` and `owned-paths` (about 0.9 s warm); first-run snapshot and publication. No freshness check may be weakened |
| 4 | Make warm post-VIR work cheaper, e.g. skip lowering and query emission for unchanged functions | about 1.5 s | up to about 10 s | Vest's 32k cache hits still cost 11.7 s wall, about plain Verus's own lowering. Needs an ADR and solver-oracle validation (charter rule 5) |
| 5 | Warm frontend and VIR across edits (charter roadmap) | 3.0 s per process | 14 s per process | M7 already pays this once versus M6's twice; removing it needs a persistent compiler |

**For lane L3:**
- Reject a failed M6 focus as an invalid sample.
- Fix or work around the Sail clean defect from anomaly 1.
- Keep the roughly 70 s per-selector generation warm-up in unmeasured setup.

## Commands and evidence

On the operator host:

```sh
scripts/perf-host.sh up l1p c7i.4xlarge
scripts/perf-host.sh sync l1p
scripts/perf-host.sh run l1p <script>
scripts/perf-host.sh down l1p
```

Inside the container:
- **Build.** `cargo build --release --locked -p sail-cli -p sail-shim`, run in a `git archive` of
  `0fde777` and in `541d3e7`.
- **Corpus.** `sail-conformance fetch --tier standard --project human-eval-verus --project vest
  --project verus-examples`.
- **Workspaces.** `python3 scripts/m7-repair-bench.py prepare` per arm.
- **Harness.** `python3 scripts/m7-repair-bench.py run --m6 … --m7 … --engine-install
  /opt/sail-engine --rounds 3 --environment "native …"`.
- **Driver.** `profile.py --project P --cycles … --rounds N`. The untraced runs set `L1_STRACE=0
  L1_CLEAN_PARTIAL=1`.

Raw evidence is outside the repo in `~/sail-raw-evidence/l1/`; `SHA256SUMS` covers every file.

| File | sha256 |
|---|---|
| `l1-raw-out.tar.gz`: all runs' records, stdout, stderr, `strace` output, Verus times, harness `results.json`, setup timers and binary hashes | `b4c2c463b5ed1070fed18d149dc8d3e9aee995342ea062df0e1487a6d492ee9c` |
| `aggregate-untraced.txt` | `e689317685d570e16cdbdbb10800b5115e1fc5ea457c2c5c375d4975df783f55` |
| `aggregate-straced.txt` | `e85a001ab9e9b82a753b3a70d193b3b107a3ad2fad175ec87ec20d9f03aa6d94` |
| `bench-summary.txt` | `e3146b58e9dd2a42daaefa08c0f9f7131ce556ca15c49396153692517d42b703` |
| `bench.log` | `55f28b865b3c5714a4694238af04bab5e0cb10f2b21162913d7343498273d857` |
| `profile.py` | `218b987f96877e00729d83090defe6be1df64a6f4e0152b403bc3c918db00a8d` |
| `analyze.py` | `e18b95a141712224aeaba8a5e39ac35a007d9fbdecfb5096b52e04b1c9242067` |
| `host-facts.txt` | `347b2683ad3b59991dafeec79490bb8bb5b58853f1f32b087b4ad6462f5c0290` |

**Limits:**
- One host type.
- Vest has 2 rounds; HumanEval has 3.
- First-actionable latency, RSS and reader latency were not analysed.
- The 387ba80 explanation is a hypothesis.
- The engine-full control was measured only under `strace`.
