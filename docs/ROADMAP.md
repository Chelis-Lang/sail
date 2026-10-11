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

| Workstream | Implementation and current validation | Remaining product gate |
|---|---|---|
| Repeat verification setup | Guarded daemon sysroot reuse is wired in. The corrected product builds and reaches the automatic fill hint, but its original cold scenario exceeds 240 seconds. The complete gate is running. | Establish actual guarded reuse, then the original cold/warm managed CLI/MCP scenario, including missing receipts and restored-mtime edits. |
| Reuse after a source edit | The unselected DeferredLocator candidate has 22 pinned-type helper controls and independent review. Its full private source build is running with actual provenance, codec and Reporter controls added separately. | Complete source checks and original fourteen-stage qualification, including genuine previous-revision focus reuse within 600 seconds. |
| Shared cache | The whole-session solver control passes. Two workspace HTTP controls fail their original 250 ms budget; phase observations are retained. | Resolve the measured delay and pass the complete unchanged gate. |
| End-to-end acceptance | CLI/MCP repair, journals, explanations, saved baselines and result clients are implemented. Full default standard validation is active. | Complete current standard coverage; real four-reader p95 below 200 ms; retained real-project CLI/MCP; three rounds with at least 2× median repair gains on each of human-eval-verus and Vest. |

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
