# Jobs, watch and MCP without proof reuse

| Field | Evidence |
|---|---|
| Status | Native workspace, solver, every end-to-end phase plus extra, PR subset, full fast and full standard pass; required PR checks are enforced separately |
| Source | Native checks qualify `b0eb7534d023d372599b017f6621114595729a05`, based on `56d58208`; the final rebase onto `24cf4c3a` adds only the merged solver-research documentation and this record/API clarification, with functional source unchanged |
| Runtime | Pinned fork `73dea021`, Z3 4.16.0; stock oracle `0.2026.10.04.426d8b0` |
| Environment | Dedicated native EC2 c7i.8xlarge (32 vCPU), us-east-1, Ubuntu 24.04; CI images `sail-ci:44261d52f2b2ff03-core` and `sail-ci:44261d52f2b2ff03`; earlier macOS non-solver host checks |
| Scope | Unsupported-setup fallback in jobs, watch and blocking/asynchronous MCP; unbound evidence comparison and repair baselines; existing default-mode corpus |
| Limits | No performance claim, release qualification or whole-corpus managed-workflow acceptance; full native results qualify the source above, not a separate run on the final documentation-only amendment |

The native workspace gate passes 644 tests, with 35 solver tests skipped;
the separate solver step passes all 35. Every end-to-end phase and local
`extra` passes. Each conformance tier runs pass-through, cache fill, cache warm
and engine arms:

| Tier | Result in each of four arms |
|---|---|
| PR subset | 33/33 Same, 311 functions |
| Full fast | 147/147 Same, 3,450 functions |
| Full standard | 159/159 Same, 12,726 functions |

Every arm has zero differences, instability, infrastructure errors and skipped
projects. The standard engine arm records one rlimit-counter drift:
`verified-node-replication/verified-node-replication/src/lib.rs`, function
`lib::spec::cyclicbuffer::log_entry_alive_value_wrap_around`, plain 51,948
against Sail 51,945. Both have 253 verified units and zero errors, and their
diagnostics match. Its existing conformance classification remains Same;
no classifier or acceptance policy changed. Every other arm records zero
counter drift. This record does not infer the drift's cause.

Regression controls retain exact output/exit behavior, mark unbound runs
Unknown with `no_reuse`, prevent their use as repair baselines, and keep
cross-setup comparisons incomparable even when saved context strings match.
Successful Cargo runs without verifier results remain NO RESULT in MCP.
Ordinary bound results retain the existing comparison behavior. The unbound
aggregate target's `execution.verifier_invocations` counts its Cargo-Verus
driver result, not actual per-crate Verus or solver work.

All 522 tracked-file hashes, the Git head/index and absence of untracked source
were verified at every stage and after the final run. Fresh owned source and
build volumes prevent inherited artifacts from qualifying another source.
Each standard arm reached durable storage before the next arm began. The
final source verification passes; standard takes 4,900.747 seconds. These
corpus checks cover fork/shim/engine behavior; project-result and fallback
lifecycles are exercised separately by regression and end-to-end tests.

Raw evidence remains outside git. The private transfer collector verifies each
archive and every member's type, permissions and SHA-256 before retrieval:

| Archive | SHA-256 |
|---|---|
| `gate.tgz` | `12cceba77d48bc84ea0ec5f64ce15f820d9dc437a373c0c08dcff2e9b1e0e5f5` |
| `solver.tgz` | `6d94554390426f613055839d31cfd6642d57207ed36b46bf3be5c0cb40bc4d4b` |
| `e2e.tgz` | `4902651563b339fc39fecae0524b6cfabb96c21437c65e94b04800cf11b3025c` |
| `pr.tgz` | `312f23faf981fd3449b210739c3db7fef0d594e286ba63f73b9ef49eeeaaf024` |
| `fast.tgz` | `65347fde1176176f5d6cb9f7d720df6f25de8371bd5eba5e57d4c6953e0bb87f` |
| `standard.tgz` | `9252d669bae3ebd8034ec0b685113f72f6924ce9578c2438d1b0c9c6e9256626` |
| `standard-engine.tgz` | `779ed7e49955a37fdb7accba2896346d3fc4a1aaf6d0a218af3677e749d3e944` |
| `final.tgz` | `8eda1613212ae0534db94db47c26bb1fcb06fc988b0bc751beaefeac344776bd` |

Earlier macOS host checks at `b8542f03` pass clippy and 618 tests, with 35
solver tests skipped. The feature/review patches are unchanged by integration;
there is no claim of a new macOS run on the joined source. Archive
`mac-review-evidence.tgz`, SHA-256
`b91086f26c50829be6e5c575666980dd0a94f7668c55ac7ed5d260967f520c10`.
Earlier native required checks at `b574e393` also pass, retained in
`fallback-parity-native-evidence.tgz`, SHA-256
`d2b40f1adaf2c6113fd526ea0133b05248150f6deaf897e62f4cd3efc4f1d638`.

Indexed failed/interrupted attempts, 2026-10-16:

- `daa48b8e`: external pause outlasted the host watchdog before final-tier
  evidence reached durable storage; no full-tier qualification is claimed.
- `b0eb7534`, first replacement: Git's different-owner preflight refused the
  container checkout before product gates; retained archive SHA-256
  `2f25afdb392478ee488c4af775733e4ce643a332050110427a5e02775e9b4b4c`.
  The exact owned checkout uses command-local `safe.directory`; no global
  trust setting changed. The successful repeat above uses durable checkpoints.
- Earlier macOS review: an `assert_is_empty` lint stopped the first run. The
  assertions kept their expectations and the rerun passes; both are retained
  in the Mac archive above.
