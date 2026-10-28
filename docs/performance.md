# Performance

Sail speeds up Verus verification by reusing a released vstd proof, previously
verified targets and exact solver answers. Edits still pay Rust compilation
and the Verus frontend for changed targets. Undoing an edit can restore an
original complete proof when retaining that target was worth its cost.

Current main includes guided installation, streaming transcript hashing,
credential-context fixes and the indexed erasure runtime. The compiler change
reduces cached Anvil compiler time by 10.6% on Linux and 13.4% on Mac.
Explicit older-Cargo script projects also preserve Cargo freshness: three
native Anvil pairs reduce unchanged calls from 60.97 to 0.89 s, without
proof reuse ([qualification](validation/unsupported-cargo-fallback.md)).

The installed measurements below use frozen Sail `e51e4660` plus runtime
`36d65c3f` and its qualified retained-proof manifests. Later main changes have
separate cumulative checks; these numbers do not claim a later main binary
was timed. Both platforms complete 31 valid comparisons and all four
three-round mutation cohorts, with 651 matching comparisons each. See the
[qualification and raw records](validation/shadow-index.md).

| Complete mutation loop | Linux speedup | Mac speedup |
|---|---:|---:|
| HumanEval | 2.71× | 2.87× |
| IronKV | 1.47× | 1.42× |
| Vest | 1.11× | 1.40× |
| CapybaraKV | 2.04× | 2.37× |

Small-project first use improves 33.17 → 13.21 s on Linux and
16.85 → 10.08 s on Mac. Linux Anvil cold improves 186.13 → 172.99 s;
body and specification edits improve 153.62 → 85.89 s and
153.06 → 33.28 s. Mac Anvil body and specification edits improve
79.43 → 41.11 s and 79.47 → 14.35 s.

Unchanged Anvil invokes zero verifiers but still costs 1.95 s versus stock
0.33 s on Linux and 1.83 s versus 0.28 s on Mac in that matrix. Current main
replays unchanged calls on both platforms; see the
[current macOS numbers](#macos-on-current-main). Tiny edits add about
0.36 s on Linux and 0.67 s on Mac. Mac Anvil cold adds 0.86 s on a roughly
100-second run; Vest adds 0.06 s. Individual slower mutation edits remain
visible. Ten Linux rows and twelve Mac rows require performance review;
functional qualification does not grant a performance waiver.

The [historical private rc.6 matrix](validation/release-matrix.md) describes
its own frozen artifacts. No new Sail release is created by these changes.

The [ordered-projection follow-up](validation/ordered-input-projection.md)
measures a small further unchanged-Anvil reduction (about 4% Linux, 3% Mac)
and separately identified Mac cold controls. The combined candidate
measures Anvil cold at 99.665 → 100.039 s and Vest at 39.116 → 38.029 s
(three rounds). Anvil cold and unchanged costs remain open. These measurements
do not replace the frozen matrix below or accept its remaining slower rows.

## macOS on current main

These numbers come from main `dca5cd43`, which includes the macOS replay fix
(#451), build-script receipts for Cargo 1.93–1.99
([ADR 0084](decisions/0084-qualified-cargo-range.md)) and the
focused fail→fix fix (#450). Sail ran as a release stage
(`scripts/release.sh stage`) with the pinned macOS engine and its shipped vstd
proof.

The host was an EC2 mac-m4.metal (Apple M4, 10 cores, 24 GiB) in us-east-1b,
running macOS 27.0 (26A428), on 2026-10-27. `scripts/perf-real.sh run`
compared Sail against stock `cargo verus verify` 0.2026.10.18.7d82994 over six
paired rounds, alternating which arm went first. Each arm started from a fresh
copy and an empty Sail cache, with vstd from crates.io.

- Vest passes `--rlimit 100` on both arms.
- Anvil keeps its own `rust-toolchain.toml` (Rust 1.98.0) on both arms. Under
  ADR 0084 it now gets receipts and replay; earlier it fell back with
  `SAIL3139`. The other projects run on 1.99.1.

The phases, in order:

1. **cold**
2. **unchanged, first:** records the replay.
3. **unchanged, second:** replays.
4. **unchanged with `SAIL_MANAGED_NOOP=0`:** the full unchanged path.
5. **one-line edit:** `body-early`, an `assert(true);` in one function.
6. **revert**
7. **fail:** a failing `assert(1 + 1 == 3);`.
8. **fix:** the same line made true, straight from the failing state.
9. **fix revert**

Each cell is the median of six rounds, with the range in brackets, in seconds.
Every exit code matched stock's: fail exits 101 on both arms.

| Project | Phase | Stock | Sail | Stock/Sail |
|---|---|---:|---:|---:|
| Vest | cold | 38.4 [38.3–38.5] | 38.2 [38.0–38.5] | 1.01× |
| | unchanged, first | 0.26 [0.26–0.26] | 0.79 [0.79–0.81] | 0.33× |
| | unchanged, replayed | 0.25 [0.25–0.26] | 0.09 [0.09–0.09] | 2.81× |
| | unchanged, `SAIL_MANAGED_NOOP=0` | 0.26 [0.25–0.26] | 0.57 [0.55–0.61] | 0.45× |
| | one-line edit | 22.0 [22.0–22.2] | 12.5 [12.5–12.7] | 1.76× |
| | revert | 22.2 [22.0–22.2] | 1.11 [1.08–1.14] | 20.0× |
| | fail | 20.6 [20.6–20.7] | 10.9 [10.8–11.0] | 1.89× |
| | fix | 22.2 [22.1–22.2] | 12.5 [12.3–12.5] | 1.78× |
| | fix revert | 22.2 [22.0–22.3] | 1.08 [1.07–1.13] | 20.6× |
| IronKV | cold | 20.1 [20.0–20.4] | 12.3 [12.2–12.4] | 1.63× |
| | unchanged, first | 0.21 [0.20–0.21] | 0.28 [0.28–0.29] | 0.74× |
| | unchanged, replayed | 0.20 [0.20–0.21] | 0.05 [0.05–0.05] | 4.05× |
| | unchanged, `SAIL_MANAGED_NOOP=0` | 0.21 [0.20–0.21] | 0.24 [0.24–0.25] | 0.84× |
| | one-line edit | 4.25 [4.24–4.28] | 3.32 [3.31–3.33] | 1.28× |
| | revert | 4.26 [4.24–4.28] | 2.21 [2.21–2.22] | 1.93× |
| | fail | 3.99 [3.97–4.01] | 3.11 [3.10–3.13] | 1.28× |
| | fix | 4.26 [4.24–4.31] | 2.94 [2.92–2.96] | 1.45× |
| | fix revert | 4.26 [4.25–4.27] | 2.21 [2.20–2.22] | 1.93× |
| CapybaraKV | cold | 31.0 [30.9–31.2] | 30.1 [29.8–30.9] | 1.03× |
| | unchanged, first | 0.22 [0.22–0.23] | 0.59 [0.58–0.62] | 0.38× |
| | unchanged, replayed | 0.22 [0.22–0.22] | 0.04 [0.04–0.05] | 5.12× |
| | unchanged, `SAIL_MANAGED_NOOP=0` | 0.22 [0.22–0.22] | 0.45 [0.44–0.48] | 0.50× |
| | one-line edit | 14.8 [14.8–14.8] | 5.98 [5.95–6.05] | 2.47× |
| | revert | 14.9 [14.8–14.9] | 5.60 [5.58–5.63] | 2.65× |
| | fail | 14.0 [14.0–14.1] | 5.03 [5.02–5.04] | 2.78× |
| | fix | 14.9 [14.9–14.9] | 5.90 [5.86–5.93] | 2.52× |
| | fix revert | 14.9 [14.8–14.9] | 5.62 [5.58–5.81] | 2.65× |
| Anvil (`--verify-module vdeployment_controller`) | cold | 99.5 [99.2–99.7] | 100.2 [100.1–100.4] | 0.99× |
| | unchanged, first | 0.30 [0.28–0.30] | 2.07 [2.03–2.11] | 0.14× |
| | unchanged, replayed | 0.24 [0.24–0.25] | 0.06 [0.06–0.06] | 4.18× |
| | unchanged, `SAIL_MANAGED_NOOP=0` | 0.24 [0.24–0.25] | 1.59 [1.58–1.60] | 0.15× |
| | one-line edit | 79.0 [78.9–79.4] | 41.0 [40.9–41.1] | 1.93× |
| | revert | 79.1 [79.0–79.3] | 14.5 [14.5–14.5] | 5.45× |
| | fail | 75.1 [75.0–75.3] | 12.0 [11.8–12.0] | 6.28× |
| | fix | 79.1 [78.9–79.6] | 23.5 [23.4–23.5] | 3.37× |
| | fix revert | 79.1 [78.9–79.4] | 14.5 [14.4–14.6] | 5.46× |

- **Edits, failing edits, fixes and reverts are faster on every project.** A
  revert takes about 1 s on Vest, against 22 s with stock. A failing edit on
  Anvil takes 12 s, against 75 s with stock.
- **Replayed unchanged calls take 0.04–0.09 s, against stock's 0.20–0.25 s.**
- **The first unchanged call after a verification is still slower than
  stock.** That call takes the full path and records the replay:
  - Anvil: 2.07 s, against 0.30 s.
  - Vest and CapybaraKV: about 0.6–0.8 s, against 0.2–0.3 s.
  - IronKV: 0.28 s, against 0.21 s.

  The `SAIL_MANAGED_NOOP=0` row shows the full path without recording. Every
  later unchanged call replays.
- **Cold runs.** IronKV is the only project that gets the shipped vstd proof,
  so it is the only one with a large cold gain. The other three verify vstd
  themselves, because each misses the proof:
  - Vest's `--rlimit 100`
  - CapybaraKV's `--no-default-features` resolution
  - Anvil's patched Verus sources

  Anvil cold is 0.7 s slower than stock on about 100 s (0.7%), with ranges
  that do not overlap. Vest and CapybaraKV cold are slightly faster.

The load average during these runs was 2–17, from the runs' own Verus and Sail
work; nothing else ran on the machine. `ci/run.sh host` on the same build passed
1134 tests, with clippy clean. Raw logs are kept outside the repository
(`mac-final-phase2.tgz`, sha256
`e1a29b7731fea26cb3ecae2c57ceefa97faedfbc602dd075bb9c74d8fd739fae`).

To repeat a project's measurement, put the stage's `bin/`, stock Verus and
`sail-conformance` on `PATH` and run:

```sh
RUSTUP_TOOLCHAIN=1.99.1 SAIL_PERF_VSTD=registry SAIL_PERF_VERUS_TIMES=0 \
SAIL_PERF_EDITS="body-early fail fix" SAIL_PERF_REVERT="body-early fix" \
SAIL_PERF_NOOPS=0 SAIL_PERF_UNCHANGED=2 SAIL_PERF_NOREPLAY=1 \
  scripts/perf-real.sh run vest 6    # vest also takes SAIL_PERF_VERUS_ARGS="--rlimit 100"
```

For Anvil, leave `RUSTUP_TOOLCHAIN` unset so that its own toolchain file
applies. Anvil also needs OpenSSL (`brew install openssl@3`, then set
`OPENSSL_DIR`).

## What to expect

- **First use.** A matching project skips vstd verification using the proof
  in the release archive. Install Rust 1.99.1 with `rust-src`, as described in
  the [README](../README.md#install). Extra compiler components, dependency
  differences or forwarded Verus arguments can make that proof unavailable;
  Sail explains the miss and verifies normally. Signed solver replies in the
  archive can still answer exact matching queries.
- **Edits.** Unchanged targets keep their reports. Changed targets run the
  compiler and verifier, using cached solver answers where the exact query
  bytes match. The result depends on module layout and the change: a common
  specification can invalidate much more work than a local proof edit.
- **Undo.** Expensive targets can restore a previously verified revision.
  Cheaper targets use ordinary solver caching instead, avoiding the cost of
  retaining every compiled revision. See [instant revert](instant-revert.md).
- **Unchanged runs and tiny edits.** Sail still checks inputs and returns a
  complete report. Plain Cargo can do less work on a no-op, so these calls
  have visible overhead. Anvil's unchanged partial verification remains a
  particular setup cost tracked in [known issues](known-issues.md).
- **CI.** A fresh runner gains from persisted solver replies, and in
  SAIL3112 mode from a persisted `target/`. Managed targets and receipts do
  not move between machines yet. See [caching Sail in CI](ci-cache.md).

The disabled v1 compiled-dependency store is not part of these claims. Other
compiled dependencies are built separately for each project and worktree
([ADR 0053](decisions/0053-disable-unqualified-native-store.md)).

The release benchmark uses seeded mutations over HumanEval, IronKV, Vest and
CapybaraKV, with plain Verus as the oracle and one daemon across each complete
edit sequence. It includes failed proofs and ordinary and forced returns to
the original. A single scripted edit cannot establish an edit-loop claim.

Corpus projects use the release's pinned vstd versions; the matrix lists the
changes from each project's own pins. Vest's default-limit corpus case still
reports the known resource-limit failure with both verifiers. Passing-workload
benchmarks use resource limit 100 on both arms. Mutation sequences include
failing proofs and require Sail to preserve their stock outcomes.
Successful shipped-proof reuse and instant undo have a separate public Vest
control in the [profile qualification](validation/rlimit-subsumption.md).

## Repeatable Cargo comparison

Use `scripts/bench-cargo-compare.py` to compare stock Verus, the plain verifier
fork, and Sail on a fixed source commit.
The runner changes only a private Git worktree, not the operator's source files.

Supply an unpacked stock release with the upstream commit from `sail engine status`.
Stock and fork runtimes must use identical Z3 bytes.
The runner selects the fork from Sail's validated engine.
Use `--engine-build` to select an external engine build.
Install its Rust toolchain before the run.
The selected project must contain a committed `Cargo.lock`.

### Scenarios

| Scenario | Cargo and proof state | Measured work |
| --- | --- | --- |
| `fresh` | New Cargo target per command, warm Sail solver cache | Complete fresh verification |
| `unchanged` | Retain artifacts after a successful baseline | CLI latency, new verification, or confirmed proof reuse |
| `proof-edit` | Retain baseline artifacts, then apply one declared edit | Fresh verification of the edited library |
| `daemon-restart` | New Cargo targets, retained disk cache, empty daemon memory | Complete verification plus new-daemon startup |

Select scenarios with `--scenarios`. The default is `fresh`.
Each scenario has its own solver cache and separate results.

Runs are serial. Six measured rounds cover all six execution orders.
Fresh, unchanged, and restart scenarios start with one warmup per verifier.
Each proof-edit round creates its own successful baseline per verifier.

Workflow scenarios (`unchanged` and `proof-edit`) need one selected library package.
For a workspace, set `--project` to the member directory.
The runner records its full package identity and uses its name for CLI selection.
An ambiguous workspace root is an error.

From the Sail checkout, the generic workload exercises all four scenarios:

```sh
REVISION="$(git rev-parse HEAD)"
python3 scripts/bench-cargo-compare.py \
  --project bench/cargo-example \
  --revision "$REVISION" \
  --stock-runtime ../verus-stock/verus-arm64-macos \
  --sail target/release/sail \
  --cargo "$CARGO_HOME/bin/cargo" \
  --cargo-home "$CARGO_HOME" \
  --rustup-home "$RUSTUP_HOME" \
  --scenarios fresh unchanged proof-edit daemon-restart \
  --edit-spec bench/cargo-example/proof-edit.json \
  --rounds 6 --threads 4 --jobs 4 --rlimit 20 \
  --out ../benchmark-results/cargo-workflows-1
```

Use the matching Linux stock archive on a native Linux host.
The output directory must be new.
Use another output directory for each sample set.

### Proof edits

The `proof-edit` scenario needs `--edit-spec`.
Its JSON object contains `path`, `before`, and `after` strings.
The path is relative to the selected Cargo project.
The runner replaces one exact match in a tracked Rust source file.
It rejects traversal, symlinks, empty or ambiguous matches, and identical replacements.

The generic [edit fixture](../bench/cargo-example/proof-edit.json) adds a bit-mask
fact to `low_bits_roundtrip` without a contract change.
Every verifier receives the same edit.
Each round uses an independent baseline artifact context.
An earlier edited receipt cannot replace fresh proof work in the next round.
The metadata records both source hashes and the complete edit specification.

Native proof counts can change when an edit adds an obligation.
The report compares stock and fork counts for the same source state.
It checks Sail's requested-root function count and identity against that pair's baseline.
Native obligation counts and Sail function counts remain separate.
These checks do not establish formal per-function conformance.

### Controls and timing

The runner records source, lock-file, executable, runtime, and implementation identities.
It checks source and runtime state before and after each command.
It uses an isolated home and Cargo configuration.
It shares only registry and Git download caches with the operator.
Verification uses `--frozen` and fixed thread, job, and resource limits.

The runtime identity includes the solver shim and the Cargo and rustc executables behind rustup proxies.
Local dependencies must remain inside the pinned Git worktree.
Ancestor Cargo configuration outside that worktree is an error.

Fresh and restart commands use new Cargo targets and disable incremental compilation.
Workflow commands retain normal Cargo artifacts and profile settings.
Managed Sail workflows retain proof receipts.
Fresh and restart cases disable whole-target reuse through explicit Cargo targets.
Wall time includes Cargo, dependency builds, verification, and output capture.

Before a measured restart, the runner drains writes and stops its private daemon.
The new daemon must retain disk entries, start with empty memory, and record disk hits.
Wall time includes new-daemon startup and readiness checks, but excludes old-daemon shutdown.
Each sample also records startup time separately.
Client CPU excludes the separate daemon process.

### Results and limits

`samples.jsonl` and `logs/` retain every baseline and measured sample.
`metadata.json` records the run configuration and provenance.
`summary.json` uses schema version 2 and separate results for each scenario.
`summary.md` includes raw sample paths, medians, ranges, sample standard deviations, and paired speedups.
There is no combined timing aggregate across scenarios.

Reports distinguish fresh proof work, confirmed root reuse, and native Cargo no-ops.
Workflow baselines must be Current, complete, and successful.
Fresh proof evidence must identify the current run, not an earlier baseline.
Missing proof evidence, failed proofs, timeouts, source changes, and incomplete sample sets invalidate the affected scenario.
Invalid scenarios retain evidence without timing aggregates.
Any invalid scenario produces a nonzero exit code.

Use an idle native host for comparative performance claims.
A local smoke run checks the protocol, not release performance.
Metadata and logs can contain project paths, source identities, and host details.
Review them before publication.
Keep raw results and private project configurations outside the repository.

## Release performance matrix

The matrix compares an installed Sail archive with stock Verus over three
rotated rounds. Each arm starts from fresh project copies and an empty Sail
cache. It records first use, cold verification, unchanged calls, edits, reverts,
fresh projects and worktrees. The real projects are Vest, IronKV, CapybaraKV,
Anvil and Verismo.

Build the corpus tool first, install the pinned Rust toolchain with `rust-src`,
and provide the stock runtime. Run on a native Linux or Mac host, with the
output outside any Cargo project and a short path for Unix sockets:

```sh
cargo build --release --locked -p sail-conformance
python3 scripts/perf-matrix.py list
python3 scripts/perf-matrix.py run --out ~/pm --rounds 3 \
  --sail-install ~/.local/opt/sail-VERSION-TARGET \
  --stock-verus ~/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux \
  --machine 'instance type, region, operating system'
```

On Mac use the stock `verus-arm64-macos` directory. Real-project preparation
needs `jq`, `perl` and crates.io access; Verismo also needs `cmake` and `ninja`.
`--project` and `--scenario` select diagnostics, and `--allow-fewer-rounds`
permits a shorter trial. Such subsets do not qualify the full release.

Both arms use the pinned Cargo and the release's registry vstd versions.
Adjustments to a project's original pins are recorded. `--vstd source` selects
the older source-checkout diagnostic setup, which cannot use a released proof.
Verismo's additional Rust target changes its sysroot identity; the script
removes that target afterward if it installed it. Each cold sample records
whether the shipped proof was served or why it missed.

Every sample is kept. Passing-workload rows require successful verification on
both arms, with matching functional results and diagnostics; matching failed
proofs cannot qualify them. Incomplete comparisons fail. The report
separately marks every slower median for review, with its range and absolute
delta. A successful report does not itself grant release acceptance.

Attach all four three-round mutation cohorts, measured with the same installed
Sail binary, when reporting the release's edit loops:

```sh
python3 scripts/perf-matrix.py report ~/pm --markdown ~/pm/matrix.md \
  --mutants ~/mutants/human-eval --mutants ~/mutants/ironkv \
  --mutants ~/mutants/vest --mutants ~/mutants/capybarakv
```

The [mutation guide](testing.md#mutation-suite) covers catalogue generation,
three-run stock qualification, one daemon across each sequence, and revert
checks. Missing or incompatible cohorts remain failures.

## Running comparisons

Use a native EC2 Linux lane through `scripts/perf-host.sh`, or an approved EC2
Mac day through `scripts/perf-host-mac.sh`. Keep builds and other work off the
machine during timings. Describe the actual hardware, region and load; never
measure under emulation. Tear down owned hosts when finished.

For narrower investigations, `scripts/perf-loops.sh demo|vest ROUNDS` measures
the ordinary CLI loop, `scripts/perf-agents.sh vest AGENTS ROUNDS` measures
concurrency, and `scripts/perf-real.sh` exercises individual real projects.
These need the Sail/shim pair, stock Verus, `git`, `perl` and `awk`; set
`SAIL_PERF_MACHINE` and keep `SAIL_PERF_OUT` outside a Cargo project.
Each script stops the daemon serving an arm's cache with
`scripts/sail-stop-daemon.sh`, which asks `sail status --json` for that
cache's actual socket and daemon pid (a long cache path gets a short socket
under the temporary directory, not under `SAIL_CACHE_DIR`). Hard-linking or
otherwise restamping an installed binary between calls ends unchanged-call
replay ([freshness](freshness.md#unchanged-call-replay)).

Earlier component measurements remain in the
[real-project report](reports/2026-10-15-real-project-performance.md),
[instant-revert qualification](validation/instant-revert.md),
[compiler-boundary qualification](validation/vstd-compile-boundary.md) and
[generated solver benchmarks](benchmarks.md). They describe their stated
sources and runtimes, and do not override the combined release results.
