# Performance

Sail speeds up new projects by shipping a verified vstd and speeds up edits by
reusing byte-identical solver answers. Returning to a previously verified
revision can also restore its proof. Unchanged calls return structured reports
and cost more than Cargo's no-op.

The combined 0.1.0 release is still being measured. The table below is a
three-round baseline of the installed candidate, not acceptance of the later
performance fixes or a promise that every project is faster. Linux uses
c7i.8xlarge in us-east-1; macOS uses mac-m4.metal in us-east-1a. Both use
Sail's f8589ed4 runtime, based on Verus 7d82994, and Rust/Cargo 1.99.1.

| Workflow | Linux stock → Sail | macOS stock → Sail |
|---|---:|---:|
| New small project and first verification | 31.84 → 13.19 s | 16.78 → 10.42 s |
| IronKV cold | 40.06 → 21.13 s | 20.36 → 14.05 s |
| CapybaraKV cold | 57.33 → 56.56 s | 31.27 → 33.36 s |
| Anvil cold, selected module | 183.80 → 181.71 s | 100.02 → 104.53 s |
| Verismo cold | 82.84 → 79.42 s | 49.76 → 49.53 s |
| Vest cold, default resource limit | 68.13 → 64.84 s | 37.26 → 39.05 s |

Vest's default-limit runs fail the same proofs in both arms. They measure the
failing workflow, not successful verification. A separate Linux successful
Vest control with `--rlimit 100` serves the shipped vstd: cold verification
takes 51.01 s against stock's 72.04 s, an edit 25.21 s against 39.50 s, and
returning to the original proof 2.08 s against 39.62 s. The Mac counterpart
is pending.

The full seeded mutation suite exercises nine classes, including proof errors,
compile errors, specification changes and edits across modules. Each project
uses a single daemon across all mutants and restores the original afterward.
Completed Linux CapybaraKV and IronKV runs match stock in all 168 and 153
comparisons respectively. CapybaraKV's three full edit-loop speedups are
1.93×, 1.92× and 1.92×. HumanEval's full run also matches; Vest and the
combined Mac suite are still running. These are broader tests than the single
scripted edits in older reports. See the [mutation suite](testing.md#mutation-suite).

Baseline source and raw data: [combined release measurements](validation/release-matrix.md).

## Remaining costs

The baseline's small-project edit is 1.13 s versus stock's 0.76 s on Linux,
and 1.17 s versus 0.45 s on Mac. Its unchanged call is 0.42 s versus 0.11 s
on Linux, and 0.58 s versus 0.21 s on Mac. Anvil's corrected unchanged
partial-scope control takes about 2 s with no verifier invocation, against
stock's 0.3 s. Receipt and input validation still need work.

Mac cold losses remain release blockers while fixes are qualified. A known
slowdown is not accepted merely because other rows are faster. The release
bar is relative: most scenarios should improve, with only modest overhead
on already tiny, unoptimizable work.

The scripts keep every sample and compare functional results with stock.
Run the full matrix from an installed archive on a native host:

```sh
python3 scripts/perf-matrix.py run --out /tmp/sail-matrix \
  --sail-install /path/to/unpacked/sail --stock-verus /path/to/stock-verus \
  --rounds 3 --machine 'instance type, region, operating system'
```

The release also needs the separate three-round mutation cohorts from the
[suite guide](testing.md#mutation-suite), bound to the same installed binary.
The matrix reports missing cohorts and slower rows explicitly.

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

Every sample is kept. Comparisons require matching exit codes and functional
verification results and diagnostics; incomplete comparisons fail. The report
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

Earlier component measurements remain in the
[real-project report](reports/2026-10-15-real-project-performance.md),
[instant-revert qualification](validation/instant-revert.md),
[compiler-boundary qualification](validation/vstd-compile-boundary.md) and
[generated solver benchmarks](benchmarks.md). They describe their stated
sources and runtimes, and do not override the combined release results.
