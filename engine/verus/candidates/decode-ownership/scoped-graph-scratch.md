# Scoped graph-query allocation admission

This unselected follow-up preserves the 8 MiB graph-validation scratch cap. The
first c39 runtime failed the module-013 checked-HIT gate because its whole-graph
estimate included unrelated owned span text for every query. The original
[failure](c39-qualification-failure.json) remains retained. No decode cap, graph
answer, preparation key, oracle assertion or proof deadline changes.

Each bounded decoded `Query` remains physically owned under its independent
8 MiB decode allowance while the scratch below is live. The decoded Portable,
graph Frames and captured graph owners also remain under their existing separate
allowances. This refinement changes neither their admission nor their releases.

## Closed query inventory

`IsCycle` reads existing maps, SCC sizes and edge slices and returns a boolean.
It allocates no graph clone; a 4 KiB fixed allowance covers the control frame.
`SccRep` clones one representative, and `SccNodes` clones one selected SCC. Their
checked allowance is `4096 + N * (8 * size_of(Node) + 1024) + 8 * sum(text.len())`,
where N and text owners come only from the actual cloned nodes. `SpanInfo.text`
is the only deeply cloned node payload; a String clone allocates from its length.
Other node variants clone Arc owners without copying their referenced heap data.
The index discovery itself borrows immutable compiler tables without a temporary
collection. Missing or malformed indices fail admission.

`FunctionOrder` consumes the trusted canonical, distinct SCC representative order
produced by the same compiler graph. It traverses the same SCCs and preserves the
original function predicate and output order. Its allowance is the largest live
SCC clone allowance plus
`input.len() * (8 * (size_of(Fun) + size_of(Vec<Fun>)) + 1024)`.
Every retained output Fun belongs to the selected input; duplicate input names
do not create duplicate graph nodes. Therefore both retained Fun count and
nonempty group count are bounded by input length.

Explicit `Vec::new` and `push` replace only the intermediate collect operation.
This prevents an in-place collection specialization from transferring a large
Node buffer to a smaller Fun output. Each output buffer has ordinary geometric
growth, covered by the unchanged 8× allowance including old/new overlap and
minimum capacity. One entire SCC node vector may overlap all previously retained
output groups, and the maximum-component term includes that overlap. SCC node
clone buffers drop before moving to the next component. Arc references remain
owned inline elements; they are not relabelled as uncounted compiler allocations.

`ShortestCycle` keeps the original conservative whole-graph linear allowance plus
`16 * N * N * size_of(usize)` for reached-set and cloned BFS paths. Edges are read
without an allocated edge worklist. `CanReach` remains unsupported by the ledger;
its edge-sized worklist is not covered by this query inventory. Arithmetic uses
checked additions/multiplications and admission precedes query replay.

## Controls and qualification

The original sixteen unrelated 100 KiB text nodes are retained as a positive
allocation-free scalar/singleton/order control. The refusal control instead joins
those text owners into one genuinely cloned SCC; its order query requires more
than 8 MiB. A shortest-cycle negative uses the same actual cyclic graph and
retains the quadratic preflight. Additional tests cover an oversized component,
missing input, arithmetic overflow, and an output vector that must not inherit
201 unselected Node slots for one selected Fun.

The [actual pinned VIR checks](scoped-query-source-checks.json) pass 37/37 tests
and strict all-target Clippy at ordinary priority with two compiler jobs. The
actual driver suite also passes 22/22 and strict all-target Clippy. The ordinary
combined build and all source qualification stages remain required. A scoped allocation argument or
passing unit control does not establish a prior-revision preparation restore.
The candidate must still pass the unchanged checked-HIT and full real HE gates
before any product qualification or runtime selection.

## Ordinary build setup

The next feature-isolated runtime is built from the pinned commit and this exact
patch. A narrow [compiler-cache setup record](scoped-query-compiler-seed.json)
retains the identities and cost of copying inactive third-party intermediates.
Each source/destination byte hash was checked; no hard links, runtime files,
receipts or seals were copied. VIR and RustVerify intermediates were excluded so
changed source must recompile. The normal builder still performs fresh vstd
verification and creates an independently validated complete runtime manifest.
This is build-cache setup, not copied proof acceptance or source qualification.
