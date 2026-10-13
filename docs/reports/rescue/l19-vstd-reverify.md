# L19: vstd re-verification in the agent loop

- **Lane:** L19, rescue. **Date:** 2026-10-13.
- **Brief:** the two defects in the L18 report's "bigger problems" section. Both
  made Sail verify vstd again, costing 27–33 s, where no input had changed.
- **Answer.**
  - *Stock path (defect 2): found and fixed.* After a run with proof errors,
    Sail cleaned every package with a build script, including vstd. Plain
    `cargo verus verify` keeps vstd fresh. Sail now does the same. The passing
    run after a failure drops from 32 s to 5.4 s on HumanEval and from 50 s
    to 24.4 s on Vest.
  - *Engine path (defect 1): already gone at the current pin.* It came from
    the dropped VI-0005 fork fix, which was in L18's engine but is not in the
    pinned fork `d2b69aea`. It does not reproduce on `rescue/m7`.
  - *M7 gate:* neither fix touches what `sail-bench repair` measures.
    `sail-bench repair` forces fresh verification and warms each repair
    function first, so it never hit either defect. Three rounds give
    HumanEval 2.14× (pass) and Vest 1.81× (fail, `SAIL7109`), matching the L12
    run (2.13×, 1.80×). The M7 gate is still failed on Vest.
  - *A third vstd cost, not fixed:* the first `repair-check` of each new
    function verifies vstd again, about 29 s on HumanEval. Each selected
    function gets its own Cargo target. Changing that needs a decision.
- **Fork:** no change needed.

## Environment

- **Host.** AWS EC2 `c7i.8xlarge` (32 vCPU), us-east-1, Ubuntu 24.04. Native
  x86-64, launched with `scripts/perf-host.sh up l19 c7i.8xlarge` and torn
  down after use.
- **Container.** CI image `sail-ci:679d9641401c264c`, built natively. Engine
  runtime `d2b69aea`, the committed pin, installed with `sail engine install`.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0, Rust 1.98.1.
- **Sail.**
  - Before: `rescue/m7` at `a8e3b69`, release `sail` sha256 `270bb140…`.
  - After: this branch, `0aa19a5`, release `sail` sha256 `cdb5eb30…`.
- **Projects.** `sail-bench repair prepare` copies of human-eval-verus
  (package `human_eval_verus`) and Vest (`vest_lib`). The failing state is
  the catalogue's failing attempt (`strlen-failing-attempt`,
  `pow128-failing-attempt`). "Original" is the unedited file, which is also
  the repaired state.
- **Method.** One private cache and `sail serve` daemon per sequence, with no
  `SAIL_MANAGED_FORCE`, as an agent runs Sail. Commands:
  `sail verify --incremental --output json --package P`,
  `sail verify --engine interactive …` and
  `sail repair-check --engine interactive --function F --output json --package P --lib`.
  The plain arm is `cargo verus verify --target-dir T -p P
  --message-format=json-render-diagnostics` in its own copy and target. It
  has no SMT cache, so its absolute times are not comparable with Sail's.
  Only its vstd freshness is.

## Defect 2: the passing stock run after a failing run

### Evidence

Sequence: original (cold), original, failing, original, failing, original,
original. Wall time in seconds. "vstd" means vstd was verified again in
that run.

| Step | HumanEval before | HumanEval after | Vest before | Vest after | Plain HumanEval | Plain Vest |
|---|---:|---:|---:|---:|---:|---:|
| 1 original, cold | 54.8 (vstd) | 49.4 (vstd) | 68.2 (vstd) | 67.7 (vstd) | 47.8 (vstd) | 65.0 (vstd) |
| 2 original, unchanged | 0.37 | 0.58 | 1.43 | 1.67 | 0.07 | 0.17 |
| 3 failing | 4.31 | 4.53 | 20.3 | 20.8 | 16.5 | 29.3 |
| **4 original, after failing** | **32.4 (vstd)** | **5.42** | **50.0 (vstd)** | **24.4** | 17.2 | 33.4 |
| 5 failing | 6.57 | 6.77 | 18.9 | 19.4 | 16.4 | 30.1 |
| **6 original, after failing** | **31.6 (vstd)** | **5.31** | **50.5 (vstd)** | **25.2** | 17.1 | 33.3 |
| 7 original, unchanged | 0.36 | 0.57 | 1.45 | 1.75 | 0.07 | 0.17 |

- **Before.** Every run after a failure had two verifier invocations and no
  reused target. vstd's three Cargo fingerprint directories were rewritten in
  each such run, and in no other run.
- **After.** Those runs have one invocation and reuse vstd's receipt. All 28
  Sail results were Current. Verdicts were unchanged: exit 101 on the failing
  state and 0 otherwise.
- **Plain Verus** reported vstd `"fresh": true` in every run after the first.
  So plain does not rebuild vstd after a failure, and matching plain is the
  floor.

### Cause

The cause is in Sail's managed runner, not in Cargo, cargo-verus or the
engine.

1. A managed run starts by consuming its target's build-script *generation*
   (ADR 0036, `build_inputs::family::begin`). The generation is the record
   that the target's build-script outputs still match their declared inputs.
2. At the end, the run republishes the generation only if it was Current and
   exited 0. A source-engine run with complete proof errors was the one
   exception (`negative_compiler_generation`, commit `947f98e`). A stock run
   with proof errors (Cargo exit 101) published nothing.
3. The next run found no generation. It marked every recorded build script as
   uncovered, and `build_inputs::stale` returned all of them. On these
   projects that is vstd, proc-macro2, quote, verus_syn and
   verus_prettyplease. `cargo clean -p` removed them, so Cargo rebuilt them
   and cargo-verus verified vstd again.

The input revision, the context digest and the receipts were all correct.
Only the build-script generation was missing.

### Fix

The ADR 0036 negative branch now also covers stock runs (commit `0aa19a5`;
ADR 0036 amendment).

- **What stays the same.** All the existing conditions still apply:
  - the run is Current and unfocused, Cargo exits 101, and input coverage
    existed at entry;
  - every recorded compiler start has a completion;
  - each selected target has exactly one completed full outcome;
  - at least one driver exited 1 with ordinary proof errors and no VIR
    error, and the report counts match.
- **What differs.** Stock Verus has no sealed terminal. In its place, the
  driver must exit normally with 0 or 1, this run must have recorded the
  invocation's completion, and the run must leave one whole `--output-json`
  document that agrees with the exit (`engine_events::stock_completed`).
- **Correctness.**
  - The branch keeps only ordinary dependencies' build-script records. Each
    record is checked again against the target's actual output at publication
    and before the next run. The failed crate gets no receipt, so it is always
    verified again.
  - Verdicts, diagnostics and SMT are unchanged (charter rule 1).
  - Invalidation stays content-addressed (rule 6).
  - A compiler error, a crash (exit 101 from the driver), a partial document,
    `--verify-function`, or uncertain inputs still refuse the branch and take
    the old cleaning path.
- **Regression test.**
  `stock_negative_generation_needs_a_complete_document_and_normal_exit`
  accepts the complete stock outcome and refuses each of these: a panic exit,
  a truncated document, a doubled document, a document that disagrees with
  the exit, a Historical run and a focused run. The existing refusal matrix
  still passes.

## Defect 1: engine runs re-verify vstd

### Evidence at the current pin

Engine runtime `d2b69aea` on HumanEval, before the fix (`a8e3b69`):

| Run | Wall (s) | Invocations | Reused targets |
|---|---:|---:|---:|
| `verify --engine interactive`, first in its context | 34.0 | 2 | 0 |
| same, unchanged ×2 | 0.59, 0.60 | 0 | 2 |
| `repair-check`, first in its context | 34.6 | 2 | 0 |
| same, unchanged ×2 | 8.27, 5.97 | 1 | 1 |

`input_revision` stayed the same on every run. After the fix, the engine
failing→passing sequences also keep vstd:

| Sequence (after) | HumanEval (s) | Vest (s) |
|---|---|---|
| `verify --engine`: unchanged, failing, passing | 0.58, 6.73, 5.35 | 1.64, 20.6, 23.9 |
| `repair-check`: unchanged, failing, passing, failing, passing | 5.87, 4.96, 5.86, 4.96, 5.86 | 24.5, 20.0, 24.5, 19.9, 24.4 |

In every one of these runs vstd was reused and the result was Current. The
Vest `repair-check` runs named a function that does not exist, so they were
not accepted. Their full verification still ran and is what the Vest column
measures.

### Cause

L18 measured an engine built from fork `3a91b1d7`, which is `m8/timers` on top
of `89709da7`. That base carried the VI-0005 fork fix.
- **What the fix did.** It recorded the forwarded Verus arguments in each
  crate's dep-info as environment dependencies.
- **Why that rebuilt vstd.** Sail forwards a log directory under the run's
  own directory and, for repair, `--sail-repair-function`. Both differ between
  runs, so every run changed vstd's Cargo fingerprint and Sail's recorded
  source inputs, and with them the input revision.
- **Earlier record.** The fix was dropped from the pinned fork `d2b69aea`
  for exactly this reason (`a9b3b9b`; VI-0005; gate 7 failed attempts 11
  and 12, 34–38 s per HumanEval sample).

On the current pin the engine's vstd dep-info declares only `VERUSDOC`.

**If VI-0005 is fixed again,** the fork must not bind the log directory, the
repair function or any other per-run argument. It should bind only arguments
that change results, as VI-0005 already notes.

## Repair benchmark (`sail-bench repair`, 3 rounds)

This run used the committed engine pin `d2b69aea`; the rebuild with L21's
determinism fix was not ready. M7 is `0aa19a5` and M6 is `26f124f`. Arm
order alternated each round. All 60 samples were valid, every M7 sample used
one driver and was Current, and every pair's outcomes matched.

| Catalogue | M6 whole loop, median (rounds) | M7 whole loop, median (rounds) | Speedup | Gate |
|---|---:|---:|---:|---|
| HumanEval | 61.0 s (63.5, 61.0, 61.0) | 28.5 s (28.8, 28.5, 28.5) | **2.14×** | pass |
| Vest | 235.4 s (235.4, 233.9, 235.4) | 130.1 s (131.0, 129.1, 130.1) | **1.81×** | fail (`SAIL7109`) |

Per-cycle medians, M6 / M7 in seconds:

| HumanEval | M6 | M7 | Vest | M6 | M7 |
|---|---:|---:|---|---:|---:|
| gcd-body | 12.6 | 5.9 | base128-body | 48.6 | 26.9 |
| concatenate-spec | 12.6 | 6.0 | pow128-spec | 48.8 | 27.2 |
| strlen-refactor | 12.7 | 5.9 | base128-refactor | 48.8 | 27.0 |
| strlen-failing-attempt | 10.6 | 5.0 | pow128-failing-attempt | 39.2 | 22.2 |
| strlen-repaired-attempt | 12.5 | 5.9 | pow128-repaired-attempt | 49.0 | 26.6 |

**Why the fixes do not move the gate.**
- The M7 arm runs `repair-check` with `SAIL_MANAGED_FORCE=1`.
- The engine path already kept dependencies after a failing run.
- Each function is warmed before it is measured.
- No measured sample verified vstd in this run or in L12's.

**What Vest needs.** Vest needs M7 at or below 117.7 s, about 2.5 s less per
cycle. A Vest M7 cycle is about 27 s, close to a stock full verify of Vest
(24–25 s). The time goes to the Verus frontend and VIR (L18: rustc 6.4 s,
VIR 7.1 s, bucket phase 5.1 s), so the levers are L18's cheaper items and M8.

Result file: `bench/results/repair-acceptance-l19.json`, sha256
`58fdb091…8b89`. The [benchmarks page](../../benchmarks.md) is regenerated.

## Gates

At `0aa19a5`, on the host above in the CI image, plus `ci/run.sh host` on
macOS:

| Step | Result |
|---|---|
| `ci/local.sh gate` | Pass: 521/521 tests, layering, deny, links, benchmarks page |
| `ci/local.sh solver` | Pass: 35/35 |
| `ci/local.sh e2e` | Pass: legacy, managed and repair phases |
| `ci/local.sh conformance fast pr` | Pass: 33/33 same in each of the three phases, and engine 33/33 same |
| `ci/run.sh host` (macOS) | Pass: clippy and 497/497 tests |

The first `gate` attempt failed its link check on two broken links in a
scratch copy of the M6 source left in the checkout. That copy was removed.

## Remaining vstd costs

These are recorded in [known issues](../../known-issues.md).

- **Per context.** Each proof context verifies vstd once in its own target:
  stock `sail verify`, `--engine interactive` and `repair-check`. That is
  about 30 s each on HumanEval, 3 × 30 s for an agent that uses all three.
- **Per repair function.** `repair-check` builds in
  `compiler-artifacts/<compiler>/repair-targets-v1/<context>`. The context
  includes the selected function, so each newly repaired function starts with
  an empty target and verifies vstd again. Measured on HumanEval after the
  engine sequences above:
  - the first `compute_gcd` repair took 35.1 s, with 2 drivers and vstd run;
  - its second repair took 6.1 s, with vstd reused;
  - the first `concatenate_impl` repair took 35.1 s, with vstd run again.

  An agent repairing *n* distinct functions pays *n* × 29 s. The benchmark
  hides this cost, because it warms every function before measuring. ADR 0036
  keeps a separate target for each function, so that a repair's certified
  dependency bytes stay alive. Proof receipts are never shared between
  contexts. Sharing vstd across functions therefore needs its own decision,
  for example through a vstd receipt bound to the compiler family rather than
  the function. This lane did not change it.
- **Plain floor.** A run after a failure still recompiles the edited crate,
  as plain does. The failing runs themselves (4.3–6.8 s on HumanEval) were
  not investigated.

## Evidence

Raw files are in `~/sail-raw-evidence/l19/` on the lead's machine.
`SHA256SUMS` covers every file.

| File | Contents | sha256 |
|---|---|---|
| `l19-sequences.tar.gz` | Every Sail and plain run of the failing→passing, engine and `repair-check` sequences: JSON, stderr, plain Cargo messages | `161ea65d621ee13c7431d3f40f9bf9e9bf903b42342ed5798af1f2efc5c0b719` |
| `l19-bench-out.tar.gz` | `sail-bench repair` raw output: commands, stdout, stderr and journal pages | `a8d3d5e818b3b4b08d52c396f4ad584fa96cd27a14f133d4b2dfc50fcafedd26` |
| `repair-acceptance-l19.json` | Benchmark result, also committed under `bench/results/` | `58fdb091a273c8711313232dd9d55283ad43ad1d66696a8c429afe6ebcb08b89` |
| `l19-scripts.tar.gz` | Drivers: state writer, sequences, benchmark and median scripts | `36120d865abe96b828c7cc5938c561bf2e3336a919f52cec181b84c90b723046` |

An earlier benchmark start at the same commit gave no M7 samples. It was
given a relative `--m7` path, which the harness runs from the project
directory, so every M7 invocation failed to start (`SAIL7101`). It was
stopped and rerun with an absolute path. Its partial result was not kept.
