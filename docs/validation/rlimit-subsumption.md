# Shipped vstd proof under a raised `--rlimit` (ADR 0060)

| | |
|---|---|
| Status | **Linux validation passes; macOS repeat pending; ADR 0060 proposed** |
| Source | Sail `feat/rlimit-subsumption` on `main` `3b9ec170`; runtime `7d829946…-4ed2dee08bc877ae` (fork `9c0ca7e7`); stock release 0.2026.10.18.7d82994; Z3 4.16.0 |
| Host | native EC2 c7i.8xlarge (us-east-1, Xeon Platinum 8488C, 32 vCPUs), Ubuntu 24.04, Rust 1.99.1 |
| Tool | `scripts/rlimit-subsumption.py --runs 3 --rlimit 100` |

## Corpus: default vs `--rlimit 100`

Each input ran 3 times at the default rlimit (10) and 3 times with `--rlimit 100`, with `--output-json --time --no-report-long-running -Z write-long-types-to-disk=no`. The inputs were:
- the fast tier's 144 example files plus 1 `--expand-errors` example;
- HumanEval (`tasks/lib.rs`);
- vstd as a Cargo dependency (`cargo verus verify -- --output-json --time [--rlimit 100]` on a crates.io `vstd =0.0.0-2026-10-18-0147` project, cold each run).

That is 147 inputs and 764 solver processes per run.

| Subject | Byte-identical apart from rlimit values (inputs, answers, resource counts, outputs) | Answers and per-query resource counts identical | A query at or above its own limit |
|---|---|---|---|
| Sail's runtime (fork `9c0ca7e7`), compared exactly | **147 / 147** | 147 / 147 | none |
| Stock Verus, compared as multisets | 134 / 147 | **147 / 147** | none |

- **Stock's 13 other inputs** are 11 examples, HumanEval and vstd. Their solver inputs differ only in declaration and `distinct` order, which stock produces from hash order (VI-0001 and kin). In 12 of the 13 the order also varies between runs of one setting. In `examples/assert_by_compute.rs` stock chose one accessor order in all default runs and another in all raised runs. Every one of the 13 has the same answers and the same `:rlimit-count` sequence under both settings, and the same stdout and stderr, apart from the order of Cargo's parallel progress lines for vstd.
- **Consumption.** The largest single query in vstd consumed 12,941,218 against its 30,000,000 limit. In HumanEval one query consumed 85,053,077 under its own higher limit: a function's explicit `#[verifier::rlimit]` value, unchanged by `--rlimit`.
- **Sail's runtime is exact everywhere.** Fork `9c0ca7e7` fixes the ordering issues, so it was compared without any multiset step and matched on all 147 inputs.

## Staged release

The release was staged with `scripts/release.sh stage`, which records the bundle with this branch's recorder. The recorder's witness passed and stored `units/<key>/rlimit.json`:

| Field | Value |
|---|---|
| recorded / witness rlimit | 10 / 100 |
| recorded / witness solver limit (most common) | 30,000,000 / 300,000,000 |
| largest query / its limit | 12,941,218 / 30,000,000 |
| queries, solver processes, `unknown` answers, queries at or over their limit | 2,061, 366, 0, 0 |
| inputs equal apart from rlimit; answers and counts equal; outputs equal | true, true, true |

Stock `cargo verus verify` and the staged `sail verify` were each run once on a fresh `sail new` project, with fresh caches outside the project and `rust-src` installed. Outputs were compared byte for byte with Cargo's timing and progress lines blanked:

| Arguments | stock vs Sail (rc, stdout, stderr) | vstd in Sail |
|---|---|---|
| (none) | identical | served, `SAIL3132` |
| `-- --rlimit 100` | identical | served `rlimit-subsumed`, `SAIL3132` |
| `--fwd-verus-args-to roots -- --rlimit 100` | identical | served, `SAIL3132` (no subsumption needed: vstd gets no forwarded flag) |
| `-- --rlimit 5` | identical | not served: "the rlimit witness does not admit this rlimit"; vstd verified |

A second, unchanged run of the same project differs from stock, with or without `--rlimit`. Sail recompiles vstd and prints its results; stock is a no-op. The same happens on pristine `main` (#259), so it is not part of this change.

## Evidence

Raw logs are outside the repository:
- corpus validation logs: sha256 `db526da89efd6d0c767de999c081517600b18cf1e852f2d83437c5ed15026cea`;
- staging, witness and comparison logs: sha256 `41515e404fbbf3a9ed32284aa404fef874ad00e9ef2b22ac6075edf6bda2a907`.

Failed or interrupted attempts:
- 2026-10-20, branch WIP: the first two stagings computed the witness's limits from the smallest set limit, which made `recorded_rlimit` 50. The cause was a vstd function's explicit rlimit. Fixed by using each query's own limit and the most common limit.
- 2026-10-20, branch WIP: release admission refused a bundle carrying the unlisted witness, so nothing was served. That is the fail-safe direction. Fixed by admitting the bound witness.
- 2026-10-20, branch WIP: compiler-read validation compared the raised arguments, so the subsumed lookup missed. Fixed by reading the recorded value for that one variable.
- 2026-10-20: comparisons with the Sail cache inside the project, or with `CARGO_TARGET_DIR` set, ran unmanaged (`SAIL3112`) or without a plan. These were harness errors and were rerun.

## Remaining

- The macOS repeat of the corpus check and of a staged witness.
- Owner sign-off of ADR 0060.
- #259 (unchanged re-run), which is independent.
