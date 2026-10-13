# Rescue L7a: engine preparation cost

Status: engine patch and Sail changes done; candidate runtime measured natively.
The artifact is **not** re-pinned. Lane L12 folds the final patch into the
private Verus fork and does the single combined rebuild and re-pin.

## Summary

- **Removed.** Prepared-lowering reuse (within a batch and persistent) and
  the same-input cached-versus-fresh oracle are gone from the engine and from
  Sail. The patch shrinks from 11,306 to 701 lines and touches four files.
- **Second cost found and fixed.** Progress observation was quadratic in the
  crate's function count, and it serialized workers on the global friendly-name
  lock. On Vest this cost as much as preparation had.
- **Result.** The engine's Verus phases now match M6 full plus the focus
  phase on both catalogues:
  - HumanEval M7/M6 went from 0.91× to 1.70× (one round, candidate);
  - Vest went from refused or 0.80× to 1.54× (one round, candidate).
  - Vest M7 samples still fail Currentness (`SAIL7102`), a separate product
    bug.

## Environment and identities

- **Host.** AWS EC2 `c7i.4xlarge`, us-east-1, Linux 7.0.0-1014-aws x86_64. The
  lane was `l7a` (`scripts/perf-host.sh`), and it is down.
- **Container.** CI image `sail-ci:8e24fb5f7f490c01`. Candidate engines were
  built natively in it with `cargo xtask build-verus-source
  --interactive-engine` (160–215 s each) and supplied as `SAIL_ENGINE_BUILD`.
- **Toolchain.** Verus `426d8b0`, Z3 4.16.0 and Rust 1.98.1.
- **M6 arms.**
  - `0fde777`, for the first measurements (L1's arm);
  - `rescue/m6-baseline` `26f124f`, for the candidate measurements. It fixes
    the `SAIL3107` focus defect that understated M6.
- **Final patch.**
  - `engine/verus/interactive.patch` sha256 `c39f2203…21aa`.
  - The stock source lock (`engine/verus/Cargo.lock`) sha256 `80c71fdd…395e`.
  - Candidate variant `5d281662a13d2227`, manifest sha256 `20b64286…`.
- **Branch.** `worktree-agent-a0bd577c6f32bd235`, rebased on `rescue/m7`
  `cfc2081`.

## What changed

| Commit | Change |
|---|---|
| `6e734bc` perf | Engine patch rebuilt on stock. It keeps the focused phase, bounded observation, compiler seals and late fatal-error handling, and drops the preparation codec, cache, span provenance, graph ledger, import capture, namespace broker and oracle (and their crate changes in `vir`, `air` and `Cargo.lock`). The focused report keeps only the selected function (L7b's `0952f1e`, re-applied by hand because the patch was rewritten). Preparation-only scripts (`test-prepared.py`, `test-real-he.py`, `qualify/`) are removed. |
| `1d92f32` refactor | Sail no longer names `SAIL_ENGINE_PREPARED_DIR`. It drops the following, none of which reached `main`: the persistent opt-in, fault invalidation of prepared entries, `batch.preparation` (`PreparationStats`) and `engine.persistent_preparation` (repair, verify-output and project-result contracts). The conformance arm loses the oracle and `--no-oracle`, `--oracle-fault` and `--oracle-fresh-arms`. It keeps the plain-Verus comparison of verdicts and diagnostics and the completion-seal check. `ci/run.sh` drops `--oracle-fresh-arms 4`. The provenance control drops its persistent and corrupted-entry cases. |
| `0583b18` fix | Retires the repair gate's `SAIL7105` (it required enabled preparation telemetry), in a separate commit for the lead to review. |
| `edb9f8e` docs | Updates the engine README, repair guide, walkthrough, output schema, divergences, trust model, workspace verification and ADR 0038 (removal note replacing the persistent-reuse condition and the oracle amendment), marks open question 15 moot, and repoints two links. |
| `60cbec7` perf | Builds the engine's friendly-name uniqueness table once per crate, bounded at 64 MiB. `unique()` used to rebuild every function's name for each obligation and each observed function, taking the global `PATH_AS_RUST_NAME_MAP` lock each time. |

Nothing in the removal looked unsafe:
- the full phase now runs stock lowering;
- removed code was reachable only through preparation or oracle paths;
- the only gate that depended on it is `SAIL7105`.

## Timings

Whole-loop totals come from `sail-bench repair`, one sample per cycle per arm.
The first two rows are medians over three rounds; the candidate rows are one
round.

| Run | Engine | M6 arm | HumanEval M6 / M7 | M7/M6 | Vest M6 / M7 | M7/M6 |
|---|---|---|---|---|---|---|
| Before (`5fb311f`) | pinned `c77e1a18` (preparation on) | `0fde777` | 66.8 / 73.5 s | 0.91× | – | – |
| Preparation off by default (superseded) | `168d1abe` | `0fde777` | 67.0 / 43.2 s | 1.55× | – | – |
| Removal only (`edb9f8e`) | `7bb4dce1` | `26f124f` | 78.1 / 46.1 s | 1.69× | 268.7 / 334.4 s | 0.80× |
| **Removal + name table (`60cbec7`)** | `5d281662` | `26f124f` | **78.6 / 46.1 s** | **1.70×** | **271.5 / 175.8 s** | **1.54×** |

- **First actionable.** M7 holds at about 5.0 s on HumanEval and 15.8 s on
  Vest; M6 gets 6–7 s and 18–24 s.
- **Vest focus.** Every Vest M7 sample now establishes focus (no `SAIL3122`).
- **Vest Currentness.** Every Vest M7 sample is `SAIL7102` (full evidence not
  Current), and the four cycles expecting success are also `SAIL7106`. This is
  L1's anomaly 4 (`SAIL3114`): M7 re-verifies vstd in each batch.
- **Gate.** No result meets the 2× gate or the three-round minimum. The
  candidate rows are one round, as requested.

The ADR 0027 gate is not met. With the 3-round HumanEval data, round 0 of M7
runs about 1.2 s per cycle slower than later rounds. At L1's later-round M7
level (8.85 s per cycle), HumanEval would be about 44 s against 78 s, or 1.78×.

### Per-phase effect (L1's method, Verus `--time`, selected crate)

Values are medians in seconds over the measured M7 runs. Thread-s is time
summed over Verus's 15 threads.

| | HE before | HE removal + table | HE M6 full | Vest removal only | Vest removal + table | Vest M6 full |
|---|---|---|---|---|---|---|
| Verus total | 11.27 | 4.97 | 4.63 | 58.43 | 26.86 | 24.72 |
| rustc frontend | 1.71 | 1.67 | 1.68 | 7.30 | 7.43 | 7.32 |
| VIR construction | 1.27 | 1.24 | 1.21 | 6.04 | 5.92 | 5.89 |
| Post-VIR wall | **8.29** | **2.09** | 1.73 | **45.02** | **13.74** | 11.66 |
| SST/poly + other, thread-s | **96.1** | **6.1** | 6.1 | **527** | **64.5** | 62.3 |
| AIR, thread-s | 3.0 | 3.6 | 3.6 | 17.3 | 18.5 | 18.2 |
| Median module verify | 0.92 | 0.10 | 0.10 | 1.72 | 0.34 | 0.34 |

- **Preparation-off build.** HumanEval post-VIR fell from 8.29 to 2.25 s with
  preparation simply off.
- **Name table.** It adds nothing measurable on HumanEval (452 functions), and
  removes 31 s of post-VIR time on Vest.
- **What remains.** M7's post-VIR now exceeds M6 full by the focus phase
  (about 0.4 s on HumanEval and 2 s on Vest).
- **Vest without Currentness.** Most of the remaining Vest gap is Sail work
  outside Verus: the re-verification of vstd caused by the Currentness bug.

## Gates and conformance (candidate `5d281662`, the run in this tree)

All steps used `ci/run.sh` steps in the CI image. The candidate engine was
supplied as `SAIL_ENGINE_BUILD`, because the committed pin is now stale and
the image build refuses it.

- **`gate`:** pass.
- **`solver`:** pass. It ran on `7bb4dce1`, with the same Sail code; solver
  tests do not use the engine.
- **`e2e`:** pass, all phases. Repair includes the updated provenance control.
- **`conformance fast pr`:**
  - pass-through, cache fill and cache warm: 33/33 same;
  - engine arm: 33/33 same, 0 different, 32 focused units. No oracle remains,
    so there are no hard or unstable oracle counts.
- **Source checks:**
  - `test-engine.py` and `test-cancel.py` pass;
  - library tests pass: `rust_verify` 12/12, `vir` 1/1 and `air` 144/144.
  The previous `driver-tests` stage fails in parallel on the old patch as
  well; that suite was preparation tests and is now gone.
- **Workspace tests.** `cargo nextest` for `sail-api`, `sail-conformance`,
  `sail-verus`, `sail-mcp`, `sail-cli` and `sail-bench` passes on macOS.

Not run: the complete fast and standard conformance tiers on the candidate,
including `basic_failure.rs` for the L4b vstd path; and Vest beyond one round.

## Superseded work, kept for reference

- **Preparation opt-in.** Before the scope changed, preparation was made
  opt-in (`SAIL_ENGINE_PREPARATION=1`).
- **Published runtime.** A runtime with L4b was published as the private
  prerelease `engine-runtime-ac70a44d-linux` (asset `634369607`, archive
  sha256 `3972894116ec…`). It passed `ci/local.sh` gate, solver, e2e and
  conformance fast pr, with the engine arm at 0 hard and 1 unstable, and
  12/13 qualification stages; `driver-tests` failed as above.
- **Not merged.** None of this is on the branch. The local branch
  `l7a-optin-archive` keeps those commits.
- **Delete the release?** The prerelease is unused; whether to delete it is
  the lead's call.

## Open items

- L12: combined rebuild and re-pin of `artifact-linux.json`, plus a new
  `qualification-linux.json`. That record still describes `aaa3b2e4`.
- Vest Currentness (`SAIL3114` / `SAIL7102`): the next large Vest cost.
- Three-round HumanEval and Vest acceptance runs on the pinned runtime.
- Sail overhead outside Verus (L1 lever 3) remains about 3 s per HumanEval
  batch.

## Doc edits for the lead (files this lane did not own)

- **CHANGELOG.** Suggested entry: "Interactive engine: removed prepared-lowering
  reuse and the same-input oracle. Repairs no longer pay for preparation, and
  progress observation no longer scans the whole crate per obligation; the
  candidate measured 1.70× M6 on HumanEval and 1.54× on Vest (one round). Removed
  `batch.preparation`, `engine.persistent_preparation`,
  `SAIL_ENGINE_PERSISTENT_PREPARATION` and the conformance arm's `--oracle*`
  options; retired `SAIL7105`."
- **`docs/known-issues.md`.** Remove the byte-oracle entry, the 512 MiB
  preparation-budget entries, "Imported metadata ranges are uncovered" and
  "Thread-local span formatting in interactive preparation" (#75). Only a
  link anchor was changed here.
- **`docs/ROADMAP.md`.**
  - Line 101 mentions batch-local preparation reuse.
  - Gate rows should name the removal.
- **`docs/validation/m7.md`.** The qualification row describes the removed
  pipeline; only the link was repointed.
- **Open question 14** (oracle and unstable declaration order) is also moot.
- **`docs/CHARTER.md`** line 201 mentions the amended oracle (owner document).
- **VI-0012** cites `engine/verus/qualify/test-private-span-cross-edit.py`,
  which is removed (still in git history).

## Raw evidence

`~/sail-raw-evidence/l7a/` on the lead's machine. `SHA256SUMS` covers every
file.

| File | sha256 |
|---|---|
| `l7a-bench-final.tgz`: pinned and candidate bench results, raw sidecars, reports, Verus `--time` JSON and engine events | `8dccdb0e06f839f4ac4a344b845190f7caf6fab7c884c632ed777b38ba6ca555` |
| `l7a-bench.tgz`: before and preparation-off three-round runs | `14bca534e7950041013eb841b12d6a5baa905fa62aea3012c389a6c4ce6bdca8` |
| `l7a-logs-final.tgz`: host scripts, gate, e2e and conformance logs, phase JSON and pipeline logs | `71f621cb603f1ee9c2470e8cc2cb5e9c0d5926043302ad0d537b03e036aa354f` |
| `l7a-source-checks.tgz`: test-engine, test-cancel and library tests on the candidate | `52245dc975f3674d4d721bb8cbedfa0c2f6d19e57ff54a1196942750dd1d004c` |
| `l7a-qualify13.tgz`: 13-stage run on the superseded `ac70a44d` runtime | `9f5e8e64cda29c71c61738b3009dc9916bbf696478d7495df653c4f780283123` |
| `release/engine-runtime.tar.gz`: superseded published archive | `3972894116ec4a4f6ac9ba38c31bf546c890f166fd1e2ad199616a0a2537eba3` |
