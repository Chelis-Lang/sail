# Survey: benchmark and conformance corpus candidates

Status: survey, 2026-10-08. Nothing here is pinned yet. Pinning happens when
`corpus/corpus.toml` lands, and each entry needs a passing plain-Verus baseline
at that point.

The charter's "How to prove it's useful" section asks for a corpus with these
properties:
- real Verus projects, pinned by commit;
- vstd, plus the examples and test suites from the Verus repository;
- public verified systems, preferring projects with long commit histories;
- the Verus version compatibility of each project recorded.

This document lists the candidates and recommends tiers. The corpus feeds three
things: the differential conformance harness (`sail-conformance`), the
benchmark workloads (cold, warm, edit, history replay, agent loop, fragility
census), and the query corpus captured from Verus's SMT logs.

**Pinned toolchain**

| Item | Value |
|---|---|
| Verus | `release/0.2026.10.04.426d8b0`, commit `426d8b01e7ffb910a36c15e1f870ff985bb61eef` |
| Z3 | 4.16.0 |
| Rust (Verus's toolchain) | 1.98.1 |
| Matching crates.io `vstd` | `0.0.0-2026-10-04-0306` (`source/vstd/Cargo.toml` at the pinned commit; listed on <https://crates.io/crates/vstd/versions>) |

**How the evidence was gathered**
- In-tree facts come from reading the Verus source at the pinned commit, and
  from running the pinned release binary (arm64 macOS) on `examples/` and vstd.
  The test machine was an Apple M3 with 8 cores and 24 GB RAM, running each file
  one after another.
- External facts come from the GitHub API, the repositories' own files, and the
  Verus project's nightly cross-project CI. Every external claim cites a URL.
- LOC figures for external projects are estimates: GitHub's Rust byte counts
  divided by about 35 bytes per line. They include generated, vendored and
  unverified code.

## Key finding: Verus already maintains a cross-project corpus (`verita`)

At the pinned commit, `tools/verita/` in the Verus repository holds a
crater-style runner:
- It reads a TOML list of projects, clones each one, and verifies it.
- It records per-function timing as JSON.
- For `cargo verus` projects it injects `[patch]` entries so that crates.io or
  git `vstd`/`verus_builtin` dependencies resolve to the local Verus checkout
  (`tools/verita/src/dependencies.rs`).

**The project list.** `tools/verita/run_configuration_all.toml` at the pinned
commit lists vstd, ironkv, node-replication, verified-storage, memory-allocator,
vest, vest-generated, human-eval, page-table, svsm, anvil, tla-rs and verismo.
Each entry gives its crate roots and extra Verus arguments.

**Nightly CI.** `.github/workflows/nightly-verita.yml` runs that list nightly
against Verus `main`, using Rust 1.98.1, the Z3 from `get-z3.sh`, and Singular,
with `--fail-on-error`. The two runs straddling the pinned release:

| Run | Verus commit | Outcome |
|---|---|---|
| <https://github.com/verus-lang/verus/actions/runs/37382832821> (2026-10-05) | `ef66d478`, release + 2 commits | 11 of 13 projects passed. **svsm** and **anvil** failed. Anvil's failure was `rlimit exceeded`, followed by Verus PR "Increase rlimit for verita anvil" (#3085). |
| <https://github.com/verus-lang/verus/actions/runs/37529962706> (2026-10-06) | `41060401`, release + 9 commits | All 13 projects passed. |

Neither run used the release commit itself, and the projects were taken at
their branch heads. The compatibility estimates below are therefore strong
evidence but not proof. The baseline run for each pin must confirm them.

**Recommendation.** Adopt verita's project list and arguments as the starting
point for the external corpus. Make two changes:
1. Pin every project to an exact commit. verita follows `main`.
2. Run against the pinned release instead of a source build of Verus `main`.

`corpus/corpus.toml` should stay close enough to verita's format that entries
can be compared and exchanged.

## 1. In-tree candidates (Verus repository at the pinned commit)

### 1.1 vstd (`source/vstd`)

**Size.** 130 `.rs` files, 56,697 lines. The top level has 53 files and 30k
lines. Subdirectories: `std_specs/` (29 files, 10.7k lines), `resource/` (22,
8.5k), `arithmetic/` (15, 5.0k), `contrib/` (8, 1.6k), `cell/` (3, 0.7k).

**How it is verified.** It is built as a single crate rooted at `vstd.rs` with
`--is-vstd`:
- Verus's build runs it through `source/vstd_build` (arguments include
  `--crate-type=lib --export vstd.vir --is-vstd --compile --multiple-errors 2`
  and `--cfg feature="std"`/`"alloc"`).
- CI builds it with
  `cargo run -p cargo-verus -- build --manifest-path vstd/Cargo.toml`
  (`.github/workflows/ci.yml`).

The release ships vstd already verified and exported (`vstd.vir`,
`libvstd.rlib`), so verifying vstd is a deliberate extra workload, not
something every user run repeats.

**Measured with the pinned release.** Invoked as
`verus --crate-type=lib --is-vstd --export <tmp>/vstd.vir --out-dir <tmp> --no-report-long-running --cfg 'feature="std"' --cfg 'feature="alloc"' --cfg 'feature="nonzero_internals"' source/vstd/vstd.rs`
with `VSTD_KIND=IsVstd`:

| Measure | Result |
|---|---|
| Outcome | 2,060 verified, 0 errors |
| Wall time | 25.4 s (52.7 s user) |
| Peak RSS | 1.9 GB |

verita's 2026-10-05 Linux run took 54 s.

**Assessment.** vstd is the single best first-tier entry. It is about 2,000
functions in one invocation, it is the library every other project depends on,
and it is compatible by definition.

### 1.2 `examples/`

**Layout.** Single-file examples only, with no `Cargo.toml` anywhere under
`examples/`. Each file is its own crate root. A few files declare inline
`mod` items, but none spans several files. In total there are 156 `.rs` files
and 29,810 lines.

| Directory | Files | Lines | Notes |
|---|---:|---:|---|
| `examples/` (top level) | 66 | 9,526 | assorted features: syntax, ADTs, linked lists, bitvectors, `assert_by_compute`, ... |
| `guide/` | 33 | 6,028 | code from the Verus guide |
| `state_machines/` (incl. `tutorial/`, `reference-examples/`) | 29 | 6,335 | VerusSync / tokenized state machines |
| `cuckoo_hash_table/` | 3 | 2,209 | largest single file (`cuckoo.rs`, 1,456 lines) |
| `resource/` | 5 | 1,573 | resource algebra examples |
| `scache/` | 1 | 1,180 | `rwlock.rs`, tagged `ignore` |
| `iterators/` | 3 | 1,042 | |
| `summer_school/` | 5 | 755 | |
| `integer_ring/` | 3 | 611 | needs Singular |
| `std_test/` | 8 | 551 | |

**How they are run.**
- Each file is verified as `verus --crate-type=lib <file>`.
- The in-tree test `source/rust_verify_test/tests/examples.rs` drives them.
  Its `examples_in_dir!` macro covers the directories above except `scache/`,
  and covers `integer_ring/` only under the `singular` feature.
- The expected outcome is read from a first-line tag
  (`// rust_verify/tests/example.rs <mode>`). The modes are `expect-success`
  (the default), `expect-failures`, `expect-errors`, `expect-warnings`,
  `expand-errors`, `no-report-long-running` and `ignore` (see
  `examples/README.md`).
- The harness passes `--internal-test-mode` and its own `--extern` flags.
  Running the files with the plain `verus` driver is equivalent for corpus
  purposes, because the verdicts match, as measured below.

**Measured with the pinned release.** All 147 non-`ignore` files were run
serially as `verus --crate-type=lib <file>`, adding the tag's flags where it
has them:

| Measure | Result |
|---|---|
| Functions verified | 1,333, summed over all files |
| Total wall time | 88 s serial |
| Slowest files | `cuckoo_hash_table/cuckoo.rs` (2.8 s), `cuckoo_hash_table/main.rs` (2.7 s), `test_expand_errors.rs` (1.7 s) |
| Typical file | under 1 s |

**Outcome against the tags.** 145 of 147 files matched their tag's expected
outcome:
- Success files verified with 0 errors.
- The 5 `expect-failures`/`expand-errors` files exited non-zero with
  verification failures.
- The two misses are `integer_ring/integer_ring.rs` and
  `integer_ring/integer_ring_bound_check.rs`. They need a Singular-enabled
  Verus, so the in-tree harness skips them too.

Eight files are tagged `ignore`, for these reasons: plain Rust, an expected
timeout (`trigger_loops.rs`, `integer_ring/circular_by_d.rs`), deprecated APIs,
or no reason given (`scache/rwlock.rs`).

**Assessment.** The examples are ideal for the fast tier and for conformance
diversity:
- They include intentional failures, which exercise Verus's follow-up queries
  after a failed `check-sat`, its error reporting and `--expand-errors`.
- They exercise bitvector and nonlinear arithmetic, `assert_by_compute`, and
  state machines.
- Each file runs in about a second.

### 1.3 `source/rust_verify_test/tests`

**Format.** 161 `.rs` files (117,580 lines). Each one is a Rust integration
test, not a Verus source file. Verus code is embedded as token trees inside
macros:

```rust
test_verify_one_file! {
    #[test] basic_while verus_code! { fn test1() { ... } } => Ok(())
}
```

The expected outcome is a Rust pattern or assertion after `=>`, for example
`Err(err) => assert_one_fails(err)`. There are about 4,400 `#[test]` items
(3,196 `test_verify_one_file!` and 1,134 `test_verify_one_file_with_options!`
invocations). The largest files are `state_machines.rs` (236 tests),
`traits.rs` (218), `modes.rs` (126) and `mut_refs.rs` (117).

`tests/cargo-tests/` holds 17 small Cargo crates (12 verified, 5 unverified)
that exercise `cargo verus`.

**How they run.** They need a source build of Verus:
- `vargo test -p rust_verify_test`, or in CI
  `cargo nextest run --release -p rust_verify_test`.
- The harness (`tests/common/mod.rs`) panics unless `VERUS_IN_VARGO` is set.
- It locates `rust_verify`, `libverus_builtin.rlib` and `vstd.vir` under the
  workspace's `target-verus/<profile>` directory.
- It writes each test's sources to `target/test_inputs/<binary>-<test>/` and
  invokes `rust_verify` with `--internal-test-mode --crate-type lib ...`.

So the tests **cannot run standalone against the release binary**. Two facts
are useful for sail:
- The harness honours `VERUS_Z3_PATH`, so a source build at the pinned commit
  can run the whole suite through `sail-z3` unchanged.
- With `VERUS_KEEP_TEST_DIR=1` the harness keeps each materialized input
  directory and prints the exact rerun command. One harness run can therefore
  export about 4,400 standalone single-file inputs plus their command lines,
  which the release `verus` can then replay.

**Assessment.** This is the widest feature-coverage set, and it includes many
expected errors and failures, which makes it good conformance material. Its
cost is a source build of Verus at the pinned commit with Rust 1.98.1. That is
acceptable for the standard tier, but too heavy for the fast tier. Two options,
deferred to a follow-up issue:
- Run it through the source build with `VERUS_Z3_PATH` pointed at the shim.
- Export the inputs once and pin them as generated corpus data, keyed by the
  Verus commit.

Either way, conformance should compare sail against plain Verus. It does not
need to re-check the test's expected-outcome patterns.

## 2. External projects

The compatibility column uses three values:
- **compatible:** tracks recent Verus and passed verita around the pinned
  release.
- **needs older Verus:** pinned to an earlier Verus or a fork.
- **unknown:** there is not enough evidence either way.

The "Head" column gives the default-branch head as of 2026-10-08. It is the
candidate pin, still to be validated by a baseline run.

### 2.1 Systems projects

**Projects in verita at the pinned commit**

| Project | Description | Size (Rust) | Commits, last commit | Head (candidate pin) | Verus pin and how | Build / verify | License | Compatibility |
|---|---|---|---|---|---|---|---|---|
| Anvil <https://github.com/anvil-verifier/anvil> | Verified Kubernetes controllers; `src/controllers/` has rabbitmq, vreplicaset, vdeployment, vstatefulset, plus composition proofs | ~120k LOC | 810, 2026-09-30 | `a597f9de` | Tracks Verus `main`. `vstd` is a git dependency with no rev; CI resolves verus `main` and runs `tools/setup-verus.sh`. | `cargo verus verify --lib [-- --verify-only-module X]`. verita uses `--verify-module vdeployment_controller`; the config notes about 30 min for all controllers. | no SPDX id (LICENSE file present) | **compatible**, but rlimit-sensitive: it failed on rlimit at release+2 and passed at release+9 |
| verified-ironkv <https://github.com/verus-lang/verified-ironkv> | IronFleet's sharded KV store ported to Verus | ~12k LOC (plus C# client) | 29, 2026-09-22 | `7e35ed09` | No pin; CI clones Verus HEAD | `verus --crate-type=dylib ironsht/src/lib.rs`; verita uses `cargo verus` on `ironsht` | MIT | **compatible** (50 s in verita) |
| verified-node-replication <https://github.com/verus-lang/verified-node-replication> | Node Replication (NrOS / IronSync) in Verus | ~19k LOC | 234, 2026-07-16 | `6aed637f` | `verus` submodule is a stale `d8f073f3` (2024-04-23). The code at HEAD has been adapted to 2026 Verus. | `verus --crate-type=dylib verified-node-replication/src/lib.rs` | MIT | **compatible at HEAD** (13 s). Ignore the submodule pin. |
| verified-storage <https://github.com/microsoft/verified-storage> | Persistent-memory storage: `pmemlog`, `multilog`, `capybaraKV/capybarakv` | ~60k LOC | 404, 2026-09-22 | `41cab950` | crates.io `vstd = "=0.0.0-2026-09-20-0158"` per crate; `rust-toolchain` 1.97.1 / nightly-2026-07-12 per crate | `cargo verus verify --no-default-features` in each crate dir | MIT | **compatible** (3.5 min); needs verita-style `[patch]` to the pinned vstd |
| verified-memory-allocator <https://github.com/verus-lang/verified-memory-allocator> | Verified mimalloc-style allocator (`verus-mimalloc/`) | ~28k LOC | 52, 2026-07-31 | `9f99d4c2` | No pin and no CI. The last commit is a "Z3 4.16.0 ... proof stabilization". | `verus-mimalloc/verify.sh`. verita builds a `libc` rlib first and passes `--extern libc=... --triggers-mode silent --rlimit 240`. **Needs Singular.** | no SPDX id | **compatible** (4.8 min) |
| vest <https://github.com/secure-foundations/vest> | Verified binary parser/serializer combinators, plus generated TLS etc. parsers | ~280k LOC incl. generated | 1,072, 2026-10-06 | `11af01dd` | `verus.json` pins release `0.2026.09.06.8dea4a2` with per-platform sha256, Rust 1.98.0, Z3 4.16.0; crates.io `vstd = "=0.0.0-2026-09-06-0133"` | `cargo verus verify --locked --check-toolchain`; verita roots `vest_lib` and `vest_tests` (`--verify-module tls`) | MIT | **compatible** (2 min + 3.7 min) |
| verified-nrkernel (page table) <https://github.com/matthias-brun/verified-nrkernel> | Verified x86-64 page table from NrOS | ~55k LOC | 1,383, 2026-10-06 | `1c861090` | CI installs the latest Verus release and Rust 1.98.1. verita uses fork branch `parno/verified-nrkernel@z3-4.16.0`, head `be213895` (2026-10-07). | `tools/verify.sh`: `verus --crate-type=lib --rlimit 100 --cfg feature="impl" page-table/src/lib.rs` | no SPDX id (MIT and Apache files) | **compatible** (9.3 min). Pin the commit verita validated. |
| verismo <https://github.com/microsoft/verismo> | Verified security module for AMD SEV-SNP confidential VMs | ~44k LOC | 77, 2026-09-18 | `6bc6dd32` | `tools/install_verus` sets `VERUS_VERSION=0.2026.09.13.671956e` and Rust 1.98.1; crates.io `vstd =0.0.0-2026-09-06-0133` | `cd source && cargo verus focus --release`; verita roots `bitflags_verus`, `verismo_tspec`, `verismo` | MIT | **compatible** (3.8 min) |
| tla-rs <https://github.com/stonysystems/tla-rs> | TLA+/Paxos-style specs and proofs in Verus | large (16.6 MB Rust bytes) | 2,063, 2026-10-07 | `94fd2a7a` | No pin found | `verus --crate-type=dylib src/lib.rs` | MIT | **compatible** (3 min). Commented out of verita on `main` after the pinned commit ("restore when it stabilizes"). |
| COCONUT-SVSM <https://github.com/coconut-svsm/svsm> | Secure VM service module with optional Verus proofs | mostly unverified Rust | 4,074, 2026-09-30 | n/a | Upstream uses crates.io `vstd =0.0.0-2026-07-12-0122`. verita uses fork branch `ziqiaozhou/svsm@verita`. | `cargo verus focus --features verus` (alias `cargo verify`); needs `x86_64-unknown-none` | MIT | **unknown/at risk**: failed at release+2, passed at release+9 |

**Projects outside verita**

| Project | Description | Size (Rust) | Commits, last commit | Head (candidate pin) | Verus pin and how | Build / verify | License | Compatibility |
|---|---|---|---|---|---|---|---|---|
| semi-persistent <https://github.com/yaspar-org/semi-persistent> | Verified semi-persistent containers / e-graph | 9.2 MB Rust bytes | 337, 2026-10-07 | `dd17c59e` | `.verus-version` = `0.2026.09.20.aef82ed`, Rust 1.98.1 | `cargo verus verify` | Apache-2.0 | likely compatible (pinned two weeks earlier); not in verita |
| owl <https://github.com/secure-foundations/owl> | Protocol verifier whose compiler emits Verus | ~45k LOC Rust + Haskell | 944, 2026-09-29 | n/a | README: Verus `0.2026.09.06.8dea4a2`, Rust 1.98.0 | Haskell toolchain, then `run_verus` on extracted code | BSD-3-Clause | likely compatible; heavy build |
| verus-tla <https://github.com/anvil-verifier/verus-tla> | TLA embedding split out of Anvil | small (155 KB) | 38, 2026-10-06 | n/a | vstd git, no rev; CI uses Verus `main` | `cargo verus verify --lib` | MIT | likely compatible |
| vostd (Asterinas) <https://github.com/asterinas/vostd> | Verified OS framework (OSTD), CortenMM line of work | ~115k LOC | 3,401, 2026-10-08 | n/a | CI uses the `asterinas/verus` fork via `cargo dv bootstrap`; upstream CI needs unmerged Verus PRs | `cargo dv verify --targets ostd` | MPL-2.0 | **needs a Verus fork** |
| Atmosphere <https://github.com/mars-research/atmosphere> | Verified microkernel | ~70k LOC | 562, 2026-02-02 | n/a | Nix flake input `mars-research/verus/mars` locked to `68dbc921` (2024-11); nightly-2024-11-01 | `nix develop`, `atmo verify` | MIT | **needs older Verus (fork)** |
| CortenMM artifact <https://github.com/TELOS-syslab/CortenMM-Artifact> | SOSP'25 memory-management artifact | large incl. kernel | 26, 2025-11-12 | n/a | Rust 1.88.0, xtask-managed Verus | `make verify` | MPL-2.0 | needs older Verus |
| Verdict <https://github.com/secure-foundations/verdict> | Verified X.509 validation | ~24k LOC | 471, 2025-11-19 | n/a | `deps/verus` submodule `112ecd1c` (2025-06), Rust 1.86.0 | `cargo verus build --release` after `tools/activate.sh` | Apache-2.0 | needs older Verus |
| rlsf-verified <https://github.com/unsoundsystem/rlsf-verified> | Verified TLSF allocator | ~32k LOC | 356, 2026-08-07 | n/a | vstd git, no rev | Makefile | MIT | unknown |
| APAS-VERUS <https://github.com/briangmilnes/APAS-VERUS> | Algorithms-textbook proofs, largely machine-written | 13 MB Rust bytes | 2,777, 2026-09-25 | n/a | crates.io `vstd = "0.0.0-2025-08-12-1837"` (caret), Rust 1.98.1 | `cargo verus` (`validate.sh`) | MIT | unknown |
| gale <https://github.com/pulseengine/gale> | Zephyr kernel primitives (Verus + Kani + Rocq) | 4.4 MB Rust bytes | 720, 2026-10-08 | n/a | Bazel `rules_verus` override; Verus version not checked | Bazel | Apache-2.0 | unknown |
| verdist <https://github.com/bsdinis/verdist> | Distributed protocols (ABD, echo) | ~20k LOC | 219, 2026-06-29 | n/a | path `../verus`, Rust 1.95.0 | justfile | none | unknown; no licence |
| verus-grove <https://github.com/upamanyus/verus-grove> | Grove port | small | 119, 2024-04-26 | n/a | not checked | n/a | none | needs older Verus; inactive |
| SOSP'24 Verus artifact <https://github.com/verus-lang/paper-sosp24-artifact> | Paper artifact | n/a | 160, 2025-01-29 | n/a | Verus `097ac7ed` (2024-08) plus case-study SHAs | scripts | no SPDX id | needs older Verus; useful only as a list of historical SHAs |

**Searched for but not found as public repos:** `Azure/verismo` (the project
is `microsoft/verismo`), `verus-lang/verus-mimalloc` (it lives in
verified-memory-allocator), `verus-lang/verified-paging-for-x86-64` and
`verus-lang/verified-nrkernel` (the page-table work lives in
`matthias-brun/verified-nrkernel`; `utaal/verified-nrkernel` redirects there),
`verus-lang/verus-benchmarks`, a standalone Verus BTree, Lion, SAFE, and a
standalone VerusSync repo (it ships in Verus's `state_machines` macros).
IronFleet's Verus port is verified-ironkv. The IronSync OSDI'23 artifact
(<https://github.com/secure-foundations/ironsync-osdi2023>) is Linear Dafny,
not Verus.

**Further discovery.** crates.io lists 18 crates depending on `vstd`
(<https://crates.io/crates/vstd/reverse_dependencies>). Examples include
`vest_lib`, `regorus`, `verified-graphs`, `semi-persistent-*` and
`smoothutf8`. Most pin a crates.io `vstd` date, which makes their Verus
compatibility easy to read. This list should be rechecked when the corpus is
next extended.

### 2.2 Agent-benchmark corpora (for the "agent loop" workload)

These are sets of many small, independent Verus tasks, each usually with a
spec, an implementation and a proof. They suit the agent-loop simulation:
- strip or perturb proofs;
- replay edit-and-verify cycles;
- measure time to verdict per cycle.

| Corpus | Contents | Activity | Verus pin | License | Compatibility |
|---|---|---|---|---|---|
| human-eval-verus <https://github.com/secure-foundations/human-eval-verus> | HumanEval tasks: 167 `tasks/human_eval_*.rs` files plus a `tasks/lib.rs` root | 103 commits, 2026-09-26; head `bbe93f73` | CI downloads the latest Verus release and runs `verus` on each file | MIT | **compatible**; whole set in 52 s in verita (root `tasks/lib.rs`, `--crate-type=lib`) |
| Verus-Bench and VeruSAGE-Bench <https://github.com/microsoft/verus-proof-synthesis> | AutoVerus Verus-Bench (150 tasks: CloverBench, MBPP, Diffy, misc) and VeruSAGE-Bench (849 repo-level tasks cut from Anvil, IronKV, the allocator, NR, NRKernel, Atmosphere, verified-storage, Vest) | 192 commits, 2026-08-24 | Verus `33269ac6` (2024-08) and `ddc66116` (2025-09) per README/Dockerfile | MIT | **needs older Verus**; tasks are snapshots and need re-validation |
| VeriContest <https://github.com/HIPREL-Group/VeriContest> | 1,006 competitive-programming problems (LeetCode, Codeforces), each with spec, code, `verified.rs` and tests | 51 commits, 2026-10-07 | Bundled Verus binary labelled "0.2026.04.10" | Apache-2.0 (+ data licence) | unknown (April 2026); needs re-validation |
| vericoding-benchmark <https://github.com/Beneficial-AI-Foundation/vericoding-benchmark> | 12,504 tasks across Dafny, Lean and Verus; `jsonl/verus_tasks.jsonl` has 2,166 Verus tasks, some not compiling per the README | 15 commits, 2026-06-05 | tools repo CI pins `release/0.2025.08.25.63ab0cb` | MIT | needs older Verus or a filtering pass |
| AlphaVerus <https://github.com/cmu-l3/alphaverus> | Dafny-to-Verus translated datasets (DafnyBench, HumanEval, MBPP) | 4 commits, 2025-05-14 | `VERUS_PATH` only | none | unknown; no licence |
| verus-mathspec-bench <https://github.com/rikosellic/verus-mathspec-bench> | Mathlib-derived spec tasks | 31 commits, 2026-05-23 | README `0.2026.05.17.e479cce` | Apache-2.0 | needs slightly older Verus |
| RVBench / RagVerus <https://github.com/GouQi12138/RVBench> | README only; data on Zenodo (record 15313495) | 2 commits | n/a | none | unknown |

Two more sources of agent-loop material:
- The in-tree `rust_verify_test` inputs (section 1.3). There are about 4,400
  of them, each with a known expected outcome, including failures.
- VeruSAGE-style tasks rebuilt from the first-tier systems projects at their
  pinned commits. Each task removes a proof from one function and re-verifies.
  This avoids depending on stale snapshots.

## 3. Recommendation

### Tier 1: pin now (fast tier and baseline; compatible with the pinned release)

**Fast conformance subset** (gates every PR once the harness exists; about
2 minutes serial on the test machine):
- **verus-examples**: the 145 non-`ignore`, non-Singular files under
  `examples/` (1,333 functions, 88 s serial). Each file keeps its tag-derived
  expected outcome as metadata. Conformance itself is differential.
- **vstd**: one invocation, 2,060 functions, 25 s, 1.9 GB.
- **human-eval-verus** at `bbe93f73`: 167 tasks, about 1 min.

**Standard tier** (main corpus, gates merges once it exists; roughly 20 to 25
minutes serial, much less in parallel). These are the verita projects that
passed around the release, pinned to the commits verita validated or to the
heads listed above:

| Project | Pin | Notes |
|---|---|---|
| verified-node-replication | `6aed637f` | 13 s |
| verified-ironkv | `7e35ed09` | 50 s |
| vest | `11af01dd` | `vest_lib`, plus `vest_tests --verify-module tls` |
| verified-storage | `41cab950` | pmemlog, multilog, capybarakv with `--no-default-features` |
| verismo | `6bc6dd32` | |
| verified-nrkernel page-table | `be213895` (verita's `parno` fork branch) or the upstream `1c861090` | 9.3 min, `--rlimit 100` |
| verified-memory-allocator | `9f99d4c2` | needs Singular, so it is optional on machines without it |
| Anvil | `a597f9de` | single module `vdeployment_controller` (about 7 min) |

**Deep tier.**
- Anvil with all controllers (about 30 min).
- tla-rs (`94fd2a7a`).
- The exported `rust_verify_test` inputs (section 1.3).
- Fragility and portfolio runs over all of the above.

**Before pinning,** each entry still needs a plain-Verus baseline with the
pinned release on our machines. Any entry that does not verify cleanly gets
its outcome recorded as-is rather than "fixed". Conformance is differential,
so a project with a known failure is still a valid entry if the failure is
stable. Unstable outcomes are themselves a finding for the fragility census.

### Tier 2: needs version work

| Project(s) | Work needed |
|---|---|
| COCONUT-SVSM | Upstream pins July 2026 vstd; failed at release+2. Use the verita fork branch, or wait. |
| semi-persistent, owl, verus-tla | Likely compatible; need a baseline run and, for owl, a Haskell build. |
| vostd (Asterinas) | Needs the `asterinas/verus` fork. |
| Atmosphere, Verdict, CortenMM | Pinned to 2024 or 2025 Verus or forks. |
| VeriContest, vericoding, Verus-Bench, VeruSAGE-Bench | Agent corpora on older Verus. Validate each task against the pinned release, and keep only those whose plain-Verus outcome is stable. |

These need either a second supported Verus pin in the compatibility matrix
(which the charter allows) or a filtering pass. Each is an explicit change
with its own conformance run.

### History-replay candidates

These projects have long histories that stay on recent Verus. Replay must pair
each commit with the Verus release that commit was written against. Commits
that need a Verus other than a supported pin are skipped and counted, not
silently run.

| Project | History | How historical Verus pins can be read |
|---|---|---|
| vest | 1,072 commits | Exact Verus release in `verus.json` at every commit since it was introduced. **The best replay candidate.** |
| verified-nrkernel | 1,383 commits | CI uses the latest release; `tools/update-verus.sh` records past pins. |
| Anvil | 810 commits | Tracks `main`, so the Verus commit is inferred from the commit date. |
| verified-storage | 404 commits | Regular "update to latest Verus" commits; pins readable from crates.io `vstd` dates in `Cargo.toml`. |
| verismo | 77 commits | `VERUS_VERSION` in `tools/install_verus`. Short but exact. |
| tla-rs, APAS-VERUS | 2,063 and 2,777 commits | Long, but pins are unclear. |
| vostd | 3,401 commits | Needs a fork. |

For a first replay experiment, take the most recent window of commits whose
recorded pin equals `0.2026.10.04` or is replay-compatible with it (to be
measured). vest and verified-storage are the obvious starting points.

## 4. Draft `corpus/corpus.toml` schema

This is a proposal only. The file itself lands with the corpus-pinning change,
together with an ADR if the format is adopted. Field names follow verita where
they overlap (`crate_roots`, `extra_verus_args`, `extra_cargo_args`,
`prepare_script`, `requires_singular`), so entries can be checked against
verita's list.

```toml
# corpus/corpus.toml (draft schema)
schema_version = 1

# The Verus pin this file is validated against; must match corpus/toolchain.toml.
verus_release = "0.2026.10.04.426d8b0"
verus_commit = "426d8b01e7ffb910a36c15e1f870ff985bb61eef"

[[project]]
name = "vstd"                                   # unique id; used in result paths
url = "https://github.com/verus-lang/verus"
commit = "426d8b01e7ffb910a36c15e1f870ff985bb61eef"  # full SHA, never a branch
license = "MIT"                                 # SPDX expression, or "NOASSERTION"
tier = "fast"                                   # fast | standard | deep | tier2
verus_version_compat = "compatible"             # compatible | needs-older | needs-fork | unknown
compat_evidence = "release binary, 2,060 verified, 0 errors (2026-10-08)"
build = "verus"                                 # verus | cargo-verus | script
crate_roots = ["source/vstd/vstd.rs"]           # .rs roots for `verus`, package dirs for `cargo-verus`
extra_verus_args = ["--crate-type=lib", "--is-vstd",
                    "--cfg", "feature=\"std\"", "--cfg", "feature=\"alloc\""]
env = { VSTD_KIND = "IsVstd" }
requires = []                                   # e.g. ["singular"], ["x86_64-unknown-none"]
workloads = ["cold", "warm", "fragility"]       # workloads this entry participates in
expected = "verified"                           # verified | failures-expected | per-entry (see entries)

[[project]]
name = "verus-examples"
url = "https://github.com/verus-lang/verus"
commit = "426d8b01e7ffb910a36c15e1f870ff985bb61eef"
license = "MIT"
tier = "fast"
verus_version_compat = "compatible"
build = "verus"
# Each file is a separate crate root; tags come from the file's first line.
crate_roots_glob = ["examples/**/*.rs"]
exclude = ["examples/scache/**", "examples/integer_ring/**"]
expected = "per-entry"
expected_from = "examples-first-line-tag"        # parse `// rust_verify/tests/example.rs <mode>`
extra_verus_args = ["--crate-type=lib"]

[[project]]
name = "verified-storage"
url = "https://github.com/microsoft/verified-storage"
commit = "41cab95051229ebd457b4eed7fc111e3e7c9099c"
license = "MIT"
tier = "standard"
verus_version_compat = "compatible"
compat_evidence = "https://github.com/verus-lang/verus/actions/runs/37529962706"
build = "cargo-verus"
crate_roots = ["capybaraKV/capybarakv", "multilog/multilog", "pmemlog"]
extra_cargo_args = ["--no-default-features"]
patch_verus_deps = true                          # rewrite vstd/verus_builtin deps to the pinned release
history = { replay = true, pin_source = "Cargo.toml vstd version" }

[[project]]
name = "verified-memory-allocator"
url = "https://github.com/verus-lang/verified-memory-allocator"
commit = "9f99d4c22e26024be4e3e2d40b7501daab570860"
license = "NOASSERTION"
tier = "standard"
verus_version_compat = "compatible"
build = "script"
prepare_script = "scripts/prepare-libc.sh"       # kept in corpus/, not inlined, so it is reviewable
crate_roots = ["verus-mimalloc/lib.rs"]
extra_verus_args = ["--extern", "libc=build/liblibc.rlib",
                    "--triggers-mode", "silent", "--rlimit", "240"]
requires = ["singular"]
```

**Field semantics**
- `commit` is always a full SHA. Branch heads are resolved once, when pinning.
- `tier` controls which testing tier runs the entry (AGENTS.md "Testing
  tiers").
- `verus_version_compat` and `compat_evidence` feed the compatibility matrix
  that the charter's "Version pinning" asks for.
- `expected` records plain Verus's observed outcome at pin time, as metadata
  only. The conformance verdict is always sail against plain Verus on the same
  run (charter rule 1), never against this field.
- `patch_verus_deps` means rewriting `vstd`/`verus_builtin`/`verus_builtin_macros`
  dependencies to the pinned release, as verita's
  `inject_verus_patches` does. The patch happens in a scratch copy, never in a
  vendored tree.
- `history` marks history-replay candidates and says where each historical
  commit's Verus pin can be read.
- `workloads` limits which benchmark workloads include the entry. When it is
  omitted, the entry takes part in all workloads its tier runs.

## Open questions for the owner

1. **The `rust_verify_test` suite.** Should it be included through a source
   build of Verus at the pinned commit (standard or deep tier), or exported once
   into a pinned generated corpus? Both need Rust 1.98.1 and roughly an hour of
   CI time.
2. **Singular.** Should Singular be a required corpus dependency (for the
   memory allocator and `examples/integer_ring`), or an optional one?
3. **Fork pins.** verita validates `verified-nrkernel` and `svsm` from personal
   fork branches. Is pinning a fork commit acceptable, or should the corpus pin
   only upstream commits?
4. **Agent corpora on older Verus.** Should we pin a second Verus release so
   that VeruSAGE-Bench and Verus-Bench can be used as published, or only use the
   tasks that re-validate on the current pin?
