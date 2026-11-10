# Next release: specification feedback through Verus mutation

Status: owner-selected feature scope, 2026-11-04; headline of v0.3.0 since
2026-11-09; not implemented as a general CLI/MCP product.
[ADR 0100](../decisions/0100-mutation-first-roadmap.md) records the ordering
and [ADR 0109](../decisions/0109-oyster-retired-v030-scope.md) the release
scope. Existing benchmark catalogues and neutral literal campaigns retain
their separate scope.

## User and session

A developer or agent has a Verus project that verifies. They request a bounded
campaign, inspect mutations whose changed implementations still verify, and
strengthen the intended specifications where that exposes an actual omission.
CLI, JSON and MCP expose the exact changes and per-function results. Ordinary
verification checks the revised implementation/specification. The tool supplies
feedback; it does not silently edit or weaken specifications.

## Required behavior

- Work on existing Verus programs independently of Oyster and neutral contracts.
- Make bounded executable-code mutations in owned copies; preserve specification,
  proof and ghost code and never edit user source in place.
- Retain an attributable baseline, source/runtime identity, exact mutation
  inventory and original backend observations.
- Report killed, survived and inconclusive outcomes per function. Show omitted,
  unsupported, cancelled and resource-limited work. A process exit alone is
  neither a proof nor a confirmed kill. Unknown remains inconclusive.
- Treat survivors as observations of specification strength, not adequacy or
  vacuity claims.
- Deliver CLI, JSON and MCP together, with agent-sized survivor feedback and
  access to exact mutations/evidence. Define the interface in the implementation
  design; existing neutral RPCs do not constitute the general Verus product.
- Enforce explicit campaign/time/resource bounds and cancellation. Retain partial
  observations without presenting an incomplete campaign as complete.

## Work order and acceptance

1. Diagnose reproduced native literal-mutant Unknown cases before campaign
   expansion. Isolate minimal examples and establish evidence that can confirm a
   kill. Preserve original results; scalar witnesses stay a separate family.
2. Qualify the executable-code mutator, then bounded execution and reporting
   through the existing Verus path. #140 owns the feature; #141 owns campaign
   strength/reporting. Neither depends on neutral-lowering completion.
3. Exercise real projects: valid baselines, weak-spec survivors, rejected body
   changes, Unknown, unsupported shapes, budgets, cancellation, source edits and
   stale evidence. Preserve non-executable bytes and require valid admission.
4. Qualify CLI/JSON/MCP agreement, native Linux/macOS behavior and conformance.
   Measure complete cold/warm campaigns against direct-backend controls,
   including setup, copies, failures, total CPU and memory. Query-hit counts or
   a single fixture do not establish general savings.

Changed-proof scheduling and cold-run improvements proceed in the performance
lane. Frontend feasibility and the Kani backend are not dependencies of this
feature. The release is v0.3.0; final composition and publication remain owner
decisions.
