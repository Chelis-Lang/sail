# 0028: Exact prepared graph-read dependencies

- **Status:** proposed candidate; compilation, runtime qualification and product acceptance pending
- **Date:** 2026-10-10

## Context

ADR 0027 requires fresh Rust checking and exact prepared lowering reuse without
changing stock SMT inputs or operation order. Qualified real-project evidence
shows the complete global call-graph capsule prevents almost every cross-edit
hit. The graph contains functions outside a pruned module; changes to those
functions need not change answers read while lowering this module. This is not
evidence that any graph component can be discarded without checking its users.
The qualified engine and its failed performance observations remain unchanged.

## Candidate decision

The separate `engine/verus/candidates/graph-ledger.patch` applies to the same
pinned Verus source and contains the existing interactive patch plus this
candidate. It is not selected by the installer, artifact pin or default shim.
Its prepared key/codec has a new version-three domain. Only the complete
function-call graph and SCC vector are replaced in the input capsule. Every
other constructor map, datatype graph, diagnostic span index, naming/counter
seed, pruned VIR, source binding, setting and runtime identity stays fully bound.

`GlobalCtx` makes both raw graph fields private. Its exact result-bearing
methods record typed inputs and outputs while `ast_to_sst` and `poly` execute.
The interval begins after the complete fresh constructor and ends before AIR
or solver operations. Bucket-local copies share the capture; each stock
`from_self_with_log` task gets a distinct capture. Full-phase contexts remain
fresh. There is no compiler context, graph or span retention across revisions.

A hit replays every recorded query against the new complete stock graph before
restoring SST or any mutable effect. Representative identity, cycle boolean,
component members and shortest-cycle paths compare exactly, including returned
order. For lowering traversal, the query retains the selected functions and
exact ordered, nonempty SCC groups. Stock empty components have no lowering
work. The wrapper uses the original SCC vector and each original node sequence;
it neither sorts nor reconstructs the graph. The stock AIR generator continues
to traverse every full SCC, including trait nodes and post-component axioms.

Raw whole-SCC inventory access invalidates an active capture. Any unsupported
read, poisoned lock, encoding/admission failure or malformed frame is a miss.
Failed lowering never stores a ledger. Replay mismatch leaves the fresh context
untouched. Existing mutable-effect and diagnostic replay is unchanged.

## Closed read-site audit

All paths below are relative to Verus `source/`. Raw fields are accessible only
inside `vir/src/context.rs`; compilation makes an unrouted outside access an
error. The static audit also counts the closed internal field accesses.

| Consumer | Routed reads | Timing |
|---|---|---|
| `vir/src/ast_to_sst.rs` | one representative | recorded lowering |
| `vir/src/ast_to_sst_func.rs` | one representative | recorded lowering |
| `vir/src/recursion.rs` | three representatives, one cycle answer | recorded lowering |
| `vir/src/ast_to_sst_crate.rs` | exact selected function order and SCC groups | recorded lowering |
| `vir/src/recursive_types.rs` | full SCC inventory, members, cycle, shortest cycle | fresh validation before capture; invalidates if moved inside capture |
| `vir/src/sst_to_air_func.rs` | shortest cycle | fresh AIR after capture |
| `rust_verify/src/commands.rs` | full inventory and three member traversals | fresh AIR after capture |
| `vir/src/context.rs` | original builder, full graph checks, clone, routed wrappers and replay | constructor results remain capsule-bound |

`recursion::get_edges_from` receives only a preliminary builder graph during
`GlobalCtx::new`; it cannot obtain the retained private global graph. `poly`
has no direct graph reads. Existing SCC/bucket/function scheduling is unchanged.
The current source has no remaining unrouted retained graph access. This is a
static coverage finding, not a substitute for compiling and executing it.

## Shape, integrity and budget

Frames contain a closed serde enum with owned typed nodes/results; the borrowed
writer has identical variant order and representation. They retain no VIR
pointer identity, rustc object or borrowed span. A dedicated deserializer checks the outer count before reserving vector
metadata and checks each frame length against the remaining allocation budget
before reserving content. Decoded frame contents and metadata together stay
within 16 MiB; the larger SST decoder limit cannot bypass this ledger cap.
Each frame is limited to one MiB, must consume its complete buffer, and must decode a supported variant.
Replay requires exactly one lowering-order query. The enclosing prepared file
still binds version, exact key, body digest, size and current span provenance.
An old, truncated, corrupt or misfiled preparation cannot authorize a hit.

All capture objects, frame capacities, linked-list metadata and finished outer
vector metadata share a process-wide 16 MiB charge. Captures refuse allocation
when that budget cannot be reserved. Frame encoding has one global lock and a
one-MiB cap; conservatively count two MiB for realloc overlap. Finishing a
ledger temporarily allocates at most another 16 MiB of outer-vector metadata
before charge admission; there is no frame-content copy. One global encoding
lock covers both frame encoding and finish conversion. Buffer destructors take
and physically drop all frames before releasing quota. The final capture clone
consumes its Arc allocation, drops the state and only then releases metadata
quota. Recording and focus diagnostic reporters also dispose their message vectors
and nested cache-owned Arc references before releasing the shared diagnostic
quota. A separate narrow overlay records that fix for the qualified baseline;
its original runtime remains unchanged. A deterministic two-thread regression blocks physical disposal and
checks that racing replacement admission remains refused. Deserialization and
replay occur under the global preparation gate; decoded frames are explicitly
dropped before effect restoration and before that gate can be released. Add this conservative
34 MiB allowance to the existing worst-case 458 MiB constructor/key phase:
492 MiB remains below the 512 MiB preparation allocation bound. The store and
restore phases remain below that bound with the same allowance. Active decoded
IR/query objects are fresh compiler working state, as in the existing codec,
and process RSS is measured separately. Retained telemetry includes the global
ledger charge; it is not a claim about peak transient allocations.

`prepared-key-components` records the exact remaining baseline digest. Graph
replay misses name their stage (`graph-answer-changed`, shape, coverage or
limit), separating graph invalidation from a changed other-global capsule.
None of these observations weakens an admission or proof check.

## Validation and consequences

Source tests exercise an unrelated edge surviving replay, a new recursive SCC
rejecting it, trait-cycle membership/path changes, component-order changes,
borrowed/owned codec equivalence, malformed/trailing data, unsupported reads, budget refusal, final-clone lifetime and racing quota
release. The candidate oracle retains all existing checkpoint checks
and exact raw SMT, verdict and diagnostic comparisons, plus recursion, trait,
bitvector and late EXEC fixtures. No check or baseline is relaxed.

The isolated host candidate passes seven graph-ledger VIR regressions and the
complete eight-test VIR library suite. The complete `rust_verify` library passes nine release tests, including the
reporter nested-Arc/racing-admission regressions; raw-field privacy also compiled. No runtime binary,
vstd verification or source manifest was built by these checks. Runtime source
oracles and product acceptance remain pending. Static coverage, patch
applicability and syntax checks also pass. A separately built variant
must pass them, source/cancellation gates and real-project raw SMT oracles before
it can replace the current artifact. Global datatype maps, imported source/span
bindings or other constructor fields may still invalidate every real body-edit
candidate. Benefit and the twofold product speed gate remain unproven.
