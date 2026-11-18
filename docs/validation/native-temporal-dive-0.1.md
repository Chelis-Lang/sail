# Native temporal checking on D.I.V.E. 0.1.0

Status: **not qualified.** The draft branch for
[ADR 0172](../decisions/0172-native-temporal-in-main-workspace.md) builds and
passes its local checks against D.I.V.E.'s main branch (`3d5923a1`; the same tree as `0ddf33e9` before D.I.V.E.'s author-history rewrite) through a
temporary `[patch.crates-io]`, including both native temporal regressions with
Sail's exact store and state/depth limits unchanged. D.I.V.E. 0.1.0 is not yet
on crates.io, so nothing
here qualifies the published version. Native conformance at the published
version, on a native Linux host (ADR 0038), remains required before merge.

## Environment

Apple M3, macOS 26.5, Rust 1.99.0 (Verus stock comparison on 1.99.1),
nextest 0.9.143, four build jobs and four test threads on a shared machine.
Pinned stock Verus `0.2026.10.18.7d82994` with Z3 4.16.0, and Sail's macOS
fork runtime installed by `sail engine install` into a scratch cache.
Linux-only assets (cvc5, the Kani bundle's `renameat2`) are unavailable on
this host. Runs used the step commands of `ci/run.sh` directly on the host,
one step at a time, so one failure did not hide later steps.

## Results

| Check | Result |
|---|---|
| `ci/run.sh gate`: fmt, strict Clippy, rustdoc, doctests, layering, `cargo deny`, benchmarks page | Passed |
| `ci/run.sh gate`: `cargo nextest run --workspace --profile ci` | 1,452 of 1,452 passed, 59 skipped |
| `ci/run.sh gate`: 37 Python and shell controls | Passed, except four: three host-environment failures that also fail on unchanged `main` (`test-kani-bundle`, `test-perf-mem`, `test-github-action`: Linux-only fixture pin and `renameat2`), and `test-check-acceptance-refinements`, which refuses the temporary patch (below) |
| `ci/run.sh solver` | 52 of 55 passed; the three cvc5 tests need the Linux-only pinned cvc5 |
| `ci/run.sh e2e` phases and `extra` | All passed: `legacy`, `output`, `flags`, `loader`, `managed`, `managed-copies`, `managed-native`, `scripts`, `repair` and `extra`, run one phase at a time |
| `ci/run.sh conformance fast pr` | Passed: pass-through, cache fill, cache warm and engine arms each 33 units, 33 same, 311 functions, 0 different, 0 infrastructure errors; 7 units with reported rlimit drift; 3,450 cache entries |

## Temporal behaviour

D.I.V.E. main checks genuine temporal properties on Core with an exact store
and honours state and depth limits. Sail's request is unchanged (exact
store, seed and fingerprint zero, one worker, the job's limits):

- `real_safety_temporal_and_stopped_outcomes_are_distinct`: `[]<>x` under
  weak fairness returns `Holds` with temporal completion and a completed
  capture.
- `real_temporal_counterexample_has_complete_independent_validation`: `[]<>x`
  without progress returns `Refuted` with an independently validated trace
  naming `P`.
- `temporal_state_and_depth_bounds_below_the_model_stop_without_a_claim`:
  `max_states = 1` or `max_depth = 0` on a three-state temporal model returns
  `Stopped` (D.I.V.E. verdict `stopped`, phase `graph-capture`, resource
  `states` or `depth`), no completion and no traces; the same model under the
  ordinary limits holds.

## Known failures

- `scripts/test-check-acceptance-refinements.py` binds the reviewed
  `Cargo.lock` digest and refuses workspace dependency overrides. It refuses
  the temporary `[patch]` ("workspace dependency overrides require review")
  and, without it, the changed lock ("explicit source review required"). The
  lock is final only after D.I.V.E. publishes; review it then and rerun the
  refinement tier.
- CI cannot fetch the private D.I.V.E. repository that the temporary patch
  names ("failed to authenticate when downloading repository"), so every CI
  job that resolves the workspace fails until D.I.V.E. 0.1.0 is on crates.io.

These are macOS host results on the pre-publication source. They are
not the native Linux qualification ADR 0038 requires.

Raw logs remain outside the repository:

| Log | SHA-256 |
|---|---|
| gate steps | `91759a7ec430a349c338342c6297f3bd8a1d8593f7caf5cb3421058663435b28` |
| solver | `a4353a2d2c5d00fea532c47d2efebc90e9dd89a22e8a66b8acdd75e63de901d4` |
| e2e `legacy` | `a540e3fae940812cae7895605f73966bbb294c6f24c58aec950b2fbdbb52e6f9` |
| e2e `output` | `81e84db9c9854b2c7173f646053231c8a4abfef104e8a0307511d35f19eefdaa` |
| e2e `managed` | `3012a3f80c8bbc34035ffef04cd82ec2ae0f39f78a9356e6c69f3063fb0c8987` |
| e2e `extra` | `c8aa3cffa455ffb6dc304e135ae53ed7424614a58a9890b9136cf5cb6b1fab51` |
| conformance fast pr | `2a9050fa97d428653f2419d006f72d39c93d66433291b3c884cd3435b8395383` |
| gate steps, prepared 0.3.0 source (same four known failures; 1,452 of 1,452 tests) | `1c4c0a8b0ebf800f93215ecf8353f3109eda4a85577f13e66886eb302b676bec` |
| `publish-crates.py --dry-run --version 0.3.0` | `7beb51b39730075cea6e92a9055b41f335de7c8d5851d888eff21e2ae5ad284b` |

## Remaining before merge

The branch is prepared as the 0.3.0 release source (workspace, internal
requirements, lock entries and Action default at 0.3.0; changelog and release
notes written). Only the last commit, the temporary `[patch.crates-io]`, is
unmergeable. `scripts/publish-crates.py --dry-run --version 0.3.0` at the
prepared source packages ten of the sixteen selected crates and then stops at
`sail-dive`: "no matching package named `dive-api` found; location searched:
crates.io index". Release day:

1. Confirm all seventeen `dive-*` crates are on crates.io at 0.1.0 from the
   tested D.I.V.E. tree (`3d5923a1`), or rerun the native temporal tests.
2. Drop the patch commit (`git reset --hard HEAD~1`). The lock then names no
   D.I.V.E. crate, so `cargo update -p dive-api -p dive-ir` reports no match;
   run `cargo metadata --format-version 1 >/dev/null` instead. The lock diff
   must add only the `dive-*` crates and their new dependencies, all
   `registry+` with checksums. Run `cargo deny --locked check` and the
   `sail-dive` `pins` test.
3. Review the lock and set its SHA-256 in `PARSER_NATIVE_SHA256["Cargo.lock"]`
   in `scripts/check-acceptance-refinements.py`; run
   `scripts/test-check-acceptance-refinements.py` and `ci/local.sh refinements`.
4. Regenerate `THIRD-PARTY-NOTICES` (`scripts/third-party-notices.py --work
   NEW_DIR`). A preview from the patched lock adds the seventeen MIT `dive-*`
   crates (`dive-sema` MIT AND BSD-2-Clause) and their dependencies; it cannot
   be committed earlier because it records the Git source.
5. `scripts/publish-crates.py --dry-run --version 0.3.0` must verify all
   sixteen crates.
6. Run the native Linux gate, solver, e2e with `extra` and `conformance fast
   pr`, and `ci/run.sh host` on macOS, at the final source; update this record.
7. Set the release date in `CHANGELOG.md`, push, require green CI and merge.
   Then follow [Releasing](../releasing.md#procedure) from clean `main`:
   `scripts/release.sh 0.3.0`, archive smoke tests, attestation, owner
   approval, publication; `publish-crates.yml` uploads the crates.

## Attempts

- e2e, first run: refused before any comparison because Rust 1.99.1 with
  `rust-src` was not installed; installed and rerun.
- e2e, second run: `legacy` reached the portfolio step without
  `SAIL_TEST_Z3`; set it to the pinned Z3 and rerun.
- `9337aa00`, 2026-11-18: both native temporal regressions returned
  `Unsupported`; Core then refused an exact store and state/depth limits for
  genuine temporal properties. Resolved by D.I.V.E. `3d5923a1`.
