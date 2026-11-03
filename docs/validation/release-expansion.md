# Release-expansion feature qualification

Status: feature `c590de7f` passes native Linux workspace, solver, complete E2E
and PR conformance gates, plus Mac platform and installed controls. The queued
origin correction has a real before/after regression witness. The corrected fifteen-edit catalogue and Anvil return controls pass their native
execution audits. Feature PR CI passes; the post-merge managed run exposes
startup polling defect #650, corrected by the bounded readiness change below.
The later complete frozen composition now has all workflow and full mutation
captures on both platforms, recorded below. Its unchanged strict reporter
accepts 25/26 rows because of explanation-note classification #655. The merged
per-code correction independently rechecks all 1,944 retained paired steps; a
separate report has 26/26 valid rows on each platform, preserving every timing
and original strict failure.
Performance acceptance and final owner approval remain open. The feature retains
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
| Real edit catalogue | Exact `81c41be8` rc.1 archives on native Linux/Mac: fifteen edits across Vest, IronKV and CapybaraKV, two seeded stock/candidate rounds plus Anvil body/failure/fix/returns; all 114 samples/platform and all four execution audits pass, with complete raw captures. The earlier lost attempt remains indexed below |
| Default-workflow timing | Latest frozen `039fb996` / A0e cohort: all 23 workflow rows and all three complete mutation catalogues captured on both platforms, six rotated blocks. Original strict reporting remains 25/26 valid; merged #657 retained-output rechecking gives a separate 26/26-valid report, with all timings unchanged; no performance acceptance or release approval follows |

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

## Complete frozen composition measurements

Source `039fb996`, pinned A0e runtime, six rotated stock/candidate blocks:
native c7i.4xlarge / us-east-1 / Ubuntu 24.04 and mac-m4.metal /
us-east-1c / macOS 26.7. All 23 paired workflow rows and the complete
52-edit Vest, 48-edit IronKV and 53-edit CapybaraKV catalogues are captured
on both platforms. Each platform has 972 paired mutation/cold/return steps,
with matching verdicts and every ordinary/forced return receiving its own
answers. The Linux capture additionally preserves all 276 workflow invocation
logs and all 1,320 Vest step streams; the independent IronKV/CapybaraKV captures
and full Mac capture retain their complete paired streams.

The original strict combined reporter accepts 25 of 26 rows on each platform.
CapybaraKV's six cold calls each print one recognized `SAIL3138` and one
recognized `SAIL3146` explanation; the old counter mistakes their combined
length for a repeated note. ADR 0099's qualified per-code correction is merged
in [#657](https://github.com/Chelis-Lang/sail/pull/657).

The corrected Rust retained-log checker at `f023b69c` independently rechecks all
1,944 original paired mutation/cold/return steps on native c7i.4xlarge. All
compare Same with zero false verified results. Each original raw archive hash
is checked, each flagged raw stderr equals its two recorded notes and stock
stderr is empty. A separate projection changes only `diagnostic_error` in the
twelve affected cold rows (six per platform); every header and every other
sample value, including timings, memory, verdicts, functions, exits, provenance,
daemon and return checks, stays identical. The unmodified frozen matrix
reporter then finds **26/26 valid rows on both platforms**. Its exit remains 2
for slower workflow medians; this is not blanket performance acceptance.

The corrected interpretation, exact checker output, source identities and
unchanged timing projections have 33 sealed members, archive SHA-256
`07f28bd8b7d7534105efd55ce98019a6def3e5738d53eeb9ea66518c6fffb289`.
Original strict reports, exits and raw samples remain unchanged.

| Valid complete mutation loop | Linux speedup | Mac speedup |
|---|---:|---:|
| Vest, 52 edits per round | 1.22x | 1.44x |
| IronKV, 48 edits per round | 1.51x | 1.55x |
| CapybaraKV, 53 edits per round | 2.19x | 2.38x |

Every individual mutant median is faster in all three complete catalogues on
both platforms. Linux still has two slower workflow medians: small edits add 0.19 s and small reverts add
0.15 s. Mac has three: small edits add 0.11 s, small reverts add 0.10 s and
Anvil cold adds 2.07 s (96.87 s stock / 98.94 s Sail). All five slower workflow ranges are disjoint in this six-round cohort, so
these are retained engineering gaps rather than dismissed as measurement noise.
Tiny operations add at most 0.19 s; Mac Anvil cold adds about 2.1%. No cause or
new optimization is inferred from those medians. Every mutant median remains
faster; full-path misses are a separate workload. Ranges and complete samples
stay in the retained reports. The separate no-replay full-path
gaps in the [runtime record](binary-compiler-inputs.md#corrected-mac-workflow-measurements)
remain open; replay improvements do not establish general parity.

| Complete raw capture | SHA-256 |
|---|---|
| Linux workflows/Vest and composition, 37,067 sealed members | `75a6399a04086a3608615dbd55fe14820e2cbb9cabd3476dde743d9694a4e1e4` |
| Linux IronKV | `677ae4da130a1d3bb693432cdf1eb43ccc671b0cd937415a68888ce41cfa1e4f` |
| Linux CapybaraKV, original classification failed | `f33e06633d2918b96c0c68719f5d72d46666dd658cdf6a08986a8de2588b2788` |
| Mac workflows/all three catalogues, 5,941 sealed members | `25be7412764974416e870c06b32d8c5b05fd319f115528f23d7f2543a241aa79` |

These measure the frozen core/archive composition, not the later Cargo
installation's exact binaries. Final candidate artifact qualification remains
separate. The fifteen incidental-edit catalogue is a different workload; its original
raw timing loss is retained below. A later rerun of that exact catalogue on
rc.1 archives repairs its evidence gap as recorded next.

## Recovered incidental-edit qualification

Exact `81c41be8` rc.1 archives, native c7i.4xlarge / us-east-1 / Ubuntu 24.04
and mac-m4.metal / us-east-1c / macOS 26.7: the original fifteen-edit catalogue
across Vest, IronKV and CapybaraKV runs in two seeded stock/candidate rounds.
All 96 samples plus Anvil's 18 body/failure/fix/return samples pass complete
output/diagnostic, exit, memory-cadence and clock audits. Each platform retains
all 114 raw samples and all four phase-audit results. These are the actual
incidental edits, not a substitution by the larger mutation catalogues.

Both archives also pass clean-PATH first-use/repair smoke and fixed three-call
ordinary Cargo JSON verification with final zero-verifier Current replay.
The initial setup refusals remain in each capture: Mac had `cargo-verus` on
PATH; Linux needed the same three catalogue prerequisites prefetched. Only
harness prerequisites changed before the complete reruns.

| Completed rc.1 archive/install/catalogue capture | SHA-256 |
|---|---|
| Linux, 8,468 sealed members | `26f5206d0b0c8b14779fb4cc2a1dfc6db3fb9004e93b10b3b9dfc47798bc2cd6` |
| Mac, 6,138 sealed members | `6667367fb05d6ededb4afef5814e93d2ef4bce1c9a333f9e14bcfc994fdd6895` |

These qualify rc.1 artifact/workload composition. Stable versioned archives
have their own [candidate record](stable-candidate.md). Neither capture closes
the separate first/full-path or Mac Anvil-cold performance goals.

## Earlier performance attempts


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
The fresh six-round Mac cohort completes all 23 paired workflow rows with
exact outputs and retained ranges; three realistic mutation cohorts remain
absent. Its [runtime record](binary-compiler-inputs.md#corrected-mac-workflow-measurements)
gives measured first-use gains and the remaining full-path/Anvil-cold gaps.
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
