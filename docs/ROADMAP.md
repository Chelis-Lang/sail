# Roadmap

The project is built in waves. Each milestone ends with evidence: green gates,
conformance results and, where relevant, benchmark data. Work widens only after
the previous milestone is shown to work.

## Initial session (foundation): done, 2026-10-08

See `docs/reports/2026-10-08-bootstrap.md` for the evidence.

- **M0: Workspace and guardrails.** The crate skeleton from the charter, the fast
  CI tier, lints, the AI-authorship guard (hook, CI and agent config), docs, and
  ADRs 0001–0002. *Exit:* both CI jobs green on `main`, the guard rejects an AI
  trailer locally and in CI, then switch to PR mode.
- **M1: Survey and integration decision.** `docs/survey/` covers Verus↔Z3
  internals for the pinned release, prior art, corpus candidates and solver
  behaviour. The pinned Verus is installed via `cargo xtask fetch-verus`. *Exit:*
  an ADR on the integration point, backed by an empirical transcript.
- **M2: Faithful pass-through.**
  - `sail-smtlib`: a byte-faithful parser and printer.
  - `sail-session`: assertion stack, options and the logical state at each
    `check-sat`.
  - `sail-z3` (`sail-shim`): forwards every byte to the pinned Z3, with an
    observation-only tap and transcript capture.

  *Exit:* a toy crate verifies identically with and without the shim.
- **M3: Corpus, conformance, baseline and profile.**
  - Pinned corpus.
  - `sail-conformance` diffs plain Verus against Verus-with-shim.
  - `sail-bench` baseline and generated `docs/benchmarks.md`.
  - Query corpus from shim transcripts, with a profile of solver time per query
    and function and of seed stability.

  *Exit:* zero verdict or diagnostic differences in pass-through mode.
- **M4: Report.** Survey findings, the integration evidence, baseline and
  profile, conformance results, and open questions.

## Initial product: complete, 2026-10-09

Milestone M5 and `docs/reports/2026-10-09-initial-product.md` record the
completion decisions and full local acceptance. The owner kept the unknown
serving cap at 50%; the later research below remains deferred.

## M6: Live workspace verification and team caching (implemented)

This milestone implements a complete local developer and agent
workflow. Ordinary single-developer use needs no credentials, server setup,
account, or container. One verification worker runs by default.

1. **Managed workspace verification** ([#64](https://github.com/Chelis-Lang/sail/issues/64)).
   Always invoke Cargo; retain complete successful per-target proof receipts in
   Sail-owned artifacts. Bind them to the exact verification context and actual
   artifacts. Return complete reports when Cargo reuses verified targets, and
   refresh missing or incompatible evidence.
2. **Persistent jobs, watch and MCP** ([#65](https://github.com/Chelis-Lang/sail/issues/65)).
   Submit, inspect, follow and cancel verification across sessions. Watch the
   workspace and local path dependencies; distinguish current results from
   historical or superseded work. Result lookups stay responsive during runs.
3. **Optional HTTP team cache** ([#66](https://github.com/Chelis-Lang/sail/issues/66)).
   Read and publish signed SMT entries over authenticated HTTPS, with an
   optional lightweight self-hosted service. Reader signing trust remains
   independent of transport authentication; remote failures cost misses.

Acceptance is retained in `docs/reports/2026-10-10-live-workspace.md`: complete
unchanged reports with zero verifier invocations, native CLI/MCP and recovery
checks, exact same-name library/binary solver inputs, full local corpus coverage,
and three-round secondary-platform resource measurements. Easy synthetic
fixtures show freshness overhead, not a general wall-time gain. Follow-ups are
safe freshness-cost reduction ([#69](https://github.com/Chelis-Lang/sail/issues/69))
and an unresolved emulated verifier wait
([#68](https://github.com/Chelis-Lang/sail/issues/68)). No deadline or check was
relaxed to classify a failed attempt as success.

Exit evidence: unchanged eligible workspaces invoke no verifier and return
complete reports; edits and verification-context changes invalidate correctly;
job cancellation/restart/currentness work across CLI and MCP; a fresh remote
reader consumes verified signed replies and silence certificates. Run full
local gates and affected conformance, with three-round performance and resource
measurements. Long tests stay local-only; every required CI job remains below
ten minutes. Persistent Verus internals, solver pools, normalization, stack-only
keys and new SMT backends remain deferred.

## M7: Interactive proof engine and repair workflow (in progress)

The approved contract is [ADR 0027](decisions/0027-interactive-proof-engine-and-repair-workflow.md).

- Optional pinned source engine: retain one compiler callback for a focused
  check followed by independent full validation; reuse conservative prepared
  lowering without retaining compiler state across source revisions.
- CLI/MCP repair checks, failure evidence, saved named baselines and comparison.
- A separate live proof-event journal with revision-aware projection and
  cancellation/reconnect handling.
- Shared complete input observation and responsive job reads, addressing #69.

Exit: exact stock full-result/diagnostic conformance and cached-versus-fresh
solver bytes from the same compiler input (ADR 0027 amendment); full local corpus
and affected source tests; established reads below 200 ms p95 during active
verification; and at least **2× median improvement on each** of frozen
human-eval-verus and Vest repair catalogues in the local Linux container.
The owner will rerun reference measurements on a native Linux host later;
container emulation is labelled and does not block implementation. Three
rounds and spread are required. Batch-local preparation reuse is distinguished
from persistent reuse across edits. No successful focus is accepted before
complete current full validation. M7 remains incomplete until these gates pass.

The private [Loaded import authority candidate](../engine/verus/candidates/loaded-foreign-authority/README.md)
addresses the measured foreign-coordinate relocation miss under proposed
[ADR0030](decisions/0030-imported-span-authority.md). Implementation and new-domain
controls exist; compilation, actual resolver-state evidence and complete source/
previous-revision proof gates remain pending. It is unselected and grants no
M7 performance or acceptance claim.

Its later `b46efd05` build passes source unit and strict lint checks, but the
original qualification fails on ordered progress diagnostics and records zero
restored preparations. The actual first refusal is an imported span whose
descriptor remains Delayed at both capture and focus. A separately reviewed
[DeferredLocator proposal](../engine/verus/candidates/deferred-import-locator/README.md)
and [consumer audit](../engine/verus/candidates/decode-ownership/deferred-locator-consumer-audit.md)
now guide a bounded implementation. This role preserves an exact diagnostic
recipe and leaves current compiler resolution untouched; actual resolver,
ownership and complete source controls are still required. The failed candidate
remains unselected.

### Current state

The active priorities are complete product workflows, with unchanged acceptance
targets. Historical passes below establish only their recorded scopes.

Current implementation work advances two blocked paths. The
[single late imported-authority handoff](../engine/verus/candidates/deferred-import-locator/late-handoff/README.md)
is implemented as an unselected source candidate: 43 reduced actual pinned API
controls pass, and independent review confirms immutable recipes, strict role
classification and bounded constructor scans. The
[complete pinned source controls](validation/m7-late-import-handoff-source-controls.md)
pass 40 VIR and 78 driver tests plus both strict all-target checks in 180.872
seconds. The ordinary runtime build now passes in 637.838 seconds, including
fresh verification of all 2,060 vstd obligations. Its thirteen runtime artifacts,
seven changed source files and source lock are sealed; independent review and
the unchanged fourteen qualification stages follow. The candidate remains
unselected until genuine previous-revision focused reuse passes.

The reader path now moves MCP response graphs and encodes outside its output
lock, with [46 targeted controls](validation/m7-mcp-output-ownership.md) passing.
The daemon retains cache charges through evicted in-flight output and accounts
for buffer capacity. The [final admission correction](validation/m7-result-frame-admission-margin.md)
reserves 6,992 bytes of retirement metadata inside the existing 128 MiB ceiling
and releases unused variants on uncached replies; all 89 daemon controls and
strict checks pass. Neither result proves the live reader latency threshold.
Bounded optional
[RPC stage timings](result-rpc-diagnostics.md) are implemented, with eight scoped
controls and strict affected checks passing, to diagnose the retained full Result
failure without changing collectors or freshness checks. The combined
[workspace gate](../bench/validation/m7-linux-core-gate-76a6976-emulated-2026-10-11.json)
passes 561 tests and all remaining checks; its separate
[solver run](../bench/validation/m7-linux-core-solver-76a6976-emulated-2026-10-11.json)
passes all 39 controls. These runs precede the final admission correction. A
complete gate and solver run on that correction is underway, followed by a new
sealed CLI/shim build and the unchanged real-reader workflow with bounded stage
timings. The last pushed
[native CI revision](../bench/validation/m7-linux-ci-08435c5-native-2026-10-11.json)
passes every check in 442 seconds. The full Result p95 failure at 394.713 ms
remains open; local component controls do not replace the live acceptance gate.

| Workstream | Implementation and current validation | Remaining product gate |
|---|---|---|
| Repeat verification setup | The actual pinned-sysroot control passes: natural publication, all 327 eligible hashes imported before local hashing, and exact context/ordered identity equality. The corrected product's original cold scenario exceeds 240 seconds. | Pass the original cold/warm managed CLI/MCP scenario, including missing receipts and restored-mtime edits; the debug context control is not proof acceptance. |
| Reuse after a source edit | The unselected DeferredLocator candidate built successfully, verified 2,060 vstd obligations and passed all 38 VIR and 55 driver tests. Strict VIR checks pass; strict driver Clippy rejects a test-only `Arc<SourceMap>`. Its test-only `Rc` correction also builds fresh vstd, but the driver test fails pinned rustc’s `DynSend` emitter bound. The actual compiler API control confirms `DynSend` requires `Arc`; the reviewed binding-local lint exception preserves all assertions. Its [new private source run](../engine/verus/candidates/deferred-import-locator/dynsend-owner/source-controls-pass.json) passes all 38 VIR and 55 driver tests and both strict all-target checks. The [ordinary runtime build](../engine/verus/candidates/deferred-import-locator/dynsend-owner/ordinary-build-pass.json) passes fresh vstd (2,060 verified, zero errors). An independent read-only review verifies all thirteen runtime hashes, seven source files, the lock and build-actor closure. The [original fourteen-stage qualification](../engine/verus/candidates/deferred-import-locator/dynsend-owner/qualification-failure.json) passes eight stages, then fails revision 0 of previous-revision focus: zero checked hits and four fresh provenance refusals, while stock verdicts and exact diagnostics still match. Later revisions and stages were not reached. The [separate bounded trace](../engine/verus/candidates/deferred-import-locator/dynsend-owner/provenance-diagnostic-result.json) is complete: five Loaded imports are eligible before a Delayed descriptor refuses admission. It does not record the refused filename or Foreign-match count. The distinct [original-descriptor candidate](../engine/verus/candidates/deferred-import-locator/original-descriptor/README.md) is implemented under a new v7 domain. Its actual pinned API and sealed-vstd metadata control pass 30/30 with zero source loading; this establishes a legal recipe in a controlled empty Foreign namespace. Its [actual source controls](../engine/verus/candidates/deferred-import-locator/original-descriptor/source-controls-pass.json) pass 39 VIR and 64 driver tests plus both strict all-target checks. The [fresh ordinary runtime build](../engine/verus/candidates/deferred-import-locator/original-descriptor/ordinary-build-pass.json) verifies 2,060 vstd obligations. Its [unchanged fourteen-stage qualification](../engine/verus/candidates/deferred-import-locator/original-descriptor/qualification-failure.json) passes eight stages then fails previous-revision focused reuse with zero hits. The [separate provenance diagnosis](../engine/verus/candidates/deferred-import-locator/original-descriptor/provenance-diagnostic-result.json) observes Foreign hash matches growing from zero to one before focus, causing the existing late guard to refuse. A [single compiler handoff](../engine/verus/candidates/deferred-import-locator/original-descriptor/handoff-design.md) is now being implemented: unclassified bounded constructor seeds preserve their exact initial resolver recipe, then undergo the unchanged strict role checks once after frontend work. No runtime selection or cross-edit reuse is inferred. | Pass the corrected complete source gate and original fourteen-stage qualification, including genuine previous-revision focus reuse within 600 seconds. |
| Shared cache | The corrected loopback refresh and private-CA rejection pass the [complete local gate](../bench/validation/m7-linux-gate-d20d034-emulated-2026-10-11.json): 528/528 workspace tests and every remaining step. The separate [39/39 solver suite](../bench/validation/m7-linux-solver-d20d034-emulated-2026-10-11.json) also passes. All original timing controls retain their bounds. The later [installer gate](../bench/validation/m7-linux-installer-gate-2d6539f-emulated-2026-10-11.json) passes 526/528 tests but fails the positive sysroot handoff and initial HTTP hit controls; causes remain unestablished. The independent readiness checks pass 7/7 with strict affected-crate Clippy. The later [complete current gate](../bench/validation/m7-linux-core-gate-b34b2be-emulated-2026-10-11.json) passes 523/528 tests but still fails the positive sysroot response and four HTTP serving controls at their original bounds; causes remain unestablished. The remaining checks and 39/39 solver controls [pass independently](../bench/validation/m7-linux-core-continuation-b34b2be-emulated-2026-10-11.json); the complete local gate still fails. The [new coverage-diagnostics gate](../bench/validation/m7-linux-core-gate-a6cb0a5-emulated-2026-10-11.json) passes 530/532 controls but fails two original HTTP bounds; all remaining checks and 39 solver controls [pass independently](../bench/validation/m7-linux-core-continuation-a6cb0a5-emulated-2026-10-11.json). The [bounded trust/configuration correction](validation/m7-shared-trust-foreground.md) addresses foreground filesystem reads under issue #76 and passes 23 focused shared controls, 26 integrations and strict Clippy. The [combined current workspace gate](../bench/validation/m7-linux-core-gate-4472279-emulated-2026-10-11.json) passes 540/540 tests and every remaining step. Its [original solver suite](../bench/validation/m7-linux-core-solver-4472279-failure-emulated-2026-10-11.json) passes 38 controls but times out in concurrent cache maintenance at the unchanged 180-second bound; cause remains unestablished and lifecycle diagnosis is underway. The subsequent [test-cleanup gate](../bench/validation/m7-linux-core-gate-37e5342-failure-emulated-2026-10-11.json) passes 540/541 tests but fails the original positive sysroot handoff at its initial `prepare` assertion; its cause remains unknown. The cleanup regression passes, and the [independent original solver suite](../bench/validation/m7-linux-core-solver-37e5342-emulated-2026-10-11.json) passes all 39 controls in 128.222 seconds. This does not reclassify either failed complete gate or the earlier solver timeout. The subsequent [complete coordinated-fixture gate](../bench/validation/m7-linux-core-gate-ec9b682-emulated-2026-10-11.json) passes all 543 workspace tests and every remaining gate step, followed immediately by the [original 39/39 solver controls](../bench/validation/m7-linux-core-solver-ec9b682-emulated-2026-10-11.json) in 122.289 seconds. Static defects and later focused passes do not establish the cause of retained failures. Prior failed scopes and the unselected dependency optimization remain recorded. | The [current 4472279 release and owned installation](../bench/validation/m7-linux-product-4472279-emulated-2026-10-11.json) pass in 183.690 seconds; they do not erase the separate solver timeout. The earlier [release build](../bench/validation/m7-linux-product-d20d034-emulated-2026-10-11.json) and [native CI](../bench/validation/m7-linux-ci-8b8288d-native-2026-10-11.json) pass; complete current-product corpus cache fill/warm rounds remain required. |
| End-to-end acceptance | CLI/MCP repair, journals, explanations, saved baselines and result clients are implemented. The full default standard run is terminal: 158 matching units and one original 3,600-second infrastructure timeout in verismo-tspec. It remains a failed acceptance scope; no cache-fill/warm run or stitched pass is claimed. The earlier real human-eval preparation and exact repair context verify all project and dependency obligations but remain Unknown, so retained acceptance stopped; see [the failed prerequisite](validation/m7-retained-he-functional-1e.md). The [single reconstructed acquisition](validation/m7-retained-he-lease-refusal-1e.md) isolates permission refusal on a foreign-owned executable; the exact inventory has two ownership mismatches. The [service-owned installer](native-product-setup.md) and Linux ownership controls are integrated. The [corrected real-project run](validation/m7-owned-reader-d20.md) establishes an unmeasured Current full baseline using the existing single declaration retry, with all ordinary/failed preparation costs retained. Independent active-project preparation also establishes Current evidence. Both original bounded baseline checks and active ordinary verification pass, but the active repair loses sysroot observation coverage and is Unknown despite successful obligations. The single workflow stops before either reader census or retained integration; its [terminal failure and sysroot audit](validation/m7-owned-reader-d20-sysroot-audit.md) remain retained. New sysroot notes identify the existing observer-specific reason; strict affected-crate checks and all six original Linux sysroot controls plus the actual explicit-files observer regression pass. The [single isolated detailed diagnosis](validation/m7-sysroot-proof-diagnostic.md) completes in 478.014 seconds with complete Current evidence, all 2,202 full functions verified and two actual invocations. It does not reproduce or explain the older coarse failure retrospectively; real reader qualification remains pending. The [current-product original reader workflow](validation/m7-current-reader-workflow.md) now passes all four bounded Current prerequisites with complete 2,202-function full reports. All 200 Python samples preserve admission and the exact live verifier. Status p95 passes at 148.430 ms; full Result p95 fails at 394.713 ms. The unchanged failfast controller leaves Rust and retained CLI/MCP unattempted; diagnosis is on the full fresh response path, not a proof setup workaround. | Complete current standard coverage; real four-reader p95 below 200 ms; retained real-project CLI/MCP; three rounds with at least 2× median repair gains on each of human-eval-verus and Vest. |

The experimental CLI/MCP workflow is implemented: exact-function repair checks,
separate focused/full evidence, resumable proof observations, failure explanation
and immutable baseline comparison. Persistent MCP readers and the Rust SDK
receive complete fresh service replies; decoded reuse grants no freshness or
proof authority. Start with [the walkthrough](proof-repair-walkthrough.md),
[the MCP guide](mcp.md) or [the SDK contract](result-client.md).

M7 is **not accepted**. The selected source baseline still lacks the required
physical preparation-memory bound. Corrective graph/decoder/observer candidates
remain unselected and require complete source qualification and genuine
previous-revision focus reuse. The complete frozen `777b86a`
[workspace gate](../bench/validation/m7-linux-gate-777-emulated-2026-10-11.json)
passes all 485 tests and the remaining gate steps. The complete
[solver attempt](../bench/validation/m7-linux-solver-777-emulated-2026-10-11.json)
passes the corrected classifier fixture and 36 other controls, but fails the
HTTP whole-session replay control: answers match the solver, yet the fresh
reader starts Z3 instead of using only signed remote replies/certificate.
The retained phase trace identifies cold HTTP transport construction consuming
the lookup budget; the configured startup fix addresses that measured cause.
The independent frozen `f94bbda`
[native Linux CI run](../bench/validation/m7-linux-ci-f94-native-2026-10-11.json)
passes all required checks: 485 workspace tests, 38 solver controls, all three
end-to-end phases and three rounds of the 33-unit conformance subset. Its longest
job is 368 seconds. This preserves the separate local replay failure and does
not qualify the real-project workload. Configured HTTP transport now starts
initialization asynchronously at daemon bind. Its complete corrected local
[workspace gate](../bench/validation/m7-linux-gate-de36-emulated-2026-10-11.json)
passes all 489 tests and every remaining gate step. Its separate
[solver suite](../bench/validation/m7-linux-solver-de36-emulated-2026-10-11.json)
passes all 38 original controls, including whole-session remote replay without
starting Z3. The subsequent frozen `d333a9c`
[native run](../bench/validation/m7-linux-ci-d333-native-failure-2026-10-11.json)
passes workspace, end-to-end and conformance but fails the original whole-session
solver control. A deterministic
[setup-contention regression](validation/m7-http-setup-contention.md) demonstrates
an immediate skip while initialization is still in progress. The helper now
waits within its existing budget. Its frozen `cbd6d60`
[complete local workspace gate](../bench/validation/m7-linux-gate-cbd6-emulated-2026-10-11.json)
passes all 491 tests and every remaining step, and the
[original solver suite](../bench/validation/m7-linux-solver-cbd6-emulated-2026-10-11.json)
passes all 38 controls. The subsequent frozen `190d4d2`
[native run](../bench/validation/m7-linux-ci-190d-native-2026-10-11.json)
passes every required check, including all three end-to-end phases and the
original solver replay control; its longest job is 339 seconds. The later
`012707a` [local gate](../bench/validation/m7-linux-gate-0127-emulated-2026-10-11.json)
passes all 498 tests and every gate step, and its
[solver suite](../bench/validation/m7-linux-solver-0127-emulated-2026-10-11.json)
passes all 38 controls after adding bounded opt-in setup timings. The later
`c5a76f1` [native run](../bench/validation/m7-linux-ci-c5a7-native-2026-10-11.json)
also passes all required checks with 498 workspace tests and 38 solver controls;
its longest job is 318 seconds. Native CI uses
the existing prebuilt source baseline. These scopes do not qualify the corrective
source, real readers or repair performance. The failed native invocation lacks a
phase trace, so its exact cause remains unknown.
The subsequent frozen `e0ca5f5` [local workspace attempt](../bench/validation/m7-linux-gate-e0ca-failure-emulated-2026-10-11.json)
passes 497/498 tests but fails the original 250 ms concurrent HTTP lookup;
later gate steps are not reached. Its independent
[solver suite](../bench/validation/m7-linux-solver-e0ca-emulated-2026-10-11.json)
passes all 38 controls. Production workspace bytes match `012707a`; the full
cause of the late HTTP delivery under shared development load is unresolved.
The later frozen `091efd5` [complete local gate](../bench/validation/m7-linux-gate-091e-emulated-2026-10-11.json)
passes all 502 tests and every remaining step after adding sticky kernel mount
coverage and retained-storage accounting. Its [solver suite](../bench/validation/m7-linux-solver-091e-emulated-2026-10-11.json)
passes all 39 controls, including the real isolated mount-restoration fault.
The original HTTP concurrency fixture passes; the earlier late-delivery failure
and its unknown complete cause remain retained. These complete local scopes
do not qualify source reuse, real-reader latency or repair performance.
The subsequent `c13b410` [native CI run](../bench/validation/m7-linux-ci-c13b410-native-2026-10-11.json)
passes every required check: 502 workspace tests, 39 solver controls, original
managed and repair scenarios, and 33/33 conformance units in each of pass-through,
fill and warm. The real mount-restoration fault passes. The longest job is 399
seconds, inside the original 600-second limit. This precedes automatic sysroot
handoff activation and does not qualify its performance or the source candidate.
The complete default standard attempt also fails: 156 Same, one diagnostic-order
Unstable and two unchanged unit timeouts, with no Different verdicts. See
[the terminal scope](../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json).
Fill and warm were not reached. The latest complete local
[end-to-end attempt](../bench/validation/m7-linux-e2e-777-failure-emulated-2026-10-11.json)
passes legacy and installed MCP controls but times out during the first cold
managed dependency build at its unchanged 240-second limit; repair is not reached.
Retained partial streams establish the last observable stage, not a cause.
The subsequent unchanged [standalone managed acceptance](validation/m7-managed-setup-012707a.md)
on frozen `012707a` passes all original checks: cold 216.080 s, unchanged
31.947 s, CLI/MCP jobs, missing-receipt recovery and a restored-mtime failed edit.
The unchanged run still spends 15.992 s hashing sysroots despite zero verifier
invocations. A bounded local daemon digest handoff is now wired into managed
verification and `sail serve`, with overlapping complete caller/daemon guards.
The frozen `7da9579` Linux product build passes, but its original scenario
[fails the first cold deadline](validation/m7-sysroot-activation-7da9579.md).
The new scope encoder independently rejects byte-keyed environment maps and
therefore never enables handoff. Its bounded streaming-pair fix passes
[Linux regressions and strict checks](validation/m7-sysroot-selection-controls.json);
complete integration remains pending. No benchmark speedup is yet accepted.
The independent frozen `7da9579`
[solver suite](../bench/validation/m7-linux-solver-7da-emulated-2026-10-11.json)
passes all 39 original controls. Its separate complete workspace attempt fails
two HTTP controls at their unchanged 250 ms lookup budgets (520/522 pass);
later gate steps are not reached. Those failures remain open. The corrected
`1e8b633` product build passes; its complete gate is a separate frozen scope.
The [original HTTP traces](validation/m7-http-gate-7da9579.md) and single
[bounded send/body diagnostic](validation/m7-http-sendbody-7da9579.json) retain
both outcomes. The latter passes the unchanged fixture but does not identify
the failure's full cause or justify a transport/profile change.
The corrected product's [original managed scenario](validation/m7-sysroot-activation-1e8b633.md)
also exceeds the first cold 240-second limit. It reaches the automatic fill hint,
but warm, CLI/MCP and mutation checks are not reached. No daemon publication,
import or speedup is inferred from the hint alone.
Its separate [complete workspace gate](validation/m7-http-gate-1e8b633.md)
passes strict Clippy and 521/524 tests, but three unchanged HTTP controls time
out. The original eight-reader fixture passes. Later gate steps are not reached;
the cause remains unestablished and no combined gate pass is claimed.
Its [independent original solver suite](../bench/validation/m7-linux-solver-1e8-emulated-2026-10-11.json)
passes all 39 controls, including signed whole-session replay without starting
Z3 and the real mount-restoration fault. This does not replace the failed gate.
The separate [actual pinned-sysroot control](validation/m7-sysroot-context-1e.md)
passes natural publication and imports all 327 eligible hashes before the second
local hash loop, preserving the exact context and complete ordered runtime
identity bytes. It performs no Cargo/proof work and is not release performance
or managed acceptance.
The [bounded HTTP owner observation](validation/m7-http-owner-1e8b633.md)
records material runtime CPU in one original fixture. Its paired reqwest-only
optimization does not improve the complete observed request; that profile is
not selected and the failed gate's complete cause remains unresolved.
The retained CLI/MCP collector now enforces its original five-second MCP bound
through partial-frame reads and JSON decoding. [Nine controls pass](validation/m7-retained-rpc-deadline.json), including
three that reject the preceding loose collector. Its [complete workspace gate](validation/m7-http-gate-fc395e5.md)
passes strict Clippy and 523/524 tests, but fails the original HTTP setup-publication
control; later steps are not reached. No real-project retained acceptance is
established. Loopback transport refresh now retains an identical connection pool
after fresh complete configuration validation; its separate validation is pending.
A [sticky kernel mount-generation guard](decisions/0032-kernel-mount-generation-coverage.md)
detects restored mount changes throughout that handoff.
The earlier [missing-fixture failure](../bench/validation/m7-linux-solver-b36-emulated-2026-10-11.json)
remains preserved. The
[installed product build](validation/m7-current-product-build.json) and
[hardened MCP transport smoke](validation/m7-installed-mcp-b36.md) pass their
recorded scopes. The bounded source candidate now builds with all 13 runtime
artifacts and 2,060 vstd obligations, but its original cross-edit test refuses
span provenance for an unchanged function after an unrelated module body edit.
All early qualification controls and complete outcomes pass; later full
human-eval qualification was not reached. See the
[retained source failure](../engine/verus/candidates/decode-ownership/scoped-query-qualification-failure.json).
The [causal trace](../engine/verus/candidates/decode-ownership/span-provenance-causal-diagnostic.json)
reproduces the first imported mismatch: all 454 foreign source-file starts move
by 25 bytes while their other fields remain unchanged. A stable import-authority
design still needs actual mapping-state validation and source qualification.
The candidate remains unselected.
Complete default standard validation, real-project reader census and both
twofold repair-loop gains remain pending.

Focused controls, source builds, offline profiles and earlier passing suites
establish only their recorded scopes. No timeout or failed run is reclassified.
[Known issues](known-issues.md) links the complete failure index; the
[retained status history](validation/m7-acceptance-status-history.md) preserves
the earlier detailed narrative. [The acceptance guide](bench/m7-repair-acceptance.md)
sets the unchanged required gates and artifact provenance.

Priority order for completing this milestone:

1. Keep the complete workspace gate green, restore complete solver/HTTP replay success and retain real-solver/fault
   evidence, finish the current CLI/MCP suite without changing assertions or deadlines,
   and finish full default standard corpus validation. The cold replay diagnostic
   identifies client construction consuming the lookup budget; separate earlier
   established-transport failures remain unresolved and independently recorded.
2. Build and qualify the bounded source candidate with the same-input lowering
   oracle, independent stock outcomes, adversarial controls and physical budget.
3. Establish genuine Current real-project results and execute the original live
   reader and retained CLI/MCP scenarios with matching frozen product/runtime.
4. Measure all frozen human-eval and Vest repair cycles for three rounds, retain
   spread and resources, and meet the twofold improvement on each catalogue.
5. Publish complete user/developer documentation and evidence; merge only with
   all local acceptance and required CI green. No pending gate is marked done.

## Later waves (from the charter's initial scope)

1. **Caching.**
   - **Done:** key v2; a disk tier, a 256 MiB memory tier and write-behind
     stores (ADR 0011, 0015); serving far-from-limit `unknown` answers
     (ADR 0009); effort recording; fault injection; a shared tier of
     signed entries in a directory store (ADR 0017); optional authenticated
     HTTP(S) reading and publication with a self-hosted server (ADR 0025).
   - **Deferred:** warm solver pools (measured; see below) and the opt-in
     stack-key mode (ADR 0008 update).
2. **CLI.**
   - **Done:** `sail verify`, `sail watch`, `sail serve`, `sail status`,
     `sail cache`, and the MCP server `sail mcp` (ADR 0016).
   - **Done:** `sail bench` delegates to the benchmark binary.
3. **Agent ergonomics.**
   - **Done:** function attribution, the ledger and `--output json` schema v1
     (ADR 0014).
   - **Done:** fragility measurement: `sail fragility`, `sail-bench
     fragility` and the fragility census (ADR 0018).
   - **Done:** exact function scores in JSON/MCP and sequential,
     complete-run `accept-first-proof` portfolios (ADRs 0018, 0020).
     Every winning attempt independently proves the requested scope.
     Racing is deferred; seed successes are never assembled across runs.
4. **Benchmarks.**
   - **Done:** cold, warm, edit, agent-loop and history-replay workloads, all
     local-only (ADR 0013).
5. **Corpus.**
   - **Done:** fast and standard tiers, plus a secondary Verus pin and
     `docs/compatibility.md`.
   - **Done:** the exclusive, strict `rust_verify_test` deep suite and
     optional Singular source-build validation (ADRs 0019, 0021).
6. **CI.**
   - **Done:** a container image, with six fast per-PR checks only
     (ADR 0013).

### Measured: warm solver pools would not pay off (2026-10-08)

The charter lists warm solver pools. We measured their value before building
them, on a vstd run on macOS arm64:

| Run | Wall time | Rust front end | Verus verify phase (CPU, all threads) | Solver (all threads) |
|---|---:|---:|---:|---:|
| Plain Verus | 18.9 s | 5.7 s | 57.9 s | 13.0 s |
| sail, warm | 17.1 s | 5.6 s | 50.3 s | 3.5 s |

- **What's left after a warm cache is Verus, not the solver.** Most of the
  remaining time is Verus's own work: the Rust front end, VIR, and AIR and
  SMT generation.
- **The Linux fast-tier warm run** started a solver for only 26 of 1,526
  sessions.

Warm pools could only shorten that small solver-side remainder, so they are
**deprioritised**. The levers that matter now:
- **The warm Verus frontend** (the charter roadmap item), which may need
  Verus's cooperation;
- **Upstream determinism (VI-0001),** to raise hit rates;
- **Fill cost on slow filesystems:** write-behind stores in the daemon.

## Charter roadmap (designed for, built later)

- Multiple SMT backends.
- Alethe and Carcara certificates.
- A Rust pre-solver.
- Unsat-core pruning.
- Instantiation replay.
- A warm Verus frontend.
- Other verifiers.

## Detailed sysroot diagnosis

The [single product-7238da8 diagnosis](validation/m7-sysroot-proof-diagnostic.md)
now completes one new complete Current full proof, with 1,871 vstd and 331
HumanEval functions verified within the original 600-second budget. Its two
actual invocations and 478.014-second outer duration are cold diagnostic costs,
not a repair gain or one-driver result. It does not reproduce or explain the
older D20 coverage failure. Original Python/Rust four-reader and retained
CLI/MCP qualification still require a separately reviewed complete workflow;
source-budget and 2× gates remain unchanged.

## Original-descriptor qualification

The unselected original-descriptor source candidate now passes its actual
39 VIR and 64 driver controls, both strict Clippy checks, and a fresh ordinary
build with 2,060 vstd functions verified and zero errors. The full ordinary
build takes 749.928 seconds including its enclosing controller; its 13 runtime
artifacts, seven source files and lockfile are sealed. The preceding runtime
and failed qualification remain unchanged. The [original 14-stage qualification](../engine/verus/candidates/deferred-import-locator/original-descriptor/qualification-failure.json)
terminates after eight passes with a previous-revision focus failure in
154.852 seconds: zero prepared hits and four provenance refusals. Stock and
engine both verify 9/0 with byte-identical stderr. The same compiler limits and
byte/verdict oracles are retained. These results do not select the candidate or
establish the physical budget or repair speedup. See the [candidate evidence](../engine/verus/candidates/deferred-import-locator/original-descriptor/README.md).
