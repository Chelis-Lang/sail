# 0094: Isolate specification termination checks from later proof history

- **Status:** proposed; bounded compiler candidate, not a selected runtime
- **Date:** 2026-10-31

## Context

The pinned Mac fork rejects semi-persistent's unchanged
`Tnum::add_carry_bitwise_has_eq`, while three stock runs pass (#634, VI-0040).
Literal SMT replay with the pinned Mac Z3 reproduces stock `unsat` and fork
`unknown`, reason `incomplete quantifiers`. The first proof query itself is
byte-identical, SHA-256
`f4950172df4f0ddb03b9f00b477023c79af08da6505235ec9c79a42c424041bb`.
The stock complete SMT input also matches between the two platforms; the fork
matches through the failing query before its error-followup commands differ.

Changing only the formal names in the two earlier specification termination
checks back to stock recovers `unsat`; changing either one alone does not.
Removing only those two completed push/pop scopes from the original fork
script also recovers `unsat`, retaining the other thirteen earlier checks,
every definition axiom and the exact proof query and resource limit. These are
offline diagnostic controls, never a Sail query-rewriting route. They isolate
solver-history sensitivity without establishing an internal Z3 defect.

VI-0034 previously exposed this class of failure. Its signature-size naming
correction recovered that proof but does not prevent the new case. A further
program-specific spelling adjustment would leave the same structural risk.

## Decision

Prototype the existing Verus spinoff mechanism for every `QueryOp::SpecTermination`.
Each such check receives a fresh pinned solver with the complete existing
bucket context at that point. It executes every original command and retains
the existing prover options, limits, errors, source spans and failure handling.
Its solver history never enters the ordinary proof solver. Nonrecursive specs
with no termination query and ordinary proof queries keep their existing route.

This is an explicit extension of ADR 0039's narrow fork policy, recorded here
because solver session boundaries change. It does not rename or normalize a
query, increase a limit, retry for a preferred answer, or accept a proof from
any seed. Existing exact cache keys bind the actual input stream and solver
identity; fresh compiler/vstd artifacts must bind the new fork identity.

Before selection require the original unchanged full package to match stock
on Mac and Linux, positive and false-termination compiler controls, all prior
fork regressions and nonignored source tests, fast/standard/deep conformance,
determinism, actual release factories and installed-output controls. Measure
the new solver startup/context cost across the broader corpus and the original
release workloads. Preserve every old failure and any newly exposed difference.
No source/profile expectation or baseline is weakened. Do not re-pin a runtime
or claim release acceptance from replay or the bounded compiler candidate.

## Consequences

The strategy removes a demonstrated cross-stage solver-history dependency
instead of selecting another canonical variable spelling. It can increase
solver startup and context parsing for recursive specifications; neither the
cost nor whole-corpus verdict agreement is established by the replay. Other
queries can still be history-sensitive and require independent evidence.
The original stock source-suite debug-order blocker and sampler-overhead
requirement remain separate.
