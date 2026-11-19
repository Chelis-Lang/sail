# Temporal job reuse on the S7 corpus

This measurement follows [Sail #774](https://github.com/Chelis-Lang/sail/issues/774). S7's pinned Examples and TLC oracle supply the models and the original expected results. Each tool received the same source bytes and configuration for each edit. All timings come from the single host described below.

## Results

Each cell reports the median of three foreground times in milliseconds. The last column reports the median of the three paired speedups against Sail. All 333 verdicts and distinct-state counts per engine matched.

| Operation | TLC | Sail 0.3.0 candidate | With reuse | Speedup vs Sail |
|---|---:|---:|---:|---:|
| Mutex, first check | 14,085 | 13,961 | 14,125 | 0.99× |
| Mutex, unchanged | 14,108 | 14,091 | 9.54 | 1,484× |
| Mutex, invariant edit | 13,004 | 13,340 | 13,662 | 0.98× |
| Mutex, revert | 14,089 | 14,086 | 9.53 | 1,492× |
| Internal memory, first check | 1,473 | 175.0 | 175.3 | 1.00× |
| Internal memory, property edit | 1,128 | 107.6 | 107.7 | 1.00× |
| DieHard, first refutation | 535.3 | 20.08 | 19.86 | 1.02× |
| DieHard, unchanged refutation | 526.1 | 19.61 | 9.30 | 2.10× |
| DieHard, edit preserving failure | 531.5 | 19.66 | 20.11 | 0.97× |
| DieHard, fix | 503.6 | 16.92 | 17.32 | 1.01× |
| DieHard, undo fix | 530.5 | 19.78 | 9.69 | 2.06× |

MCLamportMutex has 724,274 distinct states. Its unchanged check ranged from 9.38 to 9.94 ms, and its revert ranged from 9.40 to 9.56 ms. All twelve repeat and undo operations replayed their original evidence. The edited invariant and property required full exploration. DieHard’s short fresh search left less work to eliminate.

The 100-check loop took **1.592 s** with reuse (range 1.549–2.200 s), **5.145 s** on the no-reuse native candidate (5.138–5.154 s) and **73.475 s** on TLC (73.112–74.607 s). Median paired speedups were **3.23×** against the no-reuse candidate and **45.94×** against TLC. Each trial had 76 exact hits. The ranges describe the three observed runs.

The separate native extension integration completed the same 111 calls in one trial. Its mutex first check took 60.73 s, its unchanged check 15.85 ms and its undo 15.53 ms. The four-model loop took 3.213 s with 76 hits. All distinct-state counts agreed with TLC.

Independent Linux process tracing observed one worker launch for fresh Refuted and fresh Holds, then zero for each exact replay and revert. A separate CI exercise signed both real results, transferred only the bundle and signature, then imported them into a new cache with a new daemon. Both results replayed with identical evidence and zero worker starts. Changing the caller preserved the proof and updated the attribution. Changing the wall limit launched a fresh worker. Both native installations passed. Group-writable artifact copies were refused. These controls ran outside the timed samples.

The integrated extension then repeated the short controls at `e2961e68`. With both `0022` and `0002` umasks, the signed action round trip, default and explicit cache paths, daemon and embedded entry points, and preparation sharing passed. Its single 100-check run took 2.868 s with 76 hits and matched every TLC distinct count and verdict.

A changed model can still admit yesterday's violating trace while a fresh
bounded check stops at an earlier branch or encounters an evaluation error.
The production path therefore explores those edits afresh. The proposed
[bounded refutation replay API](https://github.com/Chelis-Lang/dive/issues/1041)
and [completed-graph recheck API](https://github.com/Chelis-Lang/dive/issues/1039)
describe the evidence needed to accelerate those cases.

### Prepared first check

Five rounds rotated among disabled preparation, worker-image preparation and preparation of the actual MCLiveInternalMemory job. Each trial used fresh application caches. Foreground was measured after the asynchronous preparation child completed. The table separates the median post-install handoff from time until ready and time for the first foreground check. It excludes the common download and extraction steps.

| Native 0.3.0 candidate | Handoff (ms) | Ready after handoff starts (ms) | First check (ms) |
|---|---:|---:|---:|
| disabled | 0.01 | 0.01 | 169.41 |
| worker | 1.98 | 17.40 | 169.79 |
| first-job | 2.24 | 216.32 | 3.59 |

## Measurement

One us-east-1 EC2 c7i.4xlarge ran all engines serially. It had a Xeon Platinum 8488C, 16 vCPUs, SMT enabled, 32 GiB nominal RAM, Ubuntu 24.04.5 and kernel 7.0.0-1017-aws. CPU frequency and OS page cache were uncontrolled. The original S7 used a c7i.8xlarge. Each Sail scenario sequence and each loop started with a new cache and a new daemon. Each job invoked a new client through the public Unix socket. The post-install trials used the embedded CLI. TLC started a new pinned OpenJDK 17.0.20.1+1 process per check with ParallelGC, one worker and the default heap. Sail used one worker, seed and fingerprint zero, the exact store and S7's state and depth limits.

The no-reuse Sail control is the native 0.3.0 release candidate from [Sail #763](https://github.com/Chelis-Lang/sail/pull/763), compiled from `592001a2`. The release-candidate snapshot `9004b6ad` changes nine documentation files and has identical build inputs. Both lock D.I.V.E. to `3d5923a1`. At measurement time v0.2.0 was the latest published Sail release. The reuse release candidate combines #763 at `9004b6ad` with the #774 integration at `dcc020b6` (resulting source tree `ea59979f402c91ff3ab2fdf4b3467dc000f429b0`). The separately measured `dcc020b6` extension uses D.I.V.E. `1025cd6a`. The benchmark compares elapsed foreground command times. Fixture preparation, writing request files, daemon startup and inspecting the output were outside those times.

The 100-check workload cycles through DieHard, EWD840, MCRealTimeHourClock and MCLiveInternalMemory five times each. Each batch runs the base twice, edits a predicate or finite model domain, checks the edit twice and restores the base. This creates 24 distinct requests and 76 opportunities for exact reuse. These weights describe a scripted maintenance workload. Actual workloads can have different hit rates.

The dedicated scenarios drop one invariant from MCLamportMutex, drop one temporal property from MCLiveInternalMemory, strengthen DieHard's failing invariant with an already true conjunct, change it to hold, and undo the edit. The benchmark checks each intended verdict. Every returned refutation must carry matching native and ITF traces and successful independent validations. Every replay must preserve the complete original response digest, identify a stable 256-bit key, and bind to the current job. Comparing the cross-engine result streams checks the distinct-state counts for edited models as well.

## Reproduce

Use the D.I.V.E. `94cbbed9` S7 scripts and goldens, Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, and TLC jar with SHA-256 `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`. Build each native release candidate in an exclusive checkout and target with Rust 1.99.0.

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

The external evidence archive `2026-11-19-temporal-reuse-e2961e68.tar.gz` (SHA-256 `062d951c67f2658706f512b985bd72a3adbc9fd8b476a80af7e9b78d966a9fd4`) retains raw sample rows, process counts, host and binary identities, and the measured #763 overlay.
