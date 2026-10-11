# Known issues and acceptance limits

This page separates product limitations, upstream behavior and unresolved test
failures. It describes the current M7 development branch; it does not declare
that branch released or accepted. The [roadmap](ROADMAP.md) tracks remaining work.
The [retained status history](validation/m7-acceptance-status-history.md) preserves
the detailed prior roadmap narrative and its distinct failed/setup scopes.

## Interactive repair engine (M7)

CLI/MCP repair checks, journals, explanations and immutable baselines are
implemented. The selected source runtime passes source fixtures and the small
native Linux CLI/MCP scenario. Real human-eval-verus and Vest acceptance is
unfinished. Neither a twofold whole-loop gain on each project nor the real
four-reader latency gate has been established. See the
[acceptance guide](bench/m7-repair-acceptance.md) for exact scopes and records.

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
| Latest complete native CI | Frozen `c13b410`: every required job passes, including workspace 502/502, solver 39/39, original managed/repair scenarios and 33/33 conformance units in each round. The real mount-restoration control passes. Longest job is 399 seconds. This predates automatic sysroot handoff and does not qualify source reuse, real readers or repair speed. | [Current native pass](../bench/validation/m7-linux-ci-c13b410-native-2026-10-11.json) |
| Earlier complete native CI | Frozen `c5a76f1`: all required jobs pass, including workspace 498/498, solver 38/38, all three end-to-end phases and 33/33 conformance units in each original round. Longest job is 318 seconds. The earlier native solver failure remains retained with unknown exact phase cause. | [Earlier native pass](../bench/validation/m7-linux-ci-c5a7-native-2026-10-11.json), [earlier native failure](../bench/validation/m7-linux-ci-d333-native-failure-2026-10-11.json) |
| Earlier passing native CI | Frozen `f94bbda`: all required jobs pass, including 485 workspace tests, 38 solver controls, legacy/managed/repair end-to-end and 33 conformance units in each of pass-through/fill/warm. Longest job is 368 seconds. This predates the configured HTTP setup fix and does not replace local or real-project acceptance. | [Complete native record](../bench/validation/m7-linux-ci-f94-native-2026-10-11.json) |
| Latest complete local workspace attempt | Frozen `1e8b633`: strict Clippy and 521/524 tests pass; three original HTTP controls exceed 250 ms. The eight-reader case passes. The credential/disconnect case fails before its negative assertions; later gate steps are not reached. Cause remains unresolved. | [Corrected gate failure](validation/m7-http-gate-1e8b633.md) |
| Earlier automatic-setup workspace attempt | Frozen `7da9579`: 520/522 pass; two HTTP controls exceed their unchanged 250 ms lookup budgets. Later gate steps are not reached. The single separate send/body diagnostic passes but leaves the full cause unresolved. | [Original gate failures](validation/m7-http-gate-7da9579.md), [separate diagnostic](validation/m7-http-sendbody-7da9579.json) |
| Latest passing complete local workspace gate | Frozen `091efd5`: all 502 tests and every remaining gate step pass after the sticky mount-generation guard, storage accounting and metadata-observer controls. The original 250 ms HTTP concurrency fixture passes; this does not establish the cause of the separate earlier failure. | [Complete passing scope](../bench/validation/m7-linux-gate-091e-emulated-2026-10-11.json) |
| Earlier complete local workspace failure | Frozen `e0ca5f5`: 497/498 tests pass; the original concurrent HTTP lookup exceeds its unchanged 250 ms budget. One request coalesces seven readers, but delivery completes too late. Python controls, fmt and strict Clippy pass; later gate steps are not reached. A private source build overlaps; the complete cause is unknown. Production workspace bytes are unchanged from the earlier passing `012707a` scope. | [Preserved failure](../bench/validation/m7-linux-gate-e0ca-failure-emulated-2026-10-11.json), [earlier complete pass](../bench/validation/m7-linux-gate-0127-emulated-2026-10-11.json) |
| Latest complete local solver suite | Frozen `7da9579`: all 39 original controls pass independently of its failed workspace gate. This includes pinned Z3, signed HTTP whole-session replay and the real mount-restoration control. Source, full standard, real readers and repair speed remain pending. | [Current solver scope](../bench/validation/m7-linux-solver-7da-emulated-2026-10-11.json), [earlier passing scope](../bench/validation/m7-linux-solver-091e-emulated-2026-10-11.json) |
| Earlier complete local solver attempt | Frozen `777b86a`: 37/38 pass. The fresh HTTP reader matches direct solver output but starts Z3 instead of replaying only signed remote replies and the silence certificate. A cold phase diagnostic identifies transport construction consuming the deadline. The configured setup fix now passes the complete workspace and solver gates; earlier established-transport failures retain separate scopes. | [Complete failure](../bench/validation/m7-linux-solver-777-emulated-2026-10-11.json), [cold replay investigation](validation/m7-http-session-replay-777.md) |
| Earlier integrated workspace-test attempt | Frozen `749255e`: strict Clippy and 480/483 tests passed with two independent test cases and two compiler jobs. The original four-reader and blocked-worker MCP fixtures passed; two HTTP serving checks and the original five-second job-coalescing control failed. No whole cause is established, and later gate steps were not reached. | [Full failure](../bench/validation/m7-linux-gate-749-failure-emulated-2026-10-11.json) |
| Earlier passing complete Linux workspace gate | Source `5fd7986`, all 439 workspace tests and every gate step passed, including the original HTTP concurrency/serving checks. It covers caller-entry coalescing and the dev/test arithmetic optimization; real-reader and source-candidate acceptance remain separate. | [Gate record](../bench/validation/m7-linux-gate-5fd-emulated-2026-10-10.json) |
| Earlier full workspace-test attempt | Frozen `f6508ba`: Python controls, fmt and strict all-target Clippy passed; nextest ran all 471 tests, with 452 passed and 19 failed. Six HTTP cases, the legacy MCP responsiveness fixture and twelve freshness/lease fixtures failed under their original assertions. Initial coverage failures need acquisition-stage diagnostics; causes are not established. Remaining gate steps were not reached. | [Failure](../bench/validation/m7-linux-gate-f650-failure-emulated-2026-10-10.json) |
| Integrated gate after focused fixes | Frozen `44af609` stopped during strict all-target Clippy on a timeout-retention test assertion lint. The same assertion is corrected in `7fcb720`; its full attempt then passed strict Clippy but failed 9/478 workspace tests under default inter-case scheduling. No full gate or product pass is claimed. | [Failed scope](validation/m7-integrated-gate-44af609.json) |
| Subsequent integrated workspace-test attempt | Frozen `7fcb720`: 469/478 passed, nine failed (seven HTTP/reload cases, legacy MCP and the four-reader 100 ms fixture). Earlier initial freshness/lease coverage failures were not observed. Default eight-CPU scheduling was used rather than the earlier two-case/two-build execution. A complete gate for the latest SDK/freshness changes remains required, with unchanged internal fixture concurrency and all original assertions. | [Full failure](../bench/validation/m7-linux-gate-7fcb-failure-emulated-2026-10-11.json) |
| Current frozen installed product | Frozen `b36acc7` explicit Sail/shim binary build completed in 301 seconds. Immutable hashes bind the product and unchanged reader collectors; its workspace gate passes all 485 tests. It predates the configured HTTP setup fix and remains distinct from source/product acceptance. | [Build identities](validation/m7-current-product-build.json) |
| Current installed MCP transport | Frozen `b36acc7` passed all five unchanged installed transport/lifecycle controls, including normal EOF exit. The worker was stopped/cancelled before any proof. This does not qualify Current reports, live-reader latency, source allocation or M7 overall. | [Installed smoke](validation/m7-installed-mcp-b36.md) |
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
| Latest complete standard attempt | Frozen `1dc`, default mode: 159 units, 156 Same, Anvil diagnostic ordering Unstable, zero Different, two Verismo infrastructure timeouts at the original 3,600-second limits. Pass-through fails; fill/warm are not reached. Earlier sleeping-thread observations do not establish a cause, and this old harness discarded inner timeout streams. | [Complete failure](../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json), [earlier observation](validation/m7-standard-verismo-timeout-observation.json) |

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
