# 0094: Isolate specification termination checks from later proof history

- **Status:** accepted bounded correction; `0ad405eb` qualified and selected on Linux and macOS
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

Prototype one lazy companion solver per bucket for the pinned Z3 with its
default Z3 encoding and default-prover
`QueryOp::SpecTermination` checks. Before the first check it receives the full
existing bucket context; before later checks it receives each newly admitted
context batch exactly once. This bounds total context loading to at most
twice the original, avoiding repeated growing-prefix loads. Explicit nonlinear,
bitvector, profiling reruns, CVC5 (including CVC5-compatible SMT-LIB on Z3)
and `--spinoff-all` retain their existing routes.
The companion executes every original command and retains
the existing prover options, limits, errors, source spans and failure handling.
Its solver history never enters the ordinary proof solver. Both solvers
contribute complete resource/timing totals and one combined bucket profile
whose independent graph components have disjoint instance identifiers. Nonrecursive specs
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
No source/profile expectation or baseline is weakened. Packed candidate pins
may be staged for these gates, explicitly unqualified and unpublished. Adopt
the runtime only after qualification; replay or a candidate build does not
establish release acceptance.

## Qualification correction

The `0ad405eb` candidate also inherits the ordinary context's expected solver
version when the existing check is enabled. A recursive-spec-only negative
control exposed its omission in `4cd6bd5f`. The solver version request was
already unconditional, so this restores response validation without adding
commands or changing the actual query stream. The existing explicit opt-out
remains. All 41 focused regressions pass on both platforms; full source,
ordinary-mode and final cost qualification remain separately recorded in the
[gate record](../validation/general-performance.md).

The compiler implementation is merged in
[fork PR #15](https://github.com/Chelis-Lang/verus/pull/15). Its merge of
current fork documentation changes no compiler source or Cargo input relative
to tested `0ad405eb`; native artifacts retain that exact source identity.
Both native runtime archives are published and consumed as recorded in the gate record; final Sail release composition remains separate.

## Consequences

The strategy removes a demonstrated cross-stage solver-history dependency
instead of selecting another canonical variable spelling. It can increase
solver startup and context parsing for recursive specifications; neither the
cost nor whole-corpus verdict agreement is established by the replay. Other
queries can still be history-sensitive and require independent evidence.
The original stock source-suite debug-order blocker and sampler-overhead
requirement remain separate.

## Source-engine fixture inventory

Owner approval, 2026-11-01: on the exact qualified Linux/Mac `0ad405eb`
manifests, the recursive source-engine fixture requires stock's single
`root.smt_transcript` and the fork/engine's exact pair, adding
`rootdemo!count._00.smt_transcript`. All verdict, complete diagnostic and
function-detail comparisons remain exact; fork/engine session inventories
and complete SMT bytes remain exact. Unknown runtimes, other fixtures,
missing sessions, duplicates and additional sessions retain strict rejection.
The original equal-inventory failure remains recorded; this does not excuse
any verdict difference or qualify an unmeasured runtime.
