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

The current selected source is `387ba80`; ordinary release/private installation
passes in 89.720/2.282 seconds, followed by exact acceptance byte installation
in 4.494 seconds. Its unchanged original CLI/MCP repair passes in 90.588
seconds with 299 operations and original 120/30/5 limits. Current readers,
complete conformance and real-project performance remain unqualified. The earlier `b89546c` product's complete local gate, 41/41 original
solver controls, ordinary release, private and acceptance installations, and
unchanged CLI/MCP repair fixture pass. The original three-round benchmark failed
on the first concatenate repair warmup: verification exited zero, but the full
result was Historical and unaccepted. GCD had reached Current admission on its
second attempt with three compiler invocations; this does not explain the later refusal. There are zero measured
samples, Vest is unreached, and no speedup or M7 acceptance is claimed.
The separate original b895 reader and retained CLI/MCP workflow remains a
historical pass, without qualification transfer to the current source.
Later combined source `57a319d` passes its [complete local gate and original
solver suite](validation/m7-compiler-script-bootstrap/source-inventory.json):
673/673 workspace tests, all remaining gate steps and 41/41 solver tests.
This includes the declared source-inventory repair and MCP baseline lifecycle.
Its ordinary release (93.661 seconds) and owned private installation (2.444
seconds) also pass with the same source and qualified engine identities. The
original installed CLI/MCP repair fixture also passes in 95.684 seconds with
315 operations under unchanged 120/30/5-second limits. The separate
[named-baseline feature smoke](validation/m7-mcp-baseline-product-smoke-plan.md)
also passes: actual MCP save/list/get, immutable overwrite refusal and restarted
client reads agree with CLI on full Historical provenance. Its derivative
supplies no additional original-fixture credit; readers, conformance and real-project performance
receive no transferred credit.
The later original three-round `57a319d` benchmark fails before measurement
during an observed read-only guest filesystem incident, with zero samples and
Vest unreached. Source/product/runtime seals and birth match, but closure is
UNKNOWN after census exits 133; no performance or closed-scope claim follows.
The [existing record](validation/m7-compiler-script-bootstrap/source-inventory.json)
binds retained warmups and raw failure without assigning an unproven cause.
An authorized Desktop restart restores both write/fsync probes with unchanged
container IDs/images and new births. Old prepared 57a readers remain unexecuted
and require restaging. Fresh source `387ba80` explicitly disables Cargo
incremental compilation in local gates to match CI. Its separate complete gate
passes in 434.446 seconds (673/673 workspace tests and all remaining steps),
followed by 41/41 original solver tests in 75.519 seconds. Both phases record
jobs two and nextest concurrency one, unchanged source/protected inventories
and current container birth, with no live known-group survivors or uncertainty.
Ordinary new product release/private/acceptance installations and original
repair pass separately; broader product acceptance remains pending. The [existing cleanup record](validation/m7-unused-intermediates-20261012.json)
preserves the initial 20,288-entry refusal and later exact 156-subtree retirement,
without attributing physical backing-space recovery.
Remaining delivery gates are complete current standard/cache conformance and
the two real-project speed thresholds. PR #74
remains draft.

The source engine, repair CLI/MCP, resumable proof journals, explanations,
immutable baselines and comparisons are implemented. Stock verification stays
the default. Start with [the repair walkthrough](proof-repair-walkthrough.md),
[the MCP guide](mcp.md) or [the SDK contract](result-client.md).

Compatible repair selectors now share ordinary Cargo dependency artifacts,
while proof contexts and full validation remain independent. Validated
build-script observations retain fresh input/output checks across that compiler
family. The daemon constructs retained-result output owners through a bounded
background service; foreground reads can fall back to full byte validation.
The selected output-owner overlap and small exact HTTP-body changes have passed
component validation. Preserving observers across unchanged metadata publication
now passes its
[complete Linux component replay](validation/m7-preserve-freshness-metadata-replay.md),
including sensitivity checks. The earlier installed `c2a4b23` passes the
[complete local gate and original solver suite](validation/m7-core-c2a4b23-validation.md):
651 workspace tests, every remaining gate step and 41/41 solver tests. Optional
independent ordinary-build recovery passes all ten controls and its separate
[on/off/on emission sensitivity](validation/m7-bootstrap-c2a4b23-sensitivity.md).
The later selected bootstrap recovery and optional refusal diagnostic at
`b89546c` pass the complete 658/658 workspace gate, all remaining steps and
41/41 original solver tests. The real Cargo retry/export/input-barrier control
and three diagnostic controls pass; [existing source records](validation/m7-compiler-script-bootstrap/README.md)
retain earlier failures and interruption uncertainty. Its ordinary release
(108.346 seconds) and private byte installation (2.422 seconds) pass with exact
CLI/shim and selected-runtime identities. Acceptance byte installation also
passes. The fresh original 120/30/5 repair fixture passes in 102.476 seconds
(112.975 seconds host-inclusive), with 335 operations and no cleanup/progress
errors. The [original three-round benchmark](validation/m7-compiler-script-bootstrap/scheduling-recovery.json)
fails the later concatenate warmup on Historical full evidence, with zero measured
samples and Vest unreached. The separate original b895 reader workflow passes
both unchanged collectors plus retained CLI/MCP, with exact immutable-input
checks and no cleanup or archive errors.
Exact [release and private installations](validation/m7-product-c2a4b23-installation.md)
pass, followed by the fresh unchanged
[original CLI/MCP repair fixture](validation/m7-repair-c2a4b23-terminal.md) in
99.719 seconds with 325 operations, original 120/30/5 limits and no cleanup or
progress errors. [Current PR CI at `a610357`](https://github.com/Chelis-Lang/sail/actions/runs/38209909625)
passes all ten completed jobs including completion, bound to pushed commit
`a610357902ba9f7d6cfd491370f8b5c818bf68b1`. Later unpushed documentation
(`ddefa26` and `3919482`) is outside that CI scope.

The earlier [complete original `730a321` reader workflow](validation/m7-reader-730a321-terminal.md)
remains a historical pass: both four-client collectors retain 200 total samples,
complete Current graphs and unchanged p95 limits, followed by retained CLI/MCP.
The separate b895 pass below uses its own genuine Current evidence; no earlier
reader result is transferred.
The [c2 original benchmark attempt](validation/m7-benchmark-c2a4b23-interrupted.md)
is interrupted by host checkpoint ENOSPC. [Separate recovery](validation/m7-benchmark-c2a4b23-recovery.md) retains three
warmups and no measured samples; fourth M7 focus completion, original terminal,
closure and final seals remain unknown. A separate fresh
[completed c2 benchmark](validation/m7-benchmark-c2a4b23-warmup-terminal.md)
fails required Current warmup admission despite exit-zero verification, with
no measured samples. Its full report names newly established build-script
declarations; complete postseals and bounded closure do not promote Unknown
evidence. Earlier failed gates, repairs and
benchmarks retain their original status; no paired speedup or overall M7
acceptance follows, and PR #74 remains draft.

| Workstream | Current evidence | Required next result |
|---|---|---|
| Optional source engine | The selected runtime passes all thirteen source qualification stages, exact cached-versus-fresh lowering from the same compiler input, stock verdict/diagnostic checks and fresh-vstd verification. Its private package and installation match all thirteen runtime artifacts. | Full repair workflows and frozen real-project performance. |
| Combined implementation | Current selected source `387ba80` passes [673/673 workspace tests, all remaining steps and 41/41 solver tests](validation/m7-compiler-script-bootstrap/source-inventory.json) with incremental compilation disabled; its ordinary release/private/acceptance installations and original repair also pass. Broader product qualification remains pending. Earlier `b89546c` passes [658/658 workspace tests, all remaining steps and 41/41 solver tests](validation/m7-compiler-script-bootstrap/README.md), including the new real Cargo recovery and three diagnostic controls. Its ordinary release/private and acceptance byte installations plus fresh original repair fixture pass; original real-project benchmark fails at Historical warmup with zero samples and no performance claim. Earlier frozen `c2a4b23` passes [651 workspace tests, all remaining gate steps and 41/41 solver tests](validation/m7-core-c2a4b23-validation.md). All ten ordinary-build controls and [on/off/on emission sensitivity](validation/m7-bootstrap-c2a4b23-sensitivity.md) pass. [Current PR CI at `a610357`](https://github.com/Chelis-Lang/sail/actions/runs/38209909625) passes all ten completed jobs including completion; later unpushed documentation is outside that exact commit's scope. | Complete standard/cache and real-project acceptance; retain the installed product identity and historical failures. Historical b895 readers pass separately. Local scopes receive no separate CI credit. |
| Repair workflow | Current `387ba80` [original CLI/MCP fixture](validation/m7-compiler-script-bootstrap/source-inventory.json) passes in 90.588 seconds with 299 operations and unchanged 120/30/5 limits after its exact product installation. Earlier `b89546c` [ordinary release, private/acceptance byte installations and original repair fixture](validation/m7-compiler-script-bootstrap/scheduling-recovery.json) pass. The fresh fixture completes in 102.476 seconds with 335 operations, original 120/30/5 bounds and no cleanup/progress errors. Earlier c2 [repair pass (99.719 seconds, 325 operations)](validation/m7-repair-c2a4b23-terminal.md) remains a separate pass; poll counts are not benchmark scores. | Frozen HumanEval/Vest recovery and measured whole-loop gains; small-fixture success does not qualify real-project performance. |
| Responsive reads | Earlier `b89546c` [original workflow](validation/m7-compiler-script-bootstrap/scheduling-recovery.json) passes both unchanged collectors, each with 200 total samples across four clients (100 Result and 100 Status): Python Result/Status p95 91.227/56.989 ms; Rust 83.867/44.371 ms. Python receives the complete 4,310,817-byte Result. Genuine Current evidence, active-verifier continuity, retained CLI/MCP and immutable-input/cleanup checks pass. The 730 pass remains a separate historical scope. | Preserve original complete graphs and bounds for later production revisions; this product-specific pass does not qualify newer baseline/source-inventory changes or isolate a causal speedup. |
| Default/cache conformance | The last [complete standard attempt](validation/m7-default-standard-e0ca-terminal.md) gives 158 Same and one stock Verismo timeout at its original 3,600-second limit. Native PR conformance passes pass-through, fill and warm, each 33/33 Same. | Complete current-product standard coverage and cache fill/warm. A subset pass does not complete the failed standard scope. |
| Frozen repair performance | Installed `b89546c` [original benchmark](validation/m7-compiler-script-bootstrap/scheduling-recovery.json) fails after 989.042 seconds: GCD second repair is Current/accepted, but concatenate first repair is Historical/unaccepted despite exit-zero verification. No full notes identify the cause ([Sail #77](https://github.com/Chelis-Lang/sail/issues/77)); zero measured samples and Vest unreached. The [completed c2 benchmark](validation/m7-benchmark-c2a4b23-warmup-terminal.md) remains failed on required Current warmup admission. Earlier [storage interruption](validation/m7-benchmark-c2a4b23-interrupted.md), [partial recovery](validation/m7-benchmark-c2a4b23-recovery.md) and historical [730 binding failure](validation/m7-benchmark-730a321-bootstrap-failure.md) remain separate. Recovery controls do not establish real-project benchmark benefit. | Complete the original three rounds, spread and at least 2× median improvement on each project with exact Current admission and oracles. |

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
automatically replayed after restart. Those interrupted attempts supply no
acceptance credit; the later `730a321` reader pass is a separate complete scope.

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

[Closed host conformance evidence](validation/m7-conformance-host-evidence-retirement-20261012.json)
now lives in two verified gzip archives. All 3,561 original files remain
recoverable with their byte hashes and metadata; only the verified uncompressed
copies were retired. Historical interrupted results remain unchanged, and no
physical disk recovery or validation credit is claimed.

M7 is **not accepted**. PR #74 remains draft: selected `b89546c` gate and solver
pass; its ordinary release/private and acceptance installations plus original
repair fixture also pass. Whole real-project benchmark fails on Historical warmup
admission with zero measured samples; no performance claim follows. Earlier c2 component sensitivity,
installation and original repair scopes pass. Original selected-b895 readers
and retained CLI/MCP also pass; complete standard/cache coverage and real-project
performance remain outstanding. Emulated local Linux runs are
authorized and labelled; native-host reference reruns are for later. Limits,
full reports and exact oracles remain unchanged.

A [separate closed-build cleanup](validation/m7-single-incremental-retirement-20261012.json)
removes 54 unshared incremental files from one old benchmark compiler family
(274,223,104 allocated file bytes). It preserves 945 hardlinked files and every
other entry in that audited subtree. Earlier inventory refusals remain failed;
this narrow cleanup grants no proof or whole-cache validation credit.

A separate whole-archive cleanup removes four closed b895 reader targets
(1,662,423,040 unique allocated bytes) and passes its postchecks. A five-tree
benchmark cleanup removes all 5,214 approved entries with verified archive
backing, then fails because its parent-membership check subtracts `target`
instead of the actual `debug` basename. That failure remains failed. A separate
read-only check reverifies the archive but ends UNCERTAIN after nine descriptor
holder matches; it grants no surrounding-parent certification. No cause is
inferred from those matches, and no retry runs while the benchmark is active. The
[cleanup record](validation/m7-closed-core-build-retirement.json) preserves both
scopes. Neither scope grants proof, performance or physical host-space credit.

The [retained integration history](validation/m7-core-integration-history-84346e2.md),
[earlier workstream history](validation/m7-workstream-history-cc0898e.md),
[known issues](known-issues.md) and [acceptance guide](bench/m7-repair-acceptance.md)
preserve individual source, product, environment and timeout scopes. No failed
run is reclassified by a later isolated pass.


Priority order for completing this milestone:

1. Keep the complete workspace, original solver controls and native CI green.
   Preserve failed historical scopes and original bounds.
2. Preserve the selected b895 [release, installations and passing original repair
   fixture](validation/m7-compiler-script-bootstrap/scheduling-recovery.json).
   Its original full readers and retained CLI/MCP now pass. Preserve complete
   reports, independent freshness, active-verifier continuity and lifecycle
   limits when validating later revisions. These unpaired scopes do not isolate
   the contribution of metadata preservation, output-owner overlap or dependency
   reuse, and do not establish real-project whole-loop speedups.
3. Retain source qualification and the selected runtime's exact delivery identity
   alongside product evidence. Keep proof contexts and receipts independent of
   shared ordinary Cargo artifacts; no focused success can replace full validation.
4. Resolve the stock Verismo wait sufficiently to complete current default
   standard conformance and cache fill/warm coverage.
5. Diagnose the retained selected-b895 Historical warmup refusal, preserving its
   failed scope and original Current admission. Qualify every frozen HumanEval
   and Vest repair cycle: three rounds, spread and resources, with at least
   twofold median improvement on each project. Its real Cargo recovery and
   diagnostic controls pass, but are not real-project performance evidence.
   Unsupported ordinary-build coverage still regenerates; Unknown proofs remain
   unaccepted. Preserve the earlier c2 declaration-establishment failure,
   interruption and old binding failure as separate historical scopes.
6. Keep user guides, known issues and developer records current. Merge only after
   all required local acceptance and native CI pass.

Closed host build targets were [losslessly archived](validation/m7-closed-host-targets-20261011.md), retaining test images and raw logs while reducing their logical storage by about 805 MB. Active Linux builds and runtimes are preserved.
A later whole-volume archival failed on host space before removing its original. After Docker recovery, [unused build intermediates](validation/m7-unused-intermediates-20261012.json) were removed from three unreferenced Sail targets: 1.99 GB logical bytes, with all 278 executable/library hashes checked afterward. Data volumes and active targets remain intact; resumed validation uses fresh container identities.

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

The later [exact unused Verus dependency cleanup](validation/m7-unused-intermediates-20261012.json)
retires 3,450 single-link non-executable/non-ELF compiler files across six old
variant dependency trees (5,062,496,256 allocated file bytes), preserving all
2,032 other entries, 54 hardlinks and six root identities. Selected c77 runtime
and source are outside those trees. This is per-tree preservation, without
a surrounding debug/release posthash or physical backing-space attribution.
The fresh post-recovery 387 benchmark is running separately with no performance
credit; the second whole-debug verification remains pending.
