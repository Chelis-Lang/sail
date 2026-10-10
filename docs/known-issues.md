# Known issues and acceptance limits

This page separates product limitations, upstream behavior and unresolved test
failures. It describes the current M7 development branch; it does not declare
that branch released or accepted. The [roadmap](ROADMAP.md) tracks remaining work.

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
  binds these checks to the full combined patch. An independent ordinary Linux build
  is underway; no runtime or genuine cross-edit hit is yet qualified.
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
  The independent active workspace preparation and frozen reader collectors
  are still pending. This is useful setup evidence, not a passed timed proof,
  one-driver batch or latency census; see [the preparation record](validation/m7-reader-prepared-baseline.md).
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

The local Linux container is supported for development and acceptance runs under
emulation, with that environment recorded. Native Linux reference measurements
can follow later. Docker VM interruptions invalidate incomplete runs; they do
not change a test deadline or turn observed failures into passes.

## Outstanding validation on the development branch

| Check | Latest retained scope and result | Evidence |
|---|---|---|
| Latest passing complete Linux workspace gate | Source `5fd7986`, all 439 workspace tests and every gate step passed, including the original HTTP concurrency/serving checks. It covers caller-entry coalescing and the dev/test arithmetic optimization; real-reader and source-candidate acceptance remain separate. | [Gate record](../bench/validation/m7-linux-gate-5fd-emulated-2026-10-10.json) |
| Preceding established-transport gate | Source `a9dc753`, 435/436 tests passed. The verified paused-initiator serving check failed after real transport/trust establishment; response-to-classification took 135.738 ms. Its entire CPU versus scheduling cause remains unknown. Later gate steps were not attempted. | [Failure](../bench/validation/m7-linux-gate-a9-failure-emulated-2026-10-10.json) |
| Subsequent signature CPU controls | Sixteen strict valid and forged checks took approximately 310/320 ms before package-only dev/test arithmetic optimization, and 4–7 ms afterward. All 10 HTTP cases, five signed-format negative cases and affected strict Clippy checks passed, followed by the complete `5fd7986` gate. This does not identify the original failure's entire cause or establish a release-build speedup. | [Diagnostic](validation/m7-shared-signature-cpu-diagnostic.json) |
| Earlier cold-lookup gate | Source `d12d4af`, 431/436 passed; five HTTP fixtures failed during cold lookups. Original deadlines and assertions are retained. | [Failure](../bench/validation/m7-linux-gate-d12-failure-emulated-2026-10-10.json) |
| Earlier complete Linux workspace gate | Source `62ad095`, 436/436 tests plus all gate steps passed; two test workers | [Record](../bench/validation/m7-linux-gate-62ad-emulated-2026-10-10.json) |
| Latest real solver/fault suite | Source `5fd7986`, all 37 tests passed in 182.337 seconds; the earlier timed-out shared-entry case passed in 13.751 seconds with its original 180-second test and 60-second session limits | [Record](../bench/validation/m7-linux-solver-5fd-emulated-2026-10-10.json) |
| Latest complete end-to-end attempt | Source `5fd7986`: legacy output, JSON/MCP and cold/warm seed-4 portfolio checks passed; the first cold managed verification timed out at its unchanged 240-second limit. Repair was not reached. Default fixture scratch cleanup removed its inner stdout/stderr evidence, so the timeout's cause is not established. | [Failure](../bench/validation/m7-linux-e2e-5fd-failure-emulated-2026-10-10.json) |
| Subsequent isolated repair phase | Same frozen `5fd7986` product and selected runtime: the first repair job remained running at the unchanged 120-second workflow limit. Retained worker stderr ends in dependency compilation; no complete result or later scenario was admitted. All 331 CLI operations returned within their bounds; MCP was not reached. Cleanup reported no errors. | [Failure](../bench/validation/m7-linux-repair-5fd-phase-failure-emulated-2026-10-10.json) |
| Subsequent HTTP-labelled diagnostic suite | Frozen `987aa39`: 9/12 passed, three shared-reader checks failed with their original bounds; the separately selected direct stalled-response case passed at 119.775 ms. Serial case scheduling retained all internal concurrency. No complete workspace gate or causal resolution is claimed. | [Diagnostic](validation/m7-http-987-followup.json) |
| Subsequent complete workspace gate | Frozen `7dc855d`: 437/439 tests passed; verified paused-initiator delivery took 293.834 ms against 250 ms, and cold constructor plus a stalled request failed its original 250 ms elapsed assertion. No deadline or cryptographic check changed; the entire causes remain under investigation. Later gate steps were not reached. | [Failure](../bench/validation/m7-linux-gate-7dc-failure-emulated-2026-10-10.json) |
| Preceding complete real solver/fault suite | Source `d12d4af`, all 37 tests passed in 114.446 seconds; the earlier timed-out shared-entry case passed in 9.652 seconds with unchanged assertions | [Record](../bench/validation/m7-linux-solver-d12-emulated-2026-10-10.json) |
| Earlier real solver/fault suite | Source `62ad095`, 36/37 passed; `unverifiable_shared_entries_never_change_an_answer` timed out at the unchanged 180-second test limit; original cause remains unresolved | [Failure](../bench/validation/m7-linux-solver-62ad-failure-emulated-2026-10-10.json) |
| Isolated shared-entry fault diagnostic | All five corruption cases and both sessions passed in 11.340 seconds; this does not explain or erase the full-suite timeout | [Diagnostic](../bench/validation/shared-signature-fault-diagnostic-linux-emulated-2026-10-10.json) |
| Full default corpus | Frozen `1dc` passed all 147 fast units in pass-through, fill and warm; standard phase interrupted by container loss and lacks a complete report | [Record](../bench/validation/m7-default-corpus-1dc-interrupted-emulated-2026-10-10.json) |
| Complete standard retry, still running | Frozen `1dc`, default mode: plain stock Verus on verismo exceeded the unchanged 3,600-second unit limit and was killed by the harness. Remaining units are still running; cache phases have not started. Retained sleeping-thread observations do not establish a cause or an upstream defect. | [Observation](validation/m7-standard-verismo-timeout-observation.json) |

These records cover their frozen artifacts, not every later source candidate.
The complete current solver suite passes with retained phase diagnostics; this
does not identify the cause of the earlier timeout.
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
