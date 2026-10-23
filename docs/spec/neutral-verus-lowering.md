# Neutral source to Verus, version 2

Status: implemented library slice; actual fork/stock oracle qualification is a
separate acceptance gate. The ordinary Verus input route is unchanged.

## Admission

`sail_verus::contracts::lower(source, crate_name)` accepts one complete UTF-8
source file, at most 64 KiB, and a Rust compiler crate identifier. It returns
`LoweredSource { source, manifest }` or structured `SAIL3140`. The emitted shadow
is at most 16 MiB. Lowering performs no solver or filesystem operation.

The backend rechecks the [neutral source syntax](neutral-contracts.md). It
admits Boolean and fixed-width scalar parameters and owned results, inline
modules, and nongeneric named structs deriving exactly `Copy` and `Clone`,
with direct scalar fields. Inherent constructors and shared or mutable receiver
methods are supported. Passive attributes accept only literal documentation/must-use strings, `doc(hidden)`,
and plain `inline`, `inline(always)` or `inline(never)`. Macro expressions in
attribute payloads and all parameter attributes are unsupported.
Bodies are loop-free and call-free. Each executable
function has one requirement identity and one guarantee.

Admission covers the entire file, including unannotated items. It rejects
globals, aliases, traits and trait implementations, arbitrary imports, macros,
external modules, unannotated functions, conditional attributes, unresolved
types, and attributes inside executable bodies. Fields resolve through admitted
scalar structs; user-defined dereferencing and operators cannot participate.
Only explicit, unrenamed `sail_contracts` attribute imports are allowed. `old`
and generated `__sail_` names are reserved. Structs cannot shadow scalar type
names. Mutable by-value parameters need separate prestate lowering and are
currently unsupported.

Specification division and remainder, executable runtime `Int`, quantifiers
and unsupported Rust forms fail explicitly. No signed division is silently
converted to mathematical Euclidean division. No runtime `Int` is silently
converted to a bounded proof integer.

## Value and definedness semantics

Every expression lowers to its scalar type, a mathematical value and a Boolean
definedness condition. Fixed-width arithmetic retains signedness and width
through explicit range checks. A predicate produces separate value and
definedness specification functions. Calls require both. A checked overflow
cannot become a false predicate that then becomes true under negation.
Out-of-range source literals and unsigned negation are rejected before lowering.
They cannot become impossible preconditions that make uncompilable Rust appear
vacuously verified. Signed minimum literals remain supported.

Definedness preserves short-circuit evaluation: the right side of `&&` is
required only when the left is true; the right side of `||` only when the left
is false. Conditional branches are guarded similarly. Guarantees prove both
definedness and their actual Boolean result. Requirements describe exactly
accepted entry values, including their evaluation definedness.

Field selection retains the complete base expression's definedness, including
arithmetic in a conditional selecting a record. Reading a scalar field cannot
discard an overflow obligation from evaluating that record expression.

Old captures are eager entry computations, including inside a short-circuited
guarantee branch. Their definedness is asserted at entry under the original
preconditions. Adding a stronger precondition instead could make a failing
capture verify vacuously. Mutable receiver old values use Verus's actual
`old(self)` state; ordinary mutable receiver projections in guarantees use
`final(self)`. Entry preconditions and capture assertions use the entry state.
No reference alias substitutes for a copied scalar snapshot.
The original executable body is preserved; its arithmetic safety remains a real
proof obligation. No proof scaffolding or weakened condition makes a body pass.

## Mapping and actual execution

`LoweringManifest` records semantics version, contract version, exact original
and emitted SHA-256, crate identity, arithmetic interpretation and sorted
function-to-requirement bindings. Each binding names the exact generated symbol,
canonical guarantee digest and original source range. It invents no obligation
ID and asserts no proof. Actual attributed obligations must match that symbol.

New lowering emits semantics version two. `validate_manifest(original, manifest)`
checks retained version-one or version-two records against the corrected complete
projection, preserving the recorded version in the source header. Unaffected
version-one manifests and emitted bytes remain exact. A version-one record that
dropped compound-field definedness differs from the corrected digest and is
refused, including when a precondition makes its example safe. The original
backend report is preserved; this read check neither rewrites history nor
constructs new semantic evidence. Unknown versions are unsupported.

`sail_verus::contracts::verify(original, crate_name, config, diagnostics)` lowers
the original snapshot, validates Sail's installed Verus and exact Z3 pin, and
creates an immutable shadow under `config.cache`. It never modifies the
caller's workspace. The shadow contains an isolated Cargo manifest, pinned
`vstd`, generated `src/lib.rs` and mapping manifest. Different existing bytes
are drift, not permission to overwrite evidence. Symbolic links and unexpected
input files are refused. A held shadow lock covers setup, actual `managed::run`
and a final complete readback. The managed backend uses a separate Sail-owned
sibling cache namespace, so source packages and verification bookkeeping remain
physically disjoint under the existing managed-runner invariant.

The helper currently accepts default whole-target controls and forces obligation
logging. It returns `ContractRun` with the lowering and actual `ProjectResult`,
including failures and unknowns. Shadow drift/setup errors use `SAIL3141`.
The `proof` projection has only `Proved` and `NotProved`. Proved requires current,
complete requested-target verification, complete attribution without unmatched
sessions across observed targets, and exactly attributed bound functions,
each confirmed verified with nonempty proved nonadvisory obligations and
recorded unsatisfiable checks. Any underlying unknown, SAT, missing answer or
missing source prevents proof. Counts include advisory and dependency checks;
verbatim unknown reasons and the entire original backend report are retained.
A failed legacy verdict or a model returned after unknown is never promoted to
a source-level refutation. This proves contracts under their stated requires,
not requirement entailment, precondition feasibility, vacuity freedom or a
mutation kill. A separately recorded runtime sample can show a postcondition
violation for its exact compiled source and input; it is not SMT evidence.
It does not publish per-requirement assurance or establish a caller file's
currentness from a borrowed string. The caller rechecks original currentness
and maps actual obligations. Contract-to-requirement entailment remains an
independent Oyster query.

## Translation implementation trust

The helper accepts only Sail-owned executable configuration and an explicitly
selected pinned Cargo toolchain. Client protocol fields never choose executable
paths. Before any metadata child or lockfile probe, it refuses Cargo configuration
files in the shadow's lexical and physical ancestors and Cargo home, including
symbolic links and unreadable files. It refuses compiler wrappers, compiler or
linker-loader overrides, Rust flags, Cargo patch/source/registry/build/target/profile
and unstable selectors, Verus selectors, and Sail private or research selectors.
A mismatched explicit `RUSTUP_TOOLCHAIN` is unsupported. Resource-only build jobs
and the target directory, which Sail overrides, do not change admission. This
first slice refuses even benign Cargo configuration files rather than inferring
that an unreviewed configuration preserves translation.

The optional qualified new-project lockfile seeds resolution when available.
Actual Cargo metadata then passes through the existing independent vstd source
policy admission. That policy checks the complete compiler-loaded dependency
closure, including builtin proc macros, exact versions, selected features,
registry locations and every actual source file against independent hashes.
Mutable checksum sidecars and package version strings alone do not qualify it.
If a new dependency resolution differs, the helper reports `SAIL3140`; it does
not relax policy or claim a neutral proof from an otherwise valid backend run.
No second copy of the source-policy checker is maintained.

`ContractRun.translation_trust` records the admitted source policy, actual metadata
and exact Cargo lockfile digests. The helper rechecks the control observations,
configuration absence, complete source closure, lockfile and immutable shadow
after the managed backend, including failure paths. A changed translation input
leaves the backend evidence unqualified. Original source currentness and actual
obligation mapping remain the caller's responsibility.

## Qualification

Ordinary tests compile the exact neutral fixtures with the real runtime macros
and exercise complete small scalar ranges and mutable state transitions. Other
fixtures check deterministic output, source and guarantee identities, typed
purity rejection, negated predicate definedness and eager old assertions. Shadow
tests preserve tampered evidence and refuse symbolic links and extra inputs.

The ignored native oracle runs the same emitted source on the validated pinned
fork and stock verifier. Passing controls cover arithmetic, short-circuit
behavior, signed minima and mutable wallet state. Failing controls cover a
negated overflowing predicate and an overflowing eager old capture. Both actual
JSON verdicts and counts must agree with expectations. A compiler error is not
a property refutation. Raw outputs, generated sources and mapping manifests
are retained. CI's solver lane runs this fixture with its installed runtimes.
For explicit persistent native acceptance:

```sh
SAIL_NEUTRAL_ORACLE_OUT=/fresh/evidence/directory \
  cargo test -p sail-verus --test neutral_contracts \
  native_fork_and_stock_agree_on_exact_lowered_source -- --ignored --nocapture
```

Without the optional output variable the fixture allocates a fresh temporary
directory. Missing runtimes or malformed evidence fail. Local ordinary fixture
success does not establish actual backend compatibility.

The [native qualification record](../validation/neutral-verus-lowering.md)
retains actual fork/stock results and failed setup/lowering attempts.
