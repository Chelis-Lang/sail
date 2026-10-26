# Invocation, coverage and result authority

Status: checked lifecycle model plus source-shared executable validation of
actual Verus report and selected-target records. Production paths below use the
checked validators. Complete compiler, solver-stream and filesystem observation
refinement remains separate; a report field does not prove the physical
observation occurred. The [acceptance design](evidence-acceptance-core.md) and
[proof checker](acceptance-proof-checker.md) describe the owning scope.

## Production report and scope validation

`ResolvedRequest` retains the independently resolved pre-execution target
inventory, run identity, workspace, original options, engine identity, context
digest and original input revision. The source-shared `complete_request_ordered`
factory checks those against the actual project, requires exact one-to-one
coverage, and checks every requested whole-target report before minting an
immutable `CompleteRequest` borrow token. Compile-only and partial original
requests, swapped identities, missing or ambiguous targets, and non-Current
results cannot mint that token. Matching failed observations can still be
Current; correspondence alone is not proof authority.

The pure factory proves correspondence between its concrete record inputs.
Its pre-execution producer, original-submission argument mapping, actual Cargo
scope resolution, complete process observation and retained service adapters
remain separate proof obligations. Digest equality retains collision-resistance
assumptions and does not establish complete compiler-input equality. Native
request capture and retained admission are a separate integration slice; the
factory alone does not prove all CLI, JSON, MCP or repair callers.

`crates/sail-api/src/acceptance.rs` checks actual public `Report`, `CrateReport`,
`FunctionReport`, `TargetResult` and `ProjectResult` fields. Cargo and stock
Verus compile the same executable bodies through the syntax envelopes in
`sail-acceptance`. No copied Boolean `report_valid` projection or external
specification assumes that a report is correct. The checker captures original
type declarations and their complete field inventory, then mechanically removes
serde and presentation-only declarations while retaining every authority-read
field, enum variant and the actual schema constant. This restricted declaration
mechanism is part of the proof tool's trusted source-selection boundary.

Factories check verification-enabled forwarded arguments, normal zero exit, no setup error, nonempty crate inventory,
explicit zero verifier errors, no VIR error, every function's confirmed verified
verdict and evidence tier, provenance/count consistency, every summary counter,
obligation ownership and global ID uniqueness. Count accumulation checks overflow.
A successful crate with zero functions remains legitimate; an empty crate
inventory does not establish completion.

Policies remain distinct: exact partial receipts may lack whole-crate `success`;
whole-target authority requires explicit `success: true`, and repair additionally
rejects engine fallback provenance. Whole-target factories inspect actual
forwarded scope arguments after `--`. A stored completion flag cannot overrule
an explicit partial command or partial-scope reuse.

Private `CompleteReport` and `CompleteSelection` tokens immutably borrow the
checked actual records. Only checked factories construct them. The sibling
`acceptance_sort.rs` proposes indices and cannot access their private fields.
The factory independently derives every actual obligation ID, checks the complete
forward/inverse index permutation and validates strict byte order. A broken
sorter can cause refusal but cannot hide, duplicate or replace an obligation.
Sorting is O(n log n); checked certificate traversal is O(n) plus ID bytes.
Target matching retains the existing O(requested × observed) approach with
uniqueness checked in both directions.

The pinned verifier explicitly allows `--no-verify` to return success, zero
verified/error counts and an empty function inventory. Every report-authority
policy rejects that compile-only mode. The actual per-package decoder controls
its report marker in both directions; missing or invalid decoding records an
error and withholds proof authority. Other implicit logging arguments are not
inserted into the reported command. Legitimate enabled zero-function runs still
pass. See [VI-0035](../verus-issues/VI-0035-no-verify-success.md).

Receipt format five invalidates earlier receipts. Retained jobs and ordinary
baseline files privately record a producer-admissibility revision; missing older
markers default to unrecorded, and their target-completion flags are withheld.
This preserves input currentness, reports and historical verdicts. Cached frames
bind the revision, and copying old evidence into a fresh baseline preserves its
withheld flags. The marker is neither authentication nor sufficient proof
authority; the actual report still passes all checked predicates.

| Production consumer | Checked authority |
|---|---|
| Managed receipt `complete` | Actual successful exact-scope report |
| Managed fresh/reused target completion | Whole-report factory plus invocation and scope guards |
| Successful managed requested coverage | Independent Cargo package/name/kind selection matched one-to-one to observations |
| `repair::accepted` and retained-job repair envelopes | Same selected-project factory after existing refresh and batch-error checks |
| Evidence inventory and MCP compact completion | Actual whole-target report revalidated rather than completion flag alone |

Schema two retains its layout. A stored `ProjectResult` has selected-result
flags, not the independent original expected-target inventory. Its repair
factory therefore proves authority for supplied selected targets. The live
runner checks independent request coverage while it still owns the resolved
expected set. Persistently carrying that independently bound scope through
every stored success surface requires a separate sealed-storage or schema
refinement. Selected flags alone do not prove original request completeness.

Similarly, refusing Historical/Unknown inspects the actual `Currentness` enum;
the input component and production adapters must justify Current with fresh
observations. A failed run can be Current without positive proof authority.

## Constructive record refinement and controls

`proofs/refinement/authority.rs` executes a fieldwise mapping from actual function
records into the lifecycle model. It preserves exact UTF-8 names, all verdicts,
establishment and every optional evidence strength. `ObservedStrength(0..4)`
records the actual schema tier; it does not invent Fresh/cache/engine origin
from a strength label. The actual checked factory establishes the premises
for every mapped function's `good_function` predicate. This is record refinement,
not physical verifier-lifecycle or solver-stream-capture refinement.

Positive proofs establish acceptance of coherent nonempty and zero-function
inventories with no SMT obligations. Cargo tests use real public types and the
checked-in attribution fixture to exercise malformed summaries, interrupted
exit, provisional establishment, absent tiers, provenance inconsistency,
wrong/duplicate IDs, arbitrary bad sorting certificates, partial commands and
stale/duplicate selected targets. Exact `foo-bar`/`foo_bar` and failed-sibling
controls exercise independent requested coverage.

Actual guard removals must fail ordinary full-crate stock verification:
confirmation, whole scope and terminal summary, requested coverage, summary count, normal exit,
Current selected evidence, ID uniqueness, provenance counts and verification-enabled
configuration (both forwarded and decoded options). Safety claims
remain unchanged. The owning validation record and source manifest carry final
counts and platform qualification. Local proofs do not substitute for native
gates or corpus conformance.

## Exact evidence context

Requests, started invocations and observed verifier documents carry the shared
`ModelContext`. It contains the original request bytes, structured whole/partial
scope, runtime identity, explicit solver identity, verification mode and the
actual modeled input map.
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
| `start` | A well-formed exact invocation context and scope; direct or engine origin. The invocation separately records Enabled or CompileOnly verification configuration. A fallback cannot enter through this constructor. |
| `stream_started` / `stream_closed` | A child start creates an independent mandatory ledger slot for the declared context solver; a matching well-formed transcript closes that slot exactly once. Documents cannot create or replace slots. |
| `observe` | Append a verifier document to the actual started run's observation history. The document remains a candidate, not proof authority. |
| `interrupt` / `malformed` | Permanently prevent that stream from completing; later observations cannot restore Running. |
| `complete` | Running stream, Enabled verification configuration, normal zero process exit, ordered completion point, at least one verifier document, matching run ID/target/scope/context for every document, checked successful records, every started child closed, and exact equality between the independent child ledger and the complete document inventory. |
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

A complete zero-function invocation is permitted only with Enabled configuration:
a compile-only zero-function success cannot complete. It still needs a matching
started invocation, a nonempty document collection, explicit successful verifier
termination and normal process completion. No inference from an empty query or
function collection creates an invocation-completeness fact.

The modeled invocation independently tracks child solver starts and closes.
`stream_started` creates a private mandatory slot before the document inventory
exists; `stream_closed` supplies that slot's final transcript. A slot cannot
close twice or use a solver other than the explicitly declared
`ModelContext.solver`. A started but still open child blocks completion.

Documents separately carry their observed session inventories.
`observed_transcripts` concatenates **all** document inventories in order, and
completion requires this sequence to equal the ordered started/closed-child
ledger exactly. Omitting a stream from both a document and the caller's query
list cannot remove its already recorded start slot. A closed-but-omitted stream
therefore blocks completion even when the supplied document/query inventories
are both empty. Legitimate invocations that actually start no modeled solver
children can retain an empty inventory.

The completion exposes both inventories and proves their equality. The shared
composition matches every such recorded transcript against query-origin evidence
and requires its solver to equal the context's explicit solver identity. The
actual compiler/side-channel adapter must still refine real child creation,
complete stream capture and closure into these model events. An OS child that
was never reported to the model remains an external refinement gap; neither a
document inventory nor a ledger name proves real-world capture completeness.

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
witness, explicitly emitting a child-start and child-close event for each
transcript before completing the run. The composed witness must include real nonempty input/query evidence;
legitimate no-SMT invocations may still have an empty session inventory.

Eight deliberate guard removals must fail ordinary full-crate proof checking:
whole/partial promotion, missing function confirmation, missing requested-target
coverage, completing an invocation without documents,
accepting a stale context, accepting a document from another context, and
ignoring the independent child-stream ledger, and treating compile-only success
as verification. They retain the original claims rather than weakening the safety theorem.

The owning captured manifest records the full current driver count, including
the shared context's two checked lemmas and verification-setting controls.
Checking requires normal exit zero and entire-crate verification. The pin remains upstream stock Verus
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
| Record checks | Actual typed acceptance factories and receipt `complete` | Source-shared checks and constructive function refinement are implemented; attribute records and terminal fields to the actual trusted verifier execution. Diagnostic formatting remains outside positive authority. |
| Solver inventory | Shim/side-channel capture and query association | Establish complete actual session capture and connect the observed streams to the query component's admitted origins. |
| Input observation | Managed read checks, currentness and durable refresh | Refine actual compiler-input closure, filesystem/process observations and accepted observation points on Linux/macOS. |
| Repair/job/presentation | `repair::accepted`, batch selection, retained result frames, CLI/JSON/MCP | Actual report/selected-target consumers use checked factories. Preserve independently bound request and fresh input authority across every retained surface, and justify actual batch selection/process origin. |

The trusted verifier's translation and mathematical soundness remain outside
this proof, as in the design. The external observation adapter must faithfully
identify the executed runtime and process, complete compiler-input/session
closure, structured records and termination. Model constructors do not prove
those I/O facts. Rust/Verus/vstd, Z3, build/deployment and the documented
observation-interface assumptions remain trusted.

The shared leaf and permitted crate edges are recorded in ADR 0079. Backend and
solver/compiler inputs are unchanged. No conformance or performance improvement
is claimed; the native cost of producing and checking index candidates remains
to be measured.
