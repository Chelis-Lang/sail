# Deep-tier soundness validation

This page records the deep tier (AGENTS.md, Testing tiers) run before the M7
rescue is called a release: the corpus deep tier, Verus's own
`rust_verify_test` suite ([ADR 0019](../decisions/0019-rust-verify-test-conformance.md))
and the optional Singular projects
([ADR 0021](../decisions/0021-optional-singular-source-validation.md)). Each
compares Sail with plain Verus under
[ADR 0042](../decisions/0042-diagnostics-compared-as-a-set.md)'s comparison,
and plain-Verus variation is classified as Unstable
([ADR 0009](../decisions/0009-solver-nondeterminism-and-unstable-results.md)).

| Field | Value |
|---|---|
| Status | **Pass** for default mode (pass-through, cache fill, cache warm) in every suite, and for the engine arm on the corpus. The engine lacks Verus's `singular` feature, so the engine arm counts the three Singular units as `engine-lacks-feature` (rescue L26, below), never Same or Different ([repro](../../corpus/repros/engine-without-singular/)) |
| Source | `rescue/m7` `98f6292`, 2026-10-13 (rescue lane L25) |
| Runtime | CI image `sail-ci:bc8748852984b813` built on each host from the committed inputs; stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0; engine runtime `engine-runtime-13a1176c-linux` (manifest sha256 `5b866414…7603`); `rust_verify_test` from the source build of the pinned commit `426d8b0`; Singular image adds Singular `1:4.3.2-p10+ds-1.1build1` and a Singular source build of `426d8b0` |
| Environment | Three native EC2 c7i.8xlarge hosts (32 vCPU), us-east-1, Ubuntu 24.04, kernel 7.0.0-1014-aws, one suite per host, no other load |
| Scope | `ci/local.sh conformance deep` (fast, standard and deep tiers; the two Singular projects are skipped there because the image has no Singular); `ci/local.sh deep-rvt cache` (all 4,695 tests, Verus's default threads, 10 confirming runs); `ci/local-singular.sh` (the two Singular projects, three units) |
| Limits | Correctness only; wall times are not performance evidence. Each Singular step is one run on one host |

## Summary lines

**Corpus deep tier** (`conformance-deep.log`, sha256
`c62868a77783a1b4c300f972ebf6736b41c61caff22b6e13d8994c644d0aca85`; reports
`conformance-deep-reports.tgz`, `4eca6230…1bea`):

| Arm | Units | Same | Unstable | Different | Infra | Functions |
|---|---:|---:|---:|---:|---:|---:|
| Pass-through | 159 | 159 | 0 | 0 | 0 | 12,726 |
| Cache fill | 159 | 159 | 0 | 0 | 0 | 12,726 |
| Cache warm (151,157 entries) | 159 | 159 | 0 | 0 | 0 | 12,726 |
| Engine (158 focused units) | 159 | 159 | 0 | 0 | 0 | 12,726 |

One unit had rlimit drift without a difference in each default arm, the same
as L22: VNR `log_entry_alive_value_wrap_around`, 51,945 against 51,948.

**`rust_verify_test`** (`deep-rvt-cache.log`, sha256
`467709e943d03113d132469f4960701b0c6ab44b6966f1e3d02e2b4887197e4e`; reports
`rvt-reports.tgz`, `b6c114e5…d81a`). 4,695 tests, 58 ignored; plain outcomes
4,635 ok and 2 FAILED in every mode, the two source-build
`cargo::cargo_new_*` tests (ADR 0019):

| Mode | Same | Unstable | Different | Infra | Wall s |
|---|---:|---:|---:|---:|---:|
| Pass-through | 4,691 | 4 | 0 | 0 | 1,065 |
| Cache fill | 4,690 | 5 | 0 | 0 | 1,051 |
| Cache warm (201,288 entries) | 4,692 | 3 | 0 | 0 | 1,022 |

**Singular projects** (`singular.log`, sha256
`99e572aeef719c89b41ce2986fa7477d9de3af955bcab6e48a3151715f0c5819`; reports
`singular-reports.tgz`, `f23cd982…89fd`). The step exited 1:

| Arm | Units | Same | Unstable | Different | Infra | Functions |
|---|---:|---:|---:|---:|---:|---:|
| Pass-through | 3 | 3 | 0 | 0 | 0 | 606 |
| Cache fill | 3 | 3 | 0 | 0 | 0 | 606 |
| Cache warm (8,080 entries) | 3 | 3 | 0 | 0 | 0 | 606 |
| Engine | 3 | 0 | 0 | 3 | 0 | 606 |

**Singular projects after the feature fallback** (rescue L26, branch
`fix/engine-feature-fallback` `8b40111`, tested as `56415d5` with identical code before rebasing onto #85, 2026-10-13; CI image
`sail-ci:a03f58a9b243edf0`, same engine runtime; one c7i.8xlarge, us-east-1,
kernel 7.0.0-1014-aws; `singular.log`, sha256
`b9764e1e6d85fb24b58aaf35e12f2eaa89d751b4fb17911c580f4e9d7c6b86bb`). The
engine arm checks each project's `verus-feature:*` requirements against the
engine manifest's features (none) and does not run the engine on those
units. The step exited 0:

| Arm | Units | Same | Unstable | Different | Infra | Engine lacks feature | Functions |
|---|---:|---:|---:|---:|---:|---:|---:|
| Pass-through | 3 | 3 | 0 | 0 | 0 | 0 | 606 |
| Cache fill | 3 | 3 | 0 | 0 | 0 | 0 | 606 |
| Cache warm (7,519 entries) | 3 | 3 | 0 | 0 | 0 | 0 | 606 |
| Engine | 3 | 0 | 0 | 0 | 0 | 3 (`singular`) | 0 |

## Every Unstable and Different

- **Unstable, `rust_verify_test`: `output_json` tests only.** Pass-through:
  `test_json_custom_err_on_ensures`, `test_json_proof_note_on_assume_and_req_with_no_cheating`,
  `test_json_proof_note_on_ensures`, `test_json_proof_note_on_requires`. Fill:
  those four plus `test_json_custom_err_on_requires`. Warm:
  `test_json_custom_err_on_requires`, `test_json_proof_note_on_assume_and_req_with_no_cheating`,
  `test_json_proof_note_on_requires`. Cause: plain Verus prints
  `--output-json`'s `func-details` in hash order
  ([VI-0008](../verus-issues/VI-0008-func-details-hash-order.md)). In each
  case the plain runs disagreed among themselves (2 to 11 plain runs) and
  Sail's complete output matched one of them; the only differing lines are
  the order of `func-details` keys (`test_crate::example` against
  `test_crate::caller`) and their labels. Both sides `ok`.
- **Different, Singular engine arm (L25 run): all three units**
  (`integer_ring.rs`, `integer_ring_bound_check.rs`,
  `verified-memory-allocator`). Plain (Singular source build) exits 0; the
  engine exits 1 with ``error: Please cargo build with `--features singular`
  to use integer_ring attribute`` and no function results. Cause: the engine
  runtime, like the stock release, is built without Verus's `singular`
  feature, and the engine arm checks the project's `verus-feature:singular`
  requirement only against the plain source build. The engine fails safe
  (an error, never a false verified), but the verdicts differ. Minimal
  reproduction: [`engine-without-singular`](../../corpus/repros/engine-without-singular/)
  (`repro-integer-ring.log`, sha256 `8ed6cddf…60b1`). Resolved by
  decision in L26: such targets fall back to stock in the product
  (`SAIL3124`), and the arm reports them as `engine-lacks-feature`
  ([known issues](../known-issues.md#resolved-by-decision)).

No other unit or test was Unstable or Different, and there were no
infrastructure errors.

Raw logs and reports are on the lead's machine under
`~/sail-raw-evidence/l25/` and, for the L26 Singular run,
`~/sail-raw-evidence/l26/`.

## Failed or interrupted attempts

None: each suite ran once to completion. The L25 Singular step's exit 1 is
the engine-arm result above.
