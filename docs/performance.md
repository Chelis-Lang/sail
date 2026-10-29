# Performance

Sail speeds up Verus verification by reusing a released vstd proof, previously
verified targets and exact solver answers. Edits still pay Rust compilation
and the Verus frontend for changed targets. Undoing an edit can restore an
original complete proof when retaining that target was worth its cost.

## 0.1.0-rc.7

The numbers below time the installed `0.1.0-rc.7` release archives, built from
`8797ccd4` with fork runtime `47fb3be6`, against stock `cargo verus verify`
0.2026.10.18.7d82994, on 2026-10-28. The
[release-candidate record](validation/release-candidate.md) has the
functional qualification of the same archives.

- **Linux:** four EC2 c7i.8xlarge hosts (Intel Xeon Platinum 8488C, 32
  cores), us-east-1, Ubuntu 24.04, one project per host. Archive
  `sail-0.1.0-rc.7-x86_64-unknown-linux-gnu.tar.gz` (sha256
  `d950044a…a7a773b`), installed with `scripts/install.py --archive` under
  umask 022.
- **macOS:** EC2 mac-m4.metal (Apple M4, 10 cores, 24 GiB), us-east-1c,
  macOS 27.0 (26A428); projects run one after another. Archive
  `sail-0.1.0-rc.7-aarch64-apple-darwin.tar.gz` (sha256 `59cce7c3…af10da21`).

`scripts/perf-real.sh run PROJECT 6` ran six paired rounds per project,
alternating which arm went first. Each arm started from a fresh copy and an
empty Sail cache, with vstd from crates.io.

- Vest passes `--rlimit 100` on both arms.
- Anvil keeps its own `rust-toolchain.toml` (Rust 1.98.0) on both arms and
  gets receipts and replay ([ADR 0084](decisions/0084-qualified-cargo-range.md)).
  The other projects run on 1.99.1.

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
Every exit code matched stock's in every round and phase: fail exits 101 on
both arms. Peak memory is not recorded (#562).

### Linux

| Project | Phase | Stock | Sail | Stock/Sail |
|---|---|---:|---:|---:|
| Vest | cold | 70.3 [67.8–73.4] | 66.9 [65.5–72.6] | 1.05× |
|  | unchanged, first | 0.20 [0.20–0.23] | 0.71 [0.68–0.76] | 0.29× |
|  | unchanged, replayed | 0.20 [0.20–0.23] | 0.06 [0.06–0.06] | 3.43× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.20 [0.20–0.21] | 0.52 [0.51–0.56] | 0.39× |
|  | one-line edit | 38.8 [37.9–43.9] | 23.2 [23.0–25.6] | 1.67× |
|  | revert | 39.0 [37.8–41.3] | 1.14 [1.12–1.22] | 34.08× |
|  | fail | 34.6 [34.0–36.0] | 19.9 [19.7–21.4] | 1.73× |
|  | fix | 38.7 [38.0–39.7] | 23.3 [22.8–25.0] | 1.66× |
|  | fix revert | 38.7 [38.1–41.8] | 1.14 [1.13–1.22] | 34.01× |
| IronKV | cold | 40.3 [39.9–40.7] | 18.3 [18.3–18.9] | 2.19× |
|  | unchanged, first | 0.13 [0.12–0.15] | 0.24 [0.24–0.26] | 0.52× |
|  | unchanged, replayed | 0.11 [0.11–0.11] | 0.03 [0.03–0.04] | 3.26× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.11 [0.11–0.12] | 0.21 [0.21–0.21] | 0.54× |
|  | one-line edit | 9.19 [9.06–9.43] | 7.55 [7.49–7.70] | 1.22× |
|  | revert | 9.19 [9.15–9.44] | 0.77 [0.76–0.80] | 11.94× |
|  | fail | 8.33 [8.30–8.76] | 7.77 [7.71–7.84] | 1.07× |
|  | fix | 9.21 [9.11–9.44] | 7.48 [7.41–7.67] | 1.23× |
|  | fix revert | 9.19 [9.15–9.56] | 0.78 [0.77–0.79] | 11.83× |
| CapybaraKV | cold | 64.7 [64.5–65.0] | 60.3 [60.2–61.0] | 1.07× |
|  | unchanged, first | 0.16 [0.16–0.20] | 0.63 [0.63–0.64] | 0.25× |
|  | unchanged, replayed | 0.16 [0.16–0.19] | 0.03 [0.03–0.03] | 5.77× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.16 [0.16–0.16] | 0.52 [0.52–0.53] | 0.31× |
|  | one-line edit | 29.3 [29.1–29.7] | 14.0 [13.9–14.1] | 2.09× |
|  | revert | 29.4 [29.1–29.6] | 12.3 [12.3–12.4] | 2.39× |
|  | fail | 26.8 [26.7–27.1] | 11.4 [11.3–11.4] | 2.36× |
|  | fix | 29.5 [29.1–29.8] | 13.9 [13.7–14.0] | 2.12× |
|  | fix revert | 29.5 [29.2–29.7] | 12.4 [12.3–12.5] | 2.39× |
| Anvil (`--verify-module vdeployment_controller`) | cold | 182.7 [172.7–184.4] | 179.8 [169.6–181.6] | 1.02× |
|  | unchanged, first | 0.36 [0.34–0.37] | 2.44 [2.30–2.49] | 0.15× |
|  | unchanged, replayed | 0.31 [0.29–0.32] | 0.04 [0.04–0.04] | 8.64× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.32 [0.29–0.32] | 2.13 [2.01–2.15] | 0.15× |
|  | one-line edit | 153.6 [149.7–157.4] | 87.1 [80.2–90.6] | 1.76× |
|  | revert | 154.0 [149.6–159.9] | 33.8 [32.1–34.9] | 4.55× |
|  | fail | 145.6 [140.5–151.0] | 26.7 [25.3–27.6] | 5.46× |
|  | fix | 153.8 [149.1–156.7] | 50.7 [47.9–52.8] | 3.03× |
|  | fix revert | 151.7 [148.4–161.5] | 33.6 [32.1–35.0] | 4.51× |

### macOS

| Project | Phase | Stock | Sail | Stock/Sail |
|---|---|---:|---:|---:|
| Vest | cold | 38.6 [38.5–39.0] | 38.3 [37.2–38.7] | 1.01× |
|  | unchanged, first | 0.26 [0.26–0.27] | 0.80 [0.78–0.84] | 0.32× |
|  | unchanged, replayed | 0.26 [0.25–0.26] | 0.09 [0.07–0.09] | 2.89× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.25 [0.25–0.26] | 0.57 [0.55–0.62] | 0.44× |
|  | one-line edit | 22.1 [22.0–22.1] | 12.5 [12.2–12.7] | 1.76× |
|  | revert | 22.2 [22.1–22.3] | 1.11 [1.09–12.1] | 20.09× |
|  | fail | 20.7 [20.6–20.8] | 10.9 [10.6–11.0] | 1.89× |
|  | fix | 22.2 [22.2–22.3] | 12.6 [12.2–12.7] | 1.77× |
|  | fix revert | 22.3 [22.1–22.4] | 1.12 [1.08–11.9] | 19.87× |
| IronKV | cold | 20.2 [20.0–20.4] | 12.4 [12.3–12.5] | 1.63× |
|  | unchanged, first | 0.21 [0.20–0.21] | 0.29 [0.29–0.29] | 0.72× |
|  | unchanged, replayed | 0.20 [0.20–0.20] | 0.05 [0.05–0.05] | 4.05× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.20 [0.20–0.21] | 0.24 [0.24–0.25] | 0.84× |
|  | one-line edit | 4.30 [4.28–4.30] | 3.35 [3.33–3.38] | 1.28× |
|  | revert | 4.30 [4.27–4.31] | 2.23 [2.22–2.24] | 1.93× |
|  | fail | 4.01 [3.98–4.04] | 3.14 [3.11–3.16] | 1.28× |
|  | fix | 4.29 [4.28–4.31] | 3.05 [3.02–3.07] | 1.41× |
|  | fix revert | 4.29 [4.28–4.35] | 2.22 [2.21–2.24] | 1.93× |
| CapybaraKV | cold | 31.3 [31.0–31.3] | 30.1 [30.0–30.3] | 1.04× |
|  | unchanged, first | 0.23 [0.22–0.23] | 0.59 [0.59–0.60] | 0.38× |
|  | unchanged, replayed | 0.22 [0.22–0.22] | 0.04 [0.04–0.06] | 5.25× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.22 [0.22–0.23] | 0.45 [0.45–0.47] | 0.50× |
|  | one-line edit | 14.9 [14.8–14.9] | 6.04 [6.01–6.07] | 2.47× |
|  | revert | 14.9 [14.9–15.0] | 5.66 [5.65–5.67] | 2.64× |
|  | fail | 14.1 [14.0–14.2] | 5.07 [5.03–5.11] | 2.78× |
|  | fix | 15.0 [14.9–15.1] | 6.06 [6.03–6.09] | 2.47× |
|  | fix revert | 15.0 [14.9–15.0] | 5.67 [5.64–5.77] | 2.64× |
| Anvil (`--verify-module vdeployment_controller`) | cold | 99.4 [99.2–100.3] | 100.3 [99.9–100.8] | 0.99× |
|  | unchanged, first | 0.30 [0.30–0.31] | 2.11 [2.06–2.13] | 0.14× |
|  | unchanged, replayed | 0.25 [0.24–0.25] | 0.06 [0.06–0.06] | 4.31× |
|  | unchanged, `SAIL_MANAGED_NOOP=0` | 0.25 [0.24–0.26] | 1.63 [1.61–1.64] | 0.15× |
|  | one-line edit | 78.9 [78.7–79.1] | 41.3 [41.2–41.3] | 1.91× |
|  | revert | 79.1 [78.8–79.1] | 14.7 [14.6–14.7] | 5.39× |
|  | fail | 75.0 [74.9–75.3] | 12.1 [12.0–12.2] | 6.19× |
|  | fix | 79.1 [78.9–79.6] | 23.8 [23.7–23.9] | 3.32× |
|  | fix revert | 79.2 [79.0–79.4] | 14.7 [14.6–14.8] | 5.40× |

- **Edits, failing edits, fixes and reverts are faster on every project and
  both platforms.** A Vest revert takes about 1.1 s, against 22 s (macOS) and
  39 s (Linux) with stock. A failing Anvil edit takes 12 s against 75 s on
  macOS and 27 s against 146 s on Linux.
- **Replayed unchanged calls take 0.03–0.09 s, against stock's 0.11–0.31 s.**
- **The first unchanged call after a verification is slower than stock**, and
  so is an unchanged call that misses replay (the `SAIL_MANAGED_NOOP=0` row).
  That call takes the full path and records the replay: Anvil 2.44 s against
  0.36 s on Linux and 2.11 s against 0.30 s on macOS; Vest and CapybaraKV
  0.6–0.8 s against 0.2–0.3 s; IronKV 0.24–0.29 s against 0.13–0.21 s. Every
  later unchanged call replays.
- **Cold runs.** IronKV is the only project served the shipped vstd proof
  (in all six rounds on both platforms), so it is the only one with a large
  cold gain. Vest, CapybaraKV and Anvil miss it and verify vstd themselves:
  Vest because of its vstd dependency closure and features
  ([record](validation/rlimit-subsumption.md), #238), CapybaraKV because of
  its `--no-default-features` resolution, and Anvil because of its patched
  Verus sources. Their cold medians are faster than stock, except macOS Anvil
  at 100.3 s against 99.4 s (+0.9 s, ranges overlapping). Linux Anvil and
  Vest cold ranges also overlap, with Sail's median lower.
- **macOS Vest revert outliers.** In two of six rounds the Vest revert and fix
  revert took about 12 s instead of 1.1 s: the fast revert path was missed and
  every check was served from the solver cache. Both are still faster than
  stock; the cause is not diagnosed.

The macOS medians match the earlier main `dca5cd43` measurement on the same
host type within about 2% on every row. The 1-minute load average was 2–7
(Linux) and 2–17 (macOS), from the runs' own work.

Raw logs and reports are kept outside the repository, in the evidence bucket
under `experiments/2026-10-28/rc7/`: Linux `rc7-perf-linux-logs.tgz`, sha256
`a3bd5eb3490192e392235fb1a0777085b7a4397fd662978451fd63be39d4286d`; macOS
`perf-mac-rc7.tgz`, sha256
`93c8a0740974e3ff2d921b3d5dfdc76909462e80acbb8dea392c40d903c83e7a`.

The earlier [rc.6 release matrix](validation/release-matrix.md) and its
mutation cohorts describe their own frozen artifacts. The rc.7 deep tier's
one-round mutation cohorts are in the
[release-candidate record](validation/release-candidate.md).

To repeat a project's measurement, put the installed `bin/`, stock Verus and
`sail-conformance` on `PATH` and run:

```sh
RUSTUP_TOOLCHAIN=1.99.1 SAIL_PERF_VSTD=registry SAIL_PERF_VERUS_TIMES=0 \
SAIL_PERF_EDITS="body-early fail fix" SAIL_PERF_REVERT="body-early fix" \
SAIL_PERF_NOOPS=0 SAIL_PERF_UNCHANGED=2 SAIL_PERF_NOREPLAY=1 \
  scripts/perf-real.sh run vest 6    # vest also takes SAIL_PERF_VERUS_ARGS="--rlimit 100"
```

For Anvil, leave `RUSTUP_TOOLCHAIN` unset so that its own toolchain file
applies. On macOS Anvil also needs OpenSSL (`brew install openssl@3`, then set
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
- **Unchanged runs and tiny edits.** A later unchanged call replays its
  recorded report in 0.03–0.09 s, including unchanged MCP and background job
  calls. The first unchanged call after a verification and a call that misses
  replay take the full path: Sail checks inputs and returns a complete report, which costs more
  than plain Cargo's no-op, most visibly on Anvil. Edits of tiny projects also
  have visible overhead. See [known issues](known-issues.md).
- **CI.** A fresh runner gains from persisted solver replies, and in
  SAIL3112 mode from a persisted `target/`. In managed mode, restored managed
  targets and receipts are reused on a new runner, because tools are
  identified by content (#428). See [caching Sail in CI](ci-cache.md).

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
under the temporary directory, not under `SAIL_CACHE_DIR`), and also
stops any daemon bound to a socket inside the cache, such as a `sail@NAME`
baseline's daemon of another protocol version. It refuses to run without
`SAIL_CACHE_DIR` or `SAIL_DAEMON_SOCKET`, so it never stops the default
cache's daemon. Hard-linking or
otherwise restamping an installed binary between calls ends unchanged-call
replay ([freshness](freshness.md#unchanged-call-replay)).

Earlier component measurements remain in the
[real-project report](reports/2026-10-15-real-project-performance.md),
[instant-revert qualification](validation/instant-revert.md),
[compiler-boundary qualification](validation/vstd-compile-boundary.md) and
[generated solver benchmarks](benchmarks.md). They describe their stated
sources and runtimes, and do not override the combined release results.
