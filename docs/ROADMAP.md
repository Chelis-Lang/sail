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

The source engine, repair CLI/MCP, resumable proof journals, explanations,
immutable baselines and comparisons are implemented. Stock verification stays
the default. Start with [the repair walkthrough](proof-repair-walkthrough.md),
[the MCP guide](mcp.md) or [the SDK contract](result-client.md).

The current implementation adds two core changes. Compatible repair selectors
share ordinary Cargo dependency artifacts, while proof contexts and full
validation remain independent. Validated build-script observations retain
fresh input/output checks across that compiler family. The daemon constructs
retained-result output owners through a bounded background service; foreground
reads can fall back to full byte validation. These are implemented changes,
not measured real-project speedups.
The [rebuilt combined release](../bench/validation/m7-linux-combined-product-a9609e9-emulated-2026-10-11.json)
compiles and installs as private byte-identical copies. Its production source
matches the earlier combined gate's source, whose HTTP deadline check fails;
installation supplies no acceptance credit. Its [original emulated CLI/MCP fixture](../bench/validation/m7-linux-combined-repair-a9609e9-failure-2026-10-11.json)
subsequently fails at 120.266 seconds during missing-function validation after
success, declaration follow-up, events and baseline complete. Later cases are
unattempted. No deadline changes or native-success substitution apply.
After all common-target actors closed, [1.59 GB of incremental
compiler scratch](validation/m7-closed-incremental-retirement-a9609e9.json) was
retired without removing binaries, runtime files or raw evidence.

| Workstream | Current evidence | Required next result |
|---|---|---|
| Optional source engine | The selected runtime passes all thirteen source qualification stages, exact cached-versus-fresh lowering from the same compiler input, stock verdict/diagnostic checks and fresh-vstd verification. Its private package and installation match all thirteen runtime artifacts. | Full repair workflows and frozen real-project performance. |
| Combined implementation | The [earlier complete local gate](../bench/validation/m7-linux-combined-core-a983ab2-pass-2026-10-11.json) passes all 609 workspace tests and 41 original solver controls. The [subsequent publication-timing gate](../bench/validation/m7-linux-publication-timing-core-db4c65f-failure-2026-10-11.json) passes strict Clippy and 611 of 612 tests, but again fails the unchanged HTTP coalescing deadline; its separate original solver suite passes 41/41. The latest pushed native CI is green; new timing changes remain unpushed. | A complete gate with the latest changes and installed repair validation. Historical failed scopes and their unresolved causes remain separate. |
| Repair workflow | The [updated product builds and installs](../bench/validation/m7-linux-negative-generation-product-a983ab2-emulated-2026-10-11.json). Its [original emulated fixture fails at the final late-EXEC step](../bench/validation/m7-linux-negative-generation-repair-a983ab2-failure-2026-10-11.json), after successful repair, missing selectors, full regression rejection and MCP checks. A subsequent [timing-only run](../bench/validation/m7-linux-publication-timing-original-repair-db4c65f-failure-2026-10-11.json) again exhausts the original deadline: two completed workers spend 4.73–4.88 seconds establishing independent freshness after child exit; result parsing is sub-millisecond. New native CI passes its small repair fixture. | Reduce measured freshness construction without weakening Current, then complete the original local workflow and real HumanEval/Vest acceptance. Native small-fixture success does not replace local failures or establish real-project performance. |
| Responsive reads | The last [original full reader census](validation/m7-descriptor-reader-terminal.md) retains 200 valid samples with four concurrent clients: Status p95 120.526 ms passes; complete Result p95 343.142 ms fails. The [selected output overlap change](validation/m7-result-output-overlap-native.md) passes 30 daemon controls, eight primitive controls and strict checks; its separate validation component improves p95 from 19.366 to 12.713 ms. No RPC or Python parsing is included in that component. | Both complete p95 values below 200 ms during actual verification, then original retained CLI/MCP scenarios. No reduced graphs or changed client deadlines. |
| Default/cache conformance | The last [complete standard attempt](validation/m7-default-standard-e0ca-terminal.md) gives 158 Same and one stock Verismo timeout at its original 3,600-second limit. Native PR conformance passes pass-through, fill and warm, each 33/33 Same. | Complete current-product standard coverage and cache fill/warm. A subset pass does not complete the failed standard scope. |
| Frozen repair performance | The unchanged [benchmark workflow](validation/m7-current-benchmark-launcher.md) is interrupted again during preparation by host storage exhaustion and a Docker shutdown. Four warmup records survive; no measured samples do. The reader's active preparation is also interrupted before its census. No current whole-loop speedup or reader latency result is claimed. | Three rounds, spread and at least 2× median improvement on each of HumanEval and Vest, with full verdict/diagnostic oracles. |

The separate [isolated cache-maintenance diagnostic](validation/m7-gc-c97-isolated-diagnostic.md)
passes in 8.78 seconds without reproducing the stall. It does not replace the
failed whole solver suite. That audit found that test-support timeout cleanup
can block on its output mutex before killing the child; the cleanup-first correction is
implemented, without attributing the historical timeout to that gap. The first
[combined correction gate](../bench/validation/m7-linux-cleanup-control-gate-6b1edb1-failure-2026-10-11.json)
fails strict Clippy on a test-only closure; workspace tests and solver are
unattempted there. The direct-method spelling is corrected. Its
[new complete gate](../bench/validation/m7-linux-combined-core-gate-b63af10-failure-2026-10-11.json)
passes strict Clippy and 606 of 607 workspace tests, but the eight-reader HTTP
coalescing case times out at its unchanged 250 ms budget. One request and seven
coalesced readers are recorded; all callers fail to receive the signed entry in
time. Later gate steps and the solver suite are unattempted. The later current gate passes this unchanged check. The original failed
scope and its unresolved cause remain retained; the deadline and hit oracle
were not changed.

The separate [cleanup-fixed solver attempt](../bench/validation/m7-linux-combined-core-solver-b63af10-failure-2026-10-11.json)
passes 39 of 41 cases, including GC/clear in 20.404 seconds. Two attribution
cases fail while the Linux filesystem enters emergency read-only mode; their
precise failure causes are not established. The first
[frozen benchmark attempt](../bench/validation/m7-linux-repair-benchmark-a9609e9-storage-failure-2026-10-11.json)
also stops on `EROFS`, without measured samples. After the authorized Docker
restart, private write/fsync checks pass in both containers. A recovered
benchmark setup then correctly refuses a leftover socket when its old pathname
is reused; [that failed scope](../bench/validation/m7-linux-repair-benchmark-a9609e9-stale-socket-2026-10-11.json)
is retained. A fresh explicitly named benchmark namespace now isolates work,
caches and socket selection without copying prior proof state. Reader and
repair diagnostics also use fresh scopes. A later host disk-full/Docker shutdown
interrupts the fresh benchmark and reader scopes before measurement. They remain
failed or incomplete. Docker lifecycle work is now coordinated through one
team owner, and heavy builds run serially on the shared instance; no proof is
automatically replayed after restart. These runs remain pending acceptance.

The [recovered repair diagnostic](validation/m7-repair-a960-recovered-setup.md)
still fails its original 120-second workflow budget, later at MCP repair. Both
missing-selector paths pass. A completed Current negative regression consumes
the ordinary build-script generation without republishing it; the next
same-family check recompiles unchanged macro dependencies. The branch now
implements ordinary-output retention for fully observed negative verification
without changing successful proof receipts or `TargetResult.complete`.
Independent review is complete. All fifteen [affected Linux controls](validation/m7-negative-compiler-generation-first-controls.md)
pass, including an actual Cargo dependency reuse check. Their first component
scope fails strict Clippy on a test-only missing semicolon; that correction is
integrated. The subsequent [complete workspace gate](../bench/validation/m7-linux-combined-core-a983ab2-pass-2026-10-11.json)
passes all 609 tests and every remaining step, followed by all 41 original
pinned-solver controls. The rebuilt installed product reaches MCP and the final
late-EXEC step, where the original complete fixture still exceeds 120 seconds.
Later streams show only the selected crate being checked, without the old macro
dependency rebuild pattern. Complete late-EXEC output correctly rejects E0499
by cleanup, but its publication time against the deadline was not retained.
Completed jobs have 3.17–3.47 seconds beyond the reported runner time, which
excludes some publication work. The subsequent instrumented run measures
4.73–4.88 seconds of post-child independent freshness construction in two
completed jobs. Parsing and managed publication take milliseconds. That
measurement directs the next change toward preserving valid input observers
across unchanged metadata publication; it does not identify every operation
inside freshness or qualify a speedup.

The [async HTTP candidate](decisions/0037-owned-async-shared-http-transport.md)
passes its 39 affected controls but increases combined helper/I/O CPU and complete
lookup time in its bounded comparison. It is rejected; production keeps the
blocking transport and the original deadline failure remains unresolved.

[Additional closed build cleanup](validation/m7-closed-core-build-retirement.json)
retires 758 MB of compiler intermediates, preserving and rehashing every
discovered ELF image and all raw evidence. Active targets and runtimes remain
untouched.

[Lossless compression of closed host evidence](validation/m7-closed-host-lossless-compression.json)
replaces seventeen uncompressed tar files with verified gzip representations,
saving 574 MB of logical storage. Every original byte hash remains recoverable;
the record maps retired archive paths to their compressed copies. Active native
builds, qualified runtimes and foreign containers remain untouched.

M7 is **not accepted**. PR #74 remains draft until the required product,
reader, conformance and performance gates pass. Emulated local Linux runs are
authorized and labelled; native-host reference reruns are for later. Limits,
full reports and exact oracles remain unchanged.

The [retained integration history](validation/m7-core-integration-history-84346e2.md),
[earlier workstream history](validation/m7-workstream-history-cc0898e.md),
[known issues](known-issues.md) and [acceptance guide](bench/m7-repair-acceptance.md)
preserve individual source, product, environment and timeout scopes. No failed
run is reclassified by a later isolated pass.


Priority order for completing this milestone:

1. Keep the complete workspace, original solver controls and native CI green.
   Preserve failed historical scopes and original bounds.
2. Reduce measured post-child freshness construction. Preserve continuous input
   observation when metadata bytes are unchanged, without skipping independent
   runtime/input validation or publishing Completed before results are ready.
   Ordinary dependency reuse and source qualification are implemented; installed
   product acceptance remains open.
3. Integrate the [validated output-owner overlap change](validation/m7-result-output-overlap-native.md)
   into the complete product gate, then run the unchanged four-client,
   200-sample reader census and retained CLI/MCP scenarios. Its affected controls
   and component comparison pass; component p95 falls from 19.366 to 12.713 ms.
   That measurement includes no RPC or Python decoding and does not complete
   the full reader gate. The [transport review](validation/m7-full-result-cost-followup-review.md)
   finds no additional encoding or full-frame copy on cached hits. Separately
   validate the still-unselected bounded small HTTP body reads against the
   original eight-reader 250 ms check.
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
