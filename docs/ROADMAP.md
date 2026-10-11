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

Earlier imported-span and ownership candidates remain documented in
[the retained workstream history](validation/m7-workstream-history-cc0898e.md).
Their scoped passes and failures do not qualify the selected runtime or a later
candidate.

### Current state

The active priorities are complete product workflows, with unchanged acceptance
targets. Historical passes below establish only their recorded scopes.

The corrected optional source runtime now passes its original thirteen-stage
qualification: genuine previous-revision focused reuse, all three full HumanEval
revisions, exact stock verdicts/diagnostics and cached-versus-fresh lowering from
the same compiler input. The source controls, strict checks, actual memo-bypass
sensitivity and ordinary fresh-vstd build also pass. Independent review checks
all runtime, source, lock and frozen-script seals. Local packaging and a fresh
installation match all thirteen runtime files and modes; the
[delivery record](../bench/validation/m7-linux-engine-aaa3b2e4-delivery-2026-10-11.json)
binds the private prebuilt artifact. The patch and pin select that correction
together on this in-progress branch. The rebuilt Sail release and owned installations pass. Its original repair
CLI/MCP workflow fails the 120-second budget during the missing-function case
after the successful proof and declaration-establishment follow-up; later cases
are unattempted. The failure and reduced diagnostic stay separate from the
passing source gates. Complete product integration and the frozen repair
benchmarks remain next; source qualification alone does not accept M7.
The [compiler-family change](decisions/0036-managed-compiler-artifact-families.md)
now shares ordinary targets across focus selectors while keeping proof receipts
separate. Its [11 targeted controls and strict checks](validation/m7-compilation-context-controls.md)
pass, and the rebuilt product installs. The [original repair workflow](../bench/validation/m7-linux-repair-cdb6e6d-failure-2026-10-11.json)
still fails the 120-second whole budget after two Current successful jobs;
phase diagnosis and actual whole-workflow acceptance remain required.
The subsequent [complete local gate](../bench/validation/m7-linux-selected-source-gate-a8bdac6-environment-failure-2026-10-11.json)
fails with 342 passing and 224 failing workspace cases after the Docker
filesystem becomes read-only; later gate steps are unattempted. Both actual
private scratch write probes confirm `EROFS`. The selected-runtime image
[export was cancelled](../bench/validation/m7-selected-engine-image-export-interruption-2026-10-11.json)
for low disk space. Docker subsequently restarts externally, interrupting the
stock diagnostic without a harness terminal result. Neither interruption
qualifies the product or replaces the retained failures.
A separate post-recovery [complete local gate](../bench/validation/m7-linux-selected-source-gate-0155b33-emulated-2026-10-11.json)
passes all 566 workspace tests and every remaining gate step. It covers the
selected qualified source patch and coherent artifact pin; later compiler-family
and reader component changes still require their own combined validation.
The [original solver suite](../bench/validation/m7-linux-selected-source-solver-0155b33-emulated-2026-10-11.json)
also passes all 39 pinned-solver and fault-injection controls.
The [candidate history](../engine/verus/candidates/preparation-namespace/no-original-range/README.md)
and [retained workstream history](validation/m7-workstream-history-cc0898e.md)
preserve the earlier migration, zero-hit and test-fixture failures.

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
failure without changing collectors or freshness checks. The latest combined
[workspace gate](../bench/validation/m7-linux-core-gate-0f31c1d-emulated-2026-10-11.json)
passes all 564 tests and remaining checks in 542.758 seconds, including the final
admission correction. Its separate
[solver run](../bench/validation/m7-linux-core-solver-0f31c1d-emulated-2026-10-11.json)
passes all 39 original controls in 94.628 seconds. The new [sealed product](../bench/validation/m7-linux-product-0f31c1d-emulated-2026-10-11.json)
builds in 121.145 seconds and is installed as byte-identical owned copies.
The [unchanged real-reader stage workflow](validation/m7-rpc-stage-workflow.md)
now fails complete Result p95 at 403.341 ms while Status passes at 147.290 ms.
All 200 samples retain the actual live verifier identity. The recorded tail
contains 43 Result stage samples, all cached frames without encoding; socket
write and freshness costs are material, but these samples do not establish
causality or replace the complete census. The exact per-barrier descriptor
binding optimization now passes the [complete Linux validation](validation/m7-native-path-bindings.md):
566 workspace tests, all remaining gate steps and 39 solver controls. Its
unchanged hardlink regression also detects an unsafe guard-removal variant.
The [new sealed release](validation/m7-binding-product/installed-product.json)
builds in 108.350 seconds; the stalled first build remains separately recorded.
The [unchanged live-reader workflow](validation/m7-rpc-binding-reader-terminal.md)
completes independent full Current preparation, then fails the original
600-second ordinary-verification prerequisite. It collects no latency samples.
The separate original managed fixture passes in 335.794 seconds, with cold
verification at 220.375 seconds under the unchanged 240-second bound. The latest recorded
[native CI revision](../bench/validation/m7-linux-ci-b029db5-native-2026-10-11.json)
passes all ten jobs in 482 seconds, including all 566 workspace and 39 solver tests;
the longest individual job takes 448 seconds. The separate
[observer integration gate](../bench/validation/m7-linux-observer-gate-55d14c8-emulated-2026-10-11.json)
also passes all 566 workspace tests and every remaining check, including the 12
native timeout-observation controls. Product Rust is unchanged from the preceding
solver scope. The separate
[retained CLI/MCP workflow](validation/m7-rpc-retained-functional.md)
re-establishes genuine Current evidence, then exposes a wrong-stream cursor
assertion conflicting with the documented explicit-error contract. Its original
failure is preserved. The [corrected retained run](validation/m7-rpc-retained-corrected.md)
passes exact error refusal, reconnects, complete results, immutable comparisons
and actual submission/cancellation under the original 120/30/5-second bounds.
The [complete-transfer component](validation/m7-result-transfer-profile.md)
finds socket backpressure during concurrent Python parsing, but larger send
buffers do not improve complete-client p95; no buffer change is selected.

| Workstream | Current evidence | Remaining product gate |
|---|---|---|
| Managed verification | The sealed current product passes the [original managed workflow](validation/m7-managed-current-product.md): cold proof, unchanged complete receipt reuse, missing-receipt recovery, restored-mtime failing edit, and CLI/MCP jobs. The new descriptor-bound product passes again: cold verification takes 220.375 s under its original 240 s bound. | Real HumanEval/Vest integration and repair performance; the small receipt fixture does not establish either. |
| Preparation across source edits | The selected [missing-original-range and broker-retirement correction](../engine/verus/README.md) passes all original thirteen source stages, genuine previous-revision focus and three complete HumanEval revisions. All runtime files match the independently verified prebuilt archive and fresh install. | Rebuild Sail with the selected patch, pass CLI/MCP repair integration and run the complete original HumanEval/Vest repair catalogues. Source hits alone do not establish a speedup. |
| Responsive reads | The current product passes all 89 daemon controls, the full 566-test workspace gate and 39 solver controls. The [original live census](validation/m7-current-reader-workflow.md) fails full Result p95 at 394.713 ms; Status passes. The [stage diagnostic](validation/m7-rpc-stage-workflow.md) also fails complete Result p95 at 403.341 ms. The exact freshness optimization passes its complete checks; its first [reader workflow](validation/m7-rpc-binding-reader-terminal.md) fails the 600-second prerequisite before collecting samples. The subsequent original [descriptor-bound census](validation/m7-descriptor-reader-terminal.md) retains all 200 valid Python samples: Status p95 120.526 ms passes, but complete Result p95 343.142 ms fails. Rust and retained CLI/MCP stages are unattempted after fail-fast. | Four concurrent readers, 200 samples, both p95 values below 200 ms during active verification; then retained real-project CLI/MCP within their original bounds. |
| Default and shared caching | Earlier local gates and recorded native CI pass (482 s total, longest job 448 s). The latest local diagnostic gate fails eight original deadline-sensitive cases; one focused follow-up passes seven, with no changed assertions or deadlines. The previous [complete standard attempt](validation/m7-default-standard-e0ca-terminal.md) has 158 Same and one stock Verismo timeout at the original 3,600 s bound. Its cause remains unknown. | Complete current-product standard conformance and cache fill/warm rounds. The failed standard scope remains failed. |
| Repair product acceptance | CLI/MCP repair, proof journals, explanations, named baselines and comparison are implemented. Current native CI passes all original fixture scenarios. | Complete source and real-project workflows, then three rounds with at least 2× median repair improvement on each of HumanEval and Vest. |

[Retained workstream history](validation/m7-workstream-history-cc0898e.md)
preserves the earlier source candidates, failures, observations and gate scopes.

The experimental CLI/MCP workflow is implemented: exact-function repair checks,
separate focused/full evidence, resumable proof observations, failure explanation
and immutable baseline comparison. Persistent MCP readers and the Rust SDK
receive complete fresh service replies; decoded reuse grants no freshness or
proof authority. Start with [the walkthrough](proof-repair-walkthrough.md),
[the MCP guide](mcp.md) or [the SDK contract](result-client.md).

M7 is **not accepted**. The corrected source runtime has passed source and ownership qualification,
but rebuilt-product integration and performance have not yet passed.
The current passing suites cover default workflows, and complete source
qualification now passes. Rebuilt-product repair integration, live reader
latency, complete standard conformance and repair performance remain separate
requirements. [Known issues](known-issues.md), the
[retained histories](validation/m7-workstream-history-cc0898e.md) and
[acceptance guide](bench/m7-repair-acceptance.md) preserve their exact scopes.

Focused controls, source builds, offline profiles and earlier passing suites
establish only their recorded scopes. No timeout or failed run is reclassified.
The [selected tspec logging diagnostic](validation/m7-tspec-transcript-interrupted.md)
was interrupted by filesystem/container failure and supplies no standard or product qualification.
[Known issues](known-issues.md) links the complete failure index; the
[retained status history](validation/m7-acceptance-status-history.md) preserves
the earlier detailed narrative. [The acceptance guide](bench/m7-repair-acceptance.md)
sets the unchanged required gates and artifact provenance.


Priority order for completing this milestone:

1. Keep the complete workspace, original solver controls and native CI green.
   Preserve failed historical scopes and original bounds.
2. Complete the selected runtime’s product integration. Reuse ordinary Cargo
   dependencies when only the repair focus changes, while retaining exact
   per-request proof contexts, fresh verified targets, independent full validation
   and complete receipts. Source qualification, packaging and the coherent pin
   are already complete; product acceptance remains open.
3. Finish the current live reader stage diagnosis and fix any measured dominant
   cost. Run the retained real-project CLI/MCP scenarios with complete genuine
   Current evidence and unchanged deadlines.
4. Resolve the stock Verismo wait sufficiently to complete current default
   standard conformance and cache fill/warm coverage.
5. Measure every frozen HumanEval and Vest repair cycle for three rounds, retaining
   spread and resources; meet at least twofold median improvement on each.
6. Keep user guides, known issues and developer records current. Merge only after
   all required local acceptance and native CI pass.

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

## Native-binding reader follow-up

The [original `46e1da6` workflow](validation/m7-rpc-binding-reader-terminal.md)
retains 950.644 seconds of successful independent full Current preparation,
then fails its original 600-second baseline ordinary-full prerequisite. No
reader or retained integration sample is attempted. Preserve this setup failure
alongside earlier full-Result latency failures. Next diagnosis needs complete
ownership-bound, all-task process observation; its
[real worker-thread control](validation/m7-reader-process-observation.md) now
passes on Linux. The retained main-task snapshot still cannot identify the
missing verifier/solver stage or establish a timeout cause. No new reader
workflow is implied by the observer control.

The separate [stock transport component](validation/m7-stock-smt-boundary-terminal.md)
builds and completes its normal fake-peer transfer with exact input bytes and
child shutdown. Its recorder drops two packets, so the sensitivity control
fails and the sequence stops before fault controls or the real forwarding arm.
The separately frozen [bounded metadata queue correction](validation/m7-stock-smt-recorder-corrected-terminal.md)
passes its private compile, normal transfer, all four intentional boundary pauses
and real forwarding arm. The real arm records all 45 rows and 12 response lines
without metadata loss; its original deadlines remain unchanged. It uses no
daemon cache or full-project concurrency and does not establish the historical
verification wait's cause. The separate [focused ordinary verification](validation/m7-ordinary-wait-diagnostic-terminal.md)
now completes with a full Current result in 87.054 seconds under its original
600-second bound. It preserves the ordinary context and source revision, runs
one HumanEval invocation and reuses vstd through the normal receipt path. All
113 completed shim records report cached replies without starting a solver.
Its bounded process snapshots remain partial. This success does not explain
the historical timeout or supply reader latency or repair acceptance.
