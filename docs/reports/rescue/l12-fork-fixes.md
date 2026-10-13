# Rescue lane L12: Verus issue fixes in Sail's fork

- **Sail branch:** `l12-fork-fixes`, on `rescue/m7`. The first landing
  (`b970ccd`, runtime `4881cd0f`) is merged. This report covers the final pin.
- **Fork:** [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus), branch
  `l12/engine-3` (`d2b69aea`, tagged `pinned/d2b69aea`), for the lead to move
  `sail` onto. Earlier pins are tagged `pinned/4881cd0f` and
  `pinned/89709da7`.
- **Host:** EC2 `c7i.8xlarge` (32 vCPU, us-east-1), lane `l12`, native x86-64
  Linux, Rust 1.98.1, pinned Z3 4.16.0.
- **Raw evidence:** `~/sail-raw-evidence/l12/` on the lead's machine, cited by
  file name and SHA-256 prefix.
- **Decisions:** the owner decided on 2026-10-12 that Sail stays forked and
  files nothing upstream ([ADR 0039](../../decisions/0039-private-verus-fork.md)).
  The lead decided on 2026-10-13 to drop the VI-0007 and VI-0005 fixes (see
  "Dropped fixes" below).

## Layout

The engine is built from a pinned fork commit
([engine/verus/fork.toml](../../../engine/verus/fork.toml)), not from the
upstream commit plus `engine/verus/interactive.patch`. The patch and
`qualification-linux.json` are deleted.

- The fork's `sail` branch is the upstream pin, then one commit per fix, each
  with its regression test, then `feat: interactive repair engine`. That last
  commit is L7a's 701-line patch, unchanged.
- `cargo xtask build-verus-engine` fetches the commit with a GitHub token.
- The build variant is identified by the feature `sail-fork:<commit>`.
- `ci/engine-artifact.py` pins `fork_commit` (pin schema 2), and
  `sail engine status` reports it (status version 2).
- `engine/verus/Cargo.lock` equals the fork's stock lock and stays bound.
- `engine/verus/README.md` describes the branches, the `pinned/<sha>` tag
  rule and the upgrade procedure.

One Sail fix was needed: `7772d9f`. The repair adapter runs Verus under Cargo,
so with VI-0004 the engine's full-phase logs land in `logs/<crate>`. The
adapter now reads them there. Without this, the e2e repair phase failed.

## Fixes in the pinned fork

Each fix is one commit. "Source change" counts the change without its test.
On the pinned stock source, every regression test fails and passes on the
fork. VI-0012's failure on stock is probabilistic.

| Issue | Fork commit | Source change | Regression test |
|---|---|---|---|
| (harness) | `15763f47` test: pass raw verifier arguments through the rust_verify_test harness | +5 | used by the tests below |
| VI-0001 | `9a5b2cc8` fix: generate enum arrow accessors in source order | +16 (`enum_synthesize.rs`) | `sail_vi0001_accessor_order`: seven accessors declared in source order (stock passes once in 5,040) |
| VI-0015 | `404a1e30` fix: choose automatic triggers in a deterministic search order | +22 −3 (`triggers_auto.rs`) | `sail_vi0015_trigger_order`: eight identical quantifiers get identical triggers (stock differed within one process) |
| VI-0012 | `c9de60c6` fix: report bucket start notes in queue order | +37 −7 (`verifier.rs`) | `sail_vi0012_progress_order`: 64 modules, 16 threads, 20 runs |
| VI-0008 | `bf59e1f0` fix: list output-json func-details sorted by name | +15 −2 (`main.rs`) | `sail_vi0008_func_details_order` (stock passes once in 40,320) |
| VI-0003 | `8a5d3655` fix: record bit-vector failures in the function failure set | +6 (`verifier.rs`) | `sail_vi0003_bitvector_success` |
| VI-0009 | `3566421a` fix: omit unselected functions from function timing | +15 −1 (`verifier.rs`) | `sail_vi0009_selected_timing` |
| VI-0010 | `fbde5ea3` feat: add function-path to the timing function breakdown | +17 (`main.rs`) | `sail_vi0010_function_path` |
| VI-0004 | `324c487c` fix: give each crate its own log directory under cargo verus | +29 (`verifier.rs`) | `sail_vi0004_cargo_log_dirs` |

- **VI-0012:** only the order is fixed. `--no-report-long-running` still
  does not suppress the notes, because stock never omits them. Stock
  misordered the notes in 2 of 10 runs with 64 trivial modules and 16
  threads; the fork misordered them in none of 10.
- **VI-0011:** not fixed, because it is not trivial. The launcher would need
  Cargo-Verus's package-id derivation (a hash of name, version and manifest
  path). The engine's existing workaround stays.
- **VI-0014:** moot; it only affected the removed prepared-lowering reuse.

## Dropped fixes

- **VI-0007** (`f98b87c5`, bucket-order diagnostics). The engine arm found
  `debug_expand.rs` different. Stock printed module `M4`'s error first in 3/3
  confirming runs; the fork printed it last. No deterministic order is
  provably within stock's set, so the fix fails ADR 0039's rule and was
  dropped.
- **VI-0005** (`441d6cd9`, then `b35e9226`, dep-info carry-over).
  - Sail's repair passes the selected function as a forwarded Verus argument.
    Binding forwarded arguments in dep-info therefore invalidated the build
    on every repair.
  - Each HumanEval repair rebuilt vstd and the proc-macro crates and ended
    Historical (`SAIL7102`, `SAIL7103`, `SAIL7106`). Each took about 34 s,
    against 5–6 s for a candidate without the fix (`cand-no-vi5/run.log`
    `a2648a63`).
  - The first attempt (`b35e9226`) limited the carry-over to Cargo-selected
    packages, but that did not help.
  - The cause itself is still known: the erase-macro compile pass rewrites
    the `.d` file without the env-deps. A future fix would bind only
    result-affecting arguments.

## Runtimes built and superseded

| Fork commit | Release | Outcome |
|---|---|---|
| `ddfdf197` | deleted | e2e repair phase failed (log subdirectory, fixed in Sail `7772d9f`); every HumanEval repair about 38 s (VI-0005) |
| `4881cd0f` | `engine-runtime-4881cd0f-linux`, merged pin (`b970ccd`) | gates pass; engine arm 1 Different (`debug_expand.rs`, VI-0007); repairs still slow (VI-0005, found on `89709da7`) |
| `89709da7` | deleted | engine arm 0 Different; repairs about 34 s (VI-0005) |
| `d2b69aea` | `engine-runtime-d2b69aea-linux` (asset `635491997`, archive `67f2c489…2b7d`) | **final pin** |

Releases `aaa3b2e4` and `4881cd0f` are to be deleted by the lead after this
pin merges.

## Qualification of `d2b69aea`

| Check | Result | Evidence |
|---|---|---|
| Build | vstd 2,060/2,060, both vargo and release-style | `q4-build` via `cargo xtask build-verus-engine` |
| `test-engine.py` | 11/11 fixtures match stock | `q4/test-engine.log` `0819d89b` |
| `test-cancel.py` | no seal after the kill; the restart matches stock | `q4/test-cancel.log` `0f6ac145` |
| Library tests | `rust_verify` 12, `vir` 1, `air` 144 passed | `q4/summary.txt` `bf80c984` |
| `rust_verify_test` | 4,643 passed, 2 failed, 58 ignored | `q4/rvt-full.log` `7d0ed5e5` |
| Fixes per commit | each builds and passes its test | `per-commit-build-and-test.txt` `d2464898` (original `l12/fixes` commits) |
| Fixes on stock | all regression tests fail | `stock-sail-tests.log` `77484ef8` |

The 4,643 count is the upstream suite plus the eight remaining fork tests.
The two failures are `cargo_new_builds` and `cargo_new_verifies`. `cargo verus
new` looks up a toolchain manifest for the build's version string, and a source
build derives that string from the commit (`0.2026.10.13.d2b69ae`); only
released versions have a manifest. They pass at the upstream release commit,
and no Sail path uses `cargo verus new`.

The per-commit build check ran on the original `l12/fixes` commits. The final
commits are the same diffs cherry-picked without `f98b87c5` and the two
VI-0005 commits. All of them apply cleanly, and the tip builds and passes.

## Gates on the host

All runs used CI image `sail-ci:679d9641401c264c` at Sail `6214d67`. Raw
evidence is in `final2/`.

| Step | Result | Evidence |
|---|---|---|
| `gate` | pass, 520/520 tests | `gate.log` `8a6273cb` |
| `solver` | pass, 37/37 | `solver.log` `f2117407` |
| `e2e` | legacy, managed and repair phases pass | `e2e.log` `16e2123f` |
| `conformance fast pr` | 33/33 Same in each of pass-through, fill and warm; engine 33/33 | `conformance-fast-pr.log` `3d552af5` |
| `conformance fast` | 147/147 Same in each of pass-through, fill and warm; engine 147 Same, 0 Unstable, 0 Different | `conformance-fast.log` `57a3e16e` |
| `conformance standard` | 159/159 Same in each of pass-through, fill and warm | `conformance-standard.log` `e7074b84` |
| standard engine arm | 159 Same, 0 Unstable, 0 Different | `conformance-standard-engine.log` `bd5ff6fc` |

**Interruption.** The host's 480-minute self-termination fired during the
standard engine arm. Before it did, I set the instance's shutdown behaviour to
`stop`, so the host stopped instead of terminating. I restarted it and restored
`terminate`. The engine arm was then rerun alone, after the three default
phases had completed (`summary.txt` `2cb363c7`).

**Unstable units.** Neither engine arm has any. `guide/traits.rs` and
`summer_school/chapter-6-1.rs` were unstable on the pre-fork engine; they are
now Same.

### Cache-warm difference on `debug_expand.rs`

On the `89709da7` image, an earlier standard run's default cache-warm phase
had one Different, `debug_expand.rs`. That arm runs the stock release. The
diagnostics were the same 14 errors in another module order.

Following ADR 0009, I made 10 more plain runs and 5 more cache-warm runs on
the same host (`dx/orders.txt` `04d46b87`, `dx/run.log` `f0b28268`):

- Plain Verus printed `M4`'s error first in all 19 runs.
- `sail` cache-warm printed the root module's error first in 1 of 7 runs.
- `sail` pass-through and fill printed `M4`'s error first in all 6 runs.

It is a real finding, and the classification was not changed. The suspected
cause, not established, is stock VI-0007 report-order printing under
warm-cache timing. It is captured as a repro in
[corpus/repros/sail-cache-warm-error-order](../../../corpus/repros/sail-cache-warm-error-order/README.md)
and listed in [known issues](../../known-issues.md).

## Repair benchmark

`sail-bench repair` ran three rounds with arm order alternating. M6 was
`26f124f`, M7 was `6214d67` with the pinned runtime. Every sample is valid:
one driver, Current, 0 failing samples. Results are in
`bench/results/repair-acceptance-l12-fork.json` and in
[benchmarks](../../benchmarks.md); the raw log is `bench/run.log` `5d57286b`.

| Catalogue | M6 whole loop, median (min–max) | M7 whole loop, median (min–max) | Speedup | Gate |
|---|---:|---:|---:|---|
| human-eval-verus | 66.0 s (65.8–68.4) | 31.0 s (31.0–31.2) | **2.13×** | pass |
| vest | 253.4 s (252.9–255.8) | 140.4 s (140.2–143.8) | **1.80×** | fail (`SAIL7109`) |

- **Per cycle:** HumanEval cycles take 5.4–6.5 s on M7 against 11.5–13.7 s on
  M6. Vest cycles take 23.8–29.4 s against 42.2–52.9 s.
- **First actionable:** M7 is about 3.7 s on HumanEval (M6 about 6.1 s) and
  about 14.1 s on Vest (M6 about 23.2 s).
- **Vest Currentness:** every Vest M7 sample is now Current. This needed
  L7c's freshness rework and the dropped VI-0005 fix.
