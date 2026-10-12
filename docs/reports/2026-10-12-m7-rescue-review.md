# M7 rescue review (2026-10-12)

This review covers the M7 branch `feat/m7-contracts` (draft PR #74) at `f7103bf`, at the point the owner stopped its agent and handed M7 to a rescue lead. The review was read-only, against the code, ADR 0027, the roadmap, `bench/` and `docs/validation/`. Everything from before the rescue is preserved in the tag `archive/m7-contracts-f7103bf`. The decisions that follow from this review are in [ADR 0038](../decisions/0038-m7-rescue.md).

The M7 code it reviews is on the integration branch `rescue/m7` and lands on `main` in small PRs. File paths below refer to that branch.

## Summary

- **Default-mode correctness is intact.**
  - The interactive engine is opt-in.
  - Engine receipts use separate context hashes.
  - The managed runner strips `SAIL_ENGINE*`.
  - Focused evidence is always Provisional.
  - The daemon recomputes repair acceptance against live freshness.
  - The non-engine M7 features (baselines, compare, explain, proof journal, MCP tools) have behavioural tests, and CI on #74 was green.
- **The M7 performance thesis was unproven, and the only data pointed the wrong way.** No accepted benchmark sample exists, and Vest was never reached. Every paired M6/M7 measurement showed M7 slower:

  | Source | M7 | M6 | M7 relative to M6 |
  |---|---:|---:|---:|
  | 387ba80, HumanEval GCD (M7 used three compiler drivers, refused SAIL0942) | 160.6 s | ~110.7 s | slower |
  | b89546c warm-ups | 150–164 s | 83 s | slower |
  | macOS frontend-reuse fixture | — | — | 0.51× |

  All of these ran under x86 emulation on an Apple M3, except the macOS fixture.
- **Velocity collapsed into bookkeeping.** In about 2.5 days the branch gained:
  - 939 commits, of which 451 were `docs:` and 32 were `feat:`;
  - +458k lines, with product Rust about 8% of them;
  - 125k lines across 425 files in `docs/validation`;
  - 11 MB of unreferenced candidate patches.

  The roadmap's "Current state" had become an append-only log, and everything sat in one draft PR of 455k lines.
- **The environment multiplied every cost.**
  - x86 emulation on the M3 inflated wall time by 8–15×. M6 GCD took 95 s, against about 10 s on native GHA.
  - The emulation caused timing-test flakes, such as missed 100 ms and 250 ms deadlines.
  - Benchmark attempts of 990–2,000 s aborted at the first oracle refusal with zero samples.

## Engine risks (opt-in mode)

1. **No stock confirmation.** A completed engine run is the authoritative result (`crates/sail-verus/src/adapter.rs`, `engine_output`). `engine_events::completed` checks only the shape of the output. Stock Verus runs only when the engine output is malformed.
2. **Persistent prepared-lowering reuse depends on dependency tracking.** The cross-edit cache under `<cache>/engine-prepared` is keyed on Verus's pruned krate plus a hand-picked argument tuple. That is in tension with charter rule 6. The cached-versus-fresh oracle ran only during qualification, never on real hits.
3. **Engine results look like default mode.** Engine-produced function reports carried `provenance.mode: "default"`, and the project result had no engine field (charter rule 7).
4. **The ADR and the code disagreed.**
   - ADR 0027 says an engine fault falls back to stock, but the CLI failed the repair instead.
   - The engine README named codec v2 while the code checks v3.
   - The host qualification pinned an older patch digest.

## Reader regression

Result p95 was 436 ms at 387ba80, against a 200 ms bound. Earlier builds passed: b89546c at 91 ms and 730a321 at 94 ms. The stage profile put about 170 ms in socket output of a 4.3 MB frame, about 83 ms in freshness and about 21 ms in the stdout seal. Cheap fixes were identified, but the first step is to bisect the regression.

## Code quality

- `run_engine` is about 510 lines, and `managed/runner.rs` is 3.1k lines.
- A Python helper process sits in the product's runtime path (`managed/lease.rs`). It obtains Linux file leases without unsafe Rust, but its benefit was never measured.
- Some machinery is over-precise for small gains, such as byte-exact allocator accounting in `result_cache.rs`.
- Cleanup logic lives in Python scripts instead of `sail cache gc`.
- Per-selector target directories have no garbage collection.
- A second repair in the same compiler family is refused instead of queued.

## Existing profile signal

The roadmap's warm-vstd measurement (2026-10-08) already showed that after a warm cache, Verus's own phases dominate:

| Phase | Time |
|---|---:|
| Verus phase (CPU, all threads) | 50 s |
| Rust front end | 5.6 s |
| Solver | 3.5 s |

The remaining speed lever is the Verus pipeline, not the solver.

## Tidy-up performed at handover

- **Worktrees.** Removed `m7-baseline`, which was the detached M6 baseline at `0fde777` with no changes. Removed `fix/m7-qualified-evidence-restage`, whose single commit is preserved as the tag `archive/m7-qualified-evidence-restage-c84db8c`.
- **Docker.** Stopped and removed the idle containers `sail-m7-acceptance` and `sail-m7-handoff-validation`. Removed five unused `sail-m7-*` build and cache volumes (about 3.5 GB). The `sail-ci-*` volumes are kept.
- **Raw evidence.** About 1.2 GB of loose evidence in the checkout (`_retired-evidence/`, `.claude/worktrees/*-evidence`, `conformance-out/`) was moved into one archive outside the repository. Its sha256 is `c281800ad3c42f39a71745961fc6a44a8ad327caca73df7274f55a86316317a9`.
