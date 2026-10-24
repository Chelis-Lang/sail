# Observed-input binding proof component

The [input component](../../proofs/acceptance/input_binding.rs) verifies an
executable exact-byte comparator and a pure observation-history model for the
[acceptance-core proposal](evidence-acceptance-core.md). It does not verify
Sail's filesystem adapter, establish the compiler's complete input closure, or
change production reuse. Stage E and production integration remain open.

## Checked boundary

An `Atom` records a namespace, lexical identity, resolved identity, presence bit
and exact value bytes. Source files, configuration, environment, runtime
identities, generated inputs and linked ancestors occupy separate namespaces.
The namespace assignment and conversion from real observations are adapter
obligations; the public model data is not a constructor for Current authority.
An unreadable input is not an observed absent input.

Inventories are ordered sequences. Their logical identity contains every atom
field, including the lexical-to-resolved relation. The executable `same_bytes`,
`matches_atom` and `matches_binding` loops are proved equivalent to that exact
identity. No digest function, collision assumption or path normalization is
used in these proofs. Reordering an inventory safely causes a miss.

The executable `matches_scope` comparator checks every selected position for
presence and exact atom equality. It proves equality only for those positions.
It does not choose the requested compiler-input closure. An empty selection
cannot establish whole-target completeness, and repeated indices cannot cover
additional distinct inputs. The scope-coverage component must establish that
the selection accounts for every required input.

The checked consequences are:

- Changing a source or ancestor destination changes its binding even if the
  canonical file contents remain present elsewhere in the inventory.
- Changing a lexical identity, namespace, presence bit or value bytes rejects
  an exact match.
- A matching full inventory projects to any in-range selected scope; a changed
  selected alias or an absent selected position cannot match.
- A retained binding matching a valid fresh observation matches the modeled
  inputs at that observation's point. It promises nothing about later points.
- An unchanged mutation counter preserves an original observation when that
  counter is monotone and covers every modeled binding change.
- Exact restoration allows a fresh match. It does not restore the old mutation
  counter through an ABA change. Attaching a later observation point to old
  bytes does not establish a fresh observation.

The counter-coverage predicate is an explicit premise of the guard theorem.
It is not a claim that `stat`, a digest of metadata, a watcher event count or
any current Sail stamp satisfies that predicate. The unconditional executable
comparison proofs do not rely on that premise.

## Proof checking and controls

Use the stock Verus and bundled Z3 selected and validated from
`corpus/toolchain.toml`. The proof root is checked directly as a complete crate;
Sail's driver, engine, shim and managed receipts are not proof authorities.
For a validated stock directory in `SAIL_STOCK_RUNTIME`, the direct invocation
is:

```sh
env -i HOME="$HOME" PATH="$PATH" \
  RUSTUP_TOOLCHAIN=1.99.1 \
  VERUS_Z3_PATH="$SAIL_STOCK_RUNTIME/z3" \
  "$SAIL_STOCK_RUNTIME/verus" proofs/acceptance/input_binding.rs \
  --no-cheating --output-json
```

The root uses ordinary pinned vstd specifications. It adds no project `assume`,
`admit`, `external_body` or external specification. The stock verifier, Z3,
vstd, compiler and validated runtime installation remain trusted.

The local macOS arm64 compatibility run reports whole-crate success with 22
verified checks and zero errors. The explicit crate-local function inventory
also includes specification functions, so it has 25 names rather than 22.
This is proof-component checking, not production or real-project acceptance.
The owning proof gate records exact source, runtime and native validation.

`positive_controls` supplies closed instances using the executable checkers:
unchanged binding, alias retarget, changed bytes, changed presence, exact
restoration, selected scope and an out-of-range selection. The entrypoint calls
that function. The universal lemmas separately cover lexical and namespace
changes, guarded observations and the ABA history.

Two deliberately broken source controls are rejected by stock Verus with a
verification error rather than a parse error:

| Control | Exact mutation | Obligation that must fail |
|---|---|---|
| Omit alias destination | Replace `&& same_bytes(&left.resolved, &right.resolved)` with `&& true` | `matches_atom` no longer implements exact atom equality |
| Omit selected-position bounds | Replace `if i >= left.len() || i >= right.len() {` with `if false {` | `matches_scope` cannot justify indexed access for every selection |

The owning checker requires each needle to occur exactly once and runs each
mutation in a temporary copy. Neither mutation changes the accepted proof
source or the required theorem.

## Concrete refinement still required

The present source mappings identify obligations, not completed proofs:

| Production boundary | Required relation to this component |
|---|---|
| `managed/input.rs`: `Bindings::observe`, `collect`, `stamped_snapshot` | Preserve every relevant lexical source and linked ancestor, its canonical destination and observed contents; distinguish read errors and absence; justify a coherent observation point |
| `managed/source_inputs.rs` and `managed/build_inputs_sources.rs` | Account for the compiler's actual consumed inputs, including generated files and observations learned after compilation, without dropping consuming-package attribution |
| `managed/input.rs`: `closure_digests`, `closure_bindings`, `Scope` | Map the requested dependency closure to complete selected atoms, including declared aliases outside a package and Cargo's manifest/configuration inputs |
| `managed/input.rs`: `snapshot_metadata_projected`, `snapshot_metadata_reobserved`, `unchanged_since` | Preserve the original content-read guard across a projection batch; validate the full selected namespace and actual mutation-coverage contract; refuse or reobserve unsupported projections |
| `managed/runner.rs`: `context`, `context_environment`, `context_hashes`; `managed/environment.rs`; receipts | Combine source binding with the exact relevant options, environment and executed runtime; a source revision alone is not the complete modeled input binding |
| `managed/read_check.rs`: `entries`, `record`, `check` | Preserve recorded bindings and absence, recheck at the API observation point and conservatively handle unreadable or changed inputs; show that serialization cannot mint observation authority |

The shared guarded-observation optimization keeps a stamp taken around the
original content reads and checks it after projection; it does not authorize
an unchecked projection. Proving that implementation requires connecting its
actual platform observations to the guard model. Merely giving an old snapshot
a newly read stamp would not meet the model relation.

Further trust and refinement obligations remain explicit:

- **Compiler-input closure:** prove which filesystem/configuration/environment
  observations the real compilation consumes. Passing an inventory marked
  complete cannot substitute for that proof.
- **Filesystem coverage:** justify atomicity or the precise supported
  observation semantics. The exclusions in [freshness](../freshness.md),
  including undetected mutation classes, remain exclusions; the counter theorem
  does not erase them on Linux or macOS.
- **Encoding and identity:** preserve OS path and environment bytes and namespace
  separation. Relate all digest-based production identities to byte equality
  under an explicitly named cryptographic assumption or an additional byte
  comparison. Hashes are not modeled as injective.
- **Runtime binding:** relate observed runtime identities to the actual launched
  executable and options under the [trust contract](../trust.md); a later
  path-based launch is not automatically the observed executable.
- **Input evidence origin:** validate persisted candidates and map every
  currentness/reuse caller to a valid fresh observation. Scheduling hints and
  mutation notifications do not independently supply that authority.
- **Source sharing:** production currently does not compile or call this proof
  root. Connecting or sharing executable logic needs a reviewed implementation
  boundary and the existing conformance and native cost gates.

These obligations prevent a proof of inventory comparison from being presented
as a proof that Sail observes every effective input or safely accepts every
current complete result.
