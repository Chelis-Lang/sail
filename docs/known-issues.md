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
  stock transcript instability. A new candidate must execute this gate before
  selection; static review is insufficient.
- Earlier real reader and cold repair attempts exceeded their original limits
  or could not establish Current evidence. Their records remain in
  `docs/validation/`. The a6/b346 diagnostic established genuine public Current
  baseline and active-copy proofs, then failed its Python reader census:
  125/200 samples satisfied the complete freshness/revision admission checks;
  result p95 was 402.821 ms against the unchanged 200 ms limit. All samples
  retained the same actual selected verifier. The frozen collector did not record
  the failed samples' exact freshness labels or reasons; their cause is being
  profiled. Rust and retained CLI/MCP scenarios were not attempted after this
  failure. See [all retained samples](validation/m7-a6-b346-reader-census-failure.json).
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
| Latest Linux workspace gate | Source `d12d4af`, 431/436 tests passed; five HTTP-cache concurrency tests failed. Cold construction/dispatch exhausted the serving interval in recorded traces; cause and correction remain under review. Later gate steps were not attempted. | [Failure](../bench/validation/m7-linux-gate-d12-failure-emulated-2026-10-10.json) |
| Earlier complete Linux workspace gate | Source `62ad095`, 436/436 tests plus all gate steps passed; two test workers | [Record](../bench/validation/m7-linux-gate-62ad-emulated-2026-10-10.json) |
| Real solver/fault suite | Same source, 36/37 passed; `unverifiable_shared_entries_never_change_an_answer` timed out at the unchanged 180-second test limit; cause unresolved | [Failure](../bench/validation/m7-linux-solver-62ad-failure-emulated-2026-10-10.json) |
| Isolated shared-entry fault diagnostic | All five corruption cases and both sessions passed in 11.340 seconds; this does not explain or erase the full-suite timeout | [Diagnostic](../bench/validation/shared-signature-fault-diagnostic-linux-emulated-2026-10-10.json) |
| Full default corpus | Frozen `1dc` passed all 147 fast units in pass-through, fill and warm; standard phase interrupted by container loss and lacks a complete report | [Record](../bench/validation/m7-default-corpus-1dc-interrupted-emulated-2026-10-10.json) |

These records cover their frozen artifacts, not every later source candidate.
The complete current solver suite is being checked with retained phase diagnostics.
HTTP serving/coalescing assertions and their original deadlines remain unchanged;
the earlier gate pass does not cover the latest failed run.
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
