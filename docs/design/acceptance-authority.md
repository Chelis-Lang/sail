# Invocation, coverage and result authority model

Status: checked model code for the existing Verus path. This component models
invocation lifecycle and evidence construction, not the production compiler
adapter. It does not complete production source sharing, observation refinement
or CLI/MCP integration. The [acceptance design](evidence-acceptance-core.md) and
[shared proof checker](acceptance-proof-checker.md) describe the owning scope.

## Exact evidence context

Requests, started invocations and observed verifier documents carry the shared
`ModelContext`. It contains the original request bytes, structured whole/partial
scope, runtime identity, verification mode and the actual modeled input map.
Input keys include role and lexical path. Present values include resolved path
and bytes; absence and unreadable observations are distinct. There is no opaque
input digest whose equality is assumed to establish equal effective inputs.

Target identities separately preserve package, exact Cargo name, target kind,
source and invocation bytes. `foo-bar` and `foo_bar` cannot collapse through
crate-name normalization. Every requested target must have its own matching
completion; missing failed siblings remain missing even when they emitted no
Cargo artifact. Duplicate requested targets and ambiguous duplicate observed
targets are rejected. `injective_coverage` proves that a single completion
cannot cover distinct requested targets.

## Lifecycle and construction authority

There is no caller-provided `complete`, `success`, `current` or
`invariant_valid` Boolean that grants authority. Private evidence structures
are built by the following guarded model transitions:

| Transition | Required observations and checks |
|---|---|
| `start` | A well-formed exact invocation context and scope; direct or engine origin. A fallback cannot enter through this constructor. |
| `observe` | Append a verifier document to the actual started run's observation history. The document remains a candidate, not proof authority. |
| `interrupt` / `malformed` | Permanently prevent that stream from completing; later observations cannot restore Running. |
| `complete` | Running stream, normal zero process exit, ordered completion point, at least one verifier document, matching run ID/target/scope/context for every document, checked successful records and well-formed recorded solver sessions. |
| `finish_project` | Normal successful project termination and a completion set whose contexts, observation points and origins match the selected direct/engine/fallback path. |
| `validate_scoped` | Exact acceptance context, a fresh acceptance point, complete one-to-one requested target coverage and exact requested scope. |
| `validate_current_complete` | All scoped checks plus whole-target scope. |

`CompletedInvocation` retains its actual validated model run and terminal
observation. It exposes read-only spec projections for its context, completion
point, invocation, documents and derived solver transcript inventory.
`ValidatedResult` immutably borrows the request and sealed project candidate and
records the context and observation point at which acceptance occurred. Neither
type has a public constructor or deserialization path.

The context contains mathematical maps. Context-sensitive admission is therefore
a guarded proof/model transition; executable Rust is not claimed to inspect a
ghost map. Concrete byte, target, scope and document validators remain executable
and proved. Production integration must implement and refine these guards.

## Records and summaries

A verifier document contains observed function records with qualified names,
verdicts, establishment states and explicit evidence tiers, a claimed summary, and a terminal verifier
event. The concrete `validate_document` checks that the terminal event records
zero errors and every function is verified, confirmed and has its required evidence tier. It derives successful
function/verified counts from the actual record collection and requires exact
agreement with the summary; failed, unknown and provisional counts must be zero.
Malformed/VIR output, missing terminal success, inconsistent counts and
unconfirmed records cannot authorize completion.

A complete zero-function invocation is permitted: it still needs a matching
started invocation, a nonempty document collection, explicit successful verifier
termination and normal process completion. No inference from an empty query or
function collection creates an invocation-completeness fact.

Every document also carries its observed solver-session inventory. Completion
checks its shape, and `observed_transcripts` derives the completion's inventory
by concatenating **all** document inventories in order. Callers cannot replace
it with a separate completion-success flag or supply a different final inventory.
The shared composition matches every recorded transcript against the query-origin
component. The compiler/side-channel adapter must still establish that these
observations include every actual session; this model does not obtain capture
completeness merely by calling a sequence an inventory.

## Engine failure, retained jobs and repair

`engine_failed` creates a private failure token from an actual started engine
run and observed nonzero/interrupted/malformed termination. `start_fallback`
requires that token, a fresh run ID and start point, matching request/mode and a
whole-target invocation. It starts with an empty observation history. Old engine
or partial-phase documents cannot match the fallback's run/context/scope header.
The project fallback path accepts only completions of fresh fallback invocations;
it cannot mix in old engine completions.

A fresh independent full fallback can establish ordinary current complete
verification. Repair acceptance preserves the production contract's stricter
rule: an engine fallback does not accept the repair. The repair predicate uses
the same validated result token, requires whole-target evidence, and finds the
selected named function in a requested target's completed records. Preliminary
partial evidence cannot promote itself to whole-target authority.

`project_job` refreshes retained evidence through the same current complete
factory at a new context and observation point. Changed inputs make a retained
success noncurrent. Exact restoration can establish Current again after a new
matching observation; no old Current label is trusted as evidence about today.

## Checked controls

The model checks coherent nonempty and zero-function successful lifecycles,
exact partial versus whole scope, missing siblings, duplicate observations,
interrupted/malformed streams, failed/unknown/provisional/unconfirmed functions,
context-mismatched output, retained-job input change and exact revert, and fresh
full fallback after a failed partial engine attempt. The fallback control checks
that ordinary verification succeeds while repair acceptance remains refused.

`example_candidate` constructs concrete target and confirmed-function records
and then obtains evidence through the checked lifecycle. It accepts a shared
context and complete observed transcript sequence for the composed positive
witness. The composed witness must include real nonempty input/query evidence;
legitimate no-SMT invocations may still have an empty session inventory.

Six deliberate guard removals must fail ordinary full-crate proof checking:
whole/partial promotion, missing function confirmation, missing requested-target
coverage, completing an invocation without documents,
accepting a stale context and accepting a document from another context. They
retain the original claims rather than weakening the safety theorem.

The local shared-driver run reports `verified: 55`, `errors: 0`, normal exit
zero and entire-crate verification. That count includes the shared context's
two checked lemmas. The pin remains upstream stock Verus
`0.2026.10.18.7d82994`, commit
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1`, pinned Z3 `4.16.0`.
Use the owning checker to capture the shared source graph, validate runtime
identity and run the complete driver with `--no-cheating`; this source imports
the shared context and is no longer a standalone one-file proof root. Native
workspace gates and the composed proof run have their own owning qualification.

## Refinement and remaining trust boundaries

| Model boundary | Production source | Remaining obligation |
|---|---|---|
| Exact target and invocation | Managed `TargetIdentity`, `cargo_target`, requested coverage and failed compiler calls | Relate actual Cargo metadata and compiler invocation to the model's complete target inventory and exact fields. |
| Verifier lifecycle | Driver subprocess output, structured verifier documents and normal termination | Attribute observations to the actual pinned process, preserve every document, validate real framing/error/termination behavior, and connect trusted Verus invocation completion to the exact requested work. |
| Record checks | `Report::invariant_violations`, `Summary::of`, receipt `complete` | Refine full production establishment, evidence tiers, obligation ownership/uniqueness, provenance and schema checks to these checked records; this model does not silently assume that all those checks have been proved. |
| Solver inventory | Shim/side-channel capture and query association | Establish complete actual session capture and connect the observed streams to the query component's admitted origins. |
| Input observation | Managed read checks, currentness and durable refresh | Refine actual compiler-input closure, filesystem/process observations and accepted observation points on Linux/macOS. |
| Repair/job/presentation | `repair::accepted`, batch selection, retained result frames, CLI/JSON/MCP | Connect actual selection errors, engine origin and every exposed success surface to the shared authority relation. No production surface is routed through this model yet. |

The trusted verifier's translation and mathematical soundness remain outside
this proof, as in the design. The external observation adapter must faithfully
identify the executed runtime and process, complete compiler-input/session
closure, structured records and termination. Model constructors do not prove
those I/O facts. Rust/Verus/vstd, Z3, build/deployment and the documented
observation-interface assumptions remain trusted.

There is no new crate edge, backend selection or production default. Corpus,
compiler and solver input bytes are unchanged; no conformance or performance
improvement is claimed by these model checks.
