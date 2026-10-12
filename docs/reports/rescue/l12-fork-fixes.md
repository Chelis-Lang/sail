# Rescue lane L12: Verus issue fixes in Sail's fork

- Branch: `l12-fork-fixes` (Sail), on `rescue/m7` (`3fecedf`); fork branch `l12/fixes`
  on [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus), cut from
  `sail` (= upstream `426d8b0`).
- Host: EC2 `c7i.8xlarge` (32 vCPU, us-east-1), lane `l12`, native x86-64
  Linux (Ubuntu 24.04), Rust 1.98.1, pinned Z3 4.16.0 from the
  `0.2026.10.04.426d8b0` release archive.
- Raw evidence: `~/sail-raw-evidence/l12/` on the lead's machine, cited by
  file name and SHA-256 prefix.
- Owner decision (2026-10-12): Sail stays on a Verus fork and fixes known
  issues there; nothing is filed or contributed upstream
  ([ADR 0039](../../decisions/0039-private-verus-fork.md)).

## Layout

The engine is now built from a pinned fork commit
([engine/verus/fork.toml](../../../engine/verus/fork.toml)) instead of the
upstream commit plus `engine/verus/interactive.patch`. The fork's `sail`
branch is the upstream pin plus one commit per fix (each with its regression
test), then the engine commit. `cargo xtask build-verus-engine` fetches the
commit with a GitHub token; the variant feature is `sail-fork:<commit>`;
`ci/engine-artifact.py` pins `fork_commit` (pin schema 2) and
`sail engine status` reports it (status version 2).

## Fixes

Each fix is one commit on `l12/fixes`. "Source change" counts the change
without its test (lines added and removed). Every test fails on the pinned stock source and passes on
the fork (VI-0012's stock failure is probabilistic; see below).

| Issue | Fork commit | Source change | Regression test |
|---|---|---|---|
| (harness) | `15763f47` test: pass raw verifier arguments through the rust_verify_test harness | +5 (`tests/common/mod.rs`) | used by the tests below |
| VI-0001 | `9a5b2cc8` fix: generate enum arrow accessors in source order | +16 (`enum_synthesize.rs`) | `sail_vi0001_accessor_order`: seven accessors declared in source order in the AIR log (stock passes once in 5,040) |
| VI-0015 | `404a1e30` fix: choose automatic triggers in a deterministic search order | +22 −3 (`triggers_auto.rs`) | `sail_vi0015_trigger_order`: eight identical quantifiers in one file get identical triggers (stock differed within one process) |
| VI-0007 | `f98b87c5` fix: print bucket diagnostics in bucket order | +29 −43 (`verifier.rs`) | `sail_vi0007_error_order`: five modules' errors in module order, three runs; the first module is the slowest |
| VI-0012 | `f66f8e22` fix: report bucket start notes in queue order | +37 −7 (`verifier.rs`) | `sail_vi0012_progress_order`: 64 modules, 16 threads, 20 runs; notes in queue order |
| VI-0008 | `4f9cd6d2` fix: list output-json func-details sorted by name | +15 −2 (`main.rs`) | `sail_vi0008_func_details_order`: eight functions sorted (stock passes once in 40,320) |
| VI-0003 | `11adad41` fix: record bit-vector failures in the function failure set | +6 (`verifier.rs`) | `sail_vi0003_bitvector_success`: failed `by (bit_vector)` function has `success: false` |
| VI-0009 | `338202a5` fix: omit unselected functions from function timing | +15 −1 (`verifier.rs`) | `sail_vi0009_selected_timing`: `--verify-function good` lists only `good` |
| VI-0010 | `be077769` feat: add function-path to the timing function breakdown | +17 (`main.rs`) | `sail_vi0010_function_path`: two `S::check` impls get distinct `function-path` values |
| VI-0005 | `441d6cd9` fix: keep verification dep-info in the compile pass | +41 (`cargo_verus_dep_tracker.rs`, `driver.rs`, `verifier.rs`) | `sail_vi0005_cargo_rerun`: same arguments stay fresh, changed `--rlimit` re-verifies |
| VI-0004 | `750027a9` fix: give each crate its own log directory under cargo verus | +29 (`verifier.rs`) | `sail_vi0004_cargo_log_dirs`: library and binary logs both kept, in their own subdirectories |

Notes:

- **VI-0005 cause.** rust_verify already records the forwarded Verus
  arguments as dep-info `env-dep` entries, but via Cargo it runs rustc twice;
  the second (erase-macro compile) pass rewrote the same `.d` file without
  them. The fix carries the first pass's env and file dependencies into the
  second. A package that receives the changed arguments re-verifies, which
  includes a path-dependency vstd in the test project.
- **VI-0012 rate.** Stock misordered the notes in 2 of 10 runs with 64
  trivial modules and 16 threads, and in 0 of 20 runs with 8 modules; the fork
  in 0 of 10. The test therefore passes on stock about 1% of the time.
- **Output stock can produce.** Every ordering fix keeps the stock algorithm
  and fixes one of the orders it can already take (source, bucket or queue
  order). VI-0003, 0009 and 0010 change `--output-json` content as their
  issues describe; VI-0004 and VI-0005 change Cargo log layout and
  re-verification on changed arguments (not what a verification reports).
  No verdict or diagnostic logic changed, as ADR 0039's fix rules require.

## Not fixed

- **VI-0011** (launcher macro mode for Cargo `no-vstd`): not trivial. The
  launcher would need Cargo-Verus's package-id derivation (a hash of name,
  version and manifest path) to read only the current package's driver
  arguments, by duplicating it with its `sha2` dependency or sharing a crate.
  The engine's existing relocation workaround stays.
- **VI-0012 suppression:** `--no-report-long-running` still does not
  suppress bucket-start notes. Stock never omits them, so suppression would
  be an output stock cannot produce. Only the order is fixed.

## Validation so far

- **Per commit.** Every commit of `l12/fixes` builds (`vargo build
  --release`, including vstd verification) and passes its own regression
  test on its own (`per-commit-build-and-test.txt` `d2464898`).
- **Fork tip.** All ten regression tests pass (`fork-tip-sail-tests.log`
  `33f2734e`); all ten fail on the pinned stock source with the same test
  files (`stock-sail-tests.log` `77484ef8`).
- **Full suite.** Verus's own `rust_verify_test` suite on the fixes (before
  formatting, same code): 4,647 passed, 0 failed, 58 ignored, which is the
  upstream suite plus the ten new tests (`fork-rust-verify-test-full.log`
  `643050ad`).
- **Markers.** `scripts/check-ai-markers.sh range origin/sail l12/fixes`: all
  11 commits clean.

## Remaining (after L7a lands)

Add the engine commit (`feat: interactive repair engine`, L7a's final
`interactive.patch`) on top of the fixes, delete
`engine/verus/interactive.patch`, re-pin `fork.toml`, then one combined
rebuild, the remaining qualification steps, pack/upload/re-pin
`artifact-linux.json`, and the host gates (`gate`, `solver`, `e2e`,
`conformance fast pr`, the standard tier's engine arm).
