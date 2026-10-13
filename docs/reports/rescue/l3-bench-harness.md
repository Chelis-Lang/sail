# Rescue lane L3: repair benchmark harness

## Outcome

The M7 repair benchmark is now `sail-bench repair`, a Rust workload that
follows the workspace lints. It records every sample and judges them at report
time. `sail-bench report` renders an M6 vs M7 section in `docs/benchmarks.md`
with a gate verdict per catalogue and a list of failing samples. The protocol
is defined in [`docs/bench/m7-repair-acceptance.md`](../../bench/m7-repair-acceptance.md).

## Design

- **Record, then judge.** `run` never stops at a refusal. Each invocation's
  result is kept on its sample: timeout, crash, missing JSON, non-Current
  evidence, extra compiler driver and so on. The run then continues.
  `repair::judge` applies the gate when the report is generated:
  - violations `SAIL7101` to `SAIL7106` per sample;
  - `SAIL7107` when an M6/M7 pair's outcomes differ;
  - `SAIL7108` for coverage below three complete rounds or missing cycles;
  - `SAIL7109` when the median whole-loop speedup is below 2×.

  A failing sample stays in the totals and is listed. It never counts as a
  pass.
- **Median.** The median is taken per catalogue, over complete rounds, of each
  arm's whole-loop total (the sum of its cycle wall times in that round). Arm
  order alternates by round. Per-cycle medians, spread, p95 and maximum are
  also reported.
- **Robustness.**
  - A compact result (`bench/results/NAME.json`) and a raw sidecar outside
    the repository (`--raw`; the compact file records its sha256) are both
    rewritten atomically after every sample, and
    `--resume` skips samples already recorded.
  - The deadline sends SIGINT to the command's process group, cancels the
    exact M7 job, then kills the group.
  - A daemon that died is restarted, and a note is kept.
  - A live socket that the harness does not own is never adopted.
- **Measurements.** Each sample records:
  - wall time;
  - first actionable evidence: the M6 focused result, or for M7 the first
    focused event in the public proof journal;
  - driver count;
  - full and focused Currentness, and `accepted`;
  - the canonical full outcome;
  - daemon `STATS` deltas (replies served vs solved);
  - preparation hits and misses;
  - peak RSS of the command plus the owned daemon's process tree, from `/proc`,
    with GNU `time` RSS of the command as well.
- **Module layout.** Code is in `crates/sail-bench/src/repair/`:
  - `mod.rs`: model and fact extraction;
  - `judge.rs`: the gate;
  - `render.rs`: the report section;
  - `process.rs`: daemons, calls, RSS and the journal observer;
  - `run.rs`: preparation and the measured loop.

## Python retired

- **Deleted:** `scripts/m7-repair-bench.py` and its two test files.
- **Kept and moved:** the owned-actor helpers that the reader controller still
  imports (`OwnedDaemons`, `interrupt_owned`, `write_json`, `sha`,
  `warm_repair_state`) now live in `scripts/m7-owned-actors.py`, tested by
  `scripts/test-m7-owned-actors.py`. `ci/run.sh` was updated.
- **Rebase:** `scripts/local-standard-conformance.py` was deleted on
  `rescue/m7` by L2. The rebase keeps that deletion.
- **Left in place:** the sealed-product launcher in
  `docs/validation/m7-current-benchmark-*.py`. Four other controllers import its
  grant validator. Its benchmark path (via the frozen `run-original.py` bundle)
  is now dead, and removing it is a follow-up.

## Behaviour changes for review

The old runner required the package's Cargo metadata to hold exactly one
target. Vest's `vest_lib` has a `lib` plus a `test` target (`cbor_rfc`), so
the old check would have refused Vest. The Python run never reached it.

The property the check protects is that M6's `--package` selects the same scope
as M7's `--lib`. Cargo's default selection is the library plus binaries, so the
check is now "exactly one library target and no binary target". This states
that property exactly; it is not a weakened check.

## How to run

Use the native host:

```sh
scripts/perf-host.sh up l3
scripts/perf-host.sh sync l3
```

Build the CI image there. Build the M6 binaries from `git archive 0fde777` in a
separate tree inside the image. Then, in the image:

```sh
cargo build --release --locked -p sail-cli -p sail-shim -p sail-bench
target/release/sail-bench repair prepare --scratch S
target/release/sail-bench repair run --m6 M6/sail --m6-commit 0fde777fc79ccf07e1318ce21dc0ff84eea1b2ca \
  --m7 target/release/sail --m7-commit SHA --engine /opt/sail-engine --scratch S --out O \
  --environment "native x86-64 Linux, EC2 c7i.4xlarge" --name repair-acceptance-LABEL --rounds 3
sail-bench report --out docs/benchmarks.md bench/results/*.json
```

## Smoke result (native)

- **Host and toolchain:** EC2 c7i.4xlarge, Linux 7.0.0-1014-aws x86_64,
  16 vCPU, CI image `sail-ci:8e24fb5f7f490c01`. Verus 0.2026.10.04.426d8b0,
  Z3 4.16.0.
- **M6:** `0fde777`. sail sha256 `66d842e6…`, sail-z3 `98b38f77…`.
- **M7:** pre-rebase branch commit `13a2b49`, which is now `7c391ca`. Its
  `sail` product code is `rescue/m7` `541d3e7` unchanged.
  sail sha256 `3c84a511…`, sail-z3 `d3a08f8d…`.
- **Scope:** human-eval-verus, cycle `gcd-body`, one round. Both catalogues
  were prepared, which confirms the Vest preparation.

| Arm | Wall | First actionable | Drivers | Full / accepted | Served / solved | Prep hits / misses | Peak tree RSS |
|---|---:|---:|---:|---|---:|---:|---:|
| M6 (focus + full) | 18.06 s | 6.72 s | 2 | current / n.a. | 4,225 / 319 | n.a. | 1,073 MiB |
| M7 (repair batch) | 18.50 s | 5.18 s | 1 | current / yes | 4,476 / 68 | 6 / 100 | 1,349 MiB |

- **Samples:** both measured samples are free of violations.
- **Speed:** on this one cycle M7 is 0.98× M6. That is a single sample, not an
  acceptance result.
- **Gate:** reported correctly as **fail** for both catalogues, because of
  incomplete coverage (`SAIL7108`) and no complete round (`SAIL7109`).
- **Warm-ups** (recorded, not gated):
  - engine install: 0.2 s;
  - dependency warm-up: 67.7 s for each arm;
  - M6 focused context: 35.6 s;
  - M7 repair context: 77.3 s, Current and accepted on the first attempt,
    using 2 drivers.

Evidence is in `~/sail-raw-evidence/l3/`, outside the repository:

| File | sha256 |
|---|---|
| `repair-smoke-gcd-l3.json` | `8ff617b56dd12d41996ad2e970d39574e39a4b811151eb66b6e4668b3a8b05b4` (pre-compaction format; dropped from the branch at lead review, ADR 0038) |
| `repair-smoke-gcd-l3-raw.tgz` (raw stdout, stderr, `time` output, journal pages; caches excluded) | `a3905f9c7dd98f99725a4cd9c823d513e4a2575ce0eb9fc8ca676e79e35b6e5b` |
| `smoke-gcd-2.log` (successful run) | `ff2a3d3c0302999a4b52e45ac3e2b5074fc7cfd303fc212a5a779247e92ed54d` |
| `smoke-gcd-1.log` (first attempt, refused Vest at preparation as described above) | `366c92e0029fe729febc3dc74eb05ed6bf65d53c6d7c8e2370ebf1b64b63d19e` |
| `m6-build.log` | `e5230fc2e8f67972feaf8ac98d5b8c31dc9048eb558a47a5092343dba02e8149` |
| `image-build.log` | `b1531b81999afc71cf226e3eeadb334264f48dff4737bbfd6b6f0ac5dd33c18e` |

## Gate

`ci/local.sh gate` was run on the same native host.

- **First run (failed):** before rebasing, 698 of 700 tests passed. The two
  `sail-daemon` output-overlap fixture tests failed; `rescue/m7` `76cd30a`
  fixes them. Log `gate.log`, sha256
  `4cc3310be4eb2768b91b5998be3c8d254d55c5288cb823d411878d96a3b2afb6`.
- **Second run (passed):** after rebasing onto `rescue/m7` `edb1abf`, every
  gate step passed and 700 of 700 tests passed, including the
  `docs/benchmarks.md` freshness check. Log `gate-2.log`, sha256
  `af64a1fd2477fa19188d73de9f3b66acc805b322dccdc67be90aba19f7ccfbd2`.

The host was taken down afterwards (`scripts/perf-host.sh down l3`).

## Known limits

- Verus `--time` phase data is not collected. Plumbing it would change the
  verified command line of both arms.
- Tree RSS is sampled every 100 ms and assumes 4 KiB pages. It includes the
  daemon's idle footprint, and it is Linux-only.
- `--resume` re-runs warm-ups so that the caches are warm again in a fresh
  daemon. A resumed run's warm-ups are recorded under a new `session-N`
  directory.
- First actionable evidence is not comparable across arms in every case. M6's
  value is the end of its focused check. M7's value is the first focused
  journal event (100 ms polling granularity).
- The workload, like the old runner, compares tiers in the canonical outcome.
  A tier-only difference therefore fails `SAIL7107`.

## Documentation changed

- `docs/bench/m7-repair-acceptance.md`: the benchmark sections are rewritten;
  the reader and retained-job sections are kept.
- `docs/benchmarks.md`: regenerated (no repair section until a compact
  acceptance result is committed).
- `CHANGELOG.md`: `sail-bench repair` and `SAIL7101` to `SAIL7109`, at the
  lead's request.
- This report.

ROADMAP, known issues and ADRs are untouched, as instructed.
