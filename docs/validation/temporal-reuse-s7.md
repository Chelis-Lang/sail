# Temporal job reuse on the S7 corpus

This experiment measures how much work Sail can reuse across native TLA+ checks. It compares the native 0.3.0 source, the same source with temporal receipts, and pinned TLC. All three ran on the same host with identical model and configuration bytes. [Reuse issue](https://github.com/Chelis-Lang/sail/issues/774) · [Sail v0.3.0 baseline](https://github.com/Chelis-Lang/sail/releases/tag/v0.3.0) · [Receipt implementation](https://github.com/Chelis-Lang/sail/pull/777).

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
