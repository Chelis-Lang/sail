# Native temporal checking on D.I.V.E. 0.1.0

Status: **not qualified.** The draft branch for
[ADR 0172](../decisions/0172-native-temporal-in-main-workspace.md) builds and
passes its local checks against D.I.V.E.'s main branch (`9337aa00`) through a
temporary `[patch.crates-io]`, except two native regressions that D.I.V.E.'s
engine currently refuses. D.I.V.E. 0.1.0 is not yet on crates.io, so nothing
here qualifies the published version. Native conformance at the published
version, on a native Linux host (ADR 0038), remains required before merge.

## Environment

Apple M3, macOS 26.5, Rust 1.99.0 (Verus stock comparison on 1.99.1),
nextest 0.9.143, three build jobs and three test threads on a shared machine.
Pinned stock Verus `0.2026.10.18.7d82994` with Z3 4.16.0, and Sail's macOS
fork runtime installed by `sail engine install` into a scratch cache.
Linux-only assets (cvc5, the Kani bundle's `renameat2`) are unavailable on
this host. Runs used the step commands of `ci/run.sh` directly on the host,
one step at a time, so one failure did not hide later steps.

## Results

| Check | Result |
|---|---|
| `ci/run.sh gate`: fmt, strict Clippy, rustdoc, doctests, layering, `cargo deny`, benchmarks page | Passed |
| `ci/run.sh gate`: `cargo nextest run --workspace --profile ci` | 1,449 of 1,451 passed, 59 skipped; the two failures are below |
| `ci/run.sh gate`: 37 Python and shell controls | Passed, except four host-environment failures that also fail on unchanged `main` (`test-kani-bundle`, `test-perf-mem`, `test-github-action`) or are caused by the temporary patch and lock (`test-check-acceptance-refinements`, below) |
| `ci/run.sh solver` | 52 of 55 passed; the three cvc5 tests need the Linux-only pinned cvc5 |
| `ci/run.sh e2e`: `legacy`, `output`, `flags`, `loader`, `managed`, `managed-copies`, `managed-native`, `scripts`, `repair` and `extra` | All passed (run one phase at a time) |
| `ci/run.sh conformance fast pr` | Passed: pass-through, cache fill, cache warm and engine arms each 33 units, 33 same, 311 functions, 0 different, 0 infrastructure errors; 7 units with reported rlimit drift; 3,450 cache entries |
| Staged worker beside a debug `sail` | `sail runtime capabilities` reports temporal installed, `backend_revision` `0.1.0` |
| `cargo package -p sail-dive -p sail-solve` | Refused as expected: `dive-api` is not on crates.io; the temporary patch is not carried into packages |

## Known failures

- `sail-dive::native real_safety_temporal_and_stopped_outcomes_are_distinct`
  and `real_temporal_counterexample_has_complete_independent_validation`
  return `Unsupported` for a genuine temporal property. D.I.V.E.'s Core
  engine refuses an exact store for implied temporal roles ("exact or
  spilled native temporal storage under the core engine",
  `crates/dive-api/src/core_engine.rs:1355-1367` at `9337aa00`) and refuses
  state or depth limits with temporal properties
  (`crates/dive-api/src/core_temporal.rs:96-97`). The bounded final route the
  extension used is not implemented on Core (`crates/dive-api/src/check.rs:851-860`).
  A probe with a fingerprint store and no limits produced the expected
  validated counterexample; Sail keeps its exact, bounded request and these
  regressions unchanged.
- `scripts/test-check-acceptance-refinements.py` binds the reviewed
  `Cargo.lock` digest. It refuses the temporary `[patch]` and, without it,
  the changed lock ("explicit source review required"). The lock is final
  only after D.I.V.E. publishes; review it then and rerun the refinement
  tier.

These are macOS host results on the pre-publication source. They are
not the native Linux qualification ADR 0038 requires.

Raw logs remain outside the repository:

| Log | SHA-256 |
|---|---|
| gate steps | `c0e60152ebdcc7c51bd5d4afaf5a6f4e27b86f45bd1a4a4b62d4744377c20b1c` |
| solver | `a6804528ab8cc454f49d6ebf5d9b2c8cd3a0cd44e554d1ccb59f01b8f535700e` |
| e2e `legacy` | `a434ba79349c835ea33daadbbf356e784858fe41b47b4f6f65fa84208f840a39` |
| e2e `output` | `d045587ed65ed05aee8562e3b6053be82c6b62f084bc9983c1684a905f11701c` |
| e2e `managed` | `96dfa1c3016385b0f44d1447b3966a8006d8ed16109376787f4a2916d59e0038` |
| e2e `extra` | `c8aa3cffa455ffb6dc304e135ae53ed7424614a58a9890b9136cf5cb6b1fab51` |
| conformance fast pr | `f77747acad330d4c25cbf5801ef7ab39243586e9a78b42e3e2b84a1fe1cb607f` |

## Remaining before merge

1. Publish D.I.V.E. 0.1.0; drop the temporary patch commit, run
   `cargo update -p dive-api -p dive-ir` and confirm the `pins` test sees the
   crates.io source.
2. Review the new `Cargo.lock` and update its digest in
   `scripts/check-acceptance-refinements.py`; run `ci/local.sh refinements`.
3. Regenerate `THIRD-PARTY-NOTICES`.
4. Resolve the two temporal regressions (D.I.V.E. support, or an owner policy).
5. Run the native Linux gate, solver, e2e with `extra` and `conformance fast
   pr`, and `scripts/publish-crates.py --dry-run`, at the final source.

## Attempts

- e2e, first run: refused before any comparison because Rust 1.99.1 with
  `rust-src` was not installed; installed and rerun.
- e2e, second run: `legacy` reached the portfolio step without
  `SAIL_TEST_Z3`; set it to the pinned Z3 and rerun.
