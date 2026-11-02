# Release-expansion feature qualification

Status: feature `c590de7f` passes native Linux workspace, solver, complete E2E
and PR conformance gates, plus Mac platform and installed controls. The queued
origin correction has a real before/after regression witness. The corrected fifteen-edit catalogue and Anvil return controls pass their native
execution audits. Feature PR CI passes; the post-merge managed run exposes
startup polling defect #650, corrected by the bounded readiness change below.
Performance acceptance remains open
with the evidence limitations below. The feature retains
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
| Real edit catalogue | Native `039fb996` composition: all fifteen edits across Vest, IronKV and CapybaraKV, two stock/candidate rounds, pass full output/diagnostic, exit and memory/clock audits (96 samples); Anvil body/failure/fix/returns also pass (18 samples). Complete execution log retained; raw per-sample files were lost on host expiry |
| Default-workflow timing | Corrected Linux execution reports all 23 paired-workflow rows valid; three required mutation cohorts remain unattached. Raw final Linux timing files were lost on host expiry, so this is not performance acceptance. Mac retry refuses background CPU above the unchanged 0.10 ceiling; original failures and samples are retained |

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
no additional managed verification result. The unchanged old Linux binary fails that
exact assertion; corrected Linux and Mac controls pass. Changes racing an
already-running transaction still take the existing cancellation/fail-safe path.

The post-merge `c2c000e0` managed run fails before any budget operation:
its readiness poll requires exit zero before the newly started daemon has bound
its socket. `status --json` correctly returns exit 4, `SAIL0401` and the missing-
daemon JSON. The fixture now retries only that complete documented response,
requires the child to stay alive and eventually report running, and retains the
ten-second deadline and every budget/proof assertion. Other errors still fail.

Native c7i.4xlarge / us-east-1 / Ubuntu 24.04 controls deliberately delay the
actual daemon by one second. The unchanged old fixture fails its first poll;
the corrected fixture observes missing sockets at both startup points and then
passes the complete two-workspace LRU, automatic eviction and genuine rebuild
workflow. The native workspace gate passes Clippy, all 1,320 tests and remaining
documentation/policy checks. Production Rust and proof admission are unchanged;
the later correction only validates the missing-state JSON in this Python poll.

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

## Performance evidence limits

The corrected native catalogue passes all 114 samples. The corrected six-round
Linux execution reports 23 valid paired-workflow rows; the separate realistic
mutation cohorts were not attached, so its overall report remains incomplete.
The completed hosts subsequently expired before final raw-data collection. The
complete local execution logs survive, but they cannot substitute for the raw
streams/JSON in a performance acceptance claim. Functional gate captures above
were already collected and remain intact. The missing performance evidence is
being rerun with capture queued as part of the job.

Mac's original corrected cohort preserves its first two rounds and all subsequent
refusals. Background CPU rises above the existing 0.10-core ceiling; refused
rounds are not dropped or accepted. A fresh cohort uses the same source/runtime
with the animated wallpaper paused and the original quietness check unchanged.
The separate post-return CPU controls complete for Vest, IronKV and Anvil;
Capybara's observer refuses an empty preparation queue, so its preparation cost
is unqualified. Caller and observer exits remain separate. No cause for the
empty queue is established.

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
| `guard-edit-execution-complete.log` (catalogue and Anvil execution audit; raw files unavailable) | `822537fb9d283f89a074c7a88b7b8b5361290e7364eedd267db88733791db077` |
| `guard-native-matrix-execution-complete.log` (execution only, not raw timing acceptance) | `14861f6a33dd9a60f211c708597d16d3157bfcb467a7f698f4abcab19f657f60` |
| `mac-guard-first-cohort-evidence.tgz` (original corrected cohort, including refusals and cost controls) | `675d4c6f2271aabca07721d259378d74805884703812dfb7764716d12edb2e1c` |
| `budget-readiness-native-evidence.tgz` (real delayed-daemon old/new controls) | `73eae0725c198711c105be4e558f8d5c73ecdd2697f9ed4e9c29638bef41c44a` |
| `budget-readiness-native-execution.log` (corrected actual controls) | `f08f615a5590f955e8e3c68313b19668418d001a38ad5f3ac77771caf3320852` |
| `budget-readiness-r1-execution.log` (complete workspace gate and first-patch refusal) | `b3d6ab3e69cf36dca970bf4bf8873de0ecc37d3c02f9927d8ae5a416ec3b5b60` |
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

- 2026-11-02, corrected performance qualification `039fb996`: native workflow and
  catalogue execution audits pass, but final Linux raw files are unavailable
  after host expiry. Mac's original CPU ceiling refuses later rounds; the retained
  failed cohort is followed by a fresh quiet-host retry. Performance remains open.

- 2026-11-02, `c2c000e0` main CI: budget fixture rejects documented
  `SAIL0401` before the daemon listens (#650). The first native patch preserves
  the workspace gate but incorrectly assumes empty missing-state stdout; its
  actual delayed-daemon control refuses that assumption. Requiring the exact
  missing-state JSON passes the original full workflow. First failed capture:
  `794cb63835fe570597097b66a42cbd8c732b904ab111fd3d6a9d0853a60b3768`.
