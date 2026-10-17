# Default mode on Sail's Verus (ADR 0043)

The matching `8a373f81` candidate's source-suite, full-tier, determinism,
performance and macOS evidence are in the
[combined-fork record](combined-fork-candidate.md), with its remaining
current-main integration gates. The qualification below is the historical
`73dea021` runtime; it does not qualify the later candidate by itself.

Sail runs its own Verus, the runtime built from the pinned fork commit, for
every verification ([ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)).
Charter rule 1 is read against plain stock Verus with the pinned Z3. This
record holds the evidence that Sail on the fork matches stock, for the
runtime currently pinned.

| Field | Value |
|---|---|
| Status | **Pass** on the checks below, through the standard tier, and on rc.3 the deep tier, release smoke and repair benchmark ([below](#release-candidate-rc3)); a fork-versus-stock `deep-rvt` arm not built |
| Source | Branch `chore/repin-73dea021` (lane L43), 2026-10-15 |
| Runtime | Fork `73dea021` (pinned; adds VI-0017 `9e32601e` and VI-0007 `bc9cda09` to `13a1176c`): Linux `engine-runtime-73dea021-linux` (archive `71337563…af41`, manifest `4f0d0d3f…ccdd`), macOS `engine-runtime-73dea021-macos` (archive `0819e62f…1fb0`, manifest `a42a7d19…b5c8`); version `0.2026.10.15.73dea02` |
| Oracle | Stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0 |
| Environments | Linux: three EC2 c7i.8xlarge (32 vCPU), us-east-1, Ubuntu 24.04, CI image `sail-ci:3c02e445e25df766` via `ci/local.sh`, one suite per host (`l43`: build, gate, solver, e2e, PR subset; `l43b`: standard tier; `l43c`: fast tier and determinism). macOS: developer Mac (Apple M3, macOS 26.5), shared with other work |
| Limits | Correctness only, one run of each; no performance measurement |

## Runtime qualification

Both runtimes were built with `cargo xtask build-verus-engine` from
`73dea021` (Linux inside the CI image) and packed with
`ci/engine-artifact.py pack`; the published bytes were checked anonymously
against the pins. The fork's CI on `73dea021`
([run 38736601487](https://github.com/Chelis-Lang/verus/actions/runs/38736601487))
built Verus, verified vstd and passed the `rust_verify`, `vir` and `air`
library tests and every `sail_vi*` regression test; the full
`rust_verify_test` suite on the same two fixes is in the
[determinism record](determinism.md#fixes).

| Check | Linux | macOS |
|---|---|---|
| Build, vstd verification | 2,060 verified, 0 errors; 234 s (`build.log` `cf650e3d…202b`) | 2,060 verified, 0 errors; 245 s (`mac-build.log` `f1bf5a8a…8294`) |
| `test-engine.py` | 11 of 11 fixtures match stock (`test-engine.log` `0819d89b…098c`) | 11 of 11 (`mac-test-engine.log` `0819d89b…098c`) |
| `test-cancel.py` | no seal after the kill; restart matches stock (`test-cancel.log` `0f6ac145…5988`) | the same (`mac-test-cancel.log` `0f6ac145…5988`) |
| `cargo verus new --lib demo` with the runtime on `PATH` (VI-0018) | writes `vstd = "=0.0.0-2026-10-04-0306"`; `cargo verus verify` 1 verified, 0 errors (`cargo-verus-new.log` `30f7c5a4…3e35`) | the same (`mac-cargo-verus-new.log` `9d0ad17e…bccb`) |

The macOS details are in the [macOS engine record](macos-engine.md).

## Results

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| `ci/local.sh gate` (Linux) | pass: fmt, clippy, 567 tests passed, doctests, rustdoc, links, layering, deny, benchmarks page | `gate2.log` `7d162835…e709` |
| `ci/local.sh solver` (Linux) | pass: 35 tests passed | `solver.log` `50ecc5eb…b1b9` |
| `ci/local.sh e2e` (Linux), all three phases | pass: `sail verify` on the fork byte-identical to stock `cargo verus verify` (exit 101, cold and warm; 20,236 cache entries); MCP agrees with `--output json`; portfolio cold and warm; managed acceptance; Claude Code lists 15 tools; repair CLI/MCP acceptance | `e2e.log` `01e4b52f…7c91b` |
| `ci/local.sh conformance fast pr` (Linux) | pass-through, cache fill and warm: each 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 311 functions; 3,442 cache entries; engine arm 33 same, 32 focused units, engine `4f0d0d3f4315` | `conformance-fast-pr.log` `5029113f…1030` |
| `ci/local.sh conformance fast` (Linux) | pass-through, cache fill and warm: each 147 units, 147 same, 0 unstable, 0 different, 0 infra errors, 3,450 functions, 0 units with rlimit drift; 40,361 cache entries; engine arm 147 same, 146 focused units | `conformance-fast.log` `37fdf40b…209c` |
| `ci/local.sh conformance standard` (Linux) | pass-through, cache fill and warm: each 159 units, 159 same, 0 unstable, 0 different, 0 infra errors, 12,726 functions, 0 units with rlimit drift; 144,933 cache entries; engine arm 159 same, 158 focused units | `conformance-standard.log` `843e0444…fb57` |
| `scripts/verus-determinism.py --runs 3`, fast tier direct inputs (Linux) | 153 of 153 inputs (examples and HumanEval) identical over 3 runs in solver input (373 buckets), blanked stdout, stderr and exit code | `determinism-fast.log` `2d0708f8…2bf6` |
| `ci/run.sh conformance fast pr` (macOS, `SAIL_ENGINE_BUILD` set) | pass-through, fill and warm: 33 same each, 0 unstable, 0 different; 3,442 cache entries; engine arm 33 same, 32 focused units | `mac-conformance-pr.log` `76c3d593…28d3` |
| `sail-conformance run --tier fast` (macOS, pass-through) | 147 units, 147 same, 0 unstable, 0 different, 0 infra errors; 3,450 functions; 1 unit with rlimit drift | `mac-conf-fast.log` `a5b1d2ba…4009` |
| `ci/run.sh host` (macOS) | pass: clippy, 556 tests passed | `mac-host.log` `fd229d2e…6f77` |

Raw logs: `l43-repin-73dea021.tar.gz` (sha256 `d4c98410…06f3`).

The previous pin, fork `13a1176c`, passed the same gate, solver, e2e, PR
subset, standard tier (159 of 159 same in each arm) and macOS checks on
2026-10-15 (lane L41, branch `feat/fork-verus-default`; standard tier log
`conf-standard.log` `5e0331d8…9fdc`).

Attempts:
- `854d86ad`, 2026-10-15, `ci/local.sh gate` (Linux): failed at the link
  check on two anchors this record and the determinism record did not yet
  have; rerun after adding them.

## Release candidate rc.3

`v0.1.0-rc.3` (`8bd4aed2`, lane L45, 2026-10-15) closed three of the gates
below on the `73dea021` runtime; details in the
[release-candidate record](release-candidate.md#v010-rc3):

- **Deep tier:** 159 units, 159 same, 0 Different in pass-through, cache
  fill, warm and engine (sharded by project across four hosts).
- **Release smoke:** both archives, which bundle the `73dea021` runtimes,
  passed `scripts/release-smoke.sh` with no Verus installed, and a fresh
  `ubuntu:24.04` container installed and used Sail from the README alone.
- **Repair benchmark:** one round per catalogue as a regression check:
  HumanEval M7 29.6 s (recorded 30.3 s), Vest 129.4 s (recorded 135.1 s);
  M7 first actionable evidence was no slower. VI-0007's diagnostic hold did
  not slow the repair loop.

## Remaining gates

- A per-test fork-versus-stock `deep-rvt` arm (ADR 0043). Not built: the
  fork's own full `rust_verify_test` pass (4,649 passed, 0 failed, 58
  ignored; [determinism](determinism.md#fixes)) is the evidence for the fork
  commit. `deep-rvt` still compares the stock source build with and without
  `sail-z3`; on rc.3 it reported VI-0008 output-order variants as Different
  (release-candidate finding 11).
- Performance claims in [performance](../performance.md) other than claim 6
  were measured on stock Verus and are not yet re-measured on the fork.
