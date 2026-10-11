# Known issues and acceptance limits

This page separates product limitations, upstream behavior and unresolved test
failures. It describes the current M7 development branch; it does not declare
that branch released or accepted. The [roadmap](ROADMAP.md) tracks remaining work.
The [retained status history](validation/m7-acceptance-status-history.md) preserves
the detailed prior roadmap narrative and its distinct failed/setup scopes.

For current experimental repair use:

- A successful focused check still needs complete Current full validation.
  An Unknown result, including a namespace observation failure after the
  verifier finishes, cannot authorize a repair.
- The previously built combined product discards the ordinary build-script
  generation after negative proofs. The next check can recompile unchanged macro dependencies;
  [the recovered diagnostic](validation/m7-repair-a960-recovered-setup.md)
  identifies this mechanism. The branch now retains ordinary contracts only
  after fully observed normal negative verification; its
  [Linux controls](validation/m7-negative-compiler-generation-plan.md) are
  pending. Proof acceptance and successful receipts remain independent.
- Full-result reader latency and real-project repair speed remain unqualified.
  Original workflow scopes are retained with their original bounds; use the
  [roadmap](ROADMAP.md) for current acceptance status.
- The complete gate currently has an unresolved shared HTTP-reader deadline
  failure. The async transport candidate passed affected controls but regressed
  complete-operation cost and was rejected; no increased timeout or weakened
  hit assertion is selected.

The original stock Verismo tspec deadline failure is now tracked locally as
[VI-0013](verus-issues/VI-0013-verismo-tspec-stock-wait.md). Sleeping compiler
threads and empty solver pipes do not identify its cause; the complete standard
attempt remains failed. No workaround is selected.

## Interactive repair engine (M7)

The earlier late-handoff source candidate builds and freshly verifies all 2,060
vstd obligations, but fails the original migration qualification because it
publishes no prepared lowering. Both fixture proofs succeed. Parallel workers
correctly omit a diagnostic source map; the candidate mistakenly required that
optional map for preparation. The [preparation-only namespace correction](../engine/verus/candidates/preparation-namespace/README.md)
now passes 41 VIR and 88 driver tests, both strict compiler checks and an ordinary
build with fresh vstd, plus [independent review](validation/m7-preparation-namespace-independent-review.md).
The unchanged qualification passes eight stages and fails its ninth
(previous-revision focus) with zero hits, despite stock/engine 9/0 and exact
diagnostics. The [bounded diagnostic](../engine/verus/candidates/preparation-namespace/boundary-diagnostic/revision0-result.json)
now identifies three uncovered imported ranges (tracked as
[VI-0014](verus-issues/VI-0014-exported-foreign-span-metadata-gap.md)) and a
distinct actual `pair-busy` admission refusal. The separate
[retirement acknowledgement](../engine/verus/candidates/preparation-retirement/README.md)
passes 16 reduced pinned controls, 41 VIR and 98 driver tests, and both strict
compiler checks. Removing the actual adapter's finish call fails its unchanged
guard-lifetime regression. The combined
[missing-original-range role](../engine/verus/candidates/preparation-namespace/no-original-range/README.md)
is implemented with complete imported-table identity and exact current raw-span
rebinding. Its first compiler scope passes 42 VIR controls and VIR strict, then
fails driver test compilation through a private-module reference. The failed
scope is retained. A reviewed test-only integration correction preserves all
assertions and production code. The following scope exposes an invalid digest
fixture and preserves its test-lock cascade separately. With a valid admitted
digest, [complete compiler controls](../engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/source-controls.json)
pass all 150 tests and both strict checks. The
[real memo-bypass sensitivity](../engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/memo-bypass-counterfactual/result.json)
fails the unchanged assertion with the bypass removed and passes after exact
source restoration. The separately reviewed
[ordinary runtime build](../engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/ordinary/ordinary-build-result.json)
passes with fresh verification of all 2,060 vstd obligations and all 13 runtime
artifacts sealed. Its original thirteen-stage qualification now passes, including
genuine previous-revision focus and all three full HumanEval revisions. All
fourteen frozen scripts and original checks remain unchanged. The correction is
selected together with its independently verified prebuilt runtime pin on the
in-progress branch; rebuilt-product integration, reader latency and repair
performance remain open. Earlier failed qualifications remain failed. Stock
diagnostic routing and parallelism remain unchanged. See the
[exact failure audit](validation/m7-preparation-namespace-qualification-independent-audit.md). See the
[retained qualification](../engine/verus/candidates/deferred-import-locator/late-handoff/qualification-failure.json).

The daemon's [retained-frame accounting](validation/m7-result-frame-admission-margin.md)
now covers Vec capacity, evicted live writes and fixed retirement metadata
within its existing 128 MiB ceiling. All 89 daemon controls pass. This bound
excludes uncached encoding and process RSS; it does not establish the live
Result latency target. The retained 394.713 ms p95 failure remains open. The current
[stage workflow](validation/m7-rpc-stage-workflow.md) fails complete Result p95
at 403.341 ms with all 200 samples valid and live; its 43-row Result tail has
cached frames without encoding. Freshness and socket-write costs are under
investigation. The [retained CLI/MCP scope](validation/m7-rpc-retained-functional.md)
also preserves the original wrong-stream assertion failure. Its
[corrected followup](validation/m7-rpc-retained-corrected.md) passes the exact
stable error, reconnects, complete results, baseline comparisons and actual
submission/cancellation under the original bounds. This does not pass the live
reader latency gate. A larger socket buffer was not selected: the
[offline full-transfer control](validation/m7-result-transfer-profile.md)
shows no consequential complete-client latency improvement.

An Unknown proof and unavailable validation during a later job read are distinct
conditions. Earlier managed reports collapsed precise namespace failures to
`native-observation-invalidated`; they cannot establish which event or barrier
failed. New reports retain the existing static namespace reason and first
failed barrier without additional probes. They still omit the affected file and
event cause. The [coverage diagnostic boundary](validation/m7-proof-coverage-diagnostics.md)
explains the remaining attribution limits. Earlier HE Unknown reports remain
failed acceptance evidence; these diagnostic changes do not recertify them.

The service-owned D20 workflow establishes Current preparations and passes its
bounded baseline checks, but its active repair completes with Unknown sysroot
coverage (`sysroot-namespace-invalidated`). Neither reader census nor retained
CLI/MCP integration was attempted. The [terminal scope and audit](validation/m7-owned-reader-d20-sysroot-audit.md)
retain this failure separately from the earlier executable ownership refusal.
The older coarse note does not identify which sysroot observer or event failed.

CLI/MCP repair checks, journals, explanations and immutable baselines are
implemented. The selected source runtime passes source fixtures and the small
native Linux CLI/MCP scenario. Real human-eval-verus and Vest acceptance is
unfinished. Neither a twofold whole-loop gain on each project nor the real
four-reader latency gate has been established. See the
[acceptance guide](bench/m7-repair-acceptance.md) for exact scopes and records.

MCP now moves owned result graphs and encodes complete responses outside the
stdout mutex. Exact text/structured payload, cancellation and atomic-frame
controls pass; they do not explain or replace the Python-first live-reader
failure. Slow output I/O and full response serialization remain costs. The
[scoped record](validation/m7-mcp-output-ownership.md) retains three initial
44/45 test failures and the final 46/46 pass: a peer read returned lawful EINTR,
then the fixture handled only Interrupted within its original absolute budget
while still requiring EOF. Historical cancellation failures remain separate.

- The selected baseline's diagnostic reporters release quota before nested
  allocations finish dropping. Its accounting does not establish the required
  aggregate 512 MiB physical preparation bound. The corrective source candidates
  remain unselected; [engine internals](../engine/verus/README.md) describe the
  boundary. Preparation counters also exclude ordinary compiler working RSS.
  The physical audit also found uncharged portable binding/message decoding,
  query decoding and graph-validation scratch allocations. The corrective
  candidate must admit these before allocation and retain ownership until
  physical drop; a serialized-byte limit alone does not bound decoded memory.
  The [bounded-decoding overlay](../engine/verus/candidates/decode-budget/README.md)
  has pinned-source tests, but ordinary runtime qualification is pending.
  A further audit found unbounded diagnostic-event JSON expansion before the
  observer lock, which can overlap preparation workers. The unselected
  [observer overlay](../engine/verus/candidates/observer-budget/README.md) now
  bounds that path, formatting and progress ownership. The combined allocation
  argument reaches 492 MiB. The combined candidate passes all 22 actual
  RustVerify driver tests and strict all-target Clippy with the pinned module
  layout and serialization features; ordinary runtime qualification remains
  pending. See [the source test record](../engine/verus/candidates/qualifications/bounded-private-span-host.json). Its guarded decoder admitted only 36 of 105 retained actual
  human-eval preparations and safely refused 69 under its fixed allowance.
  This is a reuse limitation requiring more precise allocation accounting,
  rather than evidence that empty-fixture passes establish product performance.
  See the [retained entry probe](../engine/verus/candidates/qualifications/bounded-decode-retained-he.json).
  The subsequent [closed ownership prototype](../engine/verus/candidates/decode-ownership/README.md)
  admits 77 and safely refuses 28 of those 105 bodies under the same allowance.
  It removes duplicate inline charges only for exact audited value/visitor
  pairs; dynamic growth, pointer pointees and unknown/custom visitors retain
  admission before allocation. Its 12 helper controls and the combined actual source
  suites (33 VIR and 22 driver tests) and both strict all-target Clippy gates pass.
  [The complete source record](../engine/verus/candidates/decode-ownership/combined-source-checks.json)
  binds these checks to the full combined patch. The independent ordinary Linux
  build now passes, including 2,060 vstd obligations and its complete artifact
  manifest; see [the build record](../engine/verus/candidates/decode-ownership/ordinary-linux-build.json).
  The following runtime pipeline passed its early controls but failed the
  required previous-revision focus gate: zero checked hits after a graph scratch
  admission refusal. Full human-eval qualification was not reached.
  [The failed source pipeline](../engine/verus/candidates/decode-ownership/c39-qualification-failure.json)
  retains every reached gate. Allocation bounds now follow the actual scratch
  owners for each query under the unchanged 8 MiB cap, with 37 pinned VIR
  controls and strict Clippy passing. The
  [allocation argument](../engine/verus/candidates/decode-ownership/scoped-graph-scratch.md)
  covers simultaneous ownership and retained vector capacity. The new ordinary runtime builds with all 13 sealed artifacts and 2,060 vstd
  obligations. Its unchanged cross-edit gate now reaches decoding and graph
  validation, but refuses span provenance for the unchanged focused function
  after an unrelated body edit. Complete full outcomes and early controls pass;
  the required edited-revision persistent focus hit does not. The candidate
  remains unselected; see the
  [full failure record](../engine/verus/candidates/decode-ownership/scoped-query-qualification-failure.json).
- The subsequent LoadedForeign candidate builds and passes its scoped source
  tests, but the original qualification fails strict progress-note ordering
  and records no restored preparations. Its first actual imported span remains
  Delayed at capture and focus, so LoadedForeign correctly refuses it. The
  [failure record](../engine/verus/candidates/decode-ownership/loaded-import-qualification-failure.json)
  preserves both problems. A bounded deferred diagnostic recipe is being
  implemented under [proposed ADR 0033](decisions/0033-deferred-import-diagnostic-locators.md),
  with current compiler resolution unchanged. It is not a selected source
  runtime or demonstrated cross-edit reuse.
- The DeferredLocator `73bd` runtime passes actual source controls and its fresh
  2,060-obligation vstd build, but its unchanged fourteen-stage qualification
  stops at revision 0 of previous-revision focus. Eight earlier stages pass;
  four fresh preparation provenance refusals leave zero hits. Stock and engine
  verdicts and exact diagnostics match. No nonzero unchecked hit is observed,
  and later cross-edit revisions were not reached. The candidate remains
  unselected. See the [terminal failure and byte seals](../engine/verus/candidates/deferred-import-locator/dynsend-owner/qualification-failure.json)
  and [independent span-form review](../engine/verus/candidates/deferred-import-locator/dynsend-owner/fresh-provenance-review.md).
  The [separate bounded import-state trace](../engine/verus/candidates/deferred-import-locator/dynsend-owner/provenance-diagnostic-result.json)
  completes with five eligible Loaded records followed by an ineligible Delayed
  descriptor. It does not record that descriptor's filename or Foreign-match
  count, and cannot attribute Local or unsupported span-form refusals. The
  [original-descriptor correction](../engine/verus/candidates/deferred-import-locator/original-descriptor/README.md)
  is implemented and passes 30 pinned API/metadata controls, but is unselected.
  Complete source tests, runtime build and original qualification remain pending;
  the diagnostic does not replace them.
- Real other-module edit controls for candidate `2459` found **zero demonstrated
  reuse across edits**. Its one persistent full-phase hit reused preparation
  written by the focused phase in that same invocation. See
  [the retained control](../engine/verus/candidates/qualifications/2459-other-module-edit.json).
- An independent stock run chose a different declaration order for identical
  human-eval source. The original candidate qualification remains failed.
  [VI-0001](verus-issues/VI-0001-nondeterministic-declaration-order.md) and
  [ADR 0027's amendment](decisions/0027-interactive-proof-engine-and-repair-workflow.md)
  explain the owner-approved comparison: cached and fresh lowering from the same
  compiler input, with stock verdict/diagnostic checks and separately reported
  stock transcript instability. Candidate `2421` completed all 106 independent
  cached/fresh bucket solver comparisons, but its separate authoritative primary
  did not finish before the unchanged 600-second human-eval deadline. The full
  report and terminal seal are absent; edited revision and trailing library
  gates were not reached. It remains unqualified and unselected. See the
  [retained failure](../engine/verus/candidates/qualifications/2421-linux-resumed-failure.json).
  A new candidate must complete this gate before selection; static review is
  insufficient.
- Earlier real reader and cold repair attempts exceeded their original limits
  or could not establish Current evidence. Their records remain in
  `docs/validation/`. The a6/b346 diagnostic established genuine public Current
  baseline and active-copy proofs, then failed its Python reader census:
  125/200 samples satisfied the complete freshness/revision admission checks;
  result p95 was 402.821 ms against the unchanged 200 ms limit. All samples
  retained the same actual selected verifier. The frozen collector did not record
  the failed samples' exact freshness labels or reasons; their cause is being
  profiled. Rust and retained CLI/MCP scenarios were not attempted after this
  failure. See [all retained samples](validation/m7-a6-b346-reader-census-failure.json). A
  separate traced retained-job diagnostic, with no active verifier or new proof,
  classified 13/200 Unknown reads as bounded runtime-validation ownership waits;
  187 were Current. It retained all namespace/runtime/lease checks and unchanged
  original job bytes. Its instrumented timings fail the latency target and do
  not explain the original live run's sustained failures. See the
  [classified diagnostic](validation/m7-retained-proof-freshness-profile.json).
  The subsequent caller-entry candidate returned 200/200 Current reads in a
  separate retained-job diagnostic, preserving all guards. Its traced status
  p95 was 209.120 ms and result p95 407.420 ms, still above the 200 ms gate.
  It had no active verifier and cannot qualify the original live workload.
  See [the candidate diagnostic](validation/m7-reader-entry-coalescing-diagnostic.json).
  A fresh `8e09eaa` attempt then timed out during ordinary baseline preparation
  at its original 600-second limit. No Current baseline or reader census was
  admitted. Its completed vstd dependency report is partial setup evidence,
  not a complete human-eval proof; see
  [the setup failure](validation/m7-reader-entry-setup-timeout.md).
  A later, explicitly unmeasured preparation established genuine Current full
  baseline evidence after 1,839.532 seconds and three actual verifier starts.
  The independent active workspace also completed preparation in 2,717.788
  seconds. Both subsequent original bounded proof checks passed, but the frozen
  Python collector failed: all 200 RPCs completed with the same active verifier;
  only 13 admission Booleans were true, and status/result p95 were 222.54/489.74
  ms. Exact currentness labels for failed admissions were not retained. The
  original fail-fast controller did not reach Rust or retained CLI/MCP checks.
  See [the complete record](validation/m7-reader-prepared-terminal.md).
  Staged ordinary SDK decoding reuse is implemented, with allocation and
  promotion-recovery controls. Its final live census remains pending; it cannot
  retroactively qualify this failed run.
- The older frozen reader controller assumed the newer `results/projects/jobs`
  storage layout, while a6 stores jobs under `projects/jobs`. Public proof
  evidence remained intact, but automatic raw archival could omit those files.
  Future controllers now resolve exactly one supported layout and reject
  missing, ambiguous or substituted evidence. The original run remains frozen;
  its raw evidence was archived separately. See [reader acceptance](reader-acceptance.md).

## Freshness and platforms

Current is an input-coverage claim, separate from a solver verdict. Missing
coverage, lost leases or uncertain declarations produce Unknown; a saved proof
does not become a new proof of changed inputs. Linux retention requires exact
kernel and namespace coverage, including lease permission for runtime files.
An ordinary user may lack permission to lease a system-owned Python interpreter.
macOS takes a conservative path and may require fresh verification.
See [freshness](freshness.md) and [trust](trust.md).

The development branch now retains a sticky kernel mount-generation guard.
Earlier mountinfo snapshot comparisons could miss a bind mount restored before
the next barrier. A real isolated control restores both bytes and mountinfo,
yet the new namespace and lease consumers correctly remain dirty. Unsupported
coverage refuses reuse. See [ADR 0032](decisions/0032-kernel-mount-generation-coverage.md)
and [its scoped validation](validation/m7-kernel-mount-generation.json);
complete affected gates remain required.

The first automatic guarded-sysroot integration also failed to enable reuse:
its scope encoder passed byte-keyed environment bindings to a JSON object
serializer, which rejects that input type. The original content-hashing path
still ran. The bounded streaming encoder of exact sorted byte pairs passes its
Linux controls and the corrected product builds. Its
[original managed scenario](validation/m7-sysroot-activation-1e8b633.md) reaches
the automatic fill hint, then exceeds the first cold 240-second deadline. Warm
import and later CLI/MCP checks are not reached. Neither the old serialization
defect nor the new hint establishes the timeout cause.

The local Linux container is supported for development and acceptance runs under
emulation, with that environment recorded. Native Linux reference measurements
can follow later. Docker VM interruptions invalidate incomplete runs; they do
not change a test deadline or turn observed failures into passes.

## Outstanding validation on the development branch

| Check | Latest retained scope and result | Evidence |
|---|---|---|
| Latest complete native CI | Frozen `8b8288d`: all ten jobs pass, including 528 workspace tests, 39 solver controls, all three end-to-end phases and 33 matching units in each conformance round. Longest job is 379 seconds. Native-product installation/readiness code is covered; the failed local timing scope and source/real-project acceptance remain separate. | [Native pass](../bench/validation/m7-linux-ci-8b8288d-native-2026-10-11.json) |
| Earlier complete native CI | Frozen `45b7d51`: all ten jobs pass, including 528 workspace tests, 39 solver controls, all three end-to-end phases and 33 matching units in each conformance round. Longest job is 422 seconds. Later installer/source-candidate and real-project acceptance remain separate. | [Native pass](../bench/validation/m7-linux-ci-45b7d51-native-2026-10-11.json) |
| Latest complete local workspace attempt | Frozen `b34b2be`: strict all-target Clippy and Python controls pass; 523/528 workspace tests pass. The positive sysroot response expires at its original 100 ms deadline and four HTTP cases miss their original 250 ms serving bounds. Independent affected sysroot controls pass 7/7, but do not replace this failure or establish its cause. The remaining gate steps and all 39 solver controls pass independently; the complete local gate still fails. | [Complete failed attempt](../bench/validation/m7-linux-core-gate-b34b2be-emulated-2026-10-11.json) |
| Earlier installer workspace attempt | Frozen `2d6539f`: strict all-target Clippy and Python controls pass; 526/528 workspace tests pass. The positive sysroot handoff expires at its original 100 ms deadline, and the HTTP disconnect control fails its initial expected hit. Causes remain unestablished; post-test gate steps are not reached. The subsequent readiness change needs separate validation. | [Complete failed attempt](../bench/validation/m7-linux-installer-gate-2d6539f-emulated-2026-10-11.json) |
| Earlier complete local workspace gate | Frozen `d20d034`: all 528 workspace tests and every remaining gate step pass with the original HTTP limits. Previous timing failures remain retained; no real-project or source-engine acceptance is inferred. | [Complete local pass](../bench/validation/m7-linux-gate-d20d034-emulated-2026-10-11.json) |
| Latest independent local continuation | Frozen `b34b2be`: installed examples, doctests, rustdoc, layering, dependency policy and generated benchmark-page equality pass after the failed gate. All 39 pinned-solver controls pass, including whole-session HTTP replay without Z3 and the restored-mount fault. This is a separate continuation, not a combined gate pass. | [Independent continuation](../bench/validation/m7-linux-core-continuation-b34b2be-emulated-2026-10-11.json) |
| Earlier complete local solver suite | Frozen `d20d034`: all 39 original pinned-solver controls pass independently, including signed whole-session HTTP replay without starting Z3 and the restored-mount fault. | [Independent solver pass](../bench/validation/m7-linux-solver-d20d034-emulated-2026-10-11.json) |
| Current release-product build | Frozen `d20d034`: release build passes; new native service-owned Sail, shim and conformance copies are byte-verified. Old sealed products remain unchanged. No proof or reader qualification follows from the copy. | [Release identity](../bench/validation/m7-linux-product-d20d034-emulated-2026-10-11.json), [ownership setup](native-product-setup.md) |
| Earlier complete native CI | Frozen `c13b410`: every required job passes, including workspace 502/502, solver 39/39, original managed/repair scenarios and 33/33 conformance units in each round. The real mount-restoration control passes. Longest job is 399 seconds. This predates automatic sysroot handoff and does not qualify source reuse, real readers or repair speed. | [Current native pass](../bench/validation/m7-linux-ci-c13b410-native-2026-10-11.json) |
| Earlier complete native CI | Frozen `c5a76f1`: all required jobs pass, including workspace 498/498, solver 38/38, all three end-to-end phases and 33/33 conformance units in each original round. Longest job is 318 seconds. The earlier native solver failure remains retained with unknown exact phase cause. | [Earlier native pass](../bench/validation/m7-linux-ci-c5a7-native-2026-10-11.json), [earlier native failure](../bench/validation/m7-linux-ci-d333-native-failure-2026-10-11.json) |
| Earlier passing native CI | Frozen `f94bbda`: all required jobs pass, including 485 workspace tests, 38 solver controls, legacy/managed/repair end-to-end and 33 conformance units in each of pass-through/fill/warm. Longest job is 368 seconds. This predates the configured HTTP setup fix and does not replace local or real-project acceptance. | [Complete native record](../bench/validation/m7-linux-ci-f94-native-2026-10-11.json) |
| Earlier complete local workspace failure | Frozen `1e8b633`: strict Clippy and 521/524 tests pass; three original HTTP controls exceed 250 ms. The eight-reader case passes. The credential/disconnect case fails before its negative assertions; later gate steps are not reached. Cause remains unresolved. | [Corrected gate failure](validation/m7-http-gate-1e8b633.md) |
| Earlier automatic-setup workspace attempt | Frozen `7da9579`: 520/522 pass; two HTTP controls exceed their unchanged 250 ms lookup budgets. Later gate steps are not reached. The single separate send/body diagnostic passes but leaves the full cause unresolved. | [Original gate failures](validation/m7-http-gate-7da9579.md), [separate diagnostic](validation/m7-http-sendbody-7da9579.json) |
| Earlier passing complete local workspace gate | Frozen `091efd5`: all 502 tests and every remaining gate step pass after the sticky mount-generation guard, storage accounting and metadata-observer controls. The original 250 ms HTTP concurrency fixture passes; this does not establish the cause of the separate earlier failure. | [Complete passing scope](../bench/validation/m7-linux-gate-091e-emulated-2026-10-11.json) |
| Earlier complete local workspace failure | Frozen `e0ca5f5`: 497/498 tests pass; the original concurrent HTTP lookup exceeds its unchanged 250 ms budget. One request coalesces seven readers, but delivery completes too late. Python controls, fmt and strict Clippy pass; later gate steps are not reached. A private source build overlaps; the complete cause is unknown. Production workspace bytes are unchanged from the earlier passing `012707a` scope. | [Preserved failure](../bench/validation/m7-linux-gate-e0ca-failure-emulated-2026-10-11.json), [earlier complete pass](../bench/validation/m7-linux-gate-0127-emulated-2026-10-11.json) |
| Earlier complete local solver suite | Frozen `1e8b633`: all 39 original controls pass independently of its failed workspace gate. This includes pinned Z3, signed HTTP whole-session replay and the real mount-restoration control. Source, full standard, real readers and repair speed remain pending. | [Current solver scope](../bench/validation/m7-linux-solver-1e8-emulated-2026-10-11.json), [earlier passing scope](../bench/validation/m7-linux-solver-7da-emulated-2026-10-11.json) |
| Earlier complete local solver attempt | Frozen `777b86a`: 37/38 pass. The fresh HTTP reader matches direct solver output but starts Z3 instead of replaying only signed remote replies and the silence certificate. A cold phase diagnostic identifies transport construction consuming the deadline. The configured setup fix now passes the complete workspace and solver gates; earlier established-transport failures retain separate scopes. | [Complete failure](../bench/validation/m7-linux-solver-777-emulated-2026-10-11.json), [cold replay investigation](validation/m7-http-session-replay-777.md) |
| Earlier integrated workspace-test attempt | Frozen `749255e`: strict Clippy and 480/483 tests passed with two independent test cases and two compiler jobs. The original four-reader and blocked-worker MCP fixtures passed; two HTTP serving checks and the original five-second job-coalescing control failed. No whole cause is established, and later gate steps were not reached. | [Full failure](../bench/validation/m7-linux-gate-749-failure-emulated-2026-10-11.json) |
| Earlier passing complete Linux workspace gate | Source `5fd7986`, all 439 workspace tests and every gate step passed, including the original HTTP concurrency/serving checks. It covers caller-entry coalescing and the dev/test arithmetic optimization; real-reader and source-candidate acceptance remain separate. | [Gate record](../bench/validation/m7-linux-gate-5fd-emulated-2026-10-10.json) |
| Earlier full workspace-test attempt | Frozen `f6508ba`: Python controls, fmt and strict all-target Clippy passed; nextest ran all 471 tests, with 452 passed and 19 failed. Six HTTP cases, the legacy MCP responsiveness fixture and twelve freshness/lease fixtures failed under their original assertions. Initial coverage failures need acquisition-stage diagnostics; causes are not established. Remaining gate steps were not reached. | [Failure](../bench/validation/m7-linux-gate-f650-failure-emulated-2026-10-10.json) |
| Integrated gate after focused fixes | Frozen `44af609` stopped during strict all-target Clippy on a timeout-retention test assertion lint. The same assertion is corrected in `7fcb720`; its full attempt then passed strict Clippy but failed 9/478 workspace tests under default inter-case scheduling. No full gate or product pass is claimed. | [Failed scope](validation/m7-integrated-gate-44af609.json) |
| Subsequent integrated workspace-test attempt | Frozen `7fcb720`: 469/478 passed, nine failed (seven HTTP/reload cases, legacy MCP and the four-reader 100 ms fixture). Earlier initial freshness/lease coverage failures were not observed. Default eight-CPU scheduling was used rather than the earlier two-case/two-build execution. A complete gate for the latest SDK/freshness changes remains required, with unchanged internal fixture concurrency and all original assertions. | [Full failure](../bench/validation/m7-linux-gate-7fcb-failure-emulated-2026-10-11.json) |
| Earlier frozen installed product | Frozen `b36acc7` explicit Sail/shim binary build completed in 301 seconds. Immutable hashes bind the product and unchanged reader collectors; its workspace gate passes all 485 tests. It predates the configured HTTP setup fix and remains distinct from source/product acceptance. | [Build identities](validation/m7-current-product-build.json) |
| Earlier installed MCP transport | Frozen `b36acc7` passed all five unchanged installed transport/lifecycle controls, including normal EOF exit. The worker was stopped/cancelled before any proof. This does not qualify Current reports, live-reader latency, source allocation or M7 overall. | [Installed smoke](validation/m7-installed-mcp-b36.md) |
| Indexed namespace and watch controls | Exact path predicates pass generated old/new equivalence and all original native mutation/overflow guards. Canonical membership preserves hardlink descriptor replacement. Local queue/setup measurements identify narrower improvements, not full service latency. | [Event filter](validation/m7-namespace-indexed-filter.json), [watch index](validation/m7-native-watch-index.json) |
| Frozen `749255e` HTTP/coalescing follow-ups | One later invocation per exact frozen fixture passed. Original serving/coalescing failures remain unresolved; a passing traced diagnostic does not establish their cause. The initial HTTP lookup assertion now records existing result/phase evidence. | [HTTP scope](validation/m7-http-749-followup.json), [coalescing scope](validation/m7-coalescing-749-diagnostic.json) |
| Freshness publication controls | Exact parser and per-slot notifications pass all 35 focused controls and affected strict Clippy. Live fixture p95 varies from 12.293 to 68.622 ms; neither fixture replaces the failed real-project census. | [Scoped record](validation/m7-freshness-publication.json) |
| Independent HTTP core diagnosis | Unchanged frozen test binary: 9/10 cases pass, concurrent lookup times out before the server's original delay finishes. Complete CPU-versus-scheduling cause remains unresolved. | [Diagnostic](validation/m7-shared-http-core-diagnostic.json) |
| Preceding established-transport gate | Source `a9dc753`, 435/436 tests passed. The verified paused-initiator serving check failed after real transport/trust establishment; response-to-classification took 135.738 ms. Its entire CPU versus scheduling cause remains unknown. Later gate steps were not attempted. | [Failure](../bench/validation/m7-linux-gate-a9-failure-emulated-2026-10-10.json) |
| Subsequent signature CPU controls | Sixteen strict valid and forged checks took approximately 310/320 ms before package-only dev/test arithmetic optimization, and 4–7 ms afterward. All 10 HTTP cases, five signed-format negative cases and affected strict Clippy checks passed, followed by the complete `5fd7986` gate. This does not identify the original failure's entire cause or establish a release-build speedup. | [Diagnostic](validation/m7-shared-signature-cpu-diagnostic.json) |
| Earlier cold-lookup gate | Source `d12d4af`, 431/436 passed; five HTTP fixtures failed during cold lookups. Original deadlines and assertions are retained. | [Failure](../bench/validation/m7-linux-gate-d12-failure-emulated-2026-10-10.json) |
| Earlier complete Linux workspace gate | Source `62ad095`, 436/436 tests plus all gate steps passed; two test workers | [Record](../bench/validation/m7-linux-gate-62ad-emulated-2026-10-10.json) |
| Earlier real solver/fault suite | Source `5fd7986`, all 37 tests passed in 182.337 seconds; the earlier timed-out shared-entry case passed in 13.751 seconds with its original 180-second test and 60-second session limits | [Record](../bench/validation/m7-linux-solver-5fd-emulated-2026-10-10.json) |
| Latest complete local end-to-end attempt | Frozen `777b86a`: legacy output, JSON/MCP, seed-4 portfolio and all five installed MCP transport controls pass; first managed cold verification exceeds its unchanged 240-second limit. Partial dependency-build stderr is retained; cause remains unestablished. Repair was not reached. | [Complete failure and raw streams](../bench/validation/m7-linux-e2e-777-failure-emulated-2026-10-11.json) |
| Subsequent standalone managed acceptance | Frozen `012707a`: the unchanged complete managed script passes cold verification in 216.080 s, unchanged reuse in 31.947 s, original CLI/MCP assertions, missing-receipt recovery and a restored-mtime failing edit. No proof/dependency prewarm or deadline changes. It does not replace the earlier aggregate failure or qualify real-project repair/read latency. | [Complete managed result and setup timings](validation/m7-managed-setup-012707a.md) |
| Earlier complete end-to-end attempt | Source `5fd7986`: legacy output, JSON/MCP and cold/warm seed-4 portfolio checks passed; the first cold managed verification timed out at its unchanged 240-second limit. Repair was not reached. Default fixture scratch cleanup removed its inner stdout/stderr evidence, so the timeout's cause is not established. | [Failure](../bench/validation/m7-linux-e2e-5fd-failure-emulated-2026-10-10.json) |
| Subsequent isolated repair phase | Same frozen `5fd7986` product and selected runtime: the first repair job remained running at the unchanged 120-second workflow limit. Retained worker stderr ends in dependency compilation; no complete result or later scenario was admitted. All 331 CLI operations returned within their bounds; MCP was not reached. Cleanup reported no errors. | [Failure](../bench/validation/m7-linux-repair-5fd-phase-failure-emulated-2026-10-10.json) |
| Subsequent HTTP-labelled diagnostic suite | Frozen `987aa39`: 9/12 passed, three shared-reader checks failed with their original bounds; the separately selected direct stalled-response case passed at 119.775 ms. Serial case scheduling retained all internal concurrency. No complete workspace gate or causal resolution is claimed. | [Diagnostic](validation/m7-http-987-followup.json) |
| Subsequent complete workspace gate | Frozen `7dc855d`: 437/439 tests passed; verified paused-initiator delivery took 293.834 ms against 250 ms, and cold constructor plus a stalled request failed its original 250 ms elapsed assertion. No deadline or cryptographic check changed; the entire causes remain under investigation. Later gate steps were not reached. | [Failure](../bench/validation/m7-linux-gate-7dc-failure-emulated-2026-10-10.json) |
| Preceding complete real solver/fault suite | Source `d12d4af`, all 37 tests passed in 114.446 seconds; the earlier timed-out shared-entry case passed in 9.652 seconds with unchanged assertions | [Record](../bench/validation/m7-linux-solver-d12-emulated-2026-10-10.json) |
| Earlier real solver/fault suite | Source `62ad095`, 36/37 passed; `unverifiable_shared_entries_never_change_an_answer` timed out at the unchanged 180-second test limit; original cause remains unresolved | [Failure](../bench/validation/m7-linux-solver-62ad-failure-emulated-2026-10-10.json) |
| Isolated shared-entry fault diagnostic | All five corruption cases and both sessions passed in 11.340 seconds; this does not explain or erase the full-suite timeout | [Diagnostic](../bench/validation/shared-signature-fault-diagnostic-linux-emulated-2026-10-10.json) |
| Full default corpus | Frozen `1dc` passed all 147 fast units in pass-through, fill and warm; standard phase interrupted by container loss and lacks a complete report | [Record](../bench/validation/m7-default-corpus-1dc-interrupted-emulated-2026-10-10.json) |
| Earlier complete standard attempt | Frozen `1dc`, default mode: 159 units, 156 Same, Anvil diagnostic ordering Unstable, zero Different, two Verismo infrastructure timeouts at the original 3,600-second limits. Pass-through fails; fill/warm are not reached. Earlier sleeping-thread observations do not establish a cause, and this old harness discarded inner timeout streams. | [Complete failure](../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json), [earlier observation](validation/m7-standard-verismo-timeout-observation.json) |
| Latest complete standard attempt | Frozen e0ca harness / 012 shim, default pass-through: all 159 units terminal, 158 Same, zero Unstable/Different, one original 3600-second plain tspec infrastructure timeout. 12,479 functions compared; zero rlimit drift or skipped projects. Complete failed report retained; fill/warm not launched. | [Complete failure](../bench/validation/m7-linux-default-standard-e0ca-failure-2026-10-11.json), [closure](validation/m7-default-standard-e0ca-terminal.md) |
| Selected tspec transcript diagnostic | Original plain unit, with only stock transcript logging added, interrupted by actual EROFS and container exit before a unit result. All 542 transcript prefixes are recovered unchanged; complete logged pairs do not identify the blocked userspace operation. No retry or acceptance credit. | [Interrupted scope](validation/m7-tspec-transcript-interrupted.md) |

These records cover their frozen artifacts, not every later source candidate.
The passing native suite and failed local suite have separate frozen identities;
neither qualifies later source changes or identifies the earlier timeout's cause.
HTTP serving/coalescing assertions and their original deadlines remain unchanged;
the subsequent `5fd7986` gate passed those assertions while both earlier failed
runs remain recorded as failures.
The separate HTTP follow-up retains signature CPU controls and a negative
TCP_NODELAY control; neither establishes the full cause of the two `7dc855d`
failures. The stalled-request fixture now records constructor, request and
client destruction costs within its original elapsed bound. See
[the follow-up record](validation/m7-http-7dc-followup.json).
A complete standard run and the
remaining M7 source/product gates are still required before merge acceptance.

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists all locally recorded upstream
issues, reproductions and workarounds: declaration and diagnostic ordering,
partial or misleading structured results, ambiguous function names, Cargo
reverification/logging behavior and release packaging. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage.

Z3 4.16.0 can vary nonlinear rlimit consumption on identical input. The local
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md),
[solver survey](survey/solver-behavior.md) and
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md)
describe unstable outcomes. Process history also affects models and effort, so
default SMT keys bind the exact process prefix. Seed-dependent proof discovery
is exposed through named opt-in portfolios, with separate keys and provenance;
it is never silently substituted into default verification.

## Earlier complete solver setup and replay failures

The frozen b36 workspace gate passes all 485 tests. Its complete solver step
passes 37 real-Z3 and fault controls, but the new classifier timing diagnostic
fails before assertions because its external fixture was absent. The original
captured bytes are now retained as its default; explicit fixture selection and
all bounds/equality assertions remain. That attempt required fresh complete validation.
[The failed attempt](../bench/validation/m7-linux-solver-b36-emulated-2026-10-11.json)
retains the entire log. This is a test setup defect, not a solver disagreement.

The fresh complete solver run passes the corrected fixture, but fails the
original HTTP whole-session replay assertion: correct answers required one
extra Z3 process (two starts before, three afterward). The failed native cache,
signed replies and certificate are retained for diagnosis. Cause is unresolved;
this full run remains failed. The later complete 38-control passes in the table
above retain separate identities and do not erase this failure. See the
[complete fresh attempt](../bench/validation/m7-linux-solver-777-emulated-2026-10-11.json).

## Unselected imported-span relocation candidate

The f833 source diagnostic identifies a genuine previous-revision focus miss:
Imported id563 in vstd div_mod keeps its original words/content/name/identity,
while all454 foreign contributing starts move25. Complete inventories reproduce
the expected/current digests. Actual Loaded/Delayed resolver state was not
observed in that trace. The earlier strict immediate-progress diagnostic-order
failure and c05 cross-edit failure remain retained; this causal finding is not
proof or performance qualification.

The private [Loaded authority candidate](../engine/verus/candidates/loaded-foreign-authority/README.md)
and [proposed ADR0030](decisions/0030-imported-span-authority.md) add conservative
original→current mapping authority, explicit state evidence and new-domain
migration refusal. Delayed/local/unknown states remain misses. Compiler/runtime,
complete source, genuine previous-revision FOCUS and HE/Vest gates are pending.
Selected runtime pins are unchanged.

The [focused Verismo timeout diagnosis](validation/m7-verismo-diagnostic-plan.md)
ran one stock unit once with the frozen e0ca harness retaining partial
streams. Bitflags was Same:48verified/0errors on both sides, with no timeout
reproduced. [The terminal record](validation/m7-verismo-bitflags-e0ca-diagnostic.json)
retains streams and bounded metadata;18process rows had explicit descriptor
truncation. Both complete standard timeouts and their unknown causes remain. This is neither a suite retry nor an optional-engine acceptance result.

## Shared-cache foreground filesystem boundary

The previous lookup implementation read the signing-key directory before its
helper timeout; startup and status also loaded or tested remote configuration on
the foreground. A blocked filesystem could therefore bypass the documented
250 ms fallback bound. [Issue 76](https://github.com/Chelis-Lang/sail/issues/76)
tracks this code-level defect. The correction moves those reads onto bounded
owned helpers and uses optional status snapshots; see
[the current behavior](http-shared-cache.md) and
[ADR 0017](decisions/0017-shared-tier-with-signed-entries.md).
This is not a measured explanation or waiver for the retained b34/a6 HTTP timing
failures. Original serving, signature, disconnect and concurrency checks remain
required, as does a new complete integrated gate.
The [focused validation](validation/m7-shared-trust-foreground.md) retains all
failed drafts and demonstrates three paused blocking counterfactuals, final
23 shared-unit/26 integration controls and strict checks. A subsequent passing
reader diagnostic does not explain or erase the retained eight-reader timeout.

## Separate detailed sysroot diagnosis

One separately frozen [cause diagnosis](validation/m7-sysroot-proof-diagnostic.md)
completes within its original 600-second proof budget with an accepted Current
full result, two actual invocations and no reuse. All 1,871 vstd and 331 HumanEval
full obligations verify; the focus remains provisional. No sysroot loss is
reproduced, so the original D20 failure's cause remains unestablished. No live
reader or retained integration test follows from this diagnostic.

The frozen `37e5342` full workspace run failed the independent sysroot handoff
fixture's single preparation assertion (540/541 passed), with the actual refusal
reason uncaptured. A test-only startup gate now preserves that assertion while
coordinating maintenance's first state-lock acquisition. Controlled contention
shows production preparation and lookup safely decline busy state; this does
not establish the old failure's cause. See the
[fixture control scope](validation/m7-sysroot-fixture-startup.md). The separate
original solver suite passed 39/39, and the earlier `4472279` timeout is retained.

The later [complete local workspace gate](../bench/validation/m7-linux-core-gate-ec9b682-emulated-2026-10-11.json) passes 543/543 and every remaining step with the same production contracts; the immediately following [39/39 solver suite](../bench/validation/m7-linux-core-solver-ec9b682-emulated-2026-10-11.json) also passes. It does not establish the cause of either earlier failure or qualify source reuse, readers or repair performance.

## Current real-project full-result read latency

The [original current-product reader run](validation/m7-current-reader-workflow.md) establishes genuine complete Current full proofs and admits all 200 Python RPC samples while the same selected verifier remains active. Status p95 is 148.430 ms; full Result p95 is 394.713 ms, failing the unchanged 200 ms requirement. Each Result contains 4,312,245 bytes. Python receives and parses the complete fresh frame and does not use the Rust decoded-result cache. Server frames are already cached, but every read still validates persisted output and current inputs. Transfer, validation and parsing costs are not separated by this run; no cause is inferred. Rust and retained CLI/MCP were not attempted after the original failfast failure. All failed scopes, raw samples and post-run seals remain retained.

## Native-binding workflow prerequisite remains unresolved

The [single original reader workflow on `46e1da6`](validation/m7-rpc-binding-reader-terminal.md)
produces independent complete Current preparation proofs, then exceeds the
unchanged 600-second baseline ordinary-full limit. Neither reader census nor
retained integration runs. Empty stdout, a HumanEval start event and partial
process metadata do not establish the cause. The partial snapshot followed only
main-task children and missed possible non-main-thread descendants; use the
existing bounded all-task observer for future diagnostics. Earlier measured
Result latency failures remain separate, with no latency credit from this run.
The [all-task observation controls](validation/m7-reader-process-observation.md)
now pass on Linux, including a real child launched by a non-main thread and
queued PID reuse refusal. They improve future diagnostic coverage; they do
not recover missing historical state or establish a timeout cause.

The separate [focused ordinary diagnostic](validation/m7-ordinary-wait-diagnostic-terminal.md)
completes with a full Current result in 87.054 seconds, preserving that same
ordinary context, revision and 600-second limit. Its one HumanEval invocation
uses the daemon cache: 113 completed shim records report 4,410 replies served
and no solver process started. All post-run seals match. The three bounded
process snapshots are explicitly partial. The historical timeout remains
unexplained, and no reader latency samples or acceptance are supplied by this
successful diagnostic.

## Local diagnostic integration gate deadline failures

The [complete local gate on `5b8dd6f`](../bench/validation/m7-linux-diagnostic-gate-5b8dd6f-failure-2026-10-11.json)
passes all new pure diagnostic controls and strict linting, then fails eight of
566 workspace cases; later gate steps are not reached. Production Rust is
unchanged from the passing `46e1da6` and `55d14c8` scopes. The one focused run of
the same eight original cases passes seven and still fails the HTTP refresh
fixture: its validated reply is published 253.329 ms after lookup begins, past
the unchanged 250 ms serving deadline. The whole failed gate remains failed.
Actual tested binaries and raw phase clocks are retained. These observations
do not establish a scheduling, transport or implementation cause; no assertion,
timeout, payload or admission rule has changed. The previous native CI passes
remain separate from this local failure.

## Rebuilt selected-engine repair integration deadline

The [original real CLI/MCP repair workflow on `071b778`](../bench/validation/m7-linux-repair-071b778-failure-2026-10-11.json)
fails its unchanged 120-second whole-workflow budget. The successful proof takes
50.807 seconds and its successful declaration-establishment follow-up takes
12.247 seconds. The missing-function job remains Running during the remaining
approximately 56 seconds. Regression and later cases are not reached. Its
Running projection has no input revision or repair result; that does not locate
the internal wait or establish a deadlock.

All 728 retained operation/summary/stream files are archived. The original
fixture removes its temporary project during cleanup, so unavailable worker
state cannot be reconstructed from that attempt. The separate [reduced diagnostic](../engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/ordinary/repair-selector-diagnostic/README.md)
preserves its worker streams but also fails the original total budget, during
the first successful-proof attempt before the missing selector is submitted. The full integration remains failed, separate from the passing
thirteen-stage source qualification and artifact installation. No deadline or
assertion was changed.

## Local Docker filesystem failure during selected-engine validation

The complete selected-source local gate on 2026-10-11 completes all 566
workspace cases with 342 passes and 224 failures. Temporary-file and lock
creation report `EROFS`; independent private writes to both container overlays
and the shared target also fail. Later gate steps are unattempted. This is a
failed gate, with raw evidence retained in the [validation record](../bench/validation/m7-linux-selected-source-gate-a8bdac6-environment-failure-2026-10-11.json),
not a successful product qualification. Individual failure causes beyond the
recorded errors are not inferred. After external Docker recovery, private
write probes succeed again. The selected image export was cancelled for low
space and was never loaded; the running stock diagnostic was interrupted
without a harness terminal result or a certified process cleanup. Retained
engine runtime files and original test limits remain unchanged.
The separate [post-recovery full gate](../bench/validation/m7-linux-selected-source-gate-0155b33-emulated-2026-10-11.json)
passes all 566 workspace tests and every remaining gate step. This establishes
that recorded integration scope; it does not reclassify the earlier failure or
qualify the later compiler-family and reader component changes.

## Repair workflow after compiler-family reuse

The [rebuilt compiler-family product](../bench/validation/m7-linux-product-cdb6e6d-compiler-family-2026-10-11.json)
installs successfully, but the [original repair workflow](../bench/validation/m7-linux-repair-cdb6e6d-failure-2026-10-11.json)
still exceeds its unchanged 120-second budget. Its successful Current proof and
Current declaration follow-up take 63.905 and 28.570 seconds from worker start
to completion. Each uses one compiler invocation. The missing-function job has
roughly 27 seconds left when submitted and remains Running at the deadline;
regression and later cases are unattempted. Managed-run times of 57.632 and
20.078 seconds are contained in those job times, not additional intervals.
This does not establish a missing-selector deadlock or an internal phase cause.
The fixture removes worker state during cleanup; all operation evidence is
retained separately. Compilation reuse and its targeted controls do not qualify
the repair loop or performance.

A separate [reduced prefix diagnostic](../bench/validation/m7-linux-repair-prefix-diagnostic-cdb6e6d-2026-10-11.json)
completes in 77.046 seconds internally (79.816 externally), retaining the original
success, declaration-establishment, event/resume, baseline and missing-function
assertions and 120/30/5-second limits. Its three jobs take 34.707, 12.516 and
29.042 seconds. The missing selector records unavailable focused evidence,
completes Current full verification and rejects repair with `SAIL3122`.
It establishes completion in that diagnostic scope; it neither replaces the
original full-workflow failure nor establishes its cause. Bounded process
snapshots show ordinary dependencies compiling during the selector change, but
their inventory is explicitly limited. Regression and later CLI/MCP cases were
not part of this diagnostic.

The later [native CI record](../bench/validation/m7-linux-native-ci-b56b0ff-repair-2026-10-11.json) passes the original complete small repair
CLI/MCP fixture in 45.847 seconds on native Linux, with all 167 operations
within their original bounds, no progress failures or cleanup errors, and the
same selected runtime manifest. It includes regression rejection, explain,
compare, MCP and late-execution cases. This closes that CI fixture scope; it
does not erase the earlier emulated `cdb6e6d` timeout or establish its cause.
Real HumanEval/Vest performance, complete current standard/cache coverage and
full-result reader latency remain open.

## Ordinary compiler-family script rebuilding

A [reduced real repair diagnostic](validation/m7-compiler-script-observations.md)
shows repeated ordinary dependency compilation after changing only the source
selector. Its old context-local ledger cleans registry build scripts even in the
same compiler-artifact family. The optional family-generation change validates
fresh ordinary contracts and actual output bytes, consumes certificates before
mutation, and checks all output units after native guard acquisition. Incomplete
or unavailable contracts still clean. This is a product optimization awaiting
whole-loop validation, not an explanation or erasure of the original 120-second
failure, a transferred proof receipt, or a measured M7 speedup.

## Combined solver cache-maintenance timeout

The [current combined solver suite](../bench/validation/m7-linux-combined-core-solver-c97de63-failure-2026-10-11.json)
runs all 41 original cases: 40 pass and `gc_and_clear_during_sessions_only_cost_misses`
times out. Its cold and warm sessions and maintenance join finish within about
7.174 seconds; final concurrent sessions do not finish before the original
nextest timeout. The reported case duration includes termination grace. An
observation sees one shim stdout thread waiting on pipe output, but it does not
establish the underlying cause or attribute it to the new result-serving path.
Raw logs and the failure are retained; diagnosis is required before claiming a
green solver gate or product qualification.

### Fault-test timeout cleanup limitation

The `c97de63` combined source gate passes 605 tests, while its complete solver
scope fails with 40 of 41 passing: GC/clear stalls in final concurrent sessions.
The [isolated diagnostic](validation/m7-gc-c97-isolated-diagnostic.md) passes in
8.78 seconds and does not identify or replace that failure. A separate
cleanup-first test-support correction avoids blocking the timeout kill on the
output-buffer mutex. It refuses incomplete reader delivery and reports bounded
retirement failure truthfully. A blocked Rust reader cannot be forcibly canceled;
its detached lifetime remains owned by the test process. This is a fixture
cleanup limitation, not evidence of a production cache defect or its absence.

### Combined product's emulated repair deadline

The [rebuilt combined product attempt](../bench/validation/m7-linux-combined-repair-a9609e9-failure-2026-10-11.json)
fails the unchanged 120-second complete workflow at 120.266 seconds internally.
Success, its declaration-establishment follow-up, events, cursor resume and
baseline complete; missing-function validation remains unfinished, and later
cases are unattempted. Public worker streams report a 66-second initial Cargo
build and a 4.71-second follow-up build. Those observations do not establish
the missing-function phase cost or a performance improvement. All 734 retained
raw archive members are independently verified; progress and cleanup report no
errors. The earlier complete native small-fixture pass stays separate.

The [first cleanup-control integration gate](../bench/validation/m7-linux-cleanup-control-gate-6b1edb1-failure-2026-10-11.json)
fails strict Clippy on a redundant test-only closure. The direct method spelling
is corrected; neither this lint failure nor its correction identifies the
historical cache-maintenance stall. Tests and solver were unattempted in that
failed gate scope.

### Shared HTTP reader deadline in the combined gate

The [b63 complete local gate](../bench/validation/m7-linux-combined-core-gate-b63af10-failure-2026-10-11.json)
passes strict Clippy and 606 of 607 workspace tests. The concurrent HTTP-reader
case fails: all eight readers miss the original 250 ms deadline, although they
coalesce into exactly one GET. The fixture receives complete request headers
and sends its response after its unchanged 75 ms delay. Retained stage timings
show the response near the first caller's deadline; they do not identify a
causal bottleneck or attribute the failure to emulation or machine load.
The remaining gate steps and pinned-solver suite are unattempted in this scope.
This failure remains separate from the earlier cache-maintenance timeout and
its passing focused cleanup controls. No deadline, fixture delay, reader count
or all-hit assertion has been weakened.

### Emergency read-only filesystem interrupted the combined runs

During the separate [b63 solver run](../bench/validation/m7-linux-combined-core-solver-b63af10-failure-2026-10-11.json),
39 of 41 cases pass, including cache maintenance. The attribution tests then
fail on a missing obligation and temporary-directory creation. Independent
process launches fail to create emulator temporary files, a repair diagnostic
reports `EROFS`, and `/proc/mounts` records ext4 `emergency_ro`. This establishes
an infrastructure failure; the two test assertions do not preserve enough IO
detail to establish their individual causes. The frozen benchmark also stops
on `EROFS` before any measured sample. Docker restart restores independently
checked writes and fsync in both Sail containers.

Reusing the interrupted benchmark's old pathname leaves its externally named
socket in place; the original runner refuses it before starting a daemon.
The launcher now supports an explicit fresh `--run-id` namespace, retaining each
failed attempt and isolating work, caches and socket selection. It performs no
automatic retries or proof-state transfer. Old process birth grants are not
reused after restart, and no descendant-closure claim is made for the interrupted
workflows.
