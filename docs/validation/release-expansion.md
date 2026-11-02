# Release-expansion feature qualification

Status: feature `c590de7f` passes native Linux workspace, solver, complete E2E
and PR conformance gates, plus Mac platform and installed controls. The queued
origin correction has a real before/after regression witness. Broader corrected
edit/timing qualification continues; PR #647 remains draft. The feature retains
qualified fork `0ad405eb`. The separate runtime upgrade and performance selection
do not select a Sail release version or publication.

## Current gates

| Gate | Source, environment and actual result |
|---|---|
| Linux workspace and solver | `c590de7f`, native c7i.8xlarge / us-east-1 / Ubuntu 24.04, pinned CI image: Clippy, 1,320 non-solver tests, docs, layering, dependency policy, generated benchmarks and 51 solver controls pass |
| Linux E2E and PR conformance | Same source: every E2E phase including local `extra` passes; all four PR arms have 33 Same / 311 function records |
| Mac platform and installed features | Same production Rust, native mac-m4.metal / us-east-1c / macOS 26.7: Clippy, 1,309 tests and actual repair, preparation and byte-budget controls pass |
| Standalone repair | Both platforms: actual CLI/MCP focused then independent full Current verification, unrelated full failure, selector refusal, durable jobs and stale evidence |
| Preparation | Both platforms: genuine zero-verifier transcript replay, immediate foreground/edit controls, policy versus ghost-read changes, exact focus/whole isolation, generated-input cold/replay/changed-script controls and rejection of an already-edited queued origin before any managed run |
| Byte budgets | Both platforms: manual LRU/newest/busy/rebuild controls and actual idle-daemon eviction from the correct managed cache root |
| Standard admission paths | Reused `91bd82a1` full standard: four arms each 159 Same / 13,166 function records, all 18 seeded mutations, cold and both returns. Later changes bind idle maintenance and add a pre-execution preparation refusal; proof/cache admission is unchanged and their changed behavior has separate actual-daemon controls |
| Hosted CI cache | Four independent Ubuntu 24.04 jobs pass at `cdb95e58`, using the attested published 0.1.0 archive; fixture scope only |
| Default-workflow timing | Original six-round candidate fails output comparison on both platforms (19/26 rows valid); corrected paired timing and the fifteen-edit catalogue are running. No full-path or Mac Anvil parity is claimed |

Scheduling/cancellation unit tests use synthetic workers; these are not proof
acceptance. Installed controls use real pinned runtimes. No source or per-file
digest prototype is selected by this feature.

## Defects found and corrected

The original manual budget test missed automatic maintenance. A real old daemon
leaves two genuinely verified workspaces despite an enabled one-byte budget:
maintenance inferred its root from the solver job store instead of the managed
project cache. Both service entry points now explicitly bind the managed cache;
the extended installed driver and managed CI phase require actual idle eviction.
Direct job-server users without that configuration do no budget maintenance.

[#648](https://github.com/Chelis-Lang/sail/issues/648) was found by real-project
edit qualification. The original worker checked its origin only after running
verification. A subsequent edit made during the queue delay could enter that
transaction; foreground preemption could then interrupt it after consuming
build-script observations. IronKV's second edit took 29.79 s with preparation
and 4.66 s with it disabled, with vstd rebuilt only in the former. These traced
runs diagnose the defect; they are not performance acceptance.

Protocol v2 carries the canonical origin workspace and public read-back revision
separately from its complete closing revision. Before entering managed
verification the worker freshly checks the origin's Current record. Every
original key/context/closing-revision equality and publication check remains.
The regression writes a failing edit while preparation is queued and requires
no additional managed verification result. The unchanged old binary fails that
exact assertion; corrected Linux and Mac controls pass. Changes racing an
already-running transaction still take the existing cancellation/fail-safe path.

The earlier generated-input correction binds the successful runner's complete
closing revision, including inputs first discovered during compilation, while
preserving public establishing revisions and provenance. Its actual generated
include/replay/changed-script controls remain active on both platforms.

## Hosted walkthrough and consumer skill

[Hosted run 41381191209](https://github.com/Chelis-Lang/sail/actions/runs/41381191209)
passes all four jobs. Managed restore is Current with zero verifier invocations
(0.69 s restored versus 11.69 s cold for this fixture). Solver-only restore hits
the fixture's own function without a speedup. Both arms reject an invalid source
edit and changed runtime, and reverify changed configuration. This does not
qualify general-project performance or the new runtime.

Fresh-context technical and usability reviews approve the canonical consumer
skill at `91bd82a1`, with its target-directory clarification incorporated. They
use 42 frozen-output assertions, 19 package/help assertions and a workflow
comparison, not a model-evaluation campaign or a new native benchmark. A clean
project-scope `skills@1.7.4` copy install, telemetry disabled, retains all four
updated files byte-for-byte. Registry exports remain at their recorded revisions.

## Evidence

Raw artifacts stay outside Git; source/runtime inventories and original streams
are retained. The Mac combined capture also contains the separate A0e composition,
whose runtime source/factory acceptance is recorded in its own upgrade.

| Capture | SHA-256 |
|---|---|
| `stale-origin-linux-evidence-native.tgz` (installed before/after, E2E and A0e composed factory) | `313e9ed3a4da11f6b78f67bdc28b3e35e79696cede0d15c67df7918ba4d2765b` |
| `stale-origin-linux-execution.log` (complete Linux gates and exact old-binary assertion failure) | `52e6b48d051abb026c2f661f8ec46904d7978452d6333a739d915ee4fc80d9be` |
| `mac-root-and-stale-origin-evidence.tgz` | `74c6bb50eb063217042cda016bb832e98a5dc86f821af7e595e2ecaf262af2d2` |
| `feature-final-standard-evidence.tgz` | `5a630b2cc9822f457758dae35eb9e17d141a06bd67a7c5215f6399b5282b7ec3` |
| `budget-root-linux-evidence.tgz` | `95e705987d5d83d7d21fb7d390588593226c41e1b0d080c547cf1b8e9e43d323` |
| `hosted-cache-evidence.tgz` | `59b39ce059b980c12d33d988d3641fc313fbfc679c15d5d975fcaf81d311cb34` |
| `iron-successive-diagnostic.tgz` (enabled) | `51cef682405e8957ef4eaa2693edd5a791e4e9c6d4912b0f8be70885d3da0143` |
| `iron-successive-disabled-diagnostic.tgz` | `783a119dfb6c3c2f6256fcc6cc6f359403f844f396f11fe819416a7c6451955a` |
| `mac-a0e-matrix-selected-evidence.tgz` (failed original cohort, complete selected streams/metrics) | `ffc60269097ca39405f652e0adbaa22469226a24ac578b60c12ebcb7daaf0062` |
| `consumer-skill-technical-review.md` | `2b00f1cea3b323132ed961d09c4b9f9c58d00d2c9e0868dcaa77c0c3f35ba235` |
| `consumer-skill-usability-review.md` | `ef933c6113dd122df6071e6c785f40f7e3dcfa3e8d041ab0cde625582e97887c` |

## Attempt index

- 2026-11-01, original feature controls: MCP fixture looked at the wrong JSON
  location; the public full graph is checked with all acceptance assertions kept.
- 2026-11-01, Mac factory: existing evidence directory refused; fresh path passes.
- 2026-11-02, composed C9: workspace/platform pass; actual factory exposes native
  BLAKE3 provenance regression. Separate checksum-corrected runtime upgrade follows.
- 2026-11-02, closing-revision fixture: missing `pub open spec` visibility refuses;
  corrected source passes unchanged cold/replay/changed-script assertions.
- 2026-11-02, generated-input Mac fixture: logical versus canonical request-path
  mismatch; exact canonical path is required, with proof checks unchanged.
- 2026-11-02, idle-budget `653223d7`: two unqualified error type names stop Clippy;
  corrected before after-fix daemon controls. The following shared helper keeps
  service setup within the existing function-size lint.
- 2026-11-02, idle-budget `f281d40f`: workspace gate passes; CI wrapper supplies an
  existing directory. A fresh child path preserves the driver's original guard.
- 2026-11-02, original default-workflow candidate `4af47cea`: both paired cohorts
  fail output identity; queued-origin defect #648 is reproduced independently.
  Background work on the next edit also makes those edit timings unsuitable for
  acceptance. Original classifications and samples remain retained.
