# Deep-tier soundness validation

## Fork source-suite gate (#124)

**Implemented and selected; the complete cc7 fork source suite passes.**
Current main `2936ead2` selects fork `cc7bbce4` through merged PR #221. The
harness runs an exact fork-with-and-without-shim arm and retains the separate
stock-with-and-without-shim control using ADR 0042's complete diagnostic-message
multiset comparison. Stock stdout, message bytes/multiplicities, outcomes and
exit codes remain exact. Fork stderr order stays exact, and every nonignored
fork libtest must pass; two matching failures cannot qualify it. These are
runtime-local arms. Stock-versus-fork equivalence remains the mandatory corpus
gate, not a relocated cross-runtime RVT comparison.

The unfiltered native cc7 run on Sail harness `d422adf0` reports **4,709 Same,
58 ignored, all 4,651 nonignored plain tests passed**, zero Different, Unstable,
infrastructure errors, failed or unknown plain tests, in 1,293.36 seconds.
All nine `output_json` tests passed on both arms. The six relevant RVT runner,
source-build, CLI, builder and CI-dispatch files are byte-identical on current
main; this is source-scoped qualification, not a rerun at the composed head.
The complete original 154 executables and source/runtime inventories remain
bound to that run. Full-suite archive SHA-256:
`4e5698ce37b3d4c3b61aa9c91ad052635c1f152c55b9f115f039cd04dc7519dd`;
report SHA-256:
`36978d5069b7ce82ef68a7372b7c7141e8ac74183b3bae0691a446178399e6cf`;
final archive SHA-256:
`aa558a7df2a0e0f248adc545b2f8ebb65b22c893a48221d7d459eafa0b3c2755`.

The separate historical stock control below remains failed; neither it nor a
cached RVT round is relabeled as passing. The source-suite gate alone does not
qualify installed packaging. Normal Linux/macOS runtime and corpus acceptance
are recorded separately in the [selected-runtime record](closure-full-identity.md).

### Earlier source-suite attempts

On macOS arm64, the first fork-test build from `73dea021` completed with 153
test binaries. A 244-test selection (`modes::` and `output_json::`, 3 ignored)
had 241 Same, 3 Different, 0 Unstable and 0 infrastructure errors. All nonignored
libtests passed. The three differences are the actual JSON proof-note order
defect [VI-0021](../verus-issues/VI-0021-proof-note-json-order.md), with raw
reports/logs archived at SHA-256
`475aa777b9e0ebd98f0ab1b3498d453d3803091aeef54c1d8f2927d43e103444`.
That failed sample is not whole-suite acceptance. The corrected combined runtime later passed its full native source suite,
as recorded below. The later selected-runtime result above supersedes the
then-pending fork acceptance.

Native source `84ca8c18305628fa18ef9e39ac7f6b9299c3c917` passed the workspace
gate (636 tests, 35 skipped), solver tests (35), all seven end-to-end phases
plus extra, PR conformance, full fast conformance and full standard conformance.
Each of four arms compared 147 fast units (3,450 functions) and 159 standard
units (12,726 functions): all Same, with zero Different, Unstable,
infrastructure errors or skipped projects. The standard engine arm recorded
one unit with rlimit drift; the other standard arms and all fast arms recorded
none. These corpus runs use the existing pinned fork runtime, not the proposed
combined runtime or the new source-suite build.

The machine was a native c7i.4xlarge in us-east-1, Ubuntu 24.04, with
16 CPUs and a 26 GiB container memory bound. Fourteen source manifests agree
on all 508 tracked file contents, independently checked against the source
commit (including the `CLAUDE.md` symlink to `AGENTS.md`). Full compressed raw
evidence SHA-256:
`eb24d0c9e2b74ec0f37bf32244709f71cd5095a21ff6c46b3a5bc7bcb936d0e6`.
Independent source/report verification SHA-256:
`b59f8e8e0ae67fd69ee71c52c83072af10c6c50a0ca799a583019e470e36f673`.

The subsequent `9fa6c9d2` candidate corrects the fork suite's identity to
describe its actual Vargo-built vstd layout, rather than the installed engine's
release layout. Its complete fork/stock source-suite outcome was not retrieved
before the host's shutdown deadline. It remains unqualified; the corpus result
above does not stand in for that source-suite gate. The subsequent main-based
rebase retained the implementation. The stock control and corrected fork
candidate retained separate qualification scopes.

The explicit fork selector now bypasses only the generic stock-source
installation lookup: its pinned solver comes from the release, and the suite's
existing manifest, artifact, feature, toolchain and solver checks remain in
force. Ordinary stock and Singular source installation retain their existing
selector behavior. On private source `168ab7ab`, all 18 actual-binary list-only selector controls
and all nine genuine fork `output_json` tests passed. Its complete fork suite
then failed strictly on a runner-supplied duplicate progress flag: 4,708 Same,
one Different, no Unstable or infrastructure errors (4,709 selected, 58 ignored).
The original test and baseline-must-pass rule remain intact. The
[source-bound option correction](rvt-progress-option.md) passed the original
VI-0012 regression, BASIC and the full genuine fork suite on private
`af339f8a`: 4,709 Same (58 ignored), all 4,651 nonignored plain tests passed,
zero Different, Unstable or infrastructure errors in 1,225.58 s. Raw SHA-256
`b2304b6dd70355fd69a0ad6241c3f2a00c25156e8443ffb357b80005ce165faf`.
The main-based harness join `2bd59014` separately passed its full native
workspace gate and controls in the main `c89e29d3` image `08bd13d8` on a
c7i.4xlarge/us-east-1; raw archive SHA-256
`9d417731ece59058f380072fb9171fb54c8120c2c76cd646c47cc5689fbc65cf`.
This is workspace qualification only, not a fork source-suite run or acceptance
of the combined runtime at that earlier source. The later selected cc7
qualification is stated above; these earlier results are not relabeled.

Failed/interrupted attempts, 2026-10-16:

- `af339f8a`: replacement remaining-gates controller fetched the complete fast
  corpus, then its subprocess proxy lacked `DEVNULL`. The helper failed before
  any verifier/SMT execution; no determinism/standard/deep result follows.
  Final raw SHA-256
  `5f7d7fdfb1d21f0dcd4dc615b58524a5d72ec1d113b3107bb8dc2636c6aea37f`.

- `af339f8a`: subsequent determinism preparation failed before any SMT
  execution because the fresh host had not fetched the complete fast corpus.
  No determinism/standard/deep result follows. Final raw SHA-256
  `eec84add58f8e7bc103ec3c5e940019fa976d5cf39623415ba016dbd48971fc2`.

- `168ab7ab` with combined fork `8a373f81`: complete genuine fork suite,
  4,708 Same/one Different; the VI-0012 regression failed identically on both
  sides because the harness injected a second `--no-report-long-running`.
  Failed raw archive SHA-256
  `c4e3826df19353f4935ad4d2b80689d2b1c754f61233f9b9aa15d1026608624a`.
  No reclassification or assertion change; see [#203](https://github.com/Chelis-Lang/sail/issues/203).
- `6880cb9b` with combined fork `8a373f81`: selected source-JSON run refused
  `SAIL3040` before any tests, because the generic stock installer consumed
  the fork-suite selector first. Final raw archive SHA-256
  `6fbbb1f78bd96633654e40f7e40be20247590ceea6d767b4208062a67714962a`.
  The correction separates release lookup from suite selection; no source
  admission check or comparison is relaxed.
- `c0e7e0f5`: full native stock control completed with 4,695 selected tests
  (58 ignored), 4,691 Same, 3 Unstable, 1 Different and 0 infrastructure
  errors in 1,174 s. `output_json::test_json_proof_note_on_requires` did
  not match any of 11 observed plain runs; plain varied. The strict gate
  failed, and the runtime after-hash stage was not executed. No JSON
  normalization or instability reclassification applies. Final raw archive
  SHA-256 `7227656b22b91737afcf3599334cd4bcdcf0edc509381bdb6ed059c62e5cc66f`.

- `9fa6c9d2`: complete fork/stock source-suite qualification interrupted from
  the lead's perspective by a prolonged session pause and host shutdown.
  Final outcome unknown; no final source-suite reports were retrieved. Earlier
  `84ca8c18` full fast/standard evidence was already archived and is retained
  above. No acceptance follows from observed intermediate progress.
- `218824c8`: interrupted selected fork run after relative `--shim` failed
  from the suite directory; path resolution corrected, comparison retained.
  Log SHA-256 `867481e25ecea9bff777e0e6f57bad7c9993ec682335f61861b9061fe7e781ec`.
- `218824c8` plus relative-path correction: rejected new cross-runtime
  experiment, 2 selected tests Different because relocating identical stock
  executables changes their reported input paths. No output exemption added;
  source-suite scope reverted to the two supported runtime-local arms.
  Reports/log SHA-256 `b676d20c982b843a451353ed51ab1154c283dd12149e60ba749501b57c2a9ec1`.

## Earlier rescue qualification

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
