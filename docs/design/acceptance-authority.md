# Scope and result-authority proof model

Status: an initial pure executable model with stock-Verus proofs. It does not
replace production acceptance, prove its adapters, or complete stage C of the
[evidence-acceptance design](evidence-acceptance-core.md). The initial scope is
Sail's existing Verus path. Neutral contracts and temporal backends are outside
this model.

## Checked boundary

`proofs/acceptance/authority.rs` separates untrusted `Candidate` data from a
`ValidatedResult`. The validated type has private fields and no deserialization
or public constructor. Its factory retains immutable borrows of the request
and candidate, so a caller cannot mutate the checked model snapshot while
holding the result.

The executable `validate_scoped` function returns authority exactly when:

- The project is Current, terminates normally with exit zero, and has no
  project, selection or engine fault.
- The request contains at least one target and has a consistent whole/partial
  selection. Whole scope has an empty selection encoding; partial scope has
  its exact nonempty ordered selection encoding.
- Requested targets and observed reports each have unique exact target
  identities. Identity compares package, exact Cargo name, kind, source and
  invocation bytes separately. No name normalization or hash equality enters
  this relation.
- Every requested target has a report for its exact identity and scope. Missing
  targets, failed requested siblings and ambiguous duplicate observations
  cannot satisfy this check.
- Each covering report is complete, terminates successfully, has no errors,
  contains verifier documents, explicitly records successful verification and
  zero verifier errors, has no VIR error or engine fallback, and has no report
  invariant violations.
- Every reported function is verified, confirmed and has an evidence tier.
  Function and verified summary counts both equal the function collection's
  length, and failed/unknown/provisional counts are zero.

`validate_current_complete` additionally requires whole scope. A successful
partial result can obtain scoped authority, but cannot obtain current complete
whole-target authority. The `injective_coverage` theorem establishes that one
observed report cannot cover two distinct requested targets. The proof covers
arbitrary vector lengths, not a bounded list of example targets.

These are checks over the model's observations. A caller supplying plausible
but false verifier flags is not ruled out by this component alone. In
particular, `complete` must ultimately be justified by trusted verification of
the required invocation; checking all supplied function records cannot prove
that the compiler supplied the complete obligation inventory.

## Positive and deliberately broken controls

The model constructs a concrete successful nonempty request, report and function
without assuming that a satisfying execution exists. `positive_control`
obtains whole-target authority. Other checked controls establish that exact
partial scope remains partial, missing target evidence and Historical status
are rejected, and unconfirmed functions or inconsistent summaries are rejected.
Legitimate complete invocations with no verification functions are permitted
when their verifier evidence is otherwise successful; the positive control
uses a nonempty function collection to avoid an empty-collection witness.

Two negative controls change executable validation while retaining its contract:

| Control | Deliberate defect | Required result |
|---|---|---|
| Whole-scope guard | Replace `if !r.scope.whole { return None; }` with `if false { return None; }` | Full-crate proof fails |
| Confirmation guard | Remove `!f.confirmed` from the function rejection condition | Full-crate proof fails |

The negative controls must fail with ordinary verification errors, not syntax,
typing, VIR or runtime-launch errors. No inconsistent axiom, `assume`, admitted
lemma or external-body annotation is used to produce these failures.

## Direct proof invocation and evidence

The stock runtime is the upstream release pinned by `corpus/toolchain.toml`:
Verus `0.2026.10.18.7d82994`, commit
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and pinned Z3 `4.16.0`.
After independently validating the runtime/archive and Z3 hashes, the complete
module is checked directly:

```sh
RUSTUP_TOOLCHAIN=1.99.1 VERUS_Z3_PATH="$SAIL_STOCK_RUNTIME/z3" \
  "$SAIL_STOCK_RUNTIME/verus" proofs/acceptance/authority.rs \
  --no-cheating --output-json
```

Inherited Sail, verifier injection, Rust flags and compiler-wrapper settings
must be cleared before invocation. This command uses neither Sail nor the
fork to establish its proof. The owning checker is responsible for validating
the runtime and binding proof source, expected complete scope and invocation;
setting a path alone does not validate an executable.

The macOS arm64 compatibility run reports `verified: 28`, `errors: 0`,
normal exit zero and `is-verifying-entire-crate: true`. Both deliberately
broken controls fail with ordinary verification errors. This is implementation
verification of the model's executable predicates, not Linux qualification,
production integration or a proof of the running service. Workspace/native
publication gates remain separate requirements.

## Production refinement still required

| Model boundary | Current production source | Remaining obligation |
|---|---|---|
| Exact target identity | `crates/sail-verus/src/managed/receipts.rs`: `TargetIdentity`, `cargo_target`; managed runner target aggregation | Prove the request inventory and actual compiler invocation map to the same package, exact Cargo name, kind, source and invocation bytes without ambiguity. A normalized crate name is insufficient. |
| Exact scope and completeness | Receipt `Invocation::successful_for`, `Receipt::valid_scoped_in`; managed runner requested coverage | Relate the model's exact selection encoding and whole/partial flag to actual ordered arguments, failed invocations without artifacts, and trusted verifier completion of the exact requested invocation. |
| Function/report checks | `crates/sail-api/src/output.rs`: `Report::invariant_violations`, `Summary::of`; receipt `complete` | Prove flattening across all verifier documents, summary derivation, establishment/tier/provenance relations, obligation ownership and uniqueness. The model's invariant-violation count must come from those real checks. |
| Repair authority | `crates/sail-api/src/repair.rs`: `accepted`, `BatchResult::accepted_with` | Relate production Current, successful project/target exits, selected-target flags, full-phase evidence and engine-fault/fallback checks to this predicate. |
| Currentness and retained jobs | Managed read checks and durable API/job result refresh | Establish the Current observation point from the input-binding component and ensure retained serialization and read-time projections preserve authority. |
| Human/JSON/MCP presentation | CLI, structured project results, repair documents and MCP/job surfaces | Prove every current complete success consumes this authority or an equivalent proved predicate. No presentation path is integrated by this model. |

The current production functions remain independently implemented ordinary Rust;
this initial module is not their shared source. Production integration needs a
reviewed source-sharing or refinement mechanism and the existing conformance
and native cost gates. The model deliberately records the gap rather than
claiming that a proof of copied predicates verifies their callers.

## Assumption inventory

- Rust/Verus/vstd semantics, the directly invoked pinned stock verifier, Z3 and
  the build toolchain remain trusted.
- Request identities and ordered selection encodings are supplied by an
  unverified adapter. The model compares exact bytes and makes no digest
  injectivity or canonical-encoding assumption; a production encoding needs
  its own injectivity and complete-input argument.
- Verifier completion, errors, function establishment, invariant violations
  and engine origin are model observations. Their authenticity and faithful
  projection from actual process output are not established here.
- Current is a supplied observation, not an assertion that a writable workspace
  remains immutable forever. The input-observation/refinement component must
  justify its acceptance-point semantics and documented filesystem limits.
- Query/reply truth and cache origins are separate obligations. This module
  neither checks SMT certificates nor proves arbitrary cached success flags.

No runtime default, crate dependency edge, backend selection or production
acceptance behavior changes with this component. Documentation impact is this
component guide; corpus/compiler/solver inputs are unchanged, and no performance
claim is made.
