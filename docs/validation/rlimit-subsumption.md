# Shipped vstd proof under a raised `--rlimit` (ADR 0060)

| | |
|---|---|
| Status | **Historical witness qualification below; current Mac recordings use exact default/100 profiles after a reproduced resource-count variation. Native Linux and Mac qualification of the new loader pass; final performance matrices remain separate gates.** |
| Source | Sail `feat/rlimit-subsumption` on `main` `d64e65d9`; runtime `7d829946…-4ed2dee08bc877ae` (fork `9c0ca7e7`); stock release 0.2026.10.18.7d82994; Z3 4.16.0 |
| Host | native EC2 c7i.8xlarge (us-east-1, Xeon Platinum 8488C, 32 vCPUs), Ubuntu 24.04, Rust 1.99.1 |
| Tool | `scripts/rlimit-subsumption.py --runs 3 --rlimit 100` |

## Exact-profile fallback, 2026-10-22

Source: main `b8395983` plus this change; the combined Mac consumer also uses
retention-budget candidate `c308bda4`. Runtime f8589ed4 / Verus 7d82994,
Rust/Cargo 1.99.1, EC2 mac-m4.metal in us-east-1c, macOS 27.0 (26A428).

The new consumer passes the Mac host gate (963 tests, clippy clean). The real
producer records two complete Current vstd proofs, each under its exact
flags and source run. Their keys and all stored members pass release-index
verification. Public Vest with released registry versions, default vstd
features and limit 100 serves the exact raised-profile proof. Cold, edit,
undo and unchanged all succeed. Undo restores the original complete proof
with zero verifier invocations. These are individual functional controls:

| Step | Stock | Sail |
|---|---:|---:|
| Cold | 38.87 s | 32.50 s |
| Edit | 22.50 s | 15.78 s |
| Undo | 22.58 s | 1.88 s |
| Unchanged | 0.22 s | 0.94 s |

Installed CLI SHA-256:
`8fb73ac60ac70d941e1f9c860913049edbd1739d2b0e3f0cc6281f8fbc15e33b`.
Raw producer reports, host log and consumer outputs:
`experiments/2026-10-22/mac-exact-profiles.tgz`, SHA-256
`6ccdc437960825451238b0abd0c59ea7d6dcfb02bdf5b2d858347b4c610fedfe`.
Native EC2 c7i.8xlarge (us-east-1, Ubuntu 24.04) qualification passes on
`ccc01205`: workspace gate, solver, full E2E including extra, PR conformance,
full fast and standard conformance. Each standard arm compares 159 units and
13,166 functions: all Same, zero different, unstable or infrastructure errors.
Producer fixtures and cancellation controls pass. A freshly staged combined
Linux payload also passes the README new/cold/unchanged walkthrough and the
real Claude Code MCP connection, listing 17 tools. The final performance
matrices remain separate gates.

Raw native logs and walkthrough:
`experiments/2026-10-22/exact-profile-native-gates.tgz`, SHA-256
`6e58d3fb2e78b3ee93b8b6d57eba5b82a52bd344ee75bcc9b4936116ccd7272d`.

The prior staged Mac witness fails on resource counts: three default and
three raised runs preserve inputs and answers, but one process's count varies
by 14. Fresh direct Z3 replays reproduce it. The complete comparison is
`experiments/2026-10-22/mac-vstd-rlimit-repro.tgz`, SHA-256
`1384a85de5721675143268bc8fa4844b3f65c4e9d12fa4f40e645583040277dc`;
direct replay log `experiments/2026-10-22/mac-vstd-rlimit-direct.tgz`, SHA-256
`726b04aabfcc019083ffa5da5067f4c5378e0d69aa44427a3c4f6c1649c746ad`.
The single-process input and replay command are in the
[corpus reproduction](../../corpus/repros/z3-resource-count-variation/README.md).
The same 48 direct Linux replays did not reproduce variation; this does not
establish that every Linux query has deterministic resource counts.

- `c308bda4`, 2026-10-22: first Mac coexistence control stopped after cold;
  both verifiers succeeded, but the expected shipped proof was absent because
  its resource-count witness failed. No retained-undo acceptance from that
  attempt. Raw `experiments/2026-10-22/mac-cost-controls-first.tgz`, SHA-256
  `ee6a948c3b1effec193e2b0a8de15415906b75a2192909e67b94f7a064f9d704`.

## Re-recording on fork `f8589ed4` (Linux)

After rebasing onto `main` `1793a129` (runtime `7d829946…-81186034d2e137a2`, fork `f8589ed4`, manifest `576558fc…`), the release was staged twice with `ci/local.sh release` on native EC2 c7i.8xlarge (us-east-1). Both recordings stored the same witness for unit `df8e0dc3…`:
- recorded/witness rlimit 10/100; solver limits 30,000,000/300,000,000;
- largest query 12,941,218 against its own limit of 30,000,000;
- 2,061 queries, 366 solver processes, 0 `unknown`, 0 at or over their limit;
- inputs equal, answers and counts equal; normalized-input digest `e57f592e…`, the same in both stagings.

The fork `9c0ca7e7` witness below does not carry over: every release records its own witness.

With the final branch's staged release, stock `cargo verus verify` and `sail verify` each ran once on a fresh `--lib` project per case, with a fresh Cargo home and Sail cache. Exit codes matched, and stdout and stderr matched as line sets after removing roots, Cargo's progress and timing lines, and Sail's notes. Times are wall seconds:

| Arguments | Stock | Sail | vstd in Sail |
|---|---:|---:|---|
| (none) | 29.9 | 12.6 | served |
| `-- --rlimit 100` | 29.9 | 12.6 | served under the raised rlimit, no note |
| `--fwd-verus-args-to roots -- --rlimit 100` | 30.3 | 12.6 | served |
| `-- --rlimit 5` | 32.8 | 37.7 | verified; `SAIL3138`: below the default rlimit 10 |
| `-- --expand-errors` | 31.6 | 32.9 | verified; `SAIL3138`: forwarded flags are bound |

### Cold first run with `-- --rlimit 100`

Stock against two staged releases: `main` `1793a129` (before) and this branch (after), both recorded on fork `f8589ed4`. Each arm of each round used a fresh copy, Cargo home and Sail cache, with human output; round order rotated. All runs exited 0, and every Sail run's stdout and stderr equal stock's as line sets after removing roots, Cargo's progress and timing, and Sail's notes. Values are wall seconds: median (min–max).

| Project | Stock | Before | After | Note in Sail |
|---|---:|---:|---:|---|
| `--lib` project (`sail new` / `cargo verus new`), 3 rounds | 30.06 (29.80–30.33) | 34.74 (34.68–34.99) | **12.64** (12.62–12.80) | before: `SAIL3138`; after: none, vstd served |
| Public-profile Vest, `-p vest_lib`, 3 rounds | 69.11 (68.90–69.22) | 68.89 (68.61–69.32) | 70.14 (68.96–75.42) | `SAIL3138` in both: vstd's closure differs |
| Vest, 2 further before/after rounds | | 72.11, 69.72 | 69.43, 69.74 | as above |

Over all five Vest rounds the before and after medians are 69.32 s and 69.74 s, the same within this host's variation. Vest does not gain: its `Cargo.toml` depends on vstd with `default-features = false` (vest_lib enables `vstd/alloc` and `vstd/std`), so its vstd is the alloc/std closure, while the shipped proof is alloc/default/std; its lock also has `equivalent` 1.0.2, `indexmap` 2.14.1 and `unicode-ident` 1.0.24 (the shipped build has 1.0.3, 2.14.2 and 1.0.27). Sail therefore makes no shipped plan for it, with or without `--rlimit` (#238). Public-profile Vest moves only the vstd requirement to crates.io `=0.0.0-2026-10-18-0147`, with the corpus pin otherwise unchanged.

With #276, Vest's `note[SAIL3138]` names these four differences and the edits that align them: three `cargo update -p … --precise …` commands and removing `default-features = false`. Applied to a copy, they let the shipped proof serve: one cold run each with `-- --rlimit 100` took 71.56 s stock and **49.95 s** Sail. Both reported 2060 and 3237 verified, 0 errors, and Sail printed no note. That changes Vest's manifest and lock, so it is the project's choice; Sail never applies it.

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

These runs redirected their output into the project directory. Under those conditions a second, unchanged run differed from stock: Sail recompiled vstd and printed its results, where stock was a no-op. The same happens on `main` and is independent of this change. The cause is the in-project output file, which makes each run Historical (#259, fixed by #266). With output written outside the project, the rerun is a Cargo no-op identical to stock.

## macOS repeat

| | |
|---|---|
| Host | EC2 mac-m4.metal (us-east-1d), macOS 27.0, Rust 1.99.1 |
| Runtime | the branch's pinned macOS runtime `7d829946…-8f917633f244b095` (fork `9c0ca7e7`, manifest `5a6114d3…`), fetched with `ci/engine-artifact.py`; stock 0.2026.10.18.7d82994 arm64-macos; Z3 4.16.0 |
| Method | as on Linux; the same 147 inputs, 3 + 3 runs, `--rlimit 100` |

| Subject | Byte-identical apart from rlimit values | Answers and per-query resource counts identical | A query at or above its own limit |
|---|---|---|---|
| Sail's runtime, compared exactly | **147 / 147** | 147 / 147 | none |
| Stock Verus, compared as multisets | 133 / 147 | 146 / 147 | none |

- **Stock's 14 differing inputs:** 12 examples, HumanEval and vstd differ in stock's hash-ordered declarations. 13 of them have identical answers and resource counts, and identical outputs as multisets.
- **Stock HumanEval:** the 6 runs fall into two classes, each mixing default and raised runs ({d0, d2, r0, r1} and {d1, r2}). They differ by one resource count in one process (148,633 against 148,650). That is stock's run-to-run variation, not the raised limit. The fork's 6 HumanEval runs are identical.
- **Consumption:** the largest vstd query consumed 3,985,998 against its 30,000,000 limit. In HumanEval one query consumed 85,053,077 under its own explicit limit.
- **Staged release** (`scripts/release.sh stage` from the branch; the recording passed). The witness reads:
  - recorded/witness rlimit 10/100, solver limits 30,000,000/300,000,000;
  - largest query 3,985,998, whose own limit is 60,000,000;
  - 2,061 queries, 366 processes, 0 `unknown`, 0 at or over their limit;
  - inputs equal and answers and counts equal.

  Proof identity is `0ad080a5…`, the same as the earlier macOS `9c0ca7e7` bundle. With a fresh `sail new --lib` project per case, caches outside it and `rust-src` installed, stock and Sail had byte-identical rc, stdout and stderr in all four cases:

  | Arguments | vstd in Sail |
  |---|---|
  | (none) | served, `SAIL3132` |
  | `-- --rlimit 100` | served under the raised rlimit, `SAIL3132` |
  | `--fwd-verus-args-to roots -- --rlimit 100` | served, `SAIL3132` |
  | `-- --rlimit 5` | not served; vstd verified |

- macOS evidence: logs, witness and stage cases, sha256 `70a3c79440f5d831ff45578a127b2fa3ad838485680bf7d1a70a135c09c9620e`, outside the repository.

## Evidence

Raw logs are outside the repository:
- corpus validation logs: sha256 `db526da89efd6d0c767de999c081517600b18cf1e852f2d83437c5ed15026cea`;
- staging, witness and comparison logs: sha256 `41515e404fbbf3a9ed32284aa404fef874ad00e9ef2b22ac6075edf6bda2a907`.
- `f8589ed4` re-recording: both witnesses, staging logs, flag cases, cold timing samples and per-run outputs, aligned-Vest runs, and the local gate logs for #268 and #276. sha256 `53ab78b54b9d8771575a8cfeb06dca16ee1466c40d52ca527a85964d6fdd0257`.

Failed or interrupted attempts:
- 2026-10-20, branch WIP: the first two stagings computed the witness's limits from the smallest set limit, which made `recorded_rlimit` 50. The cause was a vstd function's explicit rlimit. Fixed by using each query's own limit and the most common limit.
- 2026-10-20, branch WIP: release admission refused a bundle carrying the unlisted witness, so nothing was served. That is the fail-safe direction. Fixed by admitting the bound witness.
- 2026-10-20, branch WIP: compiler-read validation compared the raised arguments, so the subsumed lookup missed. Fixed by reading the recorded value for that one variable.
- 2026-10-20: comparisons with the Sail cache inside the project, or with `CARGO_TARGET_DIR` set, ran unmanaged (`SAIL3112`) or without a plan. These were harness errors and were rerun.

## Remaining

- Vest gains nothing until a shipped proof matches its vstd closure (default features off, three locked versions); see #238. The project can align it as its note says.
