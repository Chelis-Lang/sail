# Next release: specification feedback through Verus mutation

Status: owner-selected feature scope, 2026-11-04; headline of v0.3.0 since
2026-11-09; not implemented as a general CLI/MCP product.
[ADR 0100](../decisions/0100-mutation-first-roadmap.md) records the ordering,
[ADR 0109](../decisions/0109-oyster-retired-v030-scope.md) the release
scope and [ADR 0111](../decisions/0111-mutation-evidence.md) the evidence
rule. Existing benchmark catalogues and neutral literal campaigns retain
their separate scope.

## Current state

Implemented (library only; no CLI, JSON or MCP surface yet):

- The seeded generator lives in `crates/sail-verus/src/mutate/` (lexer, site
  finder, `unique_window`, catalogue and sidecar writer). It is internal to
  the workspace and carries no API stability promise. `cargo xtask
  gen-mutants` is a thin command line over it, and `cargo xtask gen-mutants
  --check` regenerates every committed catalogue from its sidecar, replays the
  recorded stock oracle and requires the committed file byte for byte. The
  check needs the fetched corpus sources, so it runs locally (the standard
  and deep conformance tiers fetch them); CI covers the generator through a
  checked-in fixture crate whose catalogue must regenerate byte for byte.
- Generator version 3 reads `spec(checked)`, `proof(axiom)` and mode
  attributes, and places a function body by the token after its brace group,
  so a brace inside a `requires`/`ensures` clause (`match r { .. },`) is no
  longer taken for the body. Version 2 labelled `spec(checked)` functions as
  exec and stopped at such a clause brace, which mislabelled
  `ironkv-constant-002` and `capybarakv-constant-002` and left the real body
  of every such function unscanned. The four catalogues in `corpus/mutants/`
  are regenerated with version 3 and settled again by the stock oracle.
- `sail_verus::mutate::exec::plan` is the exec-only planner. Its fail-closed
  mask admits only tokens of exec function bodies outside `proof { }`
  blocks, `assert`/`assume` forms, loop `invariant`/`decreases`/`ensures`
  clauses, `Ghost`/`Tracked` forms (construction and patterns, associated
  calls with any turbofish such as `Ghost::<T>::new(e)`, after any leading
  path, and types), `let ghost`/`let tracked`, attributes and macro
  arguments; spec and proof functions and every signature clause are outside
  it. A function the mask cannot classify (an unknown macro, a spec-only
  token left over, a spec closure, a ghost field or ghost-parameter call, a
  qualified or method-chained `Ghost`/`Tracked` form, a body attribute naming
  a mode, a trigger or externality, an unplaced loop or body brace) is
  reported `unsupported` with its
  reason and yields no sites. Classes: the catalogue's body operators
  restricted to the mask plus `loop-bound`, `off-by-one`, `swapped-args`,
  `dropped-else` and `early-return`. Spec-mutating classes have no exec
  class, so a plan cannot contain them. Every site is re-lexed after it is
  applied and dropped if any token outside the replaced range changes.

Working implementation: independent `sail_api::campaign` evidence types and
pure ADR 0111 classifier, with native focused fixture controls. This is not a
qualified runner or CLI/MCP product; [ADR 0114](../decisions/0114-verus-campaign-execution.md)
sets the execution design and the [campaign record](../validation/verus-campaigns.md)
keeps the remaining gates explicit.

Not yet built: campaigns (owned copies, scheduling, scoped verification,
confirmation of survivors), outcome evidence (#664), and the CLI, JSON and MCP
surfaces. The mask has been surveyed on the four pinned corpora (supported
share and site counts in the generator PR) but not qualified by stock runs of
exec mutants.

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
- Classify each mutant by [ADR 0111](../decisions/0111-mutation-evidence.md):
  `rejected`, `survived` (`survived-scoped` until an unscoped run confirms
  it), `inconclusive` with a named reason, `unviable` or `unsupported`.
  `rejected` needs a Verus diagnostic attributed to the mutated function that
  is not only a resource limit, a baseline twin that proves and used no more
  than half of any check's rlimit, and the same outcome on every repeated
  run. Survival is judged against the baseline's own failures, not the exit
  status. A process exit alone is neither a proof nor a detection.
- Show each rejection's evidence sub-class, `verifier-rejected` (solver
  `unknown` or other failure) or `counterexample-confirmed` (native `sat`),
  and keep solver answers verbatim. The score is
  `rejected / (rejected + survived)`. Inconclusive, unviable, unsupported,
  omitted and cancelled mutants are listed separately and excluded from it.
- Say plainly that detected means the specification and its proof reject the
  change, not that the specification provably forbids it. Treat survivors as
  observations of specification strength, not adequacy or vacuity claims.
  The classification appears only in campaign views, never in plain
  `sail verify` output or verdicts.
- Deliver CLI, JSON and MCP together, with agent-sized survivor feedback and
  access to exact mutations/evidence. Define the interface in the implementation
  design; existing neutral RPCs do not constitute the general Verus product.
- Enforce explicit campaign/time/resource bounds and cancellation. Retain partial
  observations without presenting an incomplete campaign as complete.

## Work order and acceptance

1. Diagnose reproduced native literal-mutant Unknown cases before campaign
   expansion. Isolate minimal examples and establish evidence that can confirm a
   kill. Preserve original results; scalar witnesses stay a separate family.
   Done for the evidence rule: the #664 spike
   ([record](../validation/mutation-evidence.md)) found that real rejections
   rest on `unknown "(incomplete quantifiers)"`, and ADR 0111 records how they
   count.
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
