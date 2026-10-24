# Observed-input transaction proof component

The [input component](../../proofs/acceptance/input_binding.rs) verifies an
inductive observation/reuse transaction model and executable exact-byte
comparators for the [acceptance core](evidence-acceptance-core.md). Its shared
[context](../../proofs/acceptance/context.rs) contains the actual modeled input
map, exact request, selection, runtime and mode. Production filesystem,
compiler-closure and caller refinements remain open; these models do not change
Sail's runtime reuse policy.

## Transaction lifecycle

`InputWorld` records a structured `ModelContext`, a mutation epoch and an
observation point. Each input key preserves a role and its lexical identity.
`ReadOutcome::Present` records resolved identity and bytes; `Absent` and
`Unreadable` are distinct variants. Source, linked-ancestor, generated-file,
configuration, environment and runtime roles have separate keys even when
names or bytes agree. The epoch and point are mathematical model values, not
an assertion that a filesystem timestamp or hash implements a monotone clock.

`RawCapture` is an unfinished transaction with private fields. It records the
original context and epoch, beginning point, required key sequence, actual read
witnesses and an invalidation flag. It is not usable input evidence.

| Transition | Checked behavior |
|---|---|
| Begin | Retains the original epoch and starts with no read witnesses |
| Read | Appends the modeled outcome for the exact key at the read's point and epoch; off-plan, repeated, out-of-order or unreadable observations invalidate the transaction |
| Change | Changes the explicit input/execution context and advances both epoch and point; old observations keep their old epochs |
| Tick | Advances the point while preserving every input fact and execution-context field |
| Invalidate | Makes sealing unavailable, including after cancellation or an observation failure represented by this event |
| Learn requirement | Expands the required inventory and invalidates the old capture; newly discovered generated or declared inputs cannot inherit old completion |
| Reobserve | Begins a new empty transaction; it cannot attach a fresh stamp to the old read witnesses |
| Project | Copies only existing reads with exact requested keys and unique in-range positions; validates the complete projected domain and preserves the original epoch |
| Seal | Validates the whole raw transaction and constructs private usable input evidence at the actual final model point |

`capture_valid` relates every captured read to the world: points and epochs are
within the transaction's observed interval, and a read still at the current
epoch has the actual current modeled outcome. Equal original/current epochs
also preserve the complete original context. Begin establishes this invariant;
every transition preserves it; `all_input_executions_preserve` proves it for
arbitrary finite sequences, including projection, mutation and reobservation.
No premise asserting that an arbitrary platform stamp detects every mutation
is used in this lifecycle induction. A modeled Change advances the epoch by
construction; proving that actual platform changes produce the corresponding
event remains a separate obligation.

`seal_capture` requires that established invariant and checks all of:

- The transaction is not invalidated, the original epoch still matches, and
  the exact request, scope, runtime, mode and input map remain the captured ones.
- The required domain is nonempty, has no duplicate keys and equals the entire
  explicit requested input-map domain.
- There is exactly one matching ordered read witness for every required key;
  every role is supported and every outcome is readable.
- The shared context is well formed. An unreadable entry cannot be represented
  by a successful absence observation.

`InputEvidence` has private construction and no deserializer. Its `view()`
returns the exact structured context; `point()` returns the validation point;
`valid_at(world)` relates its complete read witnesses to that world's actual
map. `input_evidence_facts` exports these properties for composition without
exposing construction. The required-domain check concerns explicit modeled
keys, not a Boolean whose name assumes the compiler's closure complete.

`input_execution_seals_only_current` checks the final transaction of any finite
execution. It returns evidence exactly when sealing is admissible and proves
that every returned context and point are the final modeled context and point.
The closed finite-trace control includes a real nonempty accepting execution
and rejection of the same reads followed by an input mutation.

## Projection, retained reads and exact reversion

A projected context must preserve request, runtime and mode and be an exact
restriction of the current input map. The declared required keys must cover
that projected map exactly, without duplicates. Existing read positions must
be unique and match each requested key. A projection with the same scope, or
one claiming whole scope, cannot drop any of the full map's keys. A valid
partial projection retains its exact partial selection; it does not become
whole-target evidence.

Projection preserves the original capture epoch and read witnesses. It remains
a raw candidate until final validation after the batch. A mismatch invalidates
the projected state, and subsequent reobservation starts empty. Actual
compiler closure and dependency selection must still justify which map a
requested partial selection denotes.

Retained input evidence describes its original observation point, not every
future point. `refresh_retained` requires a newly completed transaction begun
strictly after the retained evidence's point, checks the exact old context
against the newly observed context and returns newly sealed evidence at the
read point. Its postcondition also proves that every returned read witness is
later than the old evidence. Merely moving the old raw capture to a later
point does not pass this factory.

Mutation followed by exact restoration advances the modeled epoch twice.
The old capture cannot seal even when the input map has returned byte-for-byte
to its original form. A fresh complete capture after the revert can match the
old context and authorize input reuse at that later point. This distinguishes
safe content reversion from reusing an old guard through an ABA change.

The earlier `Atom`/vector comparators remain checked independently. They prove
executable exact-byte identity and selected-position comparison, including
lexical/resolved bindings and presence. Reordering conservatively misses;
no path normalization or digest injectivity is used. The earlier
`guarded_observation_unchanged` lemma is conditional on an explicitly covering
counter; it does not independently establish a concrete stamp's coverage.

## Environment and authentication selection

The current adapter excludes `SAIL_DEPENDENCY_READ_TOKEN` and
`SAIL_PRIVATE_GIT_READY` from implicit proof-context selection. Fetched source
bytes remain source inputs. Explicit `env!`/tracked compiler reads and build
scripts' declared reads still bind each variable's exact value or absence.

`environment_is_bound` verifies the corresponding decision shape: a declared
read is always bound, while authentication-only plumbing is excluded from
implicit selection. The credential control checks both cases. A modeled Tick
may represent a plumbing change only when the entire effective input context
is unchanged. A declared credential read changes the environment input fact
and must instead be modeled as a Change. Relating actual variable names,
compiler declarations and tool behavior to this classification remains part
of the environment-adapter refinement.

## Proof checking and controls

The [owning checker](acceptance-proof-checker.md) captures this module and the
shared context from Git, generates the complete driver and invokes the validated
stock Verus and pinned Z3 directly with `--no-cheating`. It checks exact
whole-driver counts and the complete function inventory. Sail's driver, fork,
shim, caches and managed receipts are not proof authorities.

```sh
python3.12 scripts/check-acceptance-proofs.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_PROOF_OUT"
```

The output directory must be outside the checkout. The manifest and owning
[validation record](../validation/evidence-acceptance-models.md) identify the
actual checked source, runtime, scope, counts and platform qualification. The
expanded module has passed the captured-driver macOS compatibility check; that
is checking an inductive model proof with stock Verus, not production acceptance or a proof
that an OS adapter refines this model.

The source adds no project `assume`, `admit`, `external_body` or external
specification. Stock Verus, Z3, pinned vstd, the compiler and the validated
runtime remain trusted. The code uses exact bytes and structured maps; it does
not model a hash as injective.

Closed controls check nonempty successful captures with all six roles,
read-time reuse, rejection of old restamped reads, mutation/ABA/exact revert,
partial reads, unreadable outcomes, valid partial projection, duplicate or
incomplete projection, false whole-scope claims and credential selection.

Deliberately broken source controls cover the actual obligations:

| Broken check | Required verification failure |
|---|---|
| Omit alias destination comparison | Executable atom comparison no longer implements exact input identity |
| Omit selected-position bounds | Selected indexing is no longer justified for every request |
| Omit the original epoch in sealing | Old or ABA observations could claim new-point evidence |
| Omit equality of required and context-map domains | Observing a plausible subset could claim complete input evidence |

The checker mutates temporary captured copies using unique manifest needles,
retains the same verification scope and requires actual verification errors.
Parse errors, dropped proof functions and resource-limit failures do not count
as successful negative controls.

## Concrete refinement still required

The completed model obligations above are separate from these open interfaces:

| Production boundary | Remaining relation |
|---|---|
| `managed/input.rs`: `Bindings::observe`, `collect`, `stamped_snapshot` | Preserve every consumed lexical source and linked ancestor, canonical destination, bytes and absence; distinguish errors; justify the observation point |
| `managed/source_inputs.rs`, `managed/build_inputs_sources.rs` | Relate the explicit required map to actual compiler-consumed files and generated inputs, including facts learned after compilation and consuming-package attribution |
| `managed/input.rs`: `closure_digests`, `closure_bindings`, `Scope` | Derive the complete requested map and correct partial restriction, preserving declared external aliases and Cargo manifests/configuration |
| `managed/input.rs`: shared projections, reobservations, `unchanged_since` | Relate actual original stamps and mutation observations to the modeled Change/Tick distinction; preserve the original guard across the entire batch and reobserve unsupported cases |
| `managed/runner.rs`: context/environment/runtime identities; `managed/environment.rs`; receipts | Bind the actual executed runtime, options, effective environment, declared reads and source map to the structured context |
| `managed/read_check.rs`: `entries`, `record`, `check` | Reconstruct fresh read witnesses at the API observation point and preserve explicit absence/bindings; deserialize only candidates and validate before usable authority |

In particular, these interfaces are not discharged by:

- An inventory marked complete, a count of observed rows, or an assertion that
  a function returns all effective inputs. The real compiler-input closure is
  still unproved.
- Filesystem metadata that can miss mutation classes. The exclusions in
  [freshness](../freshness.md) remain exclusions on Linux and macOS. Atomicity,
  counter coverage, encoded path bytes, read failures and point ordering across
  processes or recovery need concrete justification.
- Digest equality without an explicit cryptographic assumption or byte check.
  Runtime immutability/launch binding also remains part of the
  [trust contract](../trust.md).
- Loading a durable successful record, scheduling hints or mutation notices.
  Recovery must establish valid fresh observations or refuse currentness;
  private model tokens do not verify the production serialization path.
- Calling these proof functions from an unverified adapter. Production does
  not compile or call this module, and source sharing plus every acceptance
  caller still need implementation refinement, conformance and native cost
  qualification.

The model proves complete observation of its explicit requested map and safe
reuse within its transition system. It does not prove that Sail's real
compiler/filesystem observations produce that map or that every production
Current response follows this authority path.
