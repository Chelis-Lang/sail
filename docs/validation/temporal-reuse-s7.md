# Temporal job reuse on the S7 corpus

This experiment measures how much work Sail can reuse across native TLA+ checks. It compares the native 0.3.0 source, the same source with temporal receipts, and pinned TLC. All three ran on the same host with identical model and configuration bytes. [Reuse issue](https://github.com/Chelis-Lang/sail/issues/774) · [Sail v0.3.0 baseline](https://github.com/Chelis-Lang/sail/releases/tag/v0.3.0) · [Receipt implementation](https://github.com/Chelis-Lang/sail/pull/777).

For substantive edits, see [installed edit sessions and invariant removal](#native-checks-after-substantive-edits) and [retaining the first temporal property](#retaining-the-first-temporal-property). The original receipt comparison below measures identical-input reuse.

## Results

Each cell reports the median of three foreground times in milliseconds. The last column reports the median of the three paired speedups against Sail. All 333 verdicts and distinct-state counts per engine matched.

| Operation | TLC | Sail 0.3.0 source | With reuse | Speedup vs Sail |
|---|---:|---:|---:|---:|
| Mutex, first check | 13,408 | 12,285 | 12,376 | 0.99× |
| Mutex, unchanged | 13,170 | 12,360 | 9.29 | 1,330× |
| Mutex, invariant edit | 12,184 | 11,590 | 11,715 | 0.99× |
| Mutex, revert | 13,297 | 12,350 | 9.37 | 1,356× |
| Internal memory, first check | 1,231 | 169.4 | 167.7 | 1.01× |
| Internal memory, property edit | 1,098 | 103.3 | 103.4 | 1.00× |
| DieHard, first refutation | 466.5 | 17.38 | 19.06 | 0.91× |
| DieHard, unchanged refutation | 489.5 | 19.18 | 8.96 | 2.10× |
| DieHard, edit preserving failure | 488.5 | 19.36 | 19.30 | 1.01× |
| DieHard, fix | 455.1 | 15.11 | 14.97 | 1.01× |
| DieHard, undo fix | 468.0 | 17.36 | 9.01 | 1.93× |

MCLamportMutex has 724,274 distinct states. Its unchanged check ranged from 8.75 to 9.51 ms, and its revert ranged from 9.11 to 9.40 ms. All twelve repeat and undo cases in the dedicated scenarios reused their complete original evidence. The edits to the invariant and temporal property each required full exploration.

The 100-check loop took **1.454 s** (1.429–1.824 s) with reuse, **4.862 s** (4.834–4.961 s) on the merged 0.3.0 control and **67.324 s** (66.950–68.047 s) on TLC. The median paired speedups were **3.34×** over the Sail control and **46.05×** over TLC. Every trial had 76 exact hits. The ranges describe the three observed runs.

A separate Linux process audit observed one worker launch each for fresh Refuted and fresh Holds, and zero for exact replays and undo. The installed product signed both results, exported the bundle and signature, imported them into a new cache, then replayed identical evidence in a fresh daemon. A new caller received the same proof with updated attribution. A changed wall limit launched a new worker. Both `0022` and `0002` umasks passed default and explicit cache paths, embedded and daemon entry points, preparation sharing, and the signed round trip. The importer refused group-writable input. These controls ran outside the timed samples.

A changed model can still admit yesterday's violating trace while a fresh
bounded check stops at an earlier branch or encounters an evaluation error.
The production path therefore explores those edits afresh. The proposed
[bounded refutation replay API](https://github.com/Chelis-Lang/dive/issues/1041)
and [completed-graph recheck API](https://github.com/Chelis-Lang/dive/issues/1039)
describe the evidence needed to accelerate those cases.

### Prepared first check

Five rounds rotated among disabled preparation, worker-image preparation and preparation of the actual MCLiveInternalMemory job. Each trial used fresh application caches. Foreground was measured after the asynchronous preparation child completed. The table separates the median post-install handoff from time until ready and time for the first foreground check. It excludes the common download and extraction steps.

| Installed product | Handoff (ms) | Ready after handoff starts (ms) | First check (ms) |
|---|---:|---:|---:|
| disabled | 0.01 | 0.01 | 160.90 |
| worker | 1.92 | 9.28 | 159.13 |
| first-job | 2.08 | 165.90 | 2.89 |

## Measurement

One us-east-1 EC2 c7i.4xlarge ran all engines serially. It had a Xeon Platinum 8488C, 16 vCPUs, SMT enabled, 32 GiB nominal RAM, Ubuntu 24.04.5 and kernel 7.0.0-1017-aws. CPU frequency and OS page cache were uncontrolled. The original S7 used a c7i.8xlarge. Each Sail scenario sequence and each loop started with a new cache and a new daemon. Each job invoked a new client through the public Unix socket. The post-install trials used the embedded CLI. TLC started a new pinned OpenJDK 17.0.20.1+1 process per check with ParallelGC, one worker and the default heap. Sail used one worker, seed and fingerprint zero, the exact store and S7's state and depth limits.

The Sail v0.3.0 release tag points to the measured baseline, `e85e3d2b`. The reuse source `c08a2489` has that exact parent. Both built from the same Cargo.lock and the checksum-verified crates.io D.I.V.E. 0.1.0 packages, using the public native CLI and worker in `sail-solve`. The staged worker SHA-256 was `889d3c9340bad16312e83e0484ff65fbda8bed8de11eb3a4b2d7d0028038e44d` for the baseline and `a90d8b5520b75f2e8755e50988420a2861366c83e079f828914abba8976d2faa` for the PR. The benchmark compares elapsed foreground command times. Fixture preparation, writing request files, daemon startup and inspecting output were outside those times.

The 100-check workload cycles through DieHard, EWD840, MCRealTimeHourClock and MCLiveInternalMemory five times each. Each batch runs the base twice, edits a predicate or finite model domain, checks the edit twice and restores the base. This creates 24 distinct requests and 76 opportunities for exact reuse. These weights describe a scripted maintenance workload. Actual workloads can have different hit rates.

The dedicated scenarios drop one invariant from MCLamportMutex, drop one temporal property from MCLiveInternalMemory, strengthen DieHard's failing invariant with an already true conjunct, change it to hold, and undo the edit. The benchmark checks each intended verdict. Every returned refutation must carry matching native and ITF traces and successful independent validations. Every replay must preserve the complete original response digest, identify a stable 256-bit key, and bind to the current job. Comparing the cross-engine result streams checks the distinct-state counts for edited models as well.

## Reproduce

Use the D.I.V.E. `94cbbed9` S7 scripts and goldens, Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, and TLC jar with SHA-256 `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`. Build the two measured revisions in separate checkouts and targets with Rust 1.99.0.

```sh
CARGO_TARGET_DIR=target cargo build --release --locked -p sail-solve --bin sail --bin sail-runtime-worker
mkdir -p "$PREFIX/bin"
cp target/release/sail target/release/sail-runtime-worker "$PREFIX/bin/"
strip -S "$PREFIX/bin/sail" "$PREFIX/bin/sail-runtime-worker"
python3 scripts/stage-temporal-worker.py "$PREFIX/bin/sail-runtime-worker" "$PREFIX/bin/"
```

Set `FIX` to an empty directory. Set the other path variables to the named checkouts, pinned binaries and staged Sail installations.

```sh
python3 scripts/perf-temporal-reuse.py "$FIX" --suite fixtures --dive "$DIVE" --examples "$EXAMPLES"
for r in 1 2 3; do
  for suite in scenarios loop; do
    python3 scripts/perf-temporal-reuse.py "$FIX" --suite "$suite" --java "$JAVA" --jar "$JAR" > "tlc-$suite-$r.jsonl"
    python3 scripts/perf-temporal-reuse.py "$FIX" --suite "$suite" --sail "$BEFORE/bin/sail" > "before-$suite-$r.jsonl"
    python3 scripts/perf-temporal-reuse.py "$FIX" --suite "$suite" --sail "$AFTER/bin/sail" --expect-replay > "after-$suite-$r.jsonl"
    for engine in tlc before after; do
      jq -c '[.index,.content,.outcome,.distinct]' "$engine-$suite-$r.jsonl" | sha256sum
    done
  done
done
python3 scripts/perf-temporal-reuse.py "$FIX" --suite audit --sail "$AFTER/bin/sail" --expect-replay
python3 scripts/perf-temporal-reuse.py "$FIX" --suite signed-restore --sail "$AFTER/bin/sail" --receipt-action action/temporal-cache/receipts.py
python3 scripts/perf-temporal-reuse.py "$FIX" --suite prepare --sail "$AFTER/bin/sail" --installer scripts/install.py
```

The external evidence archive `2026-11-19-native-s7-e85e3d2b-c08a2489.tar.gz` (SHA-256 `35e05f4922af08e3f641428f15410d19141b9e3081d7f13e4151bb8123d0613b`) retains raw samples, process counts, host details, fixture hashes and exact binary identities.

## Reuse the measurements

The [native S7 data](data/2026-11-19-native-s7.json) contains every foreground time from the release-source comparison, the input hashes and expected results, all cache-hit positions, all five preparation rounds and the process and signed-restore controls. Under `scenarios` and `loop`, `order[i]` names the step and input digest. Look up that digest in `inputs` for the model, verdict and distinct count. For each engine, `elapsed_ms[engine][r][i]` is the time in milliseconds for the same step in zero-based trial `r`. Compute each trial's total before taking a median or a paired ratio.

For example, from the repository root:

```sh
python3 - <<'PYDATA'
import json
from statistics import median
from pathlib import Path
loop = json.loads(Path("docs/validation/data/2026-11-19-native-s7.json").read_text())["loop"]
totals = {engine: list(map(sum, trials)) for engine, trials in loop["elapsed_ms"].items()}
print({engine: round(median(ms) / 1000, 3) for engine, ms in totals.items()})
print(round(median(a / b for a, b in zip(totals["before"], totals["after"])), 2))
PYDATA
```

The [pre-release S7 data](data/2026-11-19-pre-release-s7.json) preserves the earlier candidate comparison and two exploratory extension builds. Those builds used a different D.I.V.E. source. Use the native data for comparisons to v0.3.0. Each record identifies its binaries and host.

To collect another three-trial comparison, store the driver's JSONL files together with `host.json` and `identities.json` recording the host, revisions, binary hashes and oracle pins, then run `python3 scripts/collect-temporal-reuse.py DIRECTORY`. The collector rejects incomplete or unpaired workload trials, unequal outcomes or counts, and replay evidence that differs from its original response.

## Guarded invariant recheck after a CFG edit

A separate synthetic benchmark on native EC2 measured an explicit recheck of an invariant failure after appending a comment to its CFG. The initial state has one successor for each integer from 1 through the chosen width. Only the largest successor leads to the violating state. Every full check and independent replay returned the same three-state ITF witness. [Individual samples, binary hashes and host](data/2026-11-20-guarded-invariant.json).

| Width | Distinct states in fresh check | Median fresh check | Median guarded recheck | Ratio of medians |
|---:|---:|---:|---:|---:|
| 20 | 22 | 11.95 ms | 9.83 ms | 1.22× |
| 200,000 | 200,002 | 172.90 ms | 10.04 ms | 17.22× |

Each row used a new Sail cache and five distinct comment edits. A daemon established the previous receipt. Each timed pair used a new embedded CLI process first for the edited recheck and then for a fresh check of the same bytes. The timer includes JSON decoding. Both paths authenticated the same release worker. No other build or benchmark ran concurrently. OS file-cache state and CPU frequency were uncontrolled. An earlier width-20 smoke immediately after staging took 55.56 ms guarded and 41.11 ms fresh. The small-case gain varied with the run.

The controls exercised a missing previous receipt, an exact current receipt and fresh fallbacks for changed invariant inventory, an earlier sibling error, a repaired invariant and a tightened state limit. At width 20 the sibling returned Error. At width 200,000 the pinned backend observed the earlier error and returned Unsupported because its ordered diagnostic exceeded 256 cumulative emissions. The first large run stopped on a harness expectation of Error. The corrected run required the specific refusal and compared each fallback with a separate uncached execution, including the native verdict, property, statistics, message and ITF trace. The guarded trace route used in this experiment accepts equivalent parsed CFGs. For substantive edits, the edit session can capture ordered safety states or a labelled temporal graph.

A native rerun on merged main measured 9.28 ms guarded versus 129.14 ms fresh (13.92×) at width 200,000. Native Apple M1 also passed the staged width-20 smoke and twelve focused tests. The JSON includes all rerun samples and identifies the Mac debug build separately from the timed Linux release builds.

The source and release worker are identified in the sample file. Build and stage the release binaries as above, then run both commands on the same idle native host. The driver generates the models and checks the expected outcomes.

```sh
python3 scripts/benchmark-guarded-refutation.py "$PREFIX/bin/sail" --smoke
python3 scripts/benchmark-guarded-refutation.py "$PREFIX/bin/sail"
```

## Native checks after substantive edits

The installed Linux release build checked each edited request twice, once in a session and once in a fresh process. The harness compared the current digest, outcome, five exploration counters and complete ITF witnesses. Every returned counterexample had to validate against the current model. Times below are median foreground milliseconds across five trials on one EC2 c7i.4xlarge. The speedup column is the median of the five paired ratios. Each session includes its initial capture and all edits. [Individual samples, scopes, source and installed-image hashes](data/2026-11-20-native-edit-session.json).

| Workload | Checks per session | Separate fresh CLI checks, ms | One session, ms | Paired speedup |
|---|---:|---:|---:|---:|
| Safety star, new holding and failing predicates | 3 | 350.18 | 164.37 | 2.14× |
| Safety star, changed failing predicate | 2 | 240.74 | 160.38 | 1.51× |
| Temporal star, current holding and failing properties | 3 | 95.36 | 47.76 | 2.00× |
| S7 DieHard, failure then fix | 3 | 31.63 | 21.75 | 1.46× |
| S7 LamportMutex, dropped invariant | 2 | 20,786.16 | 10,705.58 | 1.96× |
| S7 internal memory, dropped temporal property | 2 | 237.83 | 233.45 | 1.02× |

The star width was 200,000 for safety and 20,000 for temporal checking. Internal memory’s aggregate temporal property edit used ordinary native checking in this campaign. Its measured cost includes that fallback. The dataset identifies each entry as a capture, a recheck or a fresh result and includes its worker time. These worker-side times exclude process startup and transport. [Earlier single-trial development measurements](data/2026-11-20-native-edit-preflight.json).

### Reuse a previous proof across processes

On the original S7 LamportMutex, the separate `runtime recheck` command needed **22.46 ms** to check the edited inventory, versus **10.014 s** for a fresh process (**447.4×**, median paired ratio). Establishing the original receipt cost 10.731 s. Both full checks held, with 724,274 distinct and 2,729,079 generated states at depth 61. Only the previously established invariants `TypeOK` and `Mutex` remained after the edit. Each sample used a new edited input, authenticated the prior complete receipt and the native worker, and compared with a fresh execution on the same bytes.

The integrated worker also repeated all 333 checks in the earlier exact-replay S7 comparison. Each input, outcome, distinct count and hit position agreed with the published record. This run’s unchanged mutex took 9.43 ms and its 100-check loop took 1.462 s with 76 exact hits per trial. The TLC and v0.3.0 timing controls above were collected in the earlier campaign. [Integrated samples and evidence digests](data/2026-11-20-integrated-s7.json).

Reproduce the substantive edits with the original S7 fixtures and an installed release worker. The command checks result equality before reporting timings.

```sh
python3 scripts/benchmark-temporal-edit-session.py "$PREFIX/bin/sail" --s7-fixtures "$FIX" --width 200000 --samples 5
```

For exact receipts, repeat the `scenarios` and `loop` driver commands in the reproduction section three times on the new build. Name their outputs `s7-scenarios-1.jsonl` through `s7-scenarios-3.jsonl` and likewise for `s7-loop`. Compare the new input order, results and replay evidence with the original S7 record using the collector.

```sh
python3 scripts/collect-temporal-reuse.py "$RESULTS" --candidate s7 --compare docs/validation/data/2026-11-19-native-s7.json
```

## Retaining the first temporal property

MCLiveInternalMemory originally checks `LivenessProperty` followed by
`Liveness`. Removing the trailing property leaves the transition system, fairness
and first compiled property unchanged. Sail can use the previous native proof
after checking both programs and authenticating the original worker. It
requires a complete final search with no periodic prefix, no audit,
no reference atom evaluation and no fallback. The previous search generated
8 initial states and 21,400 total states, with 4,408 distinct states,
depth 10 and an empty queue. The ordinary edited check agrees on all five
counters.

On the native Linux S7 host, a separate process checked the property removal
in **15.565 ms** versus **86.734 ms** fresh, a median paired speedup of
**5.57×** across five trials. Establishing the original receipt cost
145.238 ms. A two-input session including that initial check took
156.053 ms versus 231.862 ms for two ordinary processes (**1.49×**).
The original S7 comparison timed this same property edit at 103.3 ms on
Sail 0.3.0 and 1,098 ms on TLC. Those are the earlier controls in the
[three-engine comparison](#results).

| Final native Linux workload | Fresh foreground, ms | Reused foreground, ms | Median paired speedup |
|---|---:|---:|---:|
| Internal memory, property removed in a new process | 86.734 | 15.565 | 5.57× |
| Mutex, invariant removed in a new process | 9,576.457 | 20.347 | 470.66× |
| Safety star, three-check session | 349.179 | 161.785 | 2.16× |
| Safety star, two-check refuted-prefix session | 234.438 | 159.522 | 1.46× |
| Temporal star, three-check session | 92.310 | 46.332 | 2.05× |
| DieHard, three-check fail-then-fix session | 31.336 | 21.413 | 1.45× |
| Mutex, two-check session | 19,682.230 | 10,190.446 | 1.93× |
| Internal memory, two-check session | 231.862 | 156.053 | 1.49× |

Each time column is the median of the five foreground observations. The
speedup column is the median of the five within-trial ratios. The session
rows include their first check. The separate-process rows start with an
authenticated prior ordinary receipt. On a native EC2 Apple M1, the installed
debug CLI exercised both InternalMemory routes with the same closed request
digests and counts. [All native samples, binary identities, host settings and Mac functional control](data/2026-11-20-first-temporal-claim-native.json).

The final installed build repeated all 333 entries in the original native
suite. The input digests, verdicts, distinct counts and hit positions all
matched. An unchanged mutex took 9.028 ms and undo took 9.104 ms. The
100-check loop had 76 exact hits per trial and took 1.344 s at the median.
The earlier control medians were 4.862 s on Sail 0.3.0 and 67.324 s on
pinned TLC (3.62× and 50.10×, ratios of medians). This loop uses ordinary
exact-receipt execution. Its substantive edits run fresh unless the caller
selects one of the explicit recheck APIs. [All 333 final observations](data/2026-11-20-first-temporal-claim-s7.json).

Two local three-trial runs exercised the full installed CLI during development.
In the final run the edited check cost 13.101, 12.678 and 15.058 ms in
separate processes, versus 70.057, 65.986 and 74.657 ms on fresh checking
of the same respective inputs. These shared-workstation timings include
lookup, process and worker startup, both bindings and the result. The data
also retains all six edit sessions and the mutex controls for each trial.
[Development measurements and binary hashes](data/2026-11-20-first-temporal-claim-preflight.json).

The command in the preceding section reproduces both the installed edit
session and the cross-process property removal. The script checks the current
obligation and worker identities, counts and fresh verdict before writing
each timing.
