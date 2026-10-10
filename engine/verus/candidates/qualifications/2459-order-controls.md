# Candidate 2459 declaration-order diagnosis

These controls do not qualify or activate the candidate. The independent stock
oracle remains failed at unchanged revision 1; its original evidence is retained
in `2459-linux.json`.

One fresh empty-preparation full callback verified all 441 functions and produced
114 solver streams byte-identical to the failed revision's cached engine run.
Four predeclared targeted controls then ran with the same source path, binary,
settings and selected function, recording `vir-poly` before lookup or restoration.
Both current generated accessor orders occurred:

| Current ordered VIR accessors | Cached control | Empty control | Exact SMT SHA prefix |
| --- | --- | --- | --- |
| `arrow_1`, `arrow_2` | module miss | fresh lowering | `3035cc9925fb` |
| `arrow_2`, `arrow_1` | persistent module hit | fresh lowering | `79de268eb3a2` |

Each pair has byte-identical pre-key pruned VIR and solver requests, identical
diagnostics and full results. A changed current accessor order therefore causes a
miss rather than restoring the old ordered SST vector. `Bucket.funs` is a set;
its sorted membership encoding does not remove the separately encoded ordered
`Krate.functions` vector. The graph ledger also checks the ordered function/SCC
answer. Raw records and hashes are in `2459-order-controls.json`.

A separate `human_eval_023` local-binding refactor passed exit, diagnostics,
function results, session inventory and all 114 raw solver-stream comparisons
against stock. It retained one focused GCD preparation but missed all 105 full
buckets. That demonstrates correctness for this pair, not effective cross-edit
preparation reuse. Source-only shared emulation timings were 126.4 seconds for
stock and 185.5 seconds for the candidate; these are not the frozen product-loop
performance acceptance. See `2459-other-module-edit.json`.

No transcript was normalized, no failed comparison was removed, and no control
was repeated until it happened to pass. A same-compiler-input fresh/restored
lowering oracle remains additive work, pending the owner's baseline decision.
