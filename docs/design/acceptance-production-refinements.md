# Production acceptance refinements

Status: source sharing, concrete refinement proofs and production callers pass
native macOS and Linux checks. Native pre-push gates, hosted checks and full
fast/standard conformance pass. The owner accepts the bounded native timing
result; its measurement limits and the remaining service-wide theorem are
recorded separately. The owning
[validation record](../validation/acceptance-production-refinements.md) separates
checked source from accepted integration.

The combined revision-only input projection passes fresh native gates, both
direct-stock proof suites and full fast/standard conformance. The final
documentation head must pass all hosted checks before merge.

The [acceptance design](evidence-acceptance-core.md) targets four independent
risks: wrong query/reply association, stale input reuse, incomplete requested
scope and misleading verified reports. This implementation connects selected
production operations to those models. It does not establish the proposed
service-wide theorem.

## What production executes

`crates/sail-acceptance` is a dependency-free leaf with five direct callers:
`sail-api`, `sail-verus`, `sail-shim`, `sail-cache` and `sail-daemon`.
[ADR 0079](../decisions/0079-source-shared-acceptance-refinements.md) records
that graph. The existing shim isolation rules still apply.

The leaf's syntax envelopes preserve executable arguments, types, conditions
and bodies. Ordinary Cargo erases contracts and proof hints. Direct stock
Verus checks those same bodies with a fixed `sail_verus` configuration. This
is source sharing; neither copying a predicate into a proof nor asserting that
an arbitrary candidate is valid supplies the connection.

| Boundary | Checked executable source and concrete relation | Production callers |
|---|---|---|
| Query association | Owned FIFO and bound request/reply pairs; replay cursor; permutation-checked publication order; sticky publication eligibility; recovery field admission | Shim pairing, replay and publication; cache and daemon recovery admission |
| Input reuse | Exact byte/key comparisons, complete readback ledgers, checked permutations, original capture/guard preservation and selected-scope coverage | Input snapshots, read-time refresh, receipts and parallel file/artifact observations |
| Scope and authority | Actual typed report fields; all summary counters; confirmed verdict/provenance/tier checks; complete one-to-one target coverage; distinct receipt, whole-target and repair policies | Receipt reuse, runner completion, repair results, retained job projections and compact MCP completeness |

`proofs/refinement/{query,inputs,authority}.rs` states concrete-to-model
relations. For example, a factory returning `CompleteReport` guarantees the
actual borrowed report satisfies the checked predicate; projection of any
selected function then satisfies the authority model's good-function relation.
Private fields prevent ordinary callers from constructing those authority
tokens. Sorting outside the checker only proposes an order: checked forward
and inverse mappings must independently establish complete unique coverage.

The input ledger records actual worker-returned descriptors and raw
Present/Absent/Unreadable outcomes. Expected input identities are constructed
independently. Relabeling an observed value with an expected path would defeat
the correspondence; readers preserve the returned descriptor through admission
and fold hashes only after keyed validation, in the original requested order.
This is native adapter hardening. It does not prove that the inventory contains
every compiler input or that the operating system observed an atomic snapshot.

Revision-only projections borrow the original keyed digest observations rather
than constructing an unused snapshot. They still invoke the checked complete
read ledger and original-capture projection, retaining the worker-returned path
with each outcome. The caller checks the source observation's original stamp
after the complete projection batch. This adapter remains outside the pure
source-shared theorem.

## Verification must actually run

A successful process and an empty function inventory do not distinguish a
legitimate zero-function proof from `--no-verify`. The pinned stock verifier
reports success for a deliberately false assertion when that option skips
verification; [VI-0035](../verus-issues/VI-0035-no-verify-success.md) contains
the reproduction.

Acceptance must record the effective per-package verification setting and
refuse compile-only evidence for proof authority. Inspecting only the outer
command misses package-specific overrides. Genuine successful zero-function
verification remains admissible. Current input identity is a separate fact:
compile-only or failed observations may be Current without being verified.
Retained records from an earlier producer require conservative admission; a
new envelope marker alone cannot establish that an old report was verified.
CLI job labels and MCP proof headlines consume checked target authority and
distinguish current, historical and unknown input observations.

The original lifecycle model omitted whether verification was enabled.
Comparing its completion event with the actual driver exposed that missing
premise; the direct stock reproduction established that the verifier emits that metadata.
The model now records `VerificationSetting`, and the checked production
predicate requires verification to be enabled. Removing that guard makes the
selected proof controls fail. The reproduction establishes the real behavior;
the amended proof checks the acceptance rule over its stated inputs. Native
option decoding and the complete service correspondence retain their explicit
refinement boundaries.

## Pre-execution request construction candidate

The source candidate for [#375](https://github.com/Chelis-Lang/sail/issues/375)
connects actual production construction operations to shared executable bodies:

- The CLI's `without_incremental` delegates to
  `normalize_submitted_arguments`. Its contract removes only the deprecated
  no-op spelling before the first `--`; every remaining original argument is
  copied in order, including forwarded `--incremental` text.
- `managed/request.rs::Resolution::resolve` calls
  `construct_resolved_request` with original run, rendered invocation/workspace,
  context, revision, arguments, engine, repair selection and target fields.
  The shared constructor retains every field and argument position. Its input
  record is ordinary data, not request, currentness or input-domain authority.
- Each actual native target-map step calls `construct_request_target`, preserving
  the original package ID, target and kind. The actual engine identity method
  delegates to `construct_interactive_identity`, preserving the selected digest
  without treating it as physical launch evidence.
- `expected_targets` calls `cargo_workspace_selected`. The shared iff predicate
  counts the two existing workspace flag spellings only before the first `--`,
  correcting the native scan that also counted forwarded text
  ([#467](https://github.com/Chelis-Lang/sail/issues/467)). This checks that
  actual prefix-selection operation, not the entire Cargo argument grammar.

Erased constructive controls cover nonempty original arguments and target
triples, prefix versus forwarded no-op flags, and original versus forwarded
workspace spellings. Compile-only and partial arguments produced by the actual
constructor still refuse the existing whole-request factory for arbitrary later
project rows. Normal native regressions exercise the actual nonempty resolution
builder and independent Cargo prefix selection. The complete production Stock
roots pass at `f98ea578`, including the actual constructor, key/set operations and
these constructive/refusal witnesses. Actual inventories retain every prior
function. Ordinary mutation qualification, native tests and differential/cost
qualification remain pending; native Clippy reports the style diagnostics in the
owning qualification record.

The candidate registers 22 mandatory ordinary mutations over the actual
construction, normalization and workspace-prefix bodies. Their changed source
spans are inside the named executable functions and outside their contracts and
erased proof blocks; original loop scaffolding remains. All 397 inherited
controls remain required. Actual verifier/function counts are not
guessed: the older inventories remain until new complete-root discovery, and
every new mutation still needs an ordinary failure at the full scope. The
separate known `4c43e5a9` query control 151 selector failure is corrected by
importing the exact reviewed full-method control from `00352b64`; it still needs
ordinary negative qualification. All other inherited controls are unchanged.

The current proof guidance interprets actual fixed byte-array lengths and the
standard ordered-set sequence specification explicitly. Finite witnesses reveal
both a complete partial-selector literal and its real accepted prefix. These are
erased facts about existing operations; the default Stock resource limits,
original contracts and all admission guards remain unchanged. Complete-root
discovery now passes at `f98ea578`, locking only the actual verified/function
inventories. It supplies no ordinary-negative or native-test credit; the owning
[qualification record](../validation/acceptance-boundary-refinements.md) records
the exact source, subsequent lint stop and archive.

Set iteration remains a distinct operation. The earlier original
`BTreeSet<(String, String, String)>` supplied the target vector. Current vstd
source makes set-domain
coverage conditional on comparator laws; source review alone does not establish
those laws for String keys. Dedicated complete-file support diagnostics are
separate from acceptance qualification. If necessary, a reviewed internal key
with an actual checked bytewise lexicographic comparator can preserve the old
String-tuple ordering while enabling checked set construction/iteration. That
conversion needs actual checking and native qualification; there is no assumed
ordering or supplied completeness Boolean.

Native support diagnostics now distinguish the two cases at the same pinned
Stock setup. The complete String and String-tuple order-law file fails two
ordinary postconditions; the complete actual String-tuple `BTreeSet` domain file
fails its two coverage/no-duplicate assertions. Identical numeric `u64` controls
both pass. The initial two missing-`main` attempts are invalid and earn zero
support credit. All six attempts are retained at
`experiments/2026-10-27/acceptance-request-support-4c43-native.tgz`, SHA-256
`b7860ae37002e2c74e27b8505065a9d6968743bc87bf1a9989ef0fec3c55e9d0`
(7,284,334 bytes). This is library-support evidence, not acceptance or a false
verdict finding. Independent storage readback remains separately reported.

The child source candidate adds `request_order.rs`: an immutable original
String-tuple key, actual bytewise comparison/equality/clone implementations,
and constructive byte/key order-law bodies for `OrdSpecImpl`. Its byte order
matches Rust's original String/tuple field order by source review; an ordinary
native differential fixture covers prefixes, embedded NUL, Unicode, field
precedence, duplicate keys and actual set iteration against the old tuple set.
The native differential fixture remains unrun. The complete-file Stock support
probe at `367c5e03` stops in 9.04 seconds with four VIR errors and empty function
details: the ordinary `sail_method!` envelope emits `requires(true)` on trait
implementations, which must inherit their trait preconditions
([#472](https://github.com/Chelis-Lang/sail/issues/472)). Final source/runtime
checks pass, but this attempt earns zero proof credit. Its correction adds
`sail_inherited_method!`, whose native signature/body is identical and whose
Stock expansion inherits trait preconditions and checks the original additional
postconditions. Only the four Eq/PartialOrd/Ord/Clone methods change envelopes;
their executable bodies remain identical. The reviewed syntax fingerprint and
checked-function envelope inventory include this macro, with ordinary source
guard controls retaining refusal of raw or unknown-envelope methods. Those
source-guard controls remain unrun.
The corrected trait-envelope probe first stops at two helper mode errors: its
executable wrapper called a proof function without a proof block. With those
two ghost calls correctly scoped, the unchanged whole file reaches 15 verified
and two failed functions in 10.05 seconds without resource exhaustion. The
remaining three postcondition diagnostics concern executable `Ordering`
equality in byte equality and the key comparator's first return, including its
inherited standard-trait postcondition. A source follow-up uses direct enum
variant matching for those three actual variant tests and retains every byte
scan, return value and postcondition. No
ordering equality law is assumed and no function is excluded from the probe.
The `62a1d31a` whole-file support probe passes 17 verified functions, zero errors
and all 333 actual function details in 9.589 seconds, with final source/runtime
checks and unchanged default scope/resources. All five candidate probe attempts
are retained in
`experiments/2026-10-27/acceptance-request-arc-support-native.tgz`, SHA-256
`42e4e848571415ad693fd15d6839c735146346638d4597b670c9ad15c3e2693c`
(7,662,072 bytes), with independent storage readback. This is standalone
library-support evidence, not production canonical qualification. The candidate
now connects actual set storage/insertion/iteration and request projection to
that key. This projection iterates the real original set, not a supplied
list or mint a physical compiler-input domain. New comparator primitives add
no String-order axiom, dependency, public record schema or provider selection.
The follow-up key interface exposes closed original-tuple projections and
constructor/borrow round-trip postconditions. Later projection proofs can use
those checked relations without exposing private fields or assuming a digest
or String-view injection law. Projection/header caller conversion leaves the
`62a1d31a` comparator runtime bodies unchanged.

The separate `request_targets.rs` source candidate implements operations over
the real `BTreeSet<OriginalTargetKey>`. Insertion passes the actual original
triple to the checked key constructor and standard set insertion. Projection
consumes only actual `Iterator::next` outcomes and calls the same actual target
record constructor. Its loop relates produced keys plus the iterator's remaining
keys to the immutable original dictionary: exact cardinality, set coverage,
uniqueness and order. There is no supplied complete-list or success Boolean.
An unexpected iterator end returns explicit absence, never a truncated row
list. The constructive postcondition additionally requires a present complete
projection for every valid original standard-library dictionary, so an
always-refusing implementation must fail verification.
The ordinary native fixture compares insertion, duplicate handling and complete
projection with the original String-tuple set. These bodies, their contracts
pass complete-root Stock discovery; the native fixture remains unrun. The managed callers now
use these operations, while no publication or runtime selection is implied.

The same candidate adds `OriginalRequestHeader`, a borrowed construction input
without target rows, and `construct_original_request(header, &OriginalTargets)`.
Its body projects the actual dictionary and feeds only those rows to the actual
existing request constructor. Its checked postcondition combines the existing
original-header relation with actual dictionary coverage, cardinality, uniqueness
and order. `ResolvedRequest` remains the single resulting request type; the
borrowed header carries no authority. A nonempty ordinary fixture retains every
header/engine/repair field and compile-only arguments across two actual original
targets. The operation, fixture and actual `Resolution` caller conversion
pass complete-root Stock discovery; the native fixture and surrounding caller
behavior still need native checks. No result-selected list, input-domain provider
or metadata enumeration completeness is inferred from this constructor.

The candidate caller conversion changes the actual requested-target set type,
delegates each real metadata insertion to `insert_original_target`, and makes
`Resolution` call `construct_original_request` on that original set and its
borrowed header. Both actual pre-Cargo request capture and unchanged-call
preflight consume that result. Unexpected projection absence refuses replay or
returns the existing `SAIL3115` capture error before Cargo starts; it cannot
admit a truncated request. Existing
native dependency/cleanup predicates can borrow the unchanged original tuple
through `as_tuple`; no additional String clones, row lists or namespace
reconstruction are needed. Actual metadata and Cargo-unit-graph root insertion
use the same checked insertion operation; the original metadata filters,
feature tests, target kinds and option/value-state handling are preserved.
Their JSON decoding and upstream selection remain native and unproved. Native
differential fixtures live in separate test modules, outside the captured proof
bodies. Closed canonical module/import/spec-trait inventories include both
actual modules and 29 additional executable mutation controls. Complete-root
discovery passes at `f98ea578`; ordinary negatives, native checks and cost
measurement remain pending. Older source hashes and numeric support probes cannot qualify
this join.

The first complete native source-capture attempt at `23ccd443` stops before
Stock or Rust compilation with 66 production source tests, one failure and five
errors ([#479](https://github.com/Chelis-Lang/sail/issues/479)). The strict checks
identify the stale second binding for the same API library, inconsistent CLI
dependency ordering and an assembly fixture missing the newly protected key.
The correction refreshes both actual-library bindings, uses the exact declared
dependency ordering without changing membership, and adds the key to the
existing fixture with a public-field rejection. No production body, contract,
resource limit or mutation object changes. Corrected-source qualification
remains pending; the failed attempt and exact retained source/runtime evidence
are indexed in the
[owning validation record](../validation/acceptance-boundary-refinements.md).

The corrected capture attempt passes the ordinary source suites and complete
query/input proof roots, then the full authority root refuses two unsupported
annotation forms: a direct call to the zero-argument reference-order broadcast
lemma and an undereferenced final iterator snapshot before a spec-method call.
The correction uses the standard broadcast directive and explicit snapshot
dereference. Runtime bodies and the relations they must prove stay unchanged;
the complete authority root and native stages still need qualification.

The next whole-root run reaches actual authority verification and identifies
missing argument-projection, literal, iterator-order and vector-push guidance.
The constructor now additionally exposes exact original argument sequence
equality, proved by its existing clone loop; the header/dictionary join carries
that fact directly to later scope checks. Checked mathematical helpers frame
the actual row push and byte projection. Standard-library order interpretation
and compiler literal reveals expose existing meanings without adding an order,
digest-injection, metadata-completeness or whole-compiler assumption. The finite
normalization and compile-only/partial witnesses retain their original claims
and default proof resources. No runtime body or guard changes; the
[owning record](../validation/acceptance-boundary-refinements.md) retains the
failed run and remaining qualification.

Native path rendering still uses the old `to_string_lossy` operation and the
independent raw-path binding, including invalid-byte refusal. Cargo metadata
decoding/selection, argument option/value state, per-unit effective options,
original submission/mode transport and native compiler launch remain explicit
joins. The constructor does not derive expected targets from result rows or
prove that its supplied projection exhausts Cargo's real selection. Physical
compiler-input/provider questions 38 and 39 and all six service obligations
remain open. No schema, dependency, runtime or provider is selected by this slice.

## Captured worker request selection candidate

The next source slice moves `jobs/worker.rs::captured_request`'s actual selection
loop to `request_capture.rs::select_original_request`. The native reader still
reads the original journal bytes, requires UTF-8, decodes every original
`RequestCapture` row and obtains the canonical invocation from the submitted
project directory. The checked operation consumes that actual decoded vector;
it does not receive an expected request/target list built from result rows.

Every row must retain the actual `REQUEST_CAPTURE_VERSION` and submitted worker
ID, including rows for ignored earlier runs. Only the output run is eligible;
there must be exactly one matching row, with the original canonical invocation,
submission arguments/mode and checked request/project correspondence. The
operation removes and returns that actual owned request. It grants no whole,
partial, Current or proof facet: failed result records can still have legitimate
original request data. Its constructive domain includes a nonempty matching row
behind arbitrary earlier runs, so an always-refusing selector cannot satisfy its
contract. Earlier runs' invocation fields remain ignored exactly as before.

The existing `bind_submission` contract additionally exposes its already checked
concrete argument/mode relation outside the defining module. No constructor,
guard, classification or argument normalization changes. The source closure
includes the actual `RequestCapture` fields and actual capture-version constant,
the new module/imports and the native decoder/canonicalization/caller binding.
Missing, duplicate, changed or attributed capture-version declarations refuse
the representation guard; conditional constants cannot become an unconditional
literal in the checked projection.
All four new witnesses stay inside the existing whole-authority Verus wrapper.
The worker source is bound in the input/authority dependency closure; the query
component retains its prior independent source domain.
Twelve new executable mutations retain all loop contracts and existing controls;
positive and ordinary refusal witnesses check selected ownership, missing runs,
invalid ignored-row headers and duplicate matching runs. Native journal fixtures
retain every earlier assertion and add version/invocation and ignored-header
cases. Complete-root discovery at `83627504-stock2` passes 566 authority checks
and all 972 actual authority function entries, retaining every prior function.
Full workspace/all-target strict Clippy and all 18 selected native fixtures pass.
The source-suite receipt remains `f12c50c4` with unchanged-harness checks; ordinary
negative controls and complete service qualification remain pending.

Decoding now retains the full decoded journal until selection, rather than only
the current row and saved match. Its additional allocation and rejection-path
parsing cost need measurement before merge; no speed claim is made. JSON decoding,
canonicalization, journal origin/authentication, original worker launch/mode
transport, Cargo metadata/option semantics and physical compiler-input closure
remain explicit native joins. Selecting a matching data row does not establish
that those providers observed the original physical execution.

## Effective report-mode construction candidate

The next source slice moves the actual adapter's
`report_verification_arguments` loop to
`request_mode.rs::construct_report_verification_arguments`. The real adapter
passes the same original report tokens and optional decoded per-package options
to that operation. The existing common/current-package lookup, packed decoder and
checked `effective_driver_arguments_match` remain unchanged; no new environment,
transport, runtime or physical argument selector is introduced.

The checked operation derives its mode from the provided options through
`verification_enabled_options`. Missing options return absence, while present
options construct a command. It retains every original token in order except the
existing removal of forwarded `--no-verify` for an enabled package. A disabled
package retains its original command and gets only the existing necessary final
`--no-verify` marker (plus `--` when absent). The contract records that exact
projection and equates the reported verification predicate with the observed
option-mode predicate. This prevents the construction step from turning an
observed compile-only mode into a verified-looking command or silently dropping
other original tokens, including focus flags.

The command's Cargo separator and the effective options' Rust separator keep
their existing different interpretations. Constructive witnesses cover nonempty
enabled, disabled, forwarded payload and forwarded effective-option examples;
an unknown-option witness refuses. The real native fixture retains all original
assertions and adds prefix/focus, Unicode and embedded-NUL data preservation.
Fourteen executable controls cover mode and separator guards, exact original
argument retention, missing observations, necessary/exact marker appends,
unnecessary appends and a full always-refusing operation with its loop intact.
All existing 63 request controls remain required. No new verified/function counts
are guessed; whole-root, native and ordinary-negative qualification are pending.

The proposed guarantee is conditional on the provided observed options. Their origin and
identity relative to the compiler's actual consumed arguments remain native
joins, as do original launch, metadata decoding/enumeration and full Cargo
option/value semantics. In particular, the native `expected_targets` package,
feature and target filters and its actual selector scan remain mandatory later
work in #375. This slice completes none of the physical/provider obligations.

## Independent checking

Commit the source first, then run Python 3.11 or newer:

```sh
python3 scripts/check-acceptance-refinements.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_REFINEMENT_OUTPUT"
```

The output is a new directory outside the checkout. `ci/local.sh refinements`
uses the same runner in the core CI image. The separate
**Acceptance production refinements** aggregate feeds **CI completion**.
Its query, inputs and authority jobs each check a complete root and all of
its controls; every job must succeed. Its
native wrapper takes 197 seconds on the qualified Linux host; the hosted
production-refinement and model-proof jobs also pass.

The merged draft retains proof/refinement CI wiring pending the owner's
resolution of the policy conflict: main's #434 makes these checks local-only,
while the session's AGENTS.md requires them in CI. Neither policy is selected
by this source reconciliation; final workflow policy remains an open gate.

The runner captures canonical Git blobs with inherited Git redirection and
replace refs disabled. Fixed drivers compile captured source, never reopened
working paths. It validates the pinned stock archive, every extracted runtime
file, Rust identity, module/dependency bindings, complete function inventories
and exact result counts. Source discovery, file kinds and auto-build-script
absence are checked again after verification. Stock runs directly with pinned Z3, sanitized settings,
`--no-cheating --output-json --num-threads 1`. Sail, its fork and its caches do
not certify these proofs.

Actual report types include serialization and presentation code outside stock
Verus's supported subset. A conservative declaration stencil captures the
actual authority-read fields, types and enum variants. It records all omitted
fields and hashes of the full original declarations and binding environment.
Conditional fields, custom derives, type substitutions and unsupported shapes
refuse checking. Omitted presentation fields are not assumptions of validity.
The stencil generator, syntax envelopes, compiler, ordinary sort forwarders
and Python runner remain reviewed and tested trust dependencies.

Every selected guard mutation changes the captured production dependency.
It must fail through ordinary verification errors with complete unchanged
scope. Syntax errors, missing modules, signals, malformed JSON, resource
limits and timeouts fail the gate. The report distinguishes checked bodies,
declaration projections and surrounding runtime source bound only by hash.
Binding a file's hash does not prove its behavior.

The exact-partial request full-factory refusal control retains every original
loop and invariant. Its deliberately wrong early refusal rejects actual
singleton target requests, including the existing nonempty constructive
example; it does so before the original proof-only unfolding, and the final
refusal rejects all remaining inputs. The original loop
remains reachable for non-singleton requests satisfying the original guards.
There is no unconditional early return, unreachable-loop padding or unrelated
false assertion. Production code and contracts are unchanged. The complete
function inventory and verification count stay mandatory; a resource refusal
still earns no control credit. At frozen `6a4e5b86`, the complete authority
positive and this ordinary refusal control pass with 498 obligations/all 878
functions; two selected path controls also pass. This is three controls only,
not full-component qualification. Earlier proof-opacity attempts did not remove
the resource refusal and were reverted. Qualification is recorded in the
[owning record](../validation/acceptance-boundary-refinements.md).

## What remains to prove

| Remaining obligation | Why these refinements do not discharge it |
|---|---|
| Parser/framing and exact raw SMT prefix | Checked owned pairs cannot establish that native parsing assigned the right bytes to each pair |
| Cache writer, persistence and initial origins | Field admission and association preservation cannot prove an arbitrary plausible `unsat` came from a truthful solver for that query |
| Actual compiler-input closure | Complete coverage of a supplied ledger does not prove the supplied inventory contains every input actually consumed |
| Filesystem and launch observations | Tagged read outcomes and executable hashes do not establish atomic path/process identity or observation |
| Request/output correspondence | Typed whole-report authority does not by itself prove every schema-two report is bound to the retained original request |
| All public success surfaces and composition | Individual proved operations and native callers do not constitute a service-wide transition refinement |
| Native integration and cost | Proof checking does not establish conformance, diagnostic parity or performance |

Stages C–F in the original design therefore remain open as integration gates.
Stage C has actual source-shared logic and callers; D and E have selected
concrete operations and hardened adapters. Their remaining obligations cannot
be replaced with an assumed successful-result Boolean. Shared/bundled writers,
cryptography and trusted Verus translation retain their named trust boundaries.

## Low-level host interfaces

The syntax envelopes support contract-only host trait methods and identical
ordinary `where` bounds in both profiles. Proof-only trait spec methods erase
for Cargo; native executable method signatures and generic function bodies
remain the same. A concrete implementation must satisfy those contracts or be
bound to a reviewed low-level OS/library primitive with its explicit assumption
inventory. An arbitrary native trait implementation supplies no physical-input,
solver-origin or completed-service authority. The interfaces let checked
controllers own read/launch loops instead of proving only supplied event lists.

The closed capture permits the exact reviewed `FileReadHost` declaration and
the `ScriptedReadFile`/`FileObservation` ghost declarations by token identity.
All other erased type bindings and host traits still refuse. Changing a host
contract, renaming a ghost type to shadow an executable type, or adding an
unchecked constructor for `CompletedRead` or `GuardedReadback` refuses before
verification. Reports list this abstract primitive contract separately from
checked bodies and reviewed native bridges. Accepting the declaration does not
prove the private native implementation satisfies it or establish compiler
input closure. Those production obligations remain explicit.

The query driver also captures the production `key_prefix` controller and
`sail-key::prefix_factory` body. Its declaration stencil retains every actual
`SolverIdentity` field, the 32-byte binary identity and both `Mode` variants,
including the opt-in string. The complete native key module pins the import,
factory and private SHA adapter bindings; reports distinguish those reviewed
primitives from the checked constructor and declaration projection. Key crate
redirection and late build scripts refuse the gate.

Only the exact reviewed `HashHost` contract and executable `RecordingHash`
trait implementation are admitted. The latter must remain inside the
stock-checked syntax envelope; placing identical ordinary methods outside it
still refuses. Native SHA-256 is not replaced by that recording control, and
neither the trait contract nor a digest provides solver origin. The syntax
header preserves the literal `: Sized` bound in both profiles. Whole-root
inventories and eleven additional ordinary guard controls must qualify these
changes before production selection.


## Complete controls in bounded CI jobs

The native `a615dbba` authority partition takes 549.82 seconds for its complete
44-case matrix, close to the unchanged ten-minute job deadline. Each CI
component therefore runs four fixed control partitions. Every partition checks
the entire positive root at normal resources and verifies its assigned ordinary
failing mutations; controls are assigned by their original manifest ordinal.
The required aggregate accepts only when all four jobs for all three components
succeed. Missing, cancelled, skipped or failing matrix results refuse it. Runner
failure tests check the exact CI matrices and disjoint complete control coverage.

`ci/local.sh refinements` still checks every root and every control together.
`ci/local.sh refinements authority 0/4` is one diagnostic/CI partition, whose
report explicitly withholds complete-component and complete-suite qualification.
No partition uses focused Verus verification or changes proof obligations,
function inventories, mutation definitions, resource limits or job deadlines.
