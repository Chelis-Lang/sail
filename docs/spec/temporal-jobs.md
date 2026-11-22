# Temporal jobs, version 1

Status: specification for implementation review. The stable 0.3.0 distribution links the native
D.I.V.E. 0.1.0 library. The development checkout uses the immutable source
pin described under Edit evidence below.
It never executes a D.I.V.E. command, TLC, or Java and has no alternate backend.

## Inputs and execution

A request names one operation: exhaustive model checking, finite seeded
simulation, or validation of a complete finite or periodic trace. Source files
are a sorted map of relative UTF-8 paths to exact source bytes, with an explicit
root module and configuration. Absolute paths, traversal, symbolic links,
duplicate paths, missing imports, external search paths, host operators and
ambient module resolution are not admitted. The worker materializes private
files from this immutable closure and supplies only that closure to D.I.V.E.
Built-in standard module identity is bound by the backend release.
The worker admits the pure embedded `Naturals`, `Integers`, `Sequences`,
`FiniteSets` and `Bags` modules. It also qualifies the embedded TLC operators
`:>`, `@@` and `Permutations` and the pinned bundled `FiniteSetsExt`, `Folds`
and `Functions` files. When one of these additional embedded modules loads,
the pre-binding guard checks native identities on every lowered definition,
standalone reference, callee, higher-order argument and substitution, and
checks the effects of every expression. Only original native declarations in
the embedded TLC file may remain dormant. The pinned binder skips those
native declarations when precomputing constant definitions. Source-defined
startup expressions and assumptions receive the same check as selected roles.
The CFG must use direct literal assignments to root scalar constants and
select source-defined role operators; the check covers symmetry, constraints,
properties and hooks. External search paths, unqualified bundles, native host
operations and CFG redirects in this profile refuse. A trace action label may
not select an excluded operator. Request and trace validation use the same
admitted immutable source. A source-supplied import is still checked under
its actual origin.

The fixed semantic profile is native D.I.V.E.: mathematical integers, exact
states, seed zero, fingerprint zero and one worker. Checking uses D.I.V.E.'s
ordinary dive-profile route: bound safety roles, configured action properties
included, are safety checks, and genuine temporal properties use completed-level
temporal checking with independently validated counterexamples, on the same
exact store and state/depth limits; a limit reached during graph capture is a
resource stop with no claim. The request never changes
on a diagnostic and is never retried. These choices are
part of the backend identity, not claims of TLC-equivalent arithmetic or timing.
Requested state/depth limits are explicit; reaching a limit establishes no
holding property. Simulation names positive finite trace count and depth and
never produces an exhaustive verdict.

A S.A.I.L.-owned `sail-runtime-worker` executable links the library. The parent
sends one versioned JSON request over standard input and reads one bounded
versioned response. It starts an isolated child, closes input after the
request, and owns child lifetime. Parent cancellation or deadline kills and
reaps the worker, including during binding. No missing, truncated, malformed,
oversized, wrong-version or unsuccessful worker response becomes a verdict.
The worker does not make a hard memory-limit claim. Per-process D.I.V.E. RSS
observations and cooperative checks are qualifications, not enforced RAM quotas.

## Results and evidence

The backend returns complete success, independently validated counterexample,
sampled execution, valid trace, invalid trace, unsupported input, resource stop,
rejected input, or internal error. Parent timeout/cancellation are separate
execution results and are never cached as logical answers. Every response
retains D.I.V.E.'s original versioned report and native trace when present.
Complete graph evidence is required for a model-checking success; a genuine
validated counterexample may refute a property before exhaustion. A parser
success, an `Ok(report)`, a process exit zero, or simulation completion never
substitutes for this evidence.
The response records the exact configured invariant, PROPERTY, postcondition,
reachability and deadlock checks, and a separate typed completion kind
(`none`, `safety`, `temporal`). A safety completion is not a claim that a
temporal graph was captured; JSON operation discriminators are not evidence.

Trace transport uses ITF with exact value tags, a complete ordered variable
list, complete states, and separate ending/property/loop claim metadata. A
strict reversible codec converts admitted ITF values to native `dive-trace`
version 1 and back. It preserves mathematical integers as decimal strings and
distinguishes strings, model values, sets, sequences, records and functions.
Unknown tags, partial states, duplicate variables, unsupported shapes and
representations that cannot round-trip exactly are refused. Round-trip equality
is necessary but insufficient: the original specification independently
validates the converted trace. A behavior trace establishes only its supplied
execution, not an exhaustive model proof. Runtime random draws and other
features the independent validator refuses remain unsupported.
A witness containing a native model value declares
`"value_encoding":"sail-model-values-v1"` on its trace object. In its state
values, the Sail extension `{"#sail-model":"r1"}` denotes the native atom
`r1`. A JSON string `"r1"` still denotes the ordinary TLA+ string. The tag
can occur wherever a state value can occur, including a map key. It has
exactly one string-valued field. The discriminator must be present exactly
when at least one state value uses this extension. Unknown, null, mixed or
unnecessary discriminators and malformed tagged records are rejected. The
native validator replays the decoded witness against the current model. ITF's
`#unserializable` continues to mean there is no semantic value to validate.
Legacy traces keep their six-field representation, including its field order
and canonical request digest. Earlier clients reject extended traces because
they do not recognize the additional field. [Wire decision](../decisions/0178-model-valued-temporal-witnesses.md).

## Exact check replay

The runtime stores completed `check` results under `runtime/temporal-v1` in
the configured Sail cache. It requires the typed completion and source-bound
property inventory for Holds, or the original independently validated native
and ITF counterexample for Refuted. A new call authenticates the installed
worker before lookup. Its input SHA-256 covers every supplied relative file
name and byte, root and configuration selection, operation and deterministic
limits. The key also binds the wall deadline, Sail version, worker byte digest
(including embedded standard modules), D.I.V.E. identity, platform and the
producing adapter's versioned checking policy. A changed byte or limit runs
a fresh check. Keys for previous exact inputs survive subsequent edits.

On a hit, `cache_key` names the matched receipt and `cache_hit` is true.
The outer job uses the current caller's identities. The native report and
validated counterexample, including their original execution statistics, stay
intact. Compatible socket peers negotiate `replay_context` and verify the
matched key independently. Older peers get fresh execution. Missing, corrupt
or ineligible evidence falls back to normal checking. Cancellation and the
current deadline apply during lookup. Simulation, standalone trace validation,
resource stops, refusals and execution failures always execute afresh.
The local disk tier requires trusted user-owned storage. See [trust](../trust.md).

## Explicit invariant witness recheck

`runtime recheck` accepts two version-one check envelopes in the embedded CLI.
A counterexample from a different receipt can establish a current refutation
only when the modules, selected root/config, operative CFG including declaration
order, check limits, and authenticated producer agree. The CLI requires one
previous invariant violation named by the configured inventory and a fresh
successful independent replay of its complete ITF ending against the current
model. Its output records both request digests, the previous receipt key and
the validation response. The exact current result takes precedence. A failed
guard falls back to current checking. The [command guide](../temporal-jobs.md#recheck-a-previous-invariant-failure-after-a-configuration-comment-edit)
defines output routes and deadlines.

The `unchanged-loaded-program` relation extends native recheck to project
envelopes that contain unselected siblings. The previous and current checks
require the same root, selected configuration bytes, operation, limits and
worker. Ordinary host admission and module resolution must succeed for both.
The authenticated worker compares the ordered origins and exact bytes of
the resolved modules. Extracted monoliths are ineligible. The caller accepts
this conditional relation only with an authenticated complete ordinary Holds
receipt. The result binds the current caller and request, its establishing
receipt and the ordered loaded paths. It publishes no current receipt.

## Conformance and compatibility

Tests compare embedded results with direct calls to the exact pinned native
library and consume existing recorded primary-tool fixtures where appropriate.
They execute no TLC or Java process. Positive finite examples, real property
violations, forged initial states/steps/endings, unsupported formulas,
resource stops, parent timeout/cancellation and worker failures are distinct
controls. Existing Verus conformance remains unchanged.

The common request/verdict API owns requirement IDs, provenance, content keys
and ledger semantics. This backend owns only the native job payload and
evidence; it does not invent a second ledger or duplicate caller metadata.

## Edit evidence

`edit-session` is a separate version-1 protocol. The isolated native worker
accepts one group of closed exhaustive requests and returns ordered results
after the entire group finishes. The parent authenticates the executable,
checks its linked Dive source and verifies report version, current request
digests, basis lineage, role and limits. A zero queue on a failed safety
prefix cannot establish completion. Each proof is identified by its structure,
basis, current search position and complete current evidence. Ordinary
fallbacks retain the native schema-3 response. The batch does not populate
exact-replay receipts. An `exact-request` entry carries only the zero-based
`basis_entry` of a prior `fresh` conclusive ordinary response. Both child and
parent compare the full native requests. The parent validates the actual earlier
response and the current digest in the authenticated frame, rejecting forward
or chained references, changed payloads, nonconclusive bases and incomplete
evidence. The authenticated worker semantically validates a fresh refutation.
The parent checks the received evidence shape and identity. An exact replay
refers to that same original word and validation for the identical program. No repeated trace or second verdict is stored.

The cross-process `established-invariant-subset` route requires an existing
trusted complete ordinary safety Holds receipt. The worker’s relation report
alone establishes only how the two bound programs relate. The parent checks
the previous receipt, the installed worker digest, both input digests,
effective limits and current caller attribution. A missing receipt, stopped
check, changed transition system, new obligation or inconclusive comparison
runs the current request through ordinary checking.

The `established-first-temporal-claim` route accepts an ordinary, completed
native temporal Holds result for at least two temporal properties. A session
obtains that result from its own earlier ordinary check. Across processes the
parent authenticates the ordinary receipt and selects the same installed worker.
The parent checks prior execution facts, digest, property, scope, limits and
original counts. Inside the worker Dive binds both closed inputs, checks their
execution policies and startup plans, and matches the compiled first temporal
property against the single current property. The previous result must show a
complete native final check with zero periodic prefix checks or fallback atom
evaluations. A successful comparison returns the current property and previous
search counts. Derived evidence cannot create an ordinary receipt or establish
a new basis. A comparison miss follows the existing current-check path.

The development `Cargo.toml` and `Cargo.lock` pin the Dive repository by full
revision under ADR 0172. The edit protocol’s `backend_source` identifies those
bytes and the installation manifest authenticates the worker executable.
A stable Sail crate release must first depend on published Dive crates exposing
these APIs and pass native conformance against that source.

## External symbolic predicate sessions

The local `sail runtime symbolic-check` command owns a separate working
directory and a named external Dive, selected by the caller. Proof rows have
no exploration statistics. A complete search and a validated violation use
the symbolic checker's versioned result format. Issuing a temporal job to
the shared runtime still selects its authenticated native worker. The external `--strategy occupancy` route checks the distinct `dive-occupancy-report-v1` schema and publishes to a separate candidate filename. `--max-orbits` applies only to that strategy. See the
[decision](../decisions/0177-portable-symbolic-predicates.md) and
[command and exits](../temporal-jobs.md#recheck-finite-safety-properties-with-a-portable-predicate).

With `--property-proof NAME`, the symbolic strategy has a separate candidate
file and admits the closed `dive-property-proof-v1` format. Both fresh and
transferred proofs must bind the requested module, configuration and invariant
name. The checker must return an inductive proof with a null failed obligation
and an integer transition-obligation count. A failed candidate can trigger a
fresh check. Fresh success publishes its staged predicate atomically. A
violation must identify that same invariant in the ending and independently
validated trace. Successful proof rows carry work statistics for this property.
A session directory can be reused for a different property because every
candidate is independently discharged against the receiver's source and claim.
