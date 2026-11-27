# 0185: One bounded-evidence record for Kani and D.I.V.E. symbolic results

- **Status:** proposed
- **Date:** 2026-11-27
- **Extends:** [ADR 0044](0044-output-schema-v2.md), [ADR 0128](0128-kani-bounded-observations.md), [ADR 0177](0177-portable-symbolic-predicates.md)

## Context

Two backends produce evidence that is exhaustive only inside declared bounds.
`sail-kani` decodes `bounded_success`, `counterexample` and
`bound_insufficient` into its own `Observation` (ADR 0128). D.I.V.E.'s symbolic
strategies report `success`, `bounded-safe`, `executions-too-short`,
`inductive-safety`, `induction-inconclusive` and `inconclusive`, which
`sail runtime symbolic-check` forwards as D.I.V.E.'s original JSON (ADR 0177).
Neither goes through the evidence-tier field of ADR 0044, whose `bounded` value
has no producer yet.

The Kani phase (ADR 0180) and the D.I.V.E. symbolic pickup both need to say the
same things about such a result: what was checked, under which bounds and
assumptions, how much Sail observed, and whether it was replayed. Without one
shape each backend would grow its own, and an inductive proof over a finite
model or a Kani success would be easy to present as more than it is.

## Decision

Add `sail_api::bounded`, a hidden module in the published `sail-api` crate,
following the precedent of `sail_api::campaign`: `#![doc(hidden)]`, stated as
unstable groundwork, no protocol version, no JSON Schema file. Nothing in the
CLI, MCP, JSON reports, daemon protocol or ledger uses it. `sail-kani`'s
decoder admission and `Observation` serialization are unchanged; a new
`Observation::bounded_evidence()` builds the record.

### Shape

| Field | Type | Meaning |
|---|---|---|
| `subject` | `{backend, kind, id}` | `backend` is `kani` or `dive`; `kind` is `harness`, `model` or `property`; `id` is the exact native name |
| `outcome` | enum | see below |
| `tier` | `bounded` or `null` | `bounded` exactly for `success` and `counterexample` |
| `strength` | `bounded`, `inductive` or `null` | present exactly when `tier` is |
| `bounds` | `[{kind, scope, value, source}]` | `kind`: `unwind`, `object_bits`, `path_length`, `induction_depth`; `scope`: `subject` or a native loop or function name; `source`: `annotation`, `argument`, `configuration`, `tool_default`, `reported` |
| `sufficiency` | enum | `sufficient` (covers every execution), `prefix` (checked to the bound, executions continue), `not_reached` (no execution reaches the bound), `insufficient` (an execution exceeds the bound), `not_established` |
| `assumptions` | `[{kind, name}]` | `stub`, `assumed_contract`, `loop_invariant`, `termination_unchecked`, `generated_harness`, `single_monomorphization`, `finite_layout`, `state_constraint` |
| `checks` | map of count by native status | the tool's own status names, unmodified |
| `witness` | `{format, validated_by}` or `null` | `validated_by` names the independent replayer, `null` when only the producing tool vouches for it |
| `tools` | `[{name, version, sha256}]` | `sha256` only when Sail authenticated the executable |
| `integrity` | `not_established`, `reconciled`, `supervised` | `reconciled`: exit, text, summary and per-check records agree; `supervised`: also the positive process evidence of the Kani correctness contract |
| `provenance` | `fresh`, `receipt_local`, `receipt_shared` | |
| `cost` | `{wall_ms, peak_memory_bytes}` | of the original run; `null` when not measured |

Outcomes: `success` and `counterexample` carry the tier. `bound_insufficient`
and `inconclusive` completed without one. `timeout`, `out_of_memory`,
`stopped`, `crash`, `cancelled`, `unsupported` and `build_failure` are no-claim.

### Rules

1. **Nothing upgrades across tiers.** The record's tier type has the single
   value `bounded`; `smt_verified` and above do not decode. An inductive proof
   is `strength: inductive` in the bounded tier, because it holds for the
   finite configured model only.
2. **"Verified" never appears**, as an outcome, tier, strength or field name.
3. A record is checked as a whole on construction and on decode (`SAIL6303`):
   tier and strength follow the outcome; `inductive` needs `success` with
   `sufficient`; `insufficient` goes with `bound_insufficient` and nothing
   else; only a counterexample has a witness; a tiered outcome needs at least
   `reconciled` integrity and a named tool; a receipt needs a completed outcome
   and `supervised` integrity. No-claim outcomes are therefore always `fresh`.
4. A mapping reads the native result only. It never infers a stronger
   sufficiency or strength than that result states by itself.

### Mapping from `sail_kani::Outcome`

| Kani | `outcome` | `tier` | `strength` | `sufficiency` |
|---|---|---|---|---|
| `bounded_success` | `success` | `bounded` | `bounded` | `not_established` |
| `counterexample` | `counterexample` | `bounded` | `bounded` | `not_established` |
| `bound_insufficient` | `bound_insufficient` | none | none | `insufficient` |

The converted observation also carries `integrity: reconciled`,
`provenance: fresh`, the reported `object_bits` bound, the check counts, the
admitted Kani and CBMC versions and CaDiCaL without a version, no executable
digests, no witness and no cost.

### Mapping from D.I.V.E. symbolic verdicts

The value is the row's `verdict`, or its `proof` for predicate and
named-property rows. For the strategies `sail runtime symbolic-check` runs
today, exits 0, 1, 3 and 4 correspond to `success`, `counterexample`, `crash`
and `stopped`, and exit 2 to `unsupported` or `inconclusive`.

| D.I.V.E. | `outcome` | `tier` | `strength` | `sufficiency` |
|---|---|---|---|---|
| `success` (complete search with exact counts) | `success` | `bounded` | `bounded` | `sufficient` |
| `bounded-safe` (`model_proved: false`) | `success` | `bounded` | `bounded` | `prefix` |
| `executions-too-short` (`model_proved: false`) | `success` | `bounded` | `bounded` | `not_reached` |
| `inductive-safety` (`model_proved: true` on symbolic-path) | `success` | `bounded` | `inductive` | `sufficient` |
| `invariant-violation`, `deadlock` | `counterexample` | `bounded` | `bounded` | `not_established` |
| `induction-inconclusive`, `inconclusive` | `inconclusive` | none | none | `not_established` |
| `unsupported` | `unsupported` | none | none | `not_established` |
| `error` | `crash` | none | none | `not_established` |
| `stopped` | `stopped` | none | none | `not_established` |

Any other value maps to nothing; the caller treats it as a protocol error.

## Open points

These could not be settled from the sources and are decided provisionally
above.

1. **`executions-too-short`.** It is not Kani's `bound_insufficient`: there an
   execution exceeds the bound, here none reaches it and every shorter prefix
   was checked, with exit 0. It is mapped as a bounded success with its own
   `not_reached` sufficiency. D.I.V.E. still reports `model_proved: false`,
   and a model whose executions all end early is often over-constrained, so
   whether a report should present it as a success or as a warning without a
   tier is for the symbolic pickup to decide.
2. **Kani success sufficiency.** With unwinding assertions on, a Kani success
   means the bounds cover every execution. The decoder sees neither argv nor
   whether those checks ran, so the mapping says `not_established`. The
   executor can state `sufficient` once it binds argv.
3. **Unwind bounds.** No retained 0.68.0 export fixture shows where a
   harness's unwind value appears, so the conversion records `object_bits`
   only. The executor records unwind bounds with their source.
4. **`induction-inconclusive` keeps bounded base evidence.** The row still
   lists `completed_bounds` with no bad prefix. The verdict alone maps to no
   tier; a mapping over the whole row could report the base as `bounded-safe`.
5. **`success` from a complete symbolic search** overlaps the native temporal
   contract's `holds`, which has no tier field. Whether complete finite
   searches move onto this record or stay there is open.
6. **Unmapped D.I.V.E. results:** the Core `bounded-path` strategy's `result`
   values (`assumption-failure`, `evaluation-error`, `internal-error`, its own
   exit codes), `bounded-inconclusive` from selected-root checks, and the
   source-obligations worker's `result: proved`, whose Z3 backend raises a
   tier question of its own.
7. **`stopped`** does not separate time, memory and interruption. Kani
   supervision will report `timeout` and `out_of_memory` separately.
8. **Kani witnesses.** The decoder keeps failed checks, not a trace, so the
   record has no witness. Concrete playback as `validated_by` belongs with the
   parity and harness-kind ADRs.
9. **Receipt identity and the ledger.** The record says only local or shared.
   Keys, signatures and the ledger's backend and tier columns are later work.

## Consequences

The D.I.V.E. symbolic pickup and the Kani executor have one type to land on,
and the no-upgrade rule is enforced by the type, not by each producer. The
shape is expected to change as the open points close; it stays hidden and
unversioned until a command exposes it, which needs its own decision and an
additive change to the output schema. `sail-kani` keeps its own `Outcome`
names.
