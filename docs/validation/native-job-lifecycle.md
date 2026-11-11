# Native job lifecycle and dependency-policy qualification

| Field | Current state |
|---|---|
| Tracking | #713 (canonical fixture), #714 (dependency policy), #715 (shutdown wakeup) |
| Source | `98042f076efd125ef147eabf8380f5dc7fe6e838`, based on `334686f8` |
| Linux | Native EC2 c7i.4xlarge, us-west-2; pinned CI core/MCP images built natively |
| Mac | Native EC2 mac-m4.metal, us-east-1c, macOS 27.0; Rust 1.99.0, nextest 0.9.143 |
| Status | Workspace, solver, full end-to-end, all four fast/standard corpus arms, affected model/refinement checks and extension pass on Linux; Mac host, book and extension pass. Standard mutation/revert controls also pass; native qualification is complete. |

## Changes and acceptance boundary

The retired-record fixture now uses the canonical project identity required by
the List request, retaining its two-record assertion, all `SAIL3161` refusals
and the ordinary-job control. The initial independent symlinked-TMPDIR control
links the original and corrected fixture against the same `7bd938f5` daemon
library: the original fails and the correction passes. Its log SHA-256 is
`ef2f9fa8d95ffbff3fb2b95bee0e90679c5dea8baa05e1841601e7f3a0876bf8`;
it is retained in the first-attempt archive indexed below. Current-source
canonical and symlinked controls are recorded separately below.

Shutdown publishes its stop predicate and broadcasts while holding the queue
mutex used by the worker's predicate check and condvar wait. It releases that
mutex before acquiring the jobs lock or joining threads. Normal shutdown and
all three partial-startup cleanup paths use the same helper. The controlled
regression holds the check/wait boundary; separate native Mac executions cover
the original coalescing test and a 64-worker pool. Poisoning and OS thread-start
failure paths were source-reviewed, not injected.

The `rustix` patch and its exact root-lock binding have an independent
[source review](acceptance-production-refinements.md#dependency-lock-source-review-2026-11-11).
Both workspace locks change only that package's version/checksum; no package
edges, features or solver identity change. Generated notices also reconcile
the already-removed neutral crates. The verifier source lock and runtime
archives are unchanged. There is no performance or complete-service theorem
claim, and no corpus pin, example, oracle or timeout changes.

## Executed controls

At `98042f07`, Linux `ci/local.sh gate` passes 1,323 ordinary tests (53
skipped), doctests, rustdoc, layering and all dependency-policy domains.
`solver` passes all 50 selected solver fixtures. Full `e2e` completes every
phase and the local extra cases. PR conformance reports 33 Same, zero Different,
zero Unstable and zero infrastructure errors in each of its four arms, with
311 functions per arm. The seven reported drift units remain visible.

Mac `ci/run.sh host` passes clippy and all 1,310 tests (53 skipped), the
corrected retired-record test passes, and `ci/run.sh extension` passes fmt,
clippy and all 22 tests at the unchanged private D.I.V.E. pin. Book source,
build and link checks pass; the optional Node highlighting probe is skipped
because Node is absent on that host.

Independent native checks at `14e72d92` accept all eight exact parser bindings
and reject the previous lock and a one-byte drift. The checker and every bound
input are byte-identical at `98042f07`; these controls are reused identical-source
evidence, not relabeled executions. The original parser dependency-edge,
native-binding and must-use negative controls pass. The corrected fixture
passes with both canonical and explicitly symlinked temporary directories.
On Mac the shutdown handoff, blocked endpoint shutdown, original coalescing,
64-worker coalescing and symlinked retired-record controls all pass against
the exact gate-built executables. These focused controls supplement the full
gates; they do not replace them.

Linux workspace log SHA-256:
`1770d99e7801a3b609da1c1e09ac05b434910cd77140de9c531046589186bf39`;
solver log:
`673bcbca2d833ce05d9fab5c27449ff9642ed487de6d640b4da105a5236a60d8`.
Full fast conformance passes all four arms: 147 Same units and 3,451 functions
per arm, with 25 drift units retained. Standard corpus comparison passes all
four arms: 159 Same units and 13,166 functions per arm, with 32 drift units
retained. Each arm has zero Different, Unstable, infrastructure errors, skips
or accepted exceptions. The standard catalogue regeneration check is byte-identical for CapybaraKV,
HumanEval, IronKV and Vest. All 18 HumanEval mutants, the cold baseline, revert
and forced revert pass: 21 Same comparisons, zero differences, unstable,
infrastructure failures, false verified, catalogue mismatches or accepted
exceptions. Both reverts serve the original answers. These are correctness
controls, not a performance acceptance claim.
The independently audited fast/standard report archive SHA-256 is
`becd58af1fee8aae45a6fa31b98dd0df62b2fada4f0d083b09085bfee002734d`.

The affected model checker passes all 35 actual stock-Verus runs. Production
refinement checking passes 316 runs (seven positive and 309 negative controls),
with 335 exact source bindings. Negative controls are actual intended proof
refusals, not resource failures or malformed VIR. The checker binds all 3,873
runtime files. The raw proof/refinement archive SHA-256 is
`d818f72c2e63ef9013f3a6a15ee6c40217b5961b917a5295c896fb3c6c1acea6`.
Linux temporal extension fmt, clippy and all 22 tests pass at the unchanged
private revision; no private sources or credentials enter the CI image.

Full E2E log SHA-256:
`3b29632f1a0fc4a7e8339fa9b7d09c5dd069c2c3683f40979edacf5e91279145`;
PR report archive:
`bc42ed4ccbdd4ba92917bb6b8da8795d08224bb6f92bcbd6a6efa7db32a87cc0`.
Standard suite log SHA-256:
`ec453d9b46b723af6be5421f13f551a09e399eb32165e92d2f99076cb61ef672`;
final mutation/revert archive:
`410315e49717f3d98fa73419575b2760659fed5bdf130c84cc99bb1e5e1f9503`.
The final documentation/notices candidate reruns the workspace gate at
`3e2cd4e4`: all 1,323 tests and every other workspace gate pass. Its
implementation is byte-identical to `98042f07`. The separately pinned Linux
mdBook 0.5.2 source/build/link checks pass at `5f9d31cb` (log SHA-256
`58ea3b3da359f590b9172390f7a0f1b5774556f171667767a20e61261bc7b7e5`).
Later amendments only index these attempts and their setup refusals.
Raw evidence is retained outside the repository.

## Failed and interrupted attempts

| Date | Source | Result | Raw archive SHA-256 |
|---|---|---|---|
| 2026-11-11 | `7bd938f5` | Linux workspace tests pass; original dependency policy fails RUSTSEC-2026-0486. No advisory exception. | `2db848e941ea4603363a4cf820afacb1812ad8e4a362eaab68df0f00d91bb2bb` |
| 2026-11-11 | `5e96992a` | Mac host: 1,311 pass, retired-record assertion fails (0 versus 2); canonical fixture corrected under #713. | `a0f07503ea487c8d3bedbf2b853c34799ef4866d0d8d2484bc1bb5017ebda9ab` |
| 2026-11-11 | `14e72d92` | Linux gate (1,322) and solver (50) pass; end-to-end interrupted for the independently found shutdown correction, not counted passed. | `e954646aa06b99c51844207f2e9a4d2c5a196b1d65b3a4597d8726a01def7487` |
| 2026-11-11 | `14e72d92` | Mac host: 1,308 pass, coalescing times out at the unchanged 180 seconds. Live sample shows shutdown joining the idle condvar waiter; #715 corrects the lost wakeup. | `afffdd0c130a58257e1824940b9f99b8e643298486a0fc0805d713d029111871` |
| 2026-11-11 | `3e2cd4e4` | Final workspace gate passes (1,323 tests); supplementary Linux book attempt stops after source checks because the core image has no mdBook. The pinned 0.5.2 binary is installed separately, without changing the CI image. | Log `8c46540b3532b5ba9a497298e135b0934eeef3a23317597901a3b05541b5d430` |
| 2026-11-11 | `5f9d31cb` | Pinned Linux book attempt refuses Git ownership in the root container. Explicit trust of the owned `/work` checkout fixes setup; source, build and link checks then pass. Optional Node probe remains skipped. | Failed log `28e5703f53eb82537b1b3c80502fd47110f321fb218cfcea6095a7bee79dae42` |
