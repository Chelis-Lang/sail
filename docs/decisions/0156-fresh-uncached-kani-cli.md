# 0156: Fresh experimental Kani checks through the CLI

- **Status:** implemented candidate; native and independent acceptance pending
- **Date:** 2026-11-15
- **Extends:** [ADR 0128](0128-kani-bounded-observations.md), [ADR 0154](0154-kani-original-driver-attempts.md), [ADR 0155](0155-kani-original-payload-materialization.md)

## Context

The original-driver component has bounded native execution evidence. A consumer
needs explicit setup and a fresh hand-written harness check without managing
internal bundle, Rust and tool-stage prefixes. Authentication of a reusable
source closure remains unfinished. ADR 0128 requires that authority before
result caching, while permitting uncached experimental bounded checks.

## Decision

Route `sail verify --backend kani` before Verus preparation and after the existing
retired-requirement refusal. Require the exact native harness name (including its module path where present) and explicit
positive unwind bound. Accept an optional project, prepared offline Cargo home,
child-process budget and JSON output. Refuse other Verus, background, portfolio,
forwarded or duplicate options in this mode. Default verification is unchanged.

Expose `sail engine install/status --backend kani` for the pinned original Linux
x86-64 tools. Package the exact original preparation helpers and pins with the
CLI, then run the downloader in Python isolated mode. No downloaded installer is
executed. Authenticated immutable original prefixes and their private tool stage
are shared only as tools; each check gets a fresh private capture/target directory.
Corrupt or conflicting prefixes refuse. Original captures remain diagnostic
material under Sail's cache and are not proof receipts.

Print bounded_success, counterexample or bound_insufficient, with explicit
experimental/backend/unwind identity. Exit 0 only for bounded_success, 1 for the
other two outcomes and 2 for refusal. Ctrl-C cancels the original driver through
the existing bounded process-group cleanup. Do not create Verus jobs, ordinary
verified verdicts, proof/currentness authority or result-cache entries. User
source, dependencies and Cargo configuration are ordinary local inputs; source
closure authentication is explicitly false.

Add the internal dependency `sail-solve -> sail-kani` to the layering table and
select `sail-kani` for the CLI's public package closure, extending ADR 0098. No
third-party dependency is added. This is package preparation, not authorization
to publish an exact final release. Other platforms refuse this experimental
backend until an original pinned runtime is qualified there.

## Validation and limits

Parser controls cover exact identity, required and duplicate bounds, timeout
limits, output format and separation from forwarded verifier arguments. Native
CLI/build/full-gate and direct original-Kani comparison remain pending. Existing
component evidence does not qualify the new frontend. Mac build/lint validation,
whole-pilot comparison and packaging checks remain explicit gates. No speedup,
cache reuse, selected release or general Rust verification claim follows.

The new lock has exactly one added internal CLI edge. Independent review compares
all 256 package records and their original bytes after removing only that edge;
the parser's 25-package closure, seven parser source bindings and 61 checked
context files remain exact. Advance only the parser checker's Cargo.lock binding
to `b5a0719d6ee0a88e59c871b5d8f2b2856e3ed2633471ea90ed317c92c417fa8a`.
Every body, stencil, import, other digest and broken control remains unchanged.
Original unknown-lock gate failure remains recorded; actual model/refinement
controls are required before this metadata update is qualified.

The first direct pair's whole-export comparison fails on eleven observed values:
six named CBMC wall-time statistics, two duration fields, timestamp and two output
paths under the explicitly different fresh target directories. Preserve that
strict failure and all original bytes. The bounded paired runner subsequently
compares the complete recursive export, allowing only finite nonnegative values
for `runtime_convert_ssa_s`, `runtime_decision_procedure_s`,
`runtime_post_process_s`, `runtime_postprocess_equation_s`, `runtime_solver_s`,
`runtime_symex_s`; nonnegative integer result/summary `duration_ms`; a valid
metadata timestamp; and equal relative `harness_metadata[0].goto_file` and
`project.output_dir` paths under the two exact requested targets. Retain every
actual difference. All other keys, values, multiplicities, native diagnostics,
property records, verdicts, bounds and tool settings stay exact. This is a new
Kani paired-observation acceptance scope, not normalization of production
outputs/queries or a change to the default-Verus conformance oracle.

Independent review rejects the first bounded comparator: lexical `relative_to`
admitted `..` paths, ordinary JSON loading collapsed duplicate fields, and raw
stdout/stderr were retained but not compared. Preserve that rejection and its
probes. The successor rejects duplicate JSON keys, requires canonical absolute
output spelling beneath each exact target with no dot/parent components, and
compares every original stream line with exact order/count/line ending. Only
finite nonnegative full-line values for the same six named `Runtime` observations,
`Verification Time`, Cargo's exact `Finished dev profile` duration, and the
request-bound `Reading GOTO program from file` path may differ. Every other
stdout/stderr byte, diagnostic, property/check and verdict stays exact. These
comparison rules require independent adversarial validation and native repetition;
previous tree-only pairs do not qualify stream parity.

Keep the separate temporal workspace lock consistent with the CLI edge: add only
its sail-kani package record and sail-solve dependency. Its extra sha2 version
requires the existing explicit `sha2 0.10.9` lock spelling; all existing package,
source and version records remain exact. D.I.V.E.'s revision, source and private
boundary do not change. Native extension fmt/Clippy/tests are required for this
metadata correction; main executable inputs are unchanged.

Do not infer exactness from the presence of `::`: a root-level hand-written
harness has an exact single-component native name. The original-driver
`--exact` selector and strict export identity provide the binding. The frontend
therefore accepts nonempty root names as well as complete module paths; missing,
control-character and excessive names still refuse. The initial frontend's
extra namespace-shape restriction is corrected within this unmerged work.
Native original-driver/root-name comparison remains required for the successor.
