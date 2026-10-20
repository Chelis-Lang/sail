# 0056: Stable SMT spellings for concrete trait functions

- **Status:** proposed; native-built and measured prototype03, unqualified. The
  runtime pins name its joined source, fork `cbbcdf44`; that qualification is pending
- **Date:** 2026-10-18

## Context

[VI-0030](../verus-issues/VI-0030-anonymous-impl-numbering.md) records two actual
accepted-e536 Vest calls differing only by an empty impl insertion. Twenty-three
full method paths and 30 exact solver-input sessions change, including untouched
CBOR inputs that refer to an ASN.1 Clone contract. Twenty-one changed paths are
Clone methods; two are inherent spec methods. That timing list omits spec
function declarations. Offline classification finds only five Clone-only changed
sessions; 25 also contain other symbols or raw impl labels. A source proposal
is not gain.

Earlier uncompiled prototypes attempted to replace Rust/VIR identities.
Independent review rejected associated-member parity across native ghost erasure,
then coherence-based uniqueness in the presence of trivial bounds, const
specialization and cfg-dependent marker flags. Neither was compiled or selected.

[ADR 0039](0039-private-verus-fork.md) does not already authorize this additional
symbol renaming. Logical equivalence alone cannot establish pinned-solver verdict
or resource equivalence. Complete correctness and performance gates remain required.

## Decision

Keep every original Rust DefId/DefPathHash, VIR Path/Fun/ImplPath/ClosureId,
equality/hash, lookup, sorting, diagnostic and import/export payload unchanged.
Create an invocation-local immutable map of selected exact function spellings for
AIR generation. This is Verus name generation, not normalization of Sail queries.
No alias is exported or used to match foreign native metadata to producer VIR.

Read the complete merged VIR before any crate/module/function pruning, retained
in the existing erasure hints. Both actual TraitMethodImpl and
ForeignTraitMethodImpl declarations carry the required typed trait/Self metadata
before foreign demotion. The first screen supports direct concrete trait methods
with zero-argument nominal Self and no other trait arguments. Encode full typed
crate/module/Self/trait/member and item-kind fields. Inherent methods and
unsupported/synthesized paths retain legacy spelling. No general type encoder
or inference from a friendly method name or first parameter is introduced.

Group all actual complete candidate labels, before pruning. Every duplicate
retains legacy spelling, including impossible-predicate and specialized ground
duplicates. No coherence or native header-parity assumption is needed. Distinct
selected keys use a reserved injective full-hex namespace; original symbols use
their old spelling. Never truncate or partially hash a key. Bounds are 512 bytes
per key, 16,384 candidates and 8 MiB of staged key bytes; exceeding a whole-map
bound silently retains all legacy spellings. Unexpected namespace/original-path
collisions likewise select the entirely legacy map.

Share one read-only Arc map through GlobalCtx, its simplification rebuild and
workers, and each Ctx's NameCtxt. Only NameCtxt::fun_to_string substitutes a
selected exact function spelling. First render the original path to preserve
all crate-registration and encounter-order side effects. path_to_string stays
unchanged. Function definitions/calls, req/ens, fuel, static/FnDef IDs and reverse
solver-name lookup use this same renderer. No process-global/TLS map or mutable
public PathX hash cache is added.

Raw inline AIR may mention literal original names. The allow-inline-air flag or
any AirStmt in any current/imported header/body of the complete merged VIR
silently disables aliases for the whole invocation. The literal AIR is preserved.
All new map decisions are optional spellings; no new lookup diagnostic is added.

## Consequences and remaining gates

Original function timing paths still shift after the incidental edit. Inherent
leaf_inv/nonmal_leaf_inv, accepted closure IDs, unrecorded synthetic/default/
recursive functions and raw impl/user quantifier labels remain legacy. The actual
Clone subset and residual full 30-session churn must be measured explicitly.
Keeping separate function symbols under an injective map preserves logical
structure; renamed solver input may still change Z3's answers under limits, so
whole-corpus per-function verdict/complete-diagnostic conformance remains mandatory.

Selection adds one complete-set pass, full-key grouping/storage and an exact map
lookup after existing function rendering. Full hex symbols increase SMT bytes,
memory and solver cost. There are no foreign family rescans or new dependencies.
No SCC or filename-helper change is included. Sail continues binding exact input
bytes. No gain, runtime, release or default pin is selected by this source work.

Five pure VIR controls and eight focused compiler fixtures pass on native Linux,
including independent accepted-runtime complete diagnostic comparisons. Five
initial compiler-fixture failures remain indexed; impossible duplicate declarations
are frontend-refusal controls, while the pure VIR unit exercises duplicate alias
fallback. The standalone default-suite adaptation also passes all eight native cases.

Three rotated native Vest rounds lower the empty-impl edit median from 45.089 s
to 33.955 s (24.693%), with 30 versus 24 solver starts each round. Cold and
returned-source medians are nearly unchanged. All 27 timed calls are retained;
this screen does not establish whole-project or whole-corpus acceptance. See the
[qualification record](../validation/smt-function-spellings.md) for source/runtime
identities, raw hashes, limits and remaining gates. Complete correctness,
determinism, matching macOS runtime and full fork-suite gates still precede
selection. User guides/default behavior are unchanged while this candidate
remains unselected.

## Note (2026-10-20, accepted (lead), owner review pending): keys without the stable crate ID

The prototype's key contains each path's stable crate ID, which Cargo derives
from build profiles, so a proc-macro profile change renamed every alias
(VI-0033). Fork branch `perf/7d82994` (`912052eb`) keys crates by name only
and bumps the key tag to `sail-function-alias-04`; same-named crates then
produce equal keys, which the all-duplicates rule keeps legacy.
