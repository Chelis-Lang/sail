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
assumptions and does not establish complete compiler-input equality. The
native request-capture and retained-admission integrations below have their
own adapter obligations; the factory alone does not prove all CLI, JSON, MCP
or repair callers.

`bind_replayed_project_ordered` checks a retained whole proof against a separate
current request. It compares the exact original arguments, invocation directory,
workspace, context, input revision, engine/repair selection and ordered target
inventory, then validates the original request's complete reports. A private
`ReplayReadback` can be sealed only from a real typed `GuardedReadback` whose
observations and boundary guard are unchanged and whose bound revision equals
the current request's revision. The transfer changes only the project view's
run ID; original target establishment identities and report evidence remain
intact. Refusal leaves the project unchanged. A constructive enabled
zero-function case prevents an implementation that always refuses from satisfying
the contract. Compile-only, focused and partial whole-proof transfers refuse.

The native no-op entrypoint uses this checked transfer after the independently
resolved preflight and scoped readback described below. The shared factory
proves correspondence between those supplied records; attribution of the actual
metadata resolver and observed input domain remains a native obligation.
Its checked report projection preserves every captured proof field; native
tests additionally compare the complete Rust project, including presentation
fields outside that projection. Complete physical input discovery and a single
global filesystem observation point remain separate obligations.

The managed runner now constructs the immutable request from its independently
resolved expected target set, original configuration, original stamped snapshot
and selected engine before executing Cargo. A worker-only journal carries each
actual run separately, including independent fallback runs. The daemon selects
exactly one record for the output run and compares its worker ID, canonical
invocation directory and arguments against the retained original submission.
The shared argument matcher permits only removal of the documented no-op
`--incremental` before `--`; it preserves all forwarded arguments. This is a
CLI argument correspondence check, not a claim that submitted options equal
effective per-package compiler options. The existing effective verification
decoder and actual per-target report checks remain required.

The capture destination and worker ID travel through private Sail arguments
into an explicit optional `Config` field. Sail consumes them before constructing
Cargo commands; they are never fresh environment variables or forwarded
compiler arguments. Original submitted environment values are preserved,
including user variables with the former transport names. A real-child
environment probe checks absence and original-value cases against the same
declared-read recorder used for proof inputs.

Compiler-input accounting recognizes the existing stdout completion protocol
before forwarding can acknowledge and retire its peer. It validates the
current private endpoint capability, exact marker grammar and live peer with
the same parser as the output receiver, then removes only that framing from
its accounting copy. Original bytes and acknowledgment behavior remain with
the original output path. Foreign, malformed or truncated markers cannot close
an input provider, and non-marker bytes preceding a marker remain observable.
Recognizing a completion marker establishes no metadata or verifier verdict.
This native framing adapter remains outside the stock-checked acceptance core.

Private job projections retain the independent request beside the stdout seal.
Every result read validates that binding, and immutable frame keys include the
original submission and request. Current successful projects additionally pass
the checked whole-request factory before retaining complete target flags.
Unknown and Historical projections retain their explicit evidence framing.
Baselines preserve the same binding and cannot restore completeness stripped
from a legacy job. The private producer revision is checked; absent or older
records withhold authority without changing input Currentness or verdicts.

This connects the concrete request predicate to the actual managed runner,
retained job JSON, cached frame, baseline and repair projection paths. Their
native capture, serialization, filesystem and process adapters are tested Rust
and remain outside the stock-Verus checked bodies. Cargo metadata's scope
resolution and complete discovery of generated/compiler inputs are not proved
by the request token. The pre-execution revision is never replaced from output
to satisfy the comparison. The composition interface is the token's exact
immutable request/project projections plus `request_authorized`; independent
input and query evidence must establish their own physical observation and
context relations. Plain default diagnostic forwarding is unchanged.

Repair workers additionally retain the actual pre-execution raw package
selector, optional binary name, target kind and exact function selection. The
checked `repair_request_matches` predicate compares those fields to the
original retained repair mode. Raw package selectors such as `name@version`
remain distinct from canonical Cargo package IDs in the independently resolved
target inventory. Their metadata resolution is still a native adapter
obligation. Changing a retained repair function or target cannot reuse a
previous selection's acceptance. Independent stock fallback retains its full
proof evidence and never accepts a repair. The standalone `repair-check`
document still derives acceptance from the native selected batch and checked
report predicates; its complete native selection/process correspondence is
not proved by the retained-job refinement.
Library callers of `engine::repair_selected` must establish the same native
selection-to-Cargo-argument relation that the CLI worker checks; the wrapper
does not independently resolve a contradictory library `Config` and selection.

Retained repair acceptance also scans the actual requested whole reports for
the exact captured function name through the checked
`requested_function_present` predicate. The independent complete-request token
establishes that every such function is confirmed verified. Native selection
status fields alone cannot authorize a function absent from those records.

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

### Exact partial reports

The source candidate adds a separate private `ExactScopeReport` token. Its
factory retains immutable borrows of the original report and ordered scope
arguments. It requires an explicit partial selector after `--`, exact raw
argument bytes matching the report command, verification-enabled arguments,
no engine report and the existing successful receipt-policy report checks.
An absent whole-crate success summary remains admissible under that receipt
policy; a present success flag does not widen the partial selection. Nothing
rewrites the report, selected scope or whole-target completion fields.

The native partial branch of `Invocation::successful_for` uses this factory
while retaining its original explicit selector, argument-equality and no-engine
checks. The surrounding ordinary receipt reader still validates original
selected history, commit, context, artifacts, dependency bytes and declared
environment. Whole-receipt admission retains its existing success flag and
receipt-policy report checks; whole-target authority additionally requires the
strict whole-report factory.
This connects exact partial report validation to a real consumer; it does not
promote the report to `CompleteRequest` or `CompleteSelection`. The source
refinement includes constructive receipt-scope admission, rejection of changed
arguments, engine and compile-only cases, and the whole-report factory's refusal
of an admitted partial report. This new slice is source-only and unqualified;
no compilation, stock proof or mutation-matrix result is claimed for it.

The next source candidate adds the distinct private `ExactPartialRequest`
token. `complete_exact_partial_request_ordered` checks the independently
supplied original request's header and exact one-to-one requested target
coverage. It requires Current inputs, normal zero exit, no setup error and
a plain partial invocation with no engine or repair selection. Every requested
target must retain a nonempty original establishing identity, exact
`scope_reuse` arguments matching that request, the partial report evidence,
`complete == false` and `reused == false`. This contract covers retained
partial reuse; a newly executed partial target without `scope_reuse` is a
separate observation. Complete dependency targets outside requested scope
remain intact.

`bind_replayed_exact_partial_project_ordered` checks independent original and
current requests and an actual typed guarded readback of the original revision.
It preserves the complete captured target records and reports, changes only
the project view ID, and retains exact partial authority for the current
request. Partial request authority explicitly excludes the whole-request
predicate; requested targets cannot acquire whole-completion or complete-reuse
labels. Candidate sorting supplies no authority. The source refinement includes
nonempty request/report/readback-domain admission, a legitimate zero-function
request, missing-target and whole-label refusals, bad-readback refusal, and
the existing whole-request factory's refusal of a partial token. These new
operations and lemmas remain source-only and unqualified. Their full-function
always-refuse and guard-removal mutations still require ordinary stock-Verus
failures with unchanged inventories and limits.

The native unchanged-call producer now selects the whole or exact-partial
factory from the independently captured original request. Replay selects its
separate transfer from the independently resolved current request, after actual
metadata/runtime preflight and scoped readback. Version-four records keep the
original request and ordinary whole/partial receipts; every selected/archive
history, exact scope, report and original establishment is rechecked before
transfer. Failed projects remain ineligible. This closes the source integration
needed by the unchanged exact-partial output controls in
[#430](https://github.com/Chelis-Lang/sail/issues/430), including their literal
cold-miss trace. The source regressions cover changed scope, whole promotion,
failed status, legacy records and missing original histories. They have not
been run; actual native output/conformance and component proof qualification
remain required under
[public-path composition](https://github.com/Chelis-Lang/sail/issues/376).
Scope labels, original reports and establishment IDs must survive publication and CLI/JSON/retained service views. The requested
targets' `reused == false` fields preserve their exclusion from ordinary
complete-target reuse counting; complete dependency counts are independent.
The actual compiler/metadata-to-request and selector-to-verifier relations,
complete input closure, physical observation and full service composition
remain independent joins. Record correspondence alone does not establish that
every requested target or function selection was executed. Neither the replay
branch nor these record checks close those service joins.

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

## Success presentation call graph

This is a source inventory of actual consumers and producing guards. It is not
a checked full-service refinement. A serializer, `complete_target` call or
confirmed function field cannot establish independent request scope, original
producer observation or complete physical input freshness by itself. The
default managed producer constructs its local original request before Cargo;
durable workers additionally capture that request independently of stdout.
Worker stdout seals and producer revisions retain recorded provenance; they
do not turn deserialized reports into live process-observation tokens. The
native producer-to-record relation and complete physical input readback remain
the separate obligations in #375/#376.

| Surface | Actual producing path and admission | Presentation boundary and remaining join |
|---|---|---|
| Foreground human `sail verify` | `managed_cli::verify` calls the managed runner. Its pre-Cargo request comes from actual metadata and original configuration; final requested coverage and complete-request checks control whole flags. Selected original process observations and ordinary receipts control per-target completion/reuse. | Human output forwards original verifier/Cargo bytes and status. This is a native forwarding path, not a new positive authority derived by a formatter. Complete compiler/process/input observation and forwarding refinement remain open. |
| Foreground JSON | `managed_cli::verify` serializes the resulting `ProjectResult`. The same producer establishes the request/target relation before serialization. Unbound setup fallback explicitly carries `no_reuse`, Unknown currentness and no whole completion. | Serde transports the native result; it does not independently validate request or producer history. Complete serialization and caller correspondence remain native obligations. |
| Unchanged-call replay | The CLI calls `noop::key`/`replay`; the candidate independently resolves current metadata/runtime before saved result decoding, supplies actual scoped readback, rechecks original ordinary receipts/history, then calls the separate checked whole transfer. Capture-demand workers decline this path. | Replayed output and JSON retain original target reports/establishment. Native selection, physical input closure, measured preflight cost and complete service refinement remain open. Failed no-op replay still declines; exact partial replay has its own guarded transfer. |
| Durable job reads | `worker_command` allocates the typed capture journal. `captured_request` selects exactly one request for the actual output run and matches worker/submission/mode/invocation. Worker exit agrees with the report. `store::project_with_request` checks original stdout bytes, producer revision and original request before preserving completion; freshness comes separately through `FreshnessIndex`/read-check. | `jobs::response_text` uses `complete_target` plus result currentness to choose whole-proof labels. The daemon producer must already have withheld unbound flags. Parsing, actual worker/process attribution, complete readback and RPC/JSON correspondence are native joins. |
| Cached durable frames | Before every lookup, `store::result_frame` obtains live original-request admission and binds it into `frame_key`, alongside the original request/submission, producer revision and stdout seal. The same request observation builds fresh project/repair frames; dynamic readback selects Current/Historical/Unknown. | This closes the source-level warm/fresh mismatch in [#427](https://github.com/Chelis-Lang/sail/issues/427). The transition regression and invocation-binding proof additions remain unqualified. Cached flags cannot stand in for the new live observation; filesystem observation and full frame/transport refinement remain open. |
| Default MCP `verify` and `verification_result` | `job_tools::blocking`/`read_or_cli` obtains the same daemon result, validating response identity/version. `compact::agent_view` computes whole completeness from requested target factories and distinguishes Current from Historical/Unknown. | Summary, targets and full details shape the same underlying project. `summary::effective_verdict` prevents provisional promotion but only reads function fields; it is not request or process authority. MCP identity/transport, shaping and end-to-end preservation still need service refinement. |
| `cached_result` and `counterexample` | Managed lookup prefers admitted job results. The separate legacy per-project store retains historical per-function records; its limited local source snapshot reports freshness Unknown or stale, never managed Current authority. | Recorded function verdicts remain historical evidence with their establishment/freshness context. The limited source snapshot cannot prove workspace/runtime currentness or replace the managed admission chain. |
| Repair CLI/MCP | Public repair submits a job. `retained_repair_accepted` requires the independently captured repair request, exact original selection, complete submitted full request, requested named function presence and the batch's no-selection-error/no-engine-fault/full repair policy. CLI `product_cli::repair` displays that daemon decision; MCP proof tools use the same service. | The low-level `BatchResult::accepted_with`/`repair::accepted` checks report/selection records. Private worker output and direct library use still depend on the native selected-function gatherer and original Config-to-selection mapping; those helpers alone do not establish the full original request or service relation. |
| Ordinary immutable baselines | `product::save_baseline` obtains the paired project/request from `store::result_with_request` via `completed_with_request`, preserves that single native observation through `publish_baseline`, retains producer revision, and explicitly changes the view to Historical. Load checks identity/version/Historical framing and withholds older/unbound authority. | A baseline is recorded evidence, never today's proof. Independent serialization, retained original producer attribution and baseline-to-submission correspondence remain native joins; typed report checks alone do not prove them. |
| Neutral contract results/baselines | Jobs with a contract submission branch into the contract handler rather than generic result frames. Its decode/admission binds original source/worker evidence; neutral baselines retain and recheck original source, stdout/stderr and seals separately. | This is a distinct source/contract evidence path. Its backend project/report fields cannot silently substitute for complete generic Verus request authority. Full composition with its own negotiated source contract remains separate. |

The concrete frame bypass illustrates why source inventory matters. Previously,
warm lookup returned before `retained_request` canonicalized the current
invocation directory. All serialized key fields could stay unchanged while the
fresh path refused that live observation. The source correction evaluates it
once before lookup and uses that same decision for cache identity, whole-flag
withholding and repair acceptance. The shared `bind_retained_submission` body
checks submitted arguments/mode and the separately supplied invocation value;
native canonicalization itself is not proved. The source regression warms
actual frame families, removes and restores the invocation directory, and
compares warm/fresh responses while preserving original stdout/report/identity.
It has not been run. No mathematical false-proof witness is claimed by this
admission/presentation finding.

Baseline creation's distinct interleaving is tracked in
[#429](https://github.com/Chelis-Lang/sail/issues/429). The source correction
returns the admitted request alongside the exact result from
`store::result_with_request`. `completed_with_request` captures both in one
private, non-deserializable native `CompletedObservation`; `publish_baseline`
consumes that observation without resolving the live invocation again. The
original request metadata is copied, not reminted as a live process token.
Immutable publication and the returned Historical view therefore use the same
original record, reports and establishment identities. Refusal observed at
retrieval cannot become completion merely because the directory later returns.

The source regression retrieves that pair through the actual result path,
moves the invocation directory at the publication boundary and compares the
returned and immediately loaded baseline. It also covers refusal before a
subsequent directory restoration. This regression has not been run. The native
pairing and serializer/publication bodies remain unqualified adapters, not a
stock-Verus service theorem or a demonstrated Current false-proof witness.
Existing name checks, atomic immutable publication, producer revision,
Historical framing, baseline pin/capacity policy and read-side admission remain
in place.

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
requires that token, a fresh run ID, a start point at or after the observed
failure, the exact target, and matching request/runtime/solver/input/mode and
verification settings. A whole-target restart may widen the failed partial
selection; it cannot change the non-selection subject. It starts with an empty observation history. Old engine
or partial-phase documents cannot match the fallback's run/context/scope header.
The project fallback path accepts only completions of fresh fallback invocations;
it cannot mix in old engine completions.

`finish_project_per_target` uses the same `Candidate` and current complete
acceptance factory for a project containing direct, engine and independent
fallback targets. Each member is a private completed invocation with its own
validated origin; `ExecutionPath::PerTarget` does not mint or replace that
evidence. The existing global direct/engine/fallback paths retain their original
origin restrictions. A three-target constructive control completes one target
through each origin, preserves the failed predecessor of the fallback, and
rejects an omitted sibling. The fallback constructor also refuses explicit
wrong-target and changed-runtime candidates. A selected target that required
independent fallback cannot become an accepted repair by hiding within this
mixed project.

This extends the conditional model composition, which already consumes the
same `Candidate` and its complete invocation inventory. It does not establish
the full native `ProjectResult`-to-`Candidate` relation. That relation still needs
actual target/source/effective-invocation attribution and complete process and
solver observations. Retained target transfer across different project requests
also needs an independently established physical target input domain. A supplied
declared-provider subset cannot mint that missing closure authority.

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

### Normal engine exits that do not establish completion

The shared executable engine classifier checks the decoded original terminal and
result fields, including event versions and names, full phase, explicit normal
termination, exit/success agreement, header presence, result scalar types and
complete document framing. It distinguishes actual field absence from wrong
JSON types; no `document_valid` or caller-supplied completion flag replaces those
checks. `engine_events::completed` uses this same source body in production.
The concrete-to-model projection preserves each original field and proves the
completion predicates equivalent.

`DeclinedEngine` is a separate private checked token for an actually observed
normal exit zero whose protocol/document candidates refuse completion, or whose
original stdout/stderr contains an exact missing-feature diagnostic. Its status
remains zero. The model observes those fields once, creates a distinct
`EngineDecline`, and starts an independent `RestartAfterDecline` only with the
same exact subject, a later start and fresh empty document/solver history.
Ordinary `EngineFailure` retains its original nonzero/interrupted/malformed
process guard. Both restart origins enter the same per-target candidate and
coverage factory; every fallback in the requested repair scope is rejected.
A repeated protocol observation cannot overwrite an earlier refusal.

The private `MissingEngineFeature` factory independently validates the original
raw diagnostic spelling and closing delimiter, the nonempty ASCII feature-name
grammar and absence from the runtime-feature candidate inventory. It retains
the same actual `ObservedAttempt`; an arbitrary supplied name cannot construct
the token. `engine_decision`, called by the native per-target controller,
chooses completed engine output only when no missing-feature candidate exists
and the original completion fields are consistent. A valid feature refusal can
therefore restart a normal-zero child even when its protocol is complete.
The model preserves those raw fields, rejects direct completion, and begins a
new fallback with an empty document/solver inventory and the original subject.
The concrete projection retains every protocol/document/remainder/status field.
Native JSON decoding, complete discovery of feature diagnostics, attribution of
the runtime feature inventory and physical loaded-image identity remain open
joins; this component does not infer their completeness from a token.

The native controlled witness runs a real child producing a nonempty complete
Verus fixture at exit zero with a missing terminal, checks the private decline
factory preserves that actual status/slot, and runs a distinct fresh stock
fallback with exact stdout/stderr/status. The model witness additionally rejects
old engine documents, refuses calling that zero exit a crash, and demonstrates
whole-target acceptance followed by repair refusal. These are component controls,
not a qualified real-project engine rollout. A second owned-child fixture emits
a complete protocol/result plus a missing-feature diagnostic. The checked raw
factory preserves normal exit zero; the native controller starts an independent
fallback and keeps the earlier output/event files. Bogus, malformed and already
available names refuse a feature token. The whole local stock component checks
374 obligations and six ordinary failing feature controls; the canonical
combined source capture and native qualification remain separate.

Raw JSON decoding and attribution to the original child still require native
refinement. The projection does not invent a modeled invocation from an arbitrary
supplied target/run label. A further captured witness shows why that matters:
the qualified fork can complete successfully while every optional engine event
append fails on a read-only preexisting file. The old valid terminal bytes remain
unchanged, and structural completion validation accepts them alongside the new
stdout. That is a stale-origin gap, not a demonstrated false mathematical proof.
The managed adapter allocates an exclusive slot directory before configuring
logs, provenance and engine/focus destinations. Each exact raw output is written
with exclusive creation before its terminal observation. The private selected
reference preserves the owned original run/unit/slot/final descriptor/PID/status,
raw-output digests, selected file inventory and predecessor history. A fresh
fallback receives a different directory and preserves its predecessor's files.
Engine completion, focused repair, missing-feature handling, target completeness,
receipt publication and dependency-output publication follow that same reference.
A later configured reservation makes an older selection noncurrent even if an
interrupted writer left its invocation JSON behind. Fixed target filenames do
not supply live authority.

The [producer-policy migration](https://github.com/Chelis-Lang/sail/issues/410)
uses private revision three for both retained job projections
and saved baselines. Missing, superseded and unknown revisions withhold target
completion without changing original reports or input-currentness observations.
Copying an older job into a current baseline cannot restore withheld completion.
This revision check records the producer policy; it does not itself establish
an original request, actual process or trustworthy filesystem history.

Receipt version seven rejects older producer evidence. Its current-run admission
requires the selected original namespace and reserved/terminal/raw file binding;
missing, mismatched or ambiguous producer kinds cannot mint a live receipt.
Qualified bundled compiler-boundary provenance remains a separate native import
and creates no fictional live child. The same-workspace experimental arena
uses entry version four and owns an independently copied original receipt,
commit and complete selected-attempt subtree. The entry binds its exact raw
stdout/stderr to the original selected producer. Restoration validates the full
archive before placing outputs and adopts another independent copy into the
current run's `retained-histories/<key>` namespace. The invocation retains its
original run, slot, PID, descriptor, report and dependencies through a separate
paired `retained_from`/`retained_history` reference; the archive validator never
creates a live observation or writes into the original run's namespace.
Removing the original run and evicting the arena do not erase an already
adopted copy. Missing, extra or modified history, older entry versions, subset
indexes, mismatched reports and stripped/conflicting references refuse reuse.
Current receipts still require their own final commit and exact artifact reads.
The [original-history dependence](https://github.com/Chelis-Lang/sail/issues/409)
and [archive schema checks](https://github.com/Chelis-Lang/sail/issues/411)
retain their candidate status and outstanding combined gates. The physical
target-domain and protected-storage transition proofs remain open.

The disabled native-artifact store's version-two layout copies the complete
original producer subtree, including raw streams, per-attempt records and owned
archive metadata. It checks every copied path/file and ties displayed replay
buffers to that original producer. Adoption derives the store location from the
native adoption layout and validated archive key, preserving the original run,
slot, PID and descriptor. Removing original run files does not erase the copied
history. Missing, extra, modified or omitted archive files withhold reuse; stripping
an archive reference and resealing cannot create a live origin. This layout does
not enable the store or discharge its native compiler-input blocker (#187).

These native serializers, filesystem/image reads, protected-writer properties,
child/exec/pipe/wait association and whole native event simulation remain outside
the stock-checked metadata-copy factory. In particular, self-consistent metadata
or an executable path is not proof of an immutable loaded process image. The
complete `ProjectResult`-to-model invocation/source/context relation, authentic
side-channel writer, feature-refusal branch and full compiler/solver domain still
need their own refinement. Native Linux/default conformance and cost qualification
of this namespace/archive integration remain separate gates.

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

The production `SubmissionBinding` borrows the actual retained job arguments,
`jobs::Mode` and pre-execution `ResolvedRequest`. Its checked constructor permits
only the documented pre-separator incremental no-op in argument correspondence
and checks the original repair package, target, kind and function. Named
interactive requests require the explicit interactive engine identity.
`SubmittedAuthority` combines that binding with the existing checked whole
request factory without replacing either borrowed record. Job capture, retained
request loading and retained repair acceptance use these source-shared checks.
The constructive control accepts a coherent nonempty function report; argument,
mode and repair-selection swaps are refused by the same checked constructors.

`SubmissionKind::StockForOptIn` describes a stock request under an opt-in
submission. It does not establish a failed engine attempt, a successful child
process, or accepted repair evidence. Original submission-to-request
correspondence and record validation do not yet establish the complete physical
invocation history. Canonical metadata selection, effective per-package driver
options, child identity, complete observations and retained-proof transfer must
be connected separately to the lifecycle model. A matching context digest is
still conditional on its documented input encoding and collision-resistance
assumptions; it is not a proof that the complete compiler-input maps coincide.

The effective Cargo driver decoder treats the raw common and exact current
package values as independent inputs. Native splitting proposes tokens; the
source-shared `effective_driver_arguments_match` checks their complete ordered
correspondence before the adapter uses them. Its delimiter scan proves no
separator was skipped, every token equals its original byte span, and the last
span reaches the end. This preserves ignored leading text, empty and trailing
tokens, and common-before-package ordering, including `--no-verify`. Missing
sources contribute no tokens; encoding or lookup failures still refuse
authority. This does not prove environment lookup, Cargo package-key derivation
or the trusted pinned verifier's decoder implementation. Those native and
runtime relations remain obligations of the actual launch adapter.

Native target resolution now uses the checked `compiler_unit_matches_target`
predicate for actual Cargo/Rust name and kind fields. It preserves the existing
hyphen-to-underscore Rust crate-name relation and requires the exact Cargo
binary name when the launcher provided it; normalized-name collisions remain
distinct. The native resolver retains its manifest comparison and single-match
check and refuses malformed kind arrays. The proof covers this shared name/kind
predicate. Metadata decoding, manifest/source resolution, complete target
inventory and the join to actual compiler arguments remain separate obligations.

The live driver and its independent stock fallback now use the source-shared
attempt reducer. It reserves a monotone slot before spawning, retains the raw
managed-run and compiler-unit fields, and attaches the actual returned PID.
Stdout EOF, stderr EOF and wait status are separate observations; missing or
duplicate events cannot seal an `ObservedAttempt`. The seal preserves the exact
buffers and status passed to the existing report code. Reused PIDs do not reuse
slots. Per-target journals persist the pre-spawn command observations and
terminal buffer hashes. Before request preparation, an exclusive `.allocated`
marker leases the slot across separate driver invocations in the same managed
run. A fresh local counter skips existing leases and legacy reservation/terminal
records; it never deletes or overwrites them. Collision retries are bounded and
exhaustion withholds observation evidence while preserving fresh execution.
A private native owner prevents configuring another journal's allocation, even
when run/unit/directory values match. Exclusive-create filesystem semantics and
protection of the journal directory remain native adapter obligations. This
corrects the same-run managed retry collision tracked in #384.
Allocation and configuration are separate checked transitions: an
`UnconfiguredAttempt` supplies its run/unit/slot for preparing observation paths,
then `configure` seals the final program/argv/declared-directory/overrides before
spawn. Ordinary `reserve` composes those transitions. Preparing a request never
licenses changing an already sealed descriptor. This interface adds no observer
flags to the selected runtime by itself.

Journal failure is sticky unavailable evidence. It never suppresses the fresh
spawn or discards the actual stdout, stderr or wait status. The native owner
keeps an unsealed payload on reservation or terminal-write failure; the checked
reducer also returns the original buffers when ambiguous events refuse its
seal. Report input is checked against the same complete observed payload before
live-origin admission. Without that observation, the retained report records
`SAIL3111` as an infrastructure error while its original verifier diagnostics
and per-function output remain intact. The structured error retains the journal
failure cause and path when available. Whole and exact partial receipt reuse,
retained target/selection/request authority and experimental content publishing
refuse that history. Fresh input Currentness remains independent.

`AttemptIdentity::from_observed` copies the complete observation's original
raw run, every unit field, allocated slot, final command descriptor, actual PID
and original status into an owned private record through checked deep-copy
helpers. The source-shared factory preserves absence versus empty values and
all argument/override ordering. It provides no constructor from archived bytes
back to live observations, Currentness or proof authority. A constructive
refinement control gives two observations the same PID and proves their distinct
allocated slots and exact original metadata survive ownership transfer.
Native serialization, image resolution and protected storage remain adapters.

This seal establishes complete process observations, not completed verification.
Native command construction, OS handle/pipe/wait attribution, pinned image
resolution, structured document parsing and engine-protocol interpretation
still need their concrete joins. The command descriptor explicitly records
declared argv/current-directory/environment overrides; it does not purport to
capture every inherited environment fact or compiler read. The raw Rust unit
record is distinct from resolved Cargo/source identity. Retained/bundled replay
does not create a live process seal, and its transfer relation remains a separate
obligation. No persisted journal or normal exit can independently authorize
proof success. The extra journal I/O still needs native cost qualification.

The main unchanged-call replay surface has a distinct
[producer-history integration defect](https://github.com/Chelis-Lang/sail/issues/415):
its cache scans omit original run directories. Matching cache records and a
stored Current result therefore cannot replace original receipt admission.
The candidate version-four replay record retains the original independently
resolved request and exact ordinary receipts for every positive whole or
exact-partial target. Publication and replay check target identity,
original establishing run, full report, exact scope, commit, artifact/dependency
bytes and selected or archived producer history through the ordinary receipt
reader. Missing, modified, omitted or mismatched origins take the ordinary path.
An owned-child fixture verifies the intact replay, then removes its original
run and observes refusal rather than a cached complete result. This is a native
metadata/provenance control, not real-project proof or complete input closure.

Before deserializing a saved replay result, the production entrypoint resolves
fresh Cargo metadata through the ordinary `expected_targets` resolver and
recomputes the current runtime context. Ordinary execution and replay share
the `request::Resolution` builder. Current metadata and runtime identity choose
the workspace/family ownership locks and new run directory; the saved record's
paths must match and cannot redirect that choice. A missing or failed resolver
takes the ordinary path.

The ordinary read-check implementation now passes its actual typed
`GuardedReadback` to a callback while the underlying expected facts, fresh
per-file observations and before/after guard remain alive. Replay constructs
its separate current request there, before inspecting saved result fields.
The checked transfer revalidates the original whole request, exact current
request correspondence and guarded original revision before changing only the
project view ID. It preserves original establishing IDs and report evidence;
it creates no verifier attempt. Wall time remains presentation metadata.
Failed unchanged-call replay takes the ordinary managed path. Exact-partial
replay uses its separate checked request/target scope contract; it cannot become
whole-target or complete-target reuse authority.

Native component fixtures use a separately declared one-library request and
actual guarded reads of nonempty real file observations. They qualify the
original-history/readback transfer, not the actual Cargo metadata preflight or
a real trusted Verus execution. Complete input-domain discovery, global
filesystem observation, metadata/process attribution, complete service
composition and added native preflight cost remain open. A stored Current bit
and a newly assigned run ID cannot supply those guarantees.

The daemon's retained-result path uses the source-shared
`withhold_unbound_request_authority_ordered`, `retained_currentness` and
`apply_retained_currentness` operations. The first rechecks the independently
retained request before preserving completion labels; refusal preserves the
reports and input-currentness observation. The classifier returns Current only
for an originally Current result with a complete matching revision readback.
The final operation changes the selected freshness view while preserving every
target report. Its result tells the native caller when to append the existing
lost-readback note. Historical and Unknown origins cannot become Current merely
because a later digest matches. A coherent nonempty request retains completion.

`retained_current_frame_refines_request` checks their composition: a selected
Current view with retained completion has the original request authority and
the matching complete readback. The candidate-order wrapper forwards the actual
mutable project and request unchanged. The captured API includes both project
and job Currentness enum definitions. Filesystem readback completeness, exact
serialization/cache selection, original process origins and every CLI/MCP
rendering remain independent service-refinement obligations.

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

The shared leaf and permitted crate edges are recorded in ADR 0079. The current
candidate moves existing private observation destinations into per-attempt
paths; those argv/environment values are compiler-observable under #385/#393.
Descriptor equality does not establish original compiler-input transparency.
The original-input transport must be fixed before final default selection.
No conformance or performance improvement is claimed; native qualification and
the cost of index checks and complete history transfer remain separate gates.

The separate [normal-wrapper control proposal](managed-wrapper-control-transport.md)
describes an unselected environment-preserving transport and its required native
authentication/process joins. It does not change the selected runtime or close
the reproduced environment and verification-phase input gaps.

## Capturing concrete feature refusal

The combined canonical driver captures the private `MissingEngineFeature`
factory and the native engine's actual `engine_decision` call. Its feature
observation retains original diagnostic bytes, the exact ASCII feature name
and the runtime's supplied available-feature list; it grants neither image
identity nor exhaustive compiler-input authority. Six additional ordinary
controls require byte-origin, name, availability, normal-exit, decision and
model-admission guards. The earlier exit-zero decline control now names the
complete original factory to distinguish it from the added feature factory;
its original failure condition is unchanged.

Whole-root development probes check the resulting input root at 237/0 and
authority root at 385/0, retaining every prior explicit function. These source
probes are separate from the complete committed proof/control matrix and native
service gates. The frozen `4bd22455` gate does not include this later feature
slice; no earlier receipt qualifies the added behavior.
