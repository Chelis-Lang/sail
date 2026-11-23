# Temporal job reuse on the S7 corpus

This experiment measures how much work Sail can reuse across native TLA+ checks. It compares the native 0.3.0 source, the same source with temporal receipts, and pinned TLC. All three ran on the same host with identical model and configuration bytes. [Reuse issue](https://github.com/Chelis-Lang/sail/issues/774) · [Sail v0.3.0 baseline](https://github.com/Chelis-Lang/sail/releases/tag/v0.3.0) · [Receipt implementation](https://github.com/Chelis-Lang/sail/pull/777).

For substantive edits, see [installed edit sessions and invariant removal](#native-checks-after-substantive-edits) and [retaining the first temporal property](#retaining-the-first-temporal-property). The [portable symbolic predicate measurements](#portable-symbolic-predicates) cover cross-process proofs of new safety invariants on larger KVS programs. The original receipt comparison below measures identical-input reuse.

The [installed public edit histories](#installed-public-edit-histories) follow chronological CCF and Disruptor changes through independent CLI processes. The [SlateDB WAL campaign](#slatedb-wal-maintenance-checks) measures cold, edit, repeat, undo and fresh controls on a published storage protocol.

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

## Portable population predicates

At 8,192 managers, an installed `sail runtime symbolic-check --strategy occupancy` cold search of Lamport's TCommit took 220.30 s and stored a 125-byte candidate. The same file produced at three managers verified a stronger property on the 8,192-manager source in 0.15 s. A fresh exact check took 216.63 s. Direct support induction in Dive took 0.14 s. Both proofs carried no exploration counts. Deleting abort transitions still admitted the retained predicate. The fresh graph changed to 16,385 populations and two different labeled counts.

When a new transition made the original consistency property false, the candidate failed next-step closure. Sail reran from Init and returned a validated depth-three violation in 23.07 s. A fresh search returned the same word in 22.92 s. The candidate survived both the transition violation and a second false property. The default BDD strategy retained a separate file in the same directory. [Source inputs, exact counts, control results and reproduction](https://github.com/Chelis-Lang/dive/blob/main/conformance/reports/perf/2026-11-21-occupancy-edits.json).

## Portable symbolic predicates

A prior safety check can prove a new invariant that the checker cannot prove
inductively from the new invariant and the original configuration alone.
The five-key edit below took **301 ms** to prove with a predicate from the
previous program, versus **14.40 s** for fresh exhaustive symbolic checking
of the edited program. Both times include starting Sail and Dive, reading the
current source and returning the result. [All twenty native trials, input and
executable hashes, and controls](data/2026-11-20-symbolic-predicates.json).

| Keys | Added invariant | Establish before edit (ms) | Fresh edited search (ms) | Recheck predicate (ms) | Median paired speedup | Refute NoWrites after reuse attempt (ms) |
|---:|---|---:|---:|---:|---:|---:|
| 3 | TxIsClean | 952.34 | 955.05 | 299.80 | 3.35× | 597.88 |
| 3 | AtMostOneConflict | 953.17 | 952.02 | 300.10 | 3.16× | 596.57 |
| 5 | TxIsClean | 14,449.93 | 14,347.71 | 300.00 | 47.94× | 647.99 |
| 5 | AtMostOneConflict | 14,247.92 | 14,398.47 | 300.52 | 47.79× | 647.50 |

Each row reports medians over five whole-process trials on native EC2
c7i.4xlarge. Establishment and fresh search each use an empty directory.
Rechecks bind the edited source in a new process. Trials alternate the order
of the fresh search and recheck. The three-key model had 17,220,672 distinct
transaction-symmetry classes, 365,609,473 generated and depth 22. Five keys
gave 1,183,519,405,056 distinct, 39,234,075,936,769 generated and depth 32.
The table uses shared finite-domain structure in a single open-source KVS
family. A recheck supplies a safety proof without counts. Its standalone
predicate was respectively 3,836 or 7,063 bytes.

TxIsClean requires every inactive transaction to have an empty missed-write
set. The direct induction strategy can prove this edit from the source.
AtMostOneConflict says that at most one active transaction has an overlapping
missed and written key. Direct induction failed the Next closure obligation in
all ten compound-conflict trials. The exported reachable predicate supplies
the missing information. The edit was absent when the predicate was built.
Each timed trial also added the deliberately false NoWrites invariant.
Both a new search and the reuse path returned an independently validated
write at depth 3, and the previous predicate retained its original hash.

The result depends on transaction cardinality. Expanding from two to three
transactions at three keys and three values makes AtMostOneConflict false.
A process offered the two-transaction predicate returned a validated depth-8
counterexample. Pinned one-worker TLC independently reported the same property
and depth with 19,880 generated and 4,618 distinct states before stopping.

### Reproduce the edits and the comparison

Use Sail and Dive development binaries with the symbolic CLI installed.
The campaign used debug Rust binaries and the bundled Dive symbolic worker on
Ubuntu 24.04 x86-64. It ran one checker at a time with host CPU defaults and
a 600-second per-invocation limit. The benchmark records the wall clock and
SHA-256 identities on the machine where it runs. Start from the pinned
[upstream KeyValueStore source and configurations](https://github.com/tlaplus/Examples/tree/6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f/specifications/KeyValueStore).

```sh
git clone https://github.com/tlaplus/Examples.git examples
git -C examples checkout --detach 6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f
python3 scripts/prepare-symbolic-kvs-benchmark.py examples kvs-edits
for edit in clean conflict; do
    for keys in 3 5; do
        dir=kvs-edits/$edit
        set --
        if [ "$edit" = conflict ]; then set -- --require-strengthening; fi
        python3 scripts/benchmark-symbolic-session.py "$@" \
          --sail "$SAIL" --dive "$DIVE" \
          --module examples/specifications/KeyValueStore/MCKVS.tla \
          --config "$dir/$keys-base.cfg" --edited-module "$dir/MCKVS.tla" \
          --edited-config "$dir/$keys-edit.cfg" --false-config "$dir/$keys-false.cfg" \
          > "$edit-$keys.json"
    done
done
```

For the negative cardinality control, establish a session with the original
module and three-key base configuration. Check the edited conflict module with
`kvs-edits/conflict/3-transactions.cfg` in the same session. The command must
return exit 1 and name AtMostOneConflict.

### Microsoft CCF coverage controls

Unmodified MCccfraft configurations at three historical revisions include
temporal properties. The symbolic CLI explicitly refused each one. Four
consistency safety configurations at the middle revision also hit a named
unsupported input. No row qualifies as CCF acceleration. Ordinary Core on
the single-node commit-reachability debug configuration found the expected
invariant violation at depth 5 with 17 distinct and 20 generated states.
The three revisions and individual results are in the
[measurement data](data/2026-11-20-symbolic-predicates.json).

## Installed public edit histories

Sail checked three upstream revisions each of Microsoft's CCF consistency
specification and the Disruptor ring-buffer protocol. The benchmark uses the
published input bytes. It includes every initial check, refusal and fallback.
For Disruptor each revision runs the SPMC safety/liveness configuration,
MPMC safety and MPMC liveness. The first edit changes imports and named
assumptions in the shared RingBuffer. The next strengthens the SPMC writer
assumption. Both MPMC programs remain byte-identical at that last step.
[Pinned source files and SHA-256 values](../../scripts/temporal-history-inputs.json) ·
[Disruptor model and published counts](https://github.com/tlaplus/Examples/blob/6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f/specifications/Disruptor/manifest.json).

Across all nine public Disruptor checks, whole-project recheck took **5.160 s**
against **7.129 s** for fresh checking, a **1.38×** median paired speedup.
On the final upstream edit MPMC safety took 16.27 ms instead of 970.35 ms
(59.76× paired); MPMC liveness took 16.13 ms instead of 1,074.22 ms (66.85×).
The SPMC edit required an ordinary check. Each value below is the median of
five cumulative foreground measurements in milliseconds on one native EC2
c7i.4xlarge. The runner rotated the order of four methods. The local cache
began empty for each method and each trial.
[Every paired sample, step route, input and executable identity](data/2026-11-20-installed-public-edit-histories.json).

| Input histories | Build | Isolated fresh | Shared exact receipts | Edit session | Recheck with prior |
|---|---|---:|---:|---:|---:|
| Disruptor, selected files | Before | 7,165.22 | 5,123.80 | 7,171.64 | 5,123.22 |
| Disruptor, selected files | After | 7,141.89 | 5,099.74 | 7,105.18 | 5,134.12 |
| Disruptor, whole project | Before | 7,170.33 | 7,176.12 | 7,188.09 | 7,198.77 |
| Disruptor, whole project | After | 7,128.88 | 7,115.68 | 7,085.77 | 5,160.46 |
| CCF, three refusals | Before | 47.19 | 47.15 | 57.77 | 47.70 |
| CCF, three refusals | After | 46.84 | 46.49 | 32.96 | 47.36 |

Sail now shares the current frozen input between attempted edit-session reuse
and ordinary fallback. The CCF session therefore performs prebinding once
per refused input. Those measurements describe refusal latency.

Each source revision is measured twice. In the **selected** case each request
contains its actual imports and selected CFG. In the **project** case each
contains all published source and configuration files selected for the three
entry points. The worker resolves each root using normal prebinding and
compares ordered module origins, exact loaded file bytes, the configuration
and search limits. An exact cache is already sufficient for the unchanged
MPMC requests in the selected case. The project case exercises the new
`unchanged-loaded-program` proof after the sibling edit changes the request
digest. This proof carries the current identity and the establishing receipt.
The next ordinary check at the current identity still explores fresh.

| Published Disruptor CFG | Distinct | Generated | Depth |
|---|---:|---:|---:|
| SPMC | 8,496 | 28,049 | 82 |
| MPMC | 112,929 | 422,781 | 81 |
| MPMC liveness | 14,365 | 44,581 | 61 |

The positive runs use 200,000 states, depth 128 and at most 300 seconds per
ordinary process. An edit-session has the same wall limit for its whole batch.
Each method finished inside its allowance. An initial 5,000-state diagnostic
instead stopped the two original SPMC and MPMC programs in each revision.
A limit stop supplied no proof. The three original CCF single-node
commit-reachability jobs refused during prebinding with `SAIL4202` on the
unqualified standard TLC module. Reused sessions preserved all three
refusals. [CCF consistency source](https://github.com/microsoft/CCF/tree/8c4bd0ae3c5a58d9c4f6fec1f66f8a97a7e51f56/tla/consistency).

A separate two-writer SPMC control changes the constants to two model-valued
writers, one reader, size two and two published items. Before the correction
TLC refuted NoDataRaces at depth three. The measured Sail build reported
`SAIL4203` because its trace codec did not support native model values. The
[model-valued witness control](#model-valued-disruptor-witness) below reruns
that job with the lossless codec. After the source fix, both checkers
rejected the false ExactlyOneWriter assumption. An
ordinary stop after one state cannot certify equivalence of edited sources.
Moving a TLA+ bullet can change a predicate with identical nontrivia tokens,
and a module extracted from a comment can change the running program. Edits
to selected sources therefore continue through normal checking in this
experiment.

Set `SAIL` to the installed executable alongside its manifest and native
worker. The driver downloads only the listed public files, checks their
SHA-256 values, recomputes canonical requests and verifies every result
against an independently executed, uncached current request.

```sh
python3 scripts/benchmark-public-temporal-history.py --sail "$SAIL" \
  --input-cache /tmp/sail-history-inputs --trials 5 > history.json
```

## Model-valued Disruptor witness

The two-writer control above changes four constants in the published SPMC
configuration and retains all original module bytes. At
`45c1cfd7` the staged Sail worker reported the same invariant, generated
and distinct counts, and depth as TLC. At `c757b12b` the source added
`ExactlyOneWriter`; its assumption rejected the same configuration.
[Upstream source correction](https://github.com/tlaplus/Examples/commit/c757b12b7a4de6fa39523db541133fece9ced416).

| Selected source and operation | Observed result |
|---|---|
| Older source, ordinary check | `NoDataRaces` refuted, 4 generated, 4 distinct, depth 3 |
| Same check and cache | Exact receipt returned the identical three-state witness |
| Separate validation against original source | Valid |
| Same-spelled ordinary string substituted for one initial model value | Invalid |
| Corrected source, ordinary check | False assumption at line 25, no witness |

The installed CLI and native worker, canonical requests, trace digest,
individual timings and platform are recorded in the
[measurement](data/2026-11-20-model-valued-witness.json).
The pinned TLC oracle used JAR SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
It exited 12 on the invariant and 10 on the false assumption. Sail's
runtime treats a completed check and a completed validation operation as
successful commands. Read their typed outcomes to distinguish a refutation,
a valid witness and an invalid witness. The assumption rejection exits 1.

```sh
python3 scripts/benchmark-public-temporal-history.py --sail "$SAIL" \
  --input-cache /tmp/sail-history-inputs --two-writer-control --wall-ms 10000
```

## SlateDB WAL maintenance checks

Sail checks the released SlateDB WAL module with all six configured invariants, the author's permutation symmetry and deadlock checking disabled. The original three-writer, one-restart CFG reached a deliberate 100-state limit under both the bundled library and the pinned project-provided community sources (258 generated, depth 8, 38 pending). Its earlier worker refused the imports with `SAIL4202`. The installed Sail supervisor allows five minutes per request or batch. Full completion of the released CFG was measured separately with ordinary Core and TLC. [Upstream module and CFG](https://github.com/Vanlightly/s3-wal-collection/tree/35fdc537af4d226f43b4574cded1764375f264ac/slatedb) · [Full Core/TLC comparison](https://github.com/Chelis-Lang/dive/blob/main/conformance/reports/perf/2026-11-21-slatedb-full-comparison.json).

The two completed Sail controls retain the two data values, garbage collector and original TLA+ bytes. Each uses two writers. The edit changes the operative CFG assignment `EnableFix = TRUE` to `FALSE`. Both switch positions exhaust these smaller domains with six invariants holding and an empty queue. The upstream model notes require more writers to exhibit the known regression.

| Restarts | EnableFix | Distinct symmetry representatives | Generated | Depth |
|---:|---|---:|---:|---:|
| 0 | TRUE | 74,637 | 198,123 | 69 |
| 0 | FALSE | 52,260 | 138,979 | 63 |
| 1 | TRUE | 3,761,377 | 12,508,420 | 93 |
| 1 | FALSE | 1,601,824 | 5,211,591 | 84 |

The installed `sail runtime execute --cache` launches a fresh worker for the first version and the substantive switch. Repeat and undo retrieve the matching complete ordinary response from the existing cache. Fresh controls for both versions use separate empty caches and match the counts, depth, configured checks and verdicts. The script checks the complete response digest on each receipt hit. A separate `sail runtime edit-session` batch measured the four versions through its ordinary fallback because the pinned graph-capture APIs decline this model.

Without restarts, the foreground cold/edit runs took 1.039/0.749 s and repeat/undo took 5.20/4.83 ms. With one restart they took 65.13/27.42 s and 5.20/6.83 ms. The separate one-restart fresh checks took 64.83/27.74 s. Ordinary peak RSS was 1,548,388 KiB for the fixed one-restart search and 594,892 KiB for its edit. The four-check ordinary-fallback sessions took 3.59 and 182.69 s. These single samples came from a development c7i.4xlarge with overlapping build and validation work. The JSON includes the earlier observations as well.

Use the public CLI and staged worker from the same build, the linked upstream `slatedb` directory, and the `modules` directory at the pinned CommunityModules revision. The script creates fresh temporary caches per invocation. It validates both source-byte identities and complete ordinary results, and prints a compact JSON record. [Pinned community sources](https://github.com/tlaplus/CommunityModules/tree/78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3/modules) · [All input and result hashes, worker identity, times and RSS](data/2026-11-21-slatedb-edits.json).

```sh
for name in Medium TwoRetry; do
  python3 scripts/benchmark-slatedb-edits.py --sail "$PREFIX/bin/sail" \
    --source "$WAL/slatedb" --modules "$COMMUNITY/modules" --case "$name"
done
```

### Exact results within the edit session

When graph capture declines, the isolated worker can still avoid repeating a concluded search within the same batch. Each `exact-request` points directly to the earlier complete ordinary response for the identical input. Both fixed and edited SlateDB searches run in full. Repeat of the edit cites entry 1 and undo cites entry 0. This applies to genuine refutations as well as successful exhaustive checks.

The comparison used an EC2 c7i.2xlarge reserved for the benchmark, one pinned logical CPU, identical model and configuration bytes and newly empty caches. Each round measured independent ordinary checks, persistent receipt hits, direct fresh controls and a four-entry batch. Across three rounds each worker checked both two-writer domains. The script compared all six invariants, verdicts, generated and distinct counts, final queue and depth. The source explains why the substantive switch still needs a new proof. `PostReplayState` chooses `VALIDATE_AFTER_REPLAY` when fixed and `READY` when unfixed. Reaching the same replay step can therefore produce a different successor. Reusing the earlier invariant result across this edit would require a checked relation between the two reachable graphs or a separately established inductive proof. The original three-writer CFG and the upstream failing domains have separate measurements.

| Restarts | Four-check session before (s) | Four-check session after (s) | Paired median speedup | Worker repeat / undo after (µs) |
|---:|---:|---:|---:|---:|
| 0 | 3.563 | 1.788 | 1.99× | 7 / 10 |
| 1 | 177.280 | 85.345 | 2.05× | 7 / 11 |

The table reports medians of three rounds. With one restart, the new worker’s ordinary CLI runs took 59.81 s cold and 25.11 s for the edit. Persistent receipt repeat/undo took 5.05/4.72 ms including process launch. The empty-cache fixed/edited controls took 60.79/25.07 s. In-batch repeat/undo costs above are worker time within a single CLI call. [Each step, exact request and response identities, proof scopes, counts, RSS and timings](data/2026-11-21-slatedb-exact-session.json).

The independent executable control compared 20 batch entries with 20 fresh private-cache calls. Five exact repeats/undos pointed to the original native evidence. In the false case `EventuallyTwo`, the independent trace validator accepted the returned lasso and rejected a changed initial state. A selected module byte, CFG byte, root, CFG selection, unused project file or limit changed the request identity. Stopped, rejected and unsupported repeats made a fresh attempt. A 1 ms timeout returned no partial entries. One worker process handled the entire batch. Parent transport tests rejected a chained, forward, self, out-of-range, incomplete or different-input basis.

A second negative control leaves the original WAL module and six checks intact and appends its existing `Init` as a seventh invariant. A real successor violates this deliberately stronger condition. Both workers returned the same two-state native word, also accepted by a separate installed `validate` request. Its repeat cited the original refutation. Use `--witness-only` to reproduce this exercise.

Run the earlier reproduction command with `--exact-session` on the newer installed pair. The old pair uses the same script without this option. Each invocation validates the expected routes and produces a complete JSON record.

### Cross-process action closure in multi-module models

Sail and Dive ran original imported KVS and TCommit protocol definitions through
the installed `sail runtime symbolic-check --property-proof` path, starting a
new process for each edit. Each small model had three trials covering an
unchanged revision, a restricted guard, an expanded guard, a constant that
changed the finite layout, a different requested property, a protocol fault and
undo. Cold certification, transferred predicates and authenticated transfer
agreed on all 48 comparisons. All three paths independently validated the
four-state KVS rollback bug and the eleven-state TCommit abort bug.

For the larger KVS panel, five concurrent transactions used two keys and two
values. Cold establishment of `TxLifecycle` took 87.765 s. Each subsequent
sample started a fresh Sail/Dive pair. The table gives medians over five trials
in alternating order on one core of a dedicated Linux c7i.4xlarge.

| Revision | Portable predicate, ms | Authenticated predicate, ms | Closure images reused |
| --- | ---: | ---: | ---: |
| Unchanged | 555.861 | 354.933 | 45 / 45 |
| Restrict opening a transaction | 505.004 | 354.728 | 40 / 45 |
| Expand a write guard | 555.073 | 454.053 | 25 / 65 |
| Undo | 555.277 | 355.052 | 45 / 45 |

The unsigned predicate already avoids a new fixed-point search. Authentication
reduces the remaining cost when individual transition images are expensive.
The direct Dive crossover panel broke even at four transactions. At 100
TCommit managers the unsigned checker spent 0.681–0.688 s inside its worker
and the authenticated checker spent 0.719–0.725 s, despite reusing all 300
images. The small cases favor leaving authentication disabled. When a candidate
was rejected, starting a second process for a fresh search also showed up in
the wall totals. The recorded timings separate these cases.

[All individual observations, implementation identities and commands](data/2026-11-22-named-obligations.json) · [Public model inputs and edit driver](../../scripts/benchmark-named-obligations.py) · [Dive obligation authentication](https://github.com/Chelis-Lang/dive/pull/1287).

## Repairing named temporal counterexamples

A saved Elevator counterexample still refutes the property after an eighth floor
is added, even though the original 249-state word fails the edited Next relation.
An explicit floor-coordinate mapping preserves the anchors. Dive's source-bound
repair fills 13 gaps, generates 189 successors and independently validates all
262 states and the loop. In the second public model, a transition edit forces
the real-time clock to advance one unit at a time. One gap and six generated
successors give a four-state counterexample.

A dedicated EC2 c7i.4xlarge in us-east-2 ran five sequential CLI processes per
case with a debug Sail build and release Dive. Numbers below are all individual
wall times in milliseconds and maximum peak RSS in KiB from GNU time. Sail starts
a trusted checker process for each accepted donor. Each returned word also passed
a separate standalone replay outside the timed interval. The host ran Ubuntu
24.04.5, Linux 7.0.0-1018-aws, x86-64. No cold eight-floor search was repeated.

| Case | Result | Five wall times (ms) | Median (ms) | Maximum RSS (KiB) |
|---|---|---|---:|---:|
| same-clock | unchanged, exit 13 | 13.483, 13.476, 13.130, 13.023, 12.444 | 13.130 | 19,812 |
| repair-clock | repaired, exit 13 | 15.322, 17.705, 14.943, 17.733, 16.069 | 16.069 | 20,388 |
| fixed-clock | success, exit 0 | 21.674, 21.909, 21.993, 21.665, 23.006 | 21.909 | 20,228 |
| stale-clock | liveness-violation, exit 13 | 22.126, 22.769, 22.468, 21.829, 20.788 | 22.126 | 24,544 |
| fair-clock | liveness-violation, exit 13 | 26.606, 28.529, 28.988, 30.384, 29.063 | 28.988 | 26,664 |
| malformed-donor | success, exit 0 | 13.466, 13.428, 13.484, 13.467, 13.997 | 13.467 | 19,588 |
| missing-donor | success, exit 0 | 10.956, 10.960, 10.728, 10.872, 11.407 | 10.956 | 19,752 |
| same-elevator | unchanged, exit 13 | 187.818, 187.796, 182.584, 183.539, 183.064 | 183.539 | 50,936 |
| repair-elevator | repaired, exit 13 | 232.504, 241.149, 230.952, 232.415, 227.260 | 232.415 | 52,552 |
| cold-deadline | stopped, exit 4 | 1404.861, 1406.980, 1406.896, 1405.626, 1404.817 | 1405.626 | 191,852 |

The fixed property, absent donor and malformed donor all caused an ordinary
216-state check that succeeded. Changed fairness and a changed property name
rejected the saved word. Each ordinary search found a different current
violation in the 216-state graph. The one-second deadline stopped a real cold
seven-floor search. Its wall time includes the process-group termination grace.

The original seven-floor counterexample took 246.10 s and 1,555,192 KiB to
discover on a separate isolated host. Earlier cold eight-floor search stopped
after 360 s at 7,109,953 distinct states. Saved-word validation establishes a
refutation and reports no exploration count. The original clock witness took
under 10 ms to discover, so replay has no measured latency advantage for
that small example. [Dive's independent measurements, input digests and cold
controls](https://github.com/Chelis-Lang/dive/blob/e1b70325c7a852cd26e13ac1ef17460de2425bd4/conformance/reports/perf/2026-11-22-temporal-repair.json)

Reproduce with Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` and
Dive `e1b70325c7a852cd26e13ac1ef17460de2425bd4`. The first command discovers
the seven-floor donor if it has not been retained. Pass `--seven-word FILE` to
use an existing original. Run from the Sail checkout and supply the local paths.

```sh
python3 "$DIVE_SOURCE/scripts/perf/scale/temporal-repair-panel.py" \
    "$DIVE" "$EXAMPLES/specifications" "$PANEL"
python3 "$DIVE_SOURCE/scripts/perf/scale/elevator-lift-word.py" \
    --anchors-only "$PANEL/seven.json" 1,3,4,5,6,7,8 "$PANEL/anchors.json"
python3 scripts/collect-witness-recheck.py "$SAIL" "$DIVE" \
    "$EXAMPLES/specifications" "$PANEL" "$PANEL/anchors.json"
```

Measured executable SHA-256 digests were `4761058590f02ed412aebc822abc3aaaed33cee4daae9382703d54f1f55bbe2a` (Sail) and
`7cfece0bda47bc911c6df5ecc603a387db10c39704a7159145399a6b35f37978` (Dive). Raw observation digests in run order follow.

- `bfc893cbd5d7ce31169914489e7e3a23ef8558a976446c99e025f558a48ece05`
- `8dfbb62d9af0805c46ee90e2a5688ca3663fd174350f51a14b7dd266d0f4cc7f`
- `b4096c054b76bf8eca1803628491ad559104dc94f05492a943215f2af3aaa7ba`
- `859c20eb6cad873c012ec1158ac5650d1ded2da44fa61d99115dd60bd506c597`
- `7cb110f350d6a7e21253812c4e8eaa0b3e09cc9f6fd95f636b9e7f6fdce7fba1`

### Donor selection and typed embeddings

Reproduce with `python3 scripts/collect-witness-selection.py SAIL DIVE Elevator.tla n7.cfg n8.cfg SEVEN.word`.
Use the unmodified 218-state donor from the original
[seven-floor cold search](https://github.com/Chelis-Lang/dive/blob/main/conformance/reports/perf/2026-11-22-cold-fair-elevator.json).
The panel uses source SHA-256 `c44366dbf4f4bdef30edade0c8a2faf75fd37b2b7aeb1c04b24e80eb40ed2203`
and donor SHA-256 `f1cec496a0b8a3bee51d631324cdd754565ee0e4fa3a382ad37d5e253c7816d4`.

On a dedicated c7i.4xlarge Linux host the fresh indexed eight-floor job took
285.38 ms and 42,644 KiB peak RSS. Sail declined the first donor's prior CFG,
accepted the second after prior replay, and chose `[1,3,4,5,6,7,8]`. Initial
lookup took 41.51 ms, the two original replay attempts took 23.76 and 61.05 ms,
and mapping took 1.87 ms. Dive generated 157 successors across 11 gaps in
12.39 ms and validated the complete 229-state replacement in 65.04 ms.
Separate replay accepted it in 68.38 ms. The published donor-free eight-floor
baseline took 325.10 s on the same instance type.

The typed actor control mapped one model atom and one integer coordinate,
preserving the unrelated integer field. Sail took 30.08 ms and 19,508 KiB.
Separate replay took 7.68 ms. Changed fairness and a fixed property returned
ordinary success, with ordinary checking taking 6.64 and 4.53 ms respectively.
Ambiguous fields and damaged, corrupt, or missing donors took ordinary
fallback. The one-second deadline stopped search, with a further 0.40 s for
process cleanup. SIGINT also stopped and reaped the worker.
